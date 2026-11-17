// VOID — Discord rich presence + local account link.
//
// Discord's local RPC is a named pipe (`\\.\pipe\discord-ipc-N`) carrying framed
// JSON: an i32 opcode, an i32 little-endian length, then the payload. There is
// no SDK here and no dependency — just `CreateFileW` through `std::fs`, so the
// feature costs a few kilobytes instead of a second runtime.
//
// One writer thread owns the requests; a second thread drains replies, because a
// pipe read blocks and must never stall presence updates. Everything the UI does
// is either a channel send or an atomic load.

use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
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

#[derive(Clone)]
enum Cmd {
    Set(Presence),
    /// Ask Discord who is signed in on this machine.
    WhoAmI,
    /// Drop the connection and reconnect on the next tick.
    Reconnect,
    /// Ask Discord to show its own "allow VOID?" dialog for these scopes.
    Authorize(String),
}

/// What Discord told us about the signed-in account.
#[derive(Clone, Default)]
pub struct Who {
    pub name: String,
    pub id: String,
    /// Set when Discord answered with an error (usually "not authorized").
    pub error: Option<String>,
}

/// Handle held by the app. Dropping it closes the worker.
pub struct Rpc {
    tx: Option<Sender<Cmd>>,
    connected: Arc<AtomicBool>,
    who: Arc<Mutex<Who>>,
    code: Arc<Mutex<Option<String>>>,
}

impl Rpc {
    /// Start the worker. `enabled == false` gives a handle that does nothing, so
    /// call sites never need to branch.
    pub fn start(enabled: bool) -> Rpc {
        let connected = Arc::new(AtomicBool::new(false));
        let who = Arc::new(Mutex::new(Who::default()));
        let code = Arc::new(Mutex::new(None));
        if !enabled {
            return Rpc {
                tx: None,
                connected,
                who,
                code,
            };
        }
        let (tx, rx) = channel::<Cmd>();
        let flag = connected.clone();
        let w = who.clone();
        let c = code.clone();
        std::thread::Builder::new()
            .name("void-rpc".into())
            .spawn(move || worker(rx, flag, w, c))
            .ok();
        Rpc {
            tx: Some(tx),
            connected,
            who,
            code,
        }
    }

    /// Start the OAuth consent dialog inside Discord. The one-time code lands in
    /// `take_code()` a moment later.
    pub fn authorize(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Authorize("identify".to_string()));
        }
    }

    /// The code Discord returned, if the user said yes.
    pub fn take_code(&self) -> Option<String> {
        self.code.lock().ok().and_then(|mut c| c.take())
    }

    pub fn connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// The signed-in Discord account, once the client has answered.
    pub fn who(&self) -> Who {
        self.who.lock().map(|w| w.clone()).unwrap_or_default()
    }

    /// Ask the client for the account again (also used as "retry link").
    pub fn refresh_user(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::WhoAmI);
        }
    }

    pub fn reconnect(&self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Reconnect);
        }
    }

    /// Post a new presence. Cheap, non-blocking, safe to call every frame.
    pub fn set(&self, p: Presence) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Cmd::Set(p));
        }
    }
}

// ── worker ───────────────────────────────────────────────────────────────────

struct Conn {
    pipe: Pipe,
    replies: Receiver<String>,
}

fn worker(
    rx: Receiver<Cmd>,
    connected: Arc<AtomicBool>,
    who: Arc<Mutex<Who>>,
    code: Arc<Mutex<Option<String>>>,
) {
    let mut conn: Option<Conn> = None;
    let mut last: Option<Presence> = None;
    let mut dirty = false;
    let mut want_user = true;
    let mut want_auth = false;
    let mut scopes_wanted = String::from("identify");
    let mut last_try = std::time::Instant::now() - Duration::from_secs(60);

    loop {
        let cmd = match rx.recv_timeout(Duration::from_millis(1500)) {
            Ok(c) => Some(c),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        };

        let mut force = false;
        match cmd {
            Some(Cmd::Set(p)) => {
                if last.as_ref().map(|l| !same(l, &p)).unwrap_or(true) {
                    last = Some(p);
                    dirty = true;
                }
            }
            Some(Cmd::WhoAmI) => {
                want_user = true;
                force = true;
            }
            Some(Cmd::Authorize(scopes)) => {
                scopes_wanted = scopes;
                want_auth = true;
                force = true;
            }
            Some(Cmd::Reconnect) => {
                conn = None;
                connected.store(false, Ordering::Relaxed);
                want_user = true;
                dirty = last.is_some();
                force = true;
            }
            None => {}
        }

        // connect / retry (never more than once every three seconds)
        if conn.is_none() && (force || last.is_some()) && last_try.elapsed() > Duration::from_secs(3)
        {
            last_try = std::time::Instant::now();
            if let Some(p) = Pipe::connect() {
                let replies = p.spawn_reader();
                conn = Some(Conn { pipe: p, replies });
                connected.store(true, Ordering::Relaxed);
                want_user = true;
            }
        }

        // drain replies
        if let Some(c) = conn.as_ref() {
            while let Ok(reply) = c.replies.try_recv() {
                if let Some(hit) = parse_user(&reply) {
                    if let Ok(mut w) = who.lock() {
                        *w = hit;
                    }
                }
                if reply.contains("\"cmd\":\"AUTHORIZE\"") {
                    if let Some(c) = field(&reply, "code") {
                        if let Ok(mut slot) = code.lock() {
                            *slot = Some(c);
                        }
                    } else if let Ok(mut w) = who.lock() {
                        w.error = Some("authorization was declined".to_string());
                    }
                }
            }
            // a reader thread that saw EOF means the pipe is gone
            if c.pipe.dead() {
                conn = None;
                connected.store(false, Ordering::Relaxed);
            }
        }

        let Some(c) = conn.as_mut() else { continue };

        if want_auth {
            want_auth = false;
            let scopes: Vec<String> = scopes_wanted
                .split(',')
                .map(|s| format!("\"{}\"", s.trim()))
                .collect();
            let args = format!(
                r#"{{"client_id":"{}","scopes":[{}]}}"#,
                CLIENT_ID,
                scopes.join(",")
            );
            if c.pipe.request("AUTHORIZE", &args).is_err() {
                conn = None;
                connected.store(false, Ordering::Relaxed);
                continue;
            }
        }

        if want_user {
            want_user = false;
            if let Err(e) = c.pipe.request("GET_CURRENT_USER", "{}") {
                let _ = e;
                conn = None;
                connected.store(false, Ordering::Relaxed);
                continue;
            }
        }

        if dirty {
            if let Some(p) = last.clone() {
                if c.pipe.activity(&p).is_err() {
                    conn = None;
                    connected.store(false, Ordering::Relaxed);
                    continue;
                }
                dirty = false;
            }
        }
    }
}

