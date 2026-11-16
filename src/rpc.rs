// VOID — Discord rich presence.
//
// Discord's local RPC is a named pipe (`\\.\pipe\discord-ipc-N`) carrying framed
// JSON: an i32 opcode, an i32 little-endian length, then the payload. There is
// no SDK here and no dependency — just `CreateFileW` through `std::fs`, so the
// feature costs a few kilobytes instead of a second runtime.
//
// Everything runs on one background thread. The UI only ever posts a presence
// message into a channel; if Discord is closed, the thread quietly retries and
// the app never notices.

use std::io::Write;
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::time::Duration;

/// The VOID application id registered with Discord.
pub const CLIENT_ID: &str = "1571961942044713070";

const REPO: &str = "https://github.com/husainprime20/VOID-OPTIMIZER";

#[derive(Clone, Default, Debug)]
pub struct Presence {
    pub details: String,
    pub state: String,
    pub large_text: String,
    /// Epoch seconds the session started.
    pub start: i64,
    /// e.g. "matchmode" — appended to the asset key so the card can differ.
    pub mood: String,
}

/// Handle held by the app. Dropping it closes the worker.
pub struct Rpc {
    tx: Option<Sender<Presence>>,
    connected: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Rpc {
    /// Start the worker. `enabled == false` gives a handle that does nothing, so
    /// call sites never need to branch.
    pub fn start(enabled: bool) -> Rpc {
        let connected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        if !enabled {
            return Rpc { tx: None, connected };
        }
        let (tx, rx) = channel::<Presence>();
        let flag = connected.clone();
        std::thread::Builder::new()
            .name("void-rpc".into())
            .spawn(move || worker(rx, flag))
            .ok();
        Rpc { tx: Some(tx), connected }
    }

    pub fn connected(&self) -> bool {
        self.connected.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Post a new presence. Cheap, non-blocking, safe to call every frame.
    pub fn set(&self, p: Presence) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(p);
        }
    }
}

// ── worker ───────────────────────────────────────────────────────────────────

fn worker(rx: Receiver<Presence>, connected: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let mut pipe: Option<Pipe> = None;
    let mut last: Option<Presence> = None;

    loop {
        // Wait for a message, but wake up regularly so a late-starting Discord
        // can be picked up without the user touching anything.
        let incoming = match rx.recv_timeout(Duration::from_secs(4)) {
            Ok(p) => Some(p),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };

        if let Some(p) = incoming {
            // Skip identical frames; Discord rate-limits SET_ACTIVITY to 5/20 s.
            if last.as_ref().map(|l| same(l, &p)).unwrap_or(false) && pipe.is_some() {
                continue;
            }
            last = Some(p);
        }

        let Some(p) = last.clone() else { continue };

        if pipe.is_none() {
            pipe = Pipe::connect();
            connected.store(pipe.is_some(), std::sync::atomic::Ordering::Relaxed);
            if pipe.is_none() {
                continue;
            }
        }

        let sent = pipe.as_mut().map(|pp| pp.activity(&p)).unwrap_or(false);
        if !sent {
            // The pipe went away — Discord closed, or it never finished booting.
            pipe = None;
            connected.store(false, std::sync::atomic::Ordering::Relaxed);
            // Drain so we do not spin on a single failing frame.
            match rx.try_recv() {
                Ok(_) | Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => break,
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }
}

fn same(a: &Presence, b: &Presence) -> bool {
    a.details == b.details && a.state == b.state && a.large_text == b.large_text && a.mood == b.mood
}

// ── pipe ─────────────────────────────────────────────────────────────────────

struct Pipe {
    file: std::fs::File,
}

impl Pipe {
    #[cfg(windows)]
    fn connect() -> Option<Pipe> {
        for i in 0..10 {
            let path = format!(r"\\.\pipe\discord-ipc-{}", i);
            let opened = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path);
            if let Ok(mut file) = opened {
                // op 0 = handshake
                let hello = format!(r#"{{"v":1,"client_id":"{}"}}"#, CLIENT_ID);
                if write_frame(&mut file, 0, &hello).is_ok() {
                    return Some(Pipe { file });
                }
            }
        }
        None
    }

    #[cfg(not(windows))]
    fn connect() -> Option<Pipe> {
        None
    }

    fn activity(&mut self, p: &Presence) -> bool {
        let mut activity = String::new();
        activity.push_str(&format!("\"details\":\"{}\"", esc(&p.details)));
        if !p.state.is_empty() {
            activity.push_str(&format!(",\"state\":\"{}\"", esc(&p.state)));
        }
        if p.start > 0 {
            activity.push_str(&format!(",\"timestamps\":{{\"start\":{}}}", p.start));
        }
        let asset = format!("void_{}", if p.mood.is_empty() { "idle" } else { &p.mood });
        activity.push_str(&format!(
            ",\"assets\":{{\"large_image\":\"{}\",\"large_text\":\"{}\"}}",
            esc(&asset),
            esc(&p.large_text)
        ));
        activity.push_str(&format!(
            ",\"buttons\":[{{\"label\":\"VOID on GitHub\",\"url\":\"{}\"}}]",
            REPO
        ));

        let payload = format!(
            r#"{{"cmd":"SET_ACTIVITY","nonce":"{}","args":{{"pid":{},"activity":{{{}}}}}}}"#,
            nonce(),
            std::process::id(),
            activity
        );
        write_frame(&mut self.file, 1, &payload).is_ok()
    }
}

fn write_frame(file: &mut std::fs::File, op: i32, payload: &str) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(payload.len() + 8);
    buf.extend_from_slice(&op.to_le_bytes());
    buf.extend_from_slice(&(payload.len() as i32).to_le_bytes());
    buf.extend_from_slice(payload.as_bytes());
    file.write_all(&buf)?;
    file.flush()
}

/// Monotonic-ish nonce; Discord only needs it to be unique per frame.
fn nonce() -> String {
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}", n)
}

/// JSON string escaping — enough for the ASCII labels VOID sends.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}