fn same(a: &Presence, b: &Presence) -> bool {
    a.details == b.details && a.state == b.state && a.large_text == b.large_text && a.mood == b.mood
}

/// Pull `username` / `id` out of a reply, or an error string when Discord
/// refused (a fresh install has to authorize the app once).
fn parse_user(reply: &str) -> Option<Who> {
    if reply.contains("\"evt\":\"ERROR\"") || reply.contains("\"code\":") {
        let mut w = Who::default();
        w.error = Some("authorize VOID inside Discord".to_string());
        return Some(w);
    }
    let name = field(reply, "global_name")
        .or_else(|| field(reply, "username"))
        .unwrap_or_default();
    let id = field(reply, "id").unwrap_or_default();
    if name.is_empty() {
        return None;
    }
    Some(Who {
        name,
        id,
        error: None,
    })
}

fn field(src: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\":\"", key);
    let i = src.find(&pat)? + pat.len();
    let rest = &src[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

// ── pipe ─────────────────────────────────────────────────────────────────────

pub struct Pipe {
    file: std::fs::File,
    dead: Arc<AtomicBool>,
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
                let hello = format!(r#"{{"v":1,"client_id":"{}"}}"#, CLIENT_ID);
                if write_frame(&mut file, 0, &hello).is_ok() {
                    return Some(Pipe {
                        file,
                        dead: Arc::new(AtomicBool::new(false)),
                    });
                }
            }
        }
        None
    }

    #[cfg(not(windows))]
    fn connect() -> Option<Pipe> {
        None
    }

    fn dead(&self) -> bool {
        self.dead.load(Ordering::Relaxed)
    }

    /// A second thread that only reads. Blocking reads belong on their own
    /// thread; the writer must stay free for presence updates.
    fn spawn_reader(&self) -> Receiver<String> {
        let (tx, rx) = channel::<String>();
        let dead = self.dead.clone();
        if let Ok(mut file) = self.file.try_clone() {
            std::thread::Builder::new()
                .name("void-rpc-rx".into())
                .spawn(move || loop {
                    let mut head = [0u8; 8];
                    if file.read_exact(&mut head).is_err() {
                        dead.store(true, Ordering::Relaxed);
                        break;
                    }
                    let len = i32::from_le_bytes([head[4], head[5], head[6], head[7]]) as usize;
                    if len == 0 || len > 1 << 20 {
                        continue;
                    }
                    let mut body = vec![0u8; len];
                    if file.read_exact(&mut body).is_err() {
                        dead.store(true, Ordering::Relaxed);
                        break;
                    }
                    if tx.send(String::from_utf8_lossy(&body).to_string()).is_err() {
                        break;
                    }
                })
                .ok();
        }
        rx
    }

    fn request(&mut self, cmd: &str, args: &str) -> std::io::Result<()> {
        let payload = format!(
            r#"{{"cmd":"{}","nonce":"{}","args":{}}}"#,
            cmd,
            nonce(),
            args
        );
        write_frame(&mut self.file, 1, &payload)
    }

    fn activity(&mut self, p: &Presence) -> std::io::Result<()> {
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
        let args = format!(
            r#"{{"pid":{},"activity":{{{}}}}}"#,
            std::process::id(),
            activity
        );
        self.request("SET_ACTIVITY", &args)
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
