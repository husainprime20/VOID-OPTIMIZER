//! The VOID engine: state, catalog serialisation, apply/revert, telemetry.

// The console companion only uses part of the engine; the native app uses all
// of it. Warnings about one binary not touching a helper are just noise.
#![allow(dead_code)]

pub mod catalog;

use crate::json;
use crate::platform;
use self::catalog::{Action, CATEGORIES, PROFILES, TWEAKS};
use std::collections::BTreeSet;
use std::fs;
use std::thread;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

pub const VERSION: &str = "0.10.1";

// ───────────────────────────── machine ─────────────────────────────

pub struct Machine {
    pub os: String,
    pub host: String,
    pub user: String,
    pub cpu: String,
    pub gpu: String,
    pub admin: bool,
    pub simulated: bool,
}

impl Machine {
    pub fn detect() -> Machine {
        Machine {
            os: platform::os_build(),
            host: platform::hostname(),
            user: platform::username(),
            cpu: platform::cpu_name(),
            gpu: platform::gpu_name(),
            admin: platform::is_admin(),
            simulated: !platform::is_windows(),
        }
    }
}

// ───────────────────────────── state ─────────────────────────────

#[derive(Clone)]
pub struct LogEntry {
    pub at: String,
    pub level: String,
    pub msg: String,
}

#[derive(Clone)]
pub struct State {
    pub applied: BTreeSet<String>,
    pub log: Vec<LogEntry>,
    pub volume: u8,
    pub intensity: u8,
    pub muted: bool,
    pub motion: bool,
    pub auto_backup: bool,
    pub last_profile: String,
    pub started: i64,
    pub ops: u32,
    /// 0 = Obsidian (dark), 1 = Porcelain (light).
    pub theme: u8,
    /// Discord rich presence.
    pub presence: bool,
    /// Self-healing: re-assert applied tweaks that drift.
    pub guard: bool,
    /// Ambient rain layer.
    pub rain: bool,
    /// Apply a title's profile automatically when it launches.
    pub auto_switch: bool,
    /// Discord application client secret, pasted by the user. It never leaves
    /// this machine except to call discord.com during the token exchange.
    pub secret: String,
    /// The drifting black/white band field behind the deck.
    pub bands: bool,
    /// Draw the pointer as a white dot with a lagging ring.
    pub soft_cursor: bool,
    /// Refuse the changes that are known to break an Android emulator
    /// (hypervisor / memory-integrity toggles), and never touch a guest that is
    /// still booting. On by default; it is the difference between a tune and a
    /// bricked instance.
    pub emu_guard: bool,
}

pub struct Outcome {
    pub id: String,
    pub ok: bool,
    pub msg: String,
}

fn truthy(v: &str) -> bool {
    matches!(v, "1" | "true" | "on" | "yes")
}

fn settings_path() -> std::path::PathBuf {
    platform::data_dir().join("settings.conf")
}

fn journal_path() -> std::path::PathBuf {
    platform::data_dir().join("void.log")
}

impl State {
    /// Every default, with no disk reads and no journal write — used by tools that
    /// only need the engine's policy (the repair button, the self-test).
    fn bare() -> State {
        State {
            applied: BTreeSet::new(),
            log: Vec::new(),
            volume: 34,
            intensity: 55,
            muted: false,
            motion: true,
            auto_backup: true,
            last_profile: String::new(),
            started: platform::epoch_secs(),
            ops: 0,
            theme: 0,
            presence: true,
            guard: true,
            rain: true,
            bands: true,
            soft_cursor: true,
            emu_guard: true,
            auto_switch: false,
            secret: String::new(),
        }
    }

    pub fn new() -> State {
        let mut s = State::bare();
        s.load();
        s.push(
            "sys",
            &format!(
                "engine online · {} · {}",
                platform::os_name(),
                if platform::is_windows() {
                    "native"
                } else {
                    "simulation"
                }
            ),
        );
        s
    }

    pub fn push(&mut self, level: &str, msg: &str) {
        self.log.push(LogEntry {
            at: platform::clock_label(),
            level: level.to_string(),
            msg: msg.to_string(),
        });
        if self.log.len() > 500 {
            self.log.remove(0);
        }
        if let Ok(mut f) = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(journal_path())
        {
            use std::io::Write;
            let _ = writeln!(f, "{} [{}] {}", platform::now_stamp(), level, msg);
        }
    }

    pub fn load(&mut self) {
        if let Ok(text) = fs::read_to_string(settings_path()) {
            for line in text.lines() {
                let mut it = line.splitn(2, '=');
                let k = it.next().unwrap_or("").trim();
                let v = it.next().unwrap_or("").trim();
                match k {
                    "volume" => self.volume = v.parse::<u8>().unwrap_or(self.volume).min(100),
                    "intensity" => self.intensity = v.parse::<u8>().unwrap_or(self.intensity).min(100),
                    "muted" => self.muted = truthy(v),
                    "motion" => self.motion = truthy(v),
                    "auto_backup" => self.auto_backup = truthy(v),
                    "last_profile" => self.last_profile = v.to_string(),
                    "theme" => self.theme = v.parse::<u8>().unwrap_or(0).min(1),
                    "presence" => self.presence = truthy(v),
                    "guard" => self.guard = truthy(v),
                    "rain" => self.rain = truthy(v),
                    "bands" => self.bands = truthy(v),
                    "soft_cursor" => self.soft_cursor = truthy(v),
                    "emu_guard" => self.emu_guard = truthy(v),
                    "auto_switch" => self.auto_switch = truthy(v),
                    "secret" => self.secret = v.to_string(),
                    "applied" => {
                        self.applied = v
                            .split(',')
                            .map(|x| x.trim().to_string())
                            .filter(|x| !x.is_empty())
                            .collect();
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn save(&self) {
        let body = format!(
            "volume={}\nintensity={}\nmuted={}\nmotion={}\nauto_backup={}\nlast_profile={}\ntheme={}\npresence={}\nguard={}\nrain={}\nbands={}\nsoft_cursor={}\nemu_guard={}\nauto_switch={}\nsecret={}\napplied={}\n",
            self.volume,
            self.intensity,
            if self.muted { 1 } else { 0 },
            if self.motion { 1 } else { 0 },
            if self.auto_backup { 1 } else { 0 },
            self.last_profile,
            self.theme,
            if self.presence { 1 } else { 0 },
            if self.guard { 1 } else { 0 },
            if self.rain { 1 } else { 0 },
            if self.bands { 1 } else { 0 },
            if self.soft_cursor { 1 } else { 0 },
            if self.emu_guard { 1 } else { 0 },
            if self.auto_switch { 1 } else { 0 },
            self.secret,
            self.applied.iter().cloned().collect::<Vec<String>>().join(",")
        );
        let _ = fs::write(settings_path(), body);
    }

    pub fn set_setting(&mut self, key: &str, value: &str) {
        match key {
            "volume" => {
                if let Ok(v) = value.parse::<u32>() {
                    self.volume = v.min(100) as u8;
                }
            }
            "intensity" => {
                if let Ok(v) = value.parse::<u32>() {
                    self.intensity = v.min(100) as u8;
                }
            }
            "theme" => {
                if let Ok(v) = value.parse::<u32>() {
                    self.theme = v.min(1) as u8;
                }
            }
            "presence" => self.presence = truthy(value),
            "guard" => self.guard = truthy(value),
            "rain" => self.rain = truthy(value),
            "bands" => self.bands = truthy(value),
            "soft_cursor" => self.soft_cursor = truthy(value),
            "emu_guard" => self.emu_guard = truthy(value),
            "auto_switch" => self.auto_switch = truthy(value),
            "secret" => self.secret = value.to_string(),
            "muted" => self.muted = truthy(value),
            "motion" => self.motion = truthy(value),
            "auto_backup" => self.auto_backup = truthy(value),
            _ => {}
        }
        self.save();
    }
}

/// Nothing can be written when the engine is not on Windows (CI renders the
/// UI on Linux). This is the *only* stand-in for a real write — the user-facing
/// dry-run mode is gone, and on Windows every action touches the machine.
pub fn simulated() -> bool {
    !platform::is_windows()
}

// ───────────────────────────── lookup ─────────────────────────────

pub fn catalog_len() -> usize {
    TWEAKS.len()
}

pub fn find_tweak(id: &str) -> Option<&'static catalog::Tweak> {
    TWEAKS.iter().find(|t| t.id == id)
}

pub fn find_profile(id: &str) -> Option<&'static catalog::Profile> {
    PROFILES.iter().find(|p| p.id == id)
}

// ───────────────────────────── command layer ─────────────────────────────

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").trim().chars().take(200).collect()
}

fn do_cmd(_state: &State, program: &str, args: &[&str]) -> (bool, String) {
    let shown = format!("{} {}", program, args.join(" "));
    if simulated() {
        thread::sleep(Duration::from_millis(2));
        return (true, format!("simulated :: {}", shown));
    }
    match platform::run(program, args) {
        Ok(o) => {
            if o.ok {
                (true, shown)
            } else {
                let extra = first_line(&o.combined());
                if extra.is_empty() {
                    (false, format!("{} -> exit {}", shown, o.code))
                } else {
                    (false, format!("{} -> exit {} · {}", shown, o.code, extra))
                }
            }
        }
        Err(e) => (false, format!("{} -> {}", shown, e)),
    }
}

/// Like `do_cmd`, but a non-zero exit is not fatal (missing service / value).
fn do_cmd_lenient(_state: &State, program: &str, args: &[&str]) -> (bool, String) {
    let shown = format!("{} {}", program, args.join(" "));
    if simulated() {
        thread::sleep(Duration::from_millis(2));
        return (true, format!("simulated :: {}", shown));
    }
    match platform::run(program, args) {
        Ok(_) => (true, shown),
        Err(e) => (true, format!("{} -> {}", shown, e)),
    }
}

fn is_guid(s: &str) -> bool {
    let want = [8usize, 4, 4, 4, 12];
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    for (p, w) in parts.iter().zip(want.iter()) {
        if p.len() != *w || !p.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    true
}

fn find_guid(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < 36 {
        return None;
    }
    let mut i = 0;
    while i + 36 <= chars.len() {
        let candidate: String = chars[i..i + 36].iter().collect();
        if is_guid(&candidate) {
            return Some(candidate);
        }
        i += 1;
    }
    None
}

/// Android-emulator process names (without `.exe`) that host a Free Fire client.
const FF_PROC: &[&str] = &[
    "hd-player",
    "bluestacks",
    "bluestacksapp",
    "ldvboxheadless",
    "dnplayer",
    "memu",
    "memuheadless",
    "nox",
    "noxvmhandle",
    "mumuplayer",
    "gameloop",
];

/// The player *window* processes VOID may touch. Service, ADB, frontend and
/// hypervisor hosts are deliberately absent: pinning `BstkSVC`/`HD-Adb`/a VM
/// host while BlueStacks is running is how an instance hangs and then refuses to
/// boot again. Pinning these is the whole point; touching the plumbing is not.
const FF_PLAYER: &[&str] = &[
    "hd-player",
    "dnplayer",
    "mumuplayer",
    "nemuplayer",
    "memu",
    "nox",
    "gameloop",
    "ldvboxheadless",
];

/// A guest that has been alive for less than this is still booting Android; the
/// scheduler must not be rearranged under it.
const EMU_SETTLE_SECS: f64 = 90.0;

/// Player processes that are old enough to touch, plus a note for each one that
/// was skipped because it was still booting.
fn player_pids() -> (Vec<(u32, String)>, Vec<String>) {
    let mut take: Vec<(u32, String)> = Vec::new();
    let mut skip: Vec<String> = Vec::new();
    for (name, pid, _) in platform::processes() {
        let lower = name.to_ascii_lowercase();
        let bare = lower.trim_end_matches(".exe").to_string();
        if !FF_PLAYER.iter().any(|p| bare == *p) {
            continue;
        }
        match platform::proc_age_secs(pid) {
            Some(age) if age < EMU_SETTLE_SECS => {
                skip.push(format!("{} ({}s old)", bare, age as u64));
            }
            _ => take.push((pid, bare)),
        }
    }
    (take, skip)
}

/// Core mask covering physical cores only, never fewer than four logical CPUs.
fn physical_mask() -> Option<usize> {
    let cpus = platform::logical_cpus().min(std::mem::size_of::<usize>() * 8);
    if cpus < 4 {
        return None;
    }
    let mut mask: usize = 0;
    let mut i = 0;
    while i < cpus {
        mask |= 1usize << i;
        i += 2;
    }
    Some(mask)
}

/// Is an Android emulator installed here? Returns its display name.
pub fn emulator_installed() -> Option<&'static str> {
    EMULATORS.iter().find(|s| emu_path(s).is_some()).map(|s| s.name)
}

// ── the boot environment an emulator needs back ─────────────────────────────
//
// A guest that will not boot is almost never the guest's fault. Three changes a
// "latency" tweak makes survive a reboot and land squarely on the hypervisor:
// `disabledynamictick`, `useplatformtick` and `tscsyncpolicy=enhanced` were all
// written for bare metal, and `kernel.vbs` turns the hypervisor off entirely.
// Everything else in the catalog dies with the process.

#[derive(Clone)]
pub struct BootCheck {
    pub id: &'static str,
    pub label: &'static str,
    pub detail: String,
    /// Whether VOID can put this back without the user editing anything.
    pub fix: bool,
    pub ok: bool,
}

fn bcd_enum() -> Option<String> {
    if !platform::is_windows() {
        return None;
    }
    platform::run("bcdedit", &["/enum", "{current}"])
        .ok()
        .map(|o| o.stdout)
}

/// `bcdedit` prints `key<padding>value` — split on the first whitespace run.
fn bcd_field(text: &str, key: &str) -> Option<String> {
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let Some(k) = it.next() else { continue };
        if k.eq_ignore_ascii_case(key) {
            let v: Vec<&str> = it.collect();
            if v.is_empty() {
                return None;
            }
            return Some(v.join(" "));
        }
    }
    None
}

/// Diagnostic: did the registry read find a value at all? `none` means the key
/// is missing (or the read failed), which must not be mistaken for "off".
pub fn reg_probe(path: &str, name: &str) -> String {
    match platform::reg_read_u32(path, name) {
        Some(v) => v.to_string(),
        None => "none".to_string(),
    }
}

fn hvci_on() -> bool {
    if !platform::is_windows() {
        return true;
    }
    let vbs = platform::reg_read_u32(
        r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard",
        "EnableVirtualizationBasedSecurity",
    )
    .unwrap_or(0);
    let hvci = platform::reg_read_u32(
        r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
        "Enabled",
    )
    .unwrap_or(0);
    vbs == 1 || hvci == 1
}

/// Everything about this machine that could stop an emulator instance booting.
/// Read-only, cheap, and safe to call every few seconds.
pub fn boot_checks() -> Vec<BootCheck> {
    let mut out: Vec<BootCheck> = Vec::new();
    // Nothing here exists off Windows, and an empty list keeps the page honest
    // on a platform where the question does not apply.
    if !platform::is_windows() {
        return out;
    }
    let emu = emulator_installed();
    match bcd_enum().as_deref() {
        None => out.push(BootCheck {
            id: "bcd",
            label: "boot config",
            detail: if platform::is_windows() {
                "bcdedit did not answer · run VOID as administrator to read it".to_string()
            } else {
                "not applicable on this platform".to_string()
            },
            fix: false,
            ok: true,
        }),
        Some(text) => {
            let yes = |v: Option<String>| {
                v.map(|s| s.eq_ignore_ascii_case("yes")).unwrap_or(false)
            };
            let dt = bcd_field(text, "disabledynamictick");
            let pt = bcd_field(text, "useplatformtick");
            let tsc = bcd_field(text, "tscsyncpolicy");
            let hv = bcd_field(text, "hypervisorlaunchtype");

            let tick = yes(dt.clone()) || yes(pt.clone());
            out.push(BootCheck {
                id: "tick",
                label: "kernel tick",
                detail: if tick {
                    format!(
                        "locked to the platform clock{} — virtual machines are known to hang on this",
                        if yes(dt) { " · dynamic tick disabled" } else { "" }
                    )
                } else {
                    "Windows default".to_string()
                },
                fix: tick,
                ok: !tick,
            });

            let enhanced = tsc
                .as_deref()
                .map(|v| v.eq_ignore_ascii_case("enhanced"))
                .unwrap_or(false);
            out.push(BootCheck {
                id: "tsc",
                label: "tsc sync",
                detail: if enhanced {
                    "forced enhanced — a guest can disagree with its host clock".to_string()
                } else {
                    "Windows default".to_string()
                },
                fix: enhanced,
                ok: !enhanced,
            });

            let off = hv
                .as_deref()
                .map(|v| v.eq_ignore_ascii_case("off"))
                .unwrap_or(false);
            out.push(BootCheck {
                id: "hypervisor",
                label: "hypervisor",
                detail: if off {
                    "launch type off — an instance created under Hyper-V can fail to boot".to_string()
                } else {
                    format!("{}", hv.unwrap_or_else(|| "auto".to_string()))
                },
                fix: off,
                ok: !off,
            });
        }
    }

    let hvci = hvci_on();
    out.push(BootCheck {
        id: "hvci",
        label: "memory integrity",
        detail: if hvci {
            "on".to_string()
        } else {
            "off — the hypervisor it guards is what an instance was built against".to_string()
        },
        fix: !hvci,
        ok: hvci,
    });

    if let Some(name) = emu {
        let bad = out.iter().filter(|c| !c.ok).count();
        out.push(BootCheck {
            id: "verdict",
            label: "verdict",
            detail: if bad == 0 {
                format!("{} is installed · the boot environment is clean", name)
            } else {
                format!(
                    "{} is installed and {} boot setting{} would stop it — repair restores them",
                    name,
                    bad,
                    if bad == 1 { "" } else { "s" }
                )
            },
            fix: bad > 0,
            ok: bad == 0,
        });
    }
    out
}

pub fn boot_ok() -> bool {
    boot_checks().iter().all(|c| c.ok)
}

fn outcome(id: &str, ok: bool, msg: impl Into<String>) -> Outcome {
    Outcome {
        id: id.to_string(),
        ok,
        msg: msg.into(),
    }
}

/// Put the boot environment back to Windows defaults: the three BCD values the
/// catalog can change, the hypervisor launch type, and memory integrity. Only
/// the ones that are actually wrong are touched, so this is safe to press twice.
pub fn boot_repair() -> Vec<Outcome> {
    let mut out: Vec<Outcome> = Vec::new();
    if !platform::is_windows() {
        out.push(outcome("boot", false, "boot settings are Windows-only"));
        return out;
    }
    for check in boot_checks() {
        match check.id {
            "tick" => {
                for key in ["disabledynamictick", "useplatformtick"] {
                    out.push(match platform::run("bcdedit", &["/set", key, "default"]) {
                        Ok(o) if o.ok => outcome(key, true, format!("{} → default", key)),
                        Ok(o) => outcome(key, false, format!("bcdedit refused · {}", o.stderr.trim())),
                        Err(e) => outcome(key, false, e),
                    });
                }
            }
            "tsc" => out.push(match platform::run("bcdedit", &["/set", "tscsyncpolicy", "default"]) {
                Ok(o) if o.ok => outcome("tscsyncpolicy", true, "tscsyncpolicy → default"),
                Ok(o) => outcome("tscsyncpolicy", false, format!("bcdedit refused · {}", o.stderr.trim())),
                Err(e) => outcome("tscsyncpolicy", false, e),
            }),
            "hypervisor" => out.push(
                match platform::run("bcdedit", &["/set", "hypervisorlaunchtype", "auto"]) {
                    Ok(o) if o.ok => outcome("hypervisorlaunchtype", true, "hypervisor → auto"),
                    Ok(o) => outcome(
                        "hypervisorlaunchtype",
                        false,
                        format!("bcdedit refused · {}", o.stderr.trim()),
                    ),
                    Err(e) => outcome("hypervisorlaunchtype", false, e),
                },
            ),
            "hvci" => {
                let a = platform::reg_write(
                    r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard",
                    "EnableVirtualizationBasedSecurity",
                    platform::RegKind::Dword,
                    "1",
                );
                let b = platform::reg_write(
                    r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
                    "Enabled",
                    platform::RegKind::Dword,
                    "1",
                );
                out.push(outcome(
                    "hvci",
                    a.is_ok() && b.is_ok(),
                    "memory integrity → on (takes effect at the next restart)",
                ));
            }
            _ => {}
        }
    }
    out.push(outcome(
        "reboot",
        true,
        "restart Windows once for the boot changes to take hold",
    ));
    out
}

// ───────────────────────────── specials ─────────────────────────────

/// PIDs of every process whose image name matches an Android emulator.
fn emulator_pids() -> Vec<u32> {
    platform::processes()
        .into_iter()
        .filter(|(name, _, _)| {
            let bare = name.to_ascii_lowercase();
            let bare = bare.trim_end_matches(".exe").to_string();
            FF_PROC.iter().any(|p| bare == *p || bare.starts_with(p))
        })
        .map(|(_, pid, _)| pid)
        .collect()
}

/// Purge %TEMP% and %WINDIR%\Temp using std only — no PowerShell, no console.
fn clean_temp() -> (bool, String) {
    if simulated() {
        return (true, "simulated :: temp purge".to_string());
    }
    // BlueStacks and friends keep live guest state under %TEMP% while they run.
    // Deleting underneath them is a documented way to lose an instance.
    if !emulator_pids().is_empty() {
        return (
            false,
            "an emulator is running — close it before clearing temp".to_string(),
        );
    }
    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    for key in ["TEMP", "TMP"] {
        if let Ok(v) = std::env::var(key) {
            roots.push(std::path::PathBuf::from(v));
        }
    }
    if let Ok(w) = std::env::var("WINDIR") {
        roots.push(std::path::PathBuf::from(w).join("Temp"));
    }
    let mut freed: u64 = 0;
    let mut removed: u32 = 0;
    for root in roots.iter() {
        let entries = match fs::read_dir(root) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let meta = entry.metadata();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let ok = if path.is_dir() {
                fs::remove_dir_all(&path).is_ok()
            } else {
                fs::remove_file(&path).is_ok()
            };
            if ok {
                freed = freed.saturating_add(size);
                removed += 1;
            }
        }
    }
    (
        true,
        format!("{} item(s) cleared  ·  {:.0} MB", removed, freed as f64 / (1024.0 * 1024.0)),
    )
}

fn run_special(state: &mut State, name: &str, revert: bool) -> (bool, String) {
    match name {
        "power_ultimate" => {
            if simulated() {
                thread::sleep(Duration::from_millis(55));
                return (
                    true,
                    "simulated :: powercfg -duplicatescheme <ultimate> -> powercfg -setactive <new guid>"
                        .to_string(),
                );
            }
            // The duplicate gets a *new* GUID, so we must read it back from the
            // command output instead of reusing the well-known one.
            match platform::run("powercfg", &["-duplicatescheme", catalog::ULTIMATE_SCHEME_GUID]) {
                Ok(o) => {
                    let guid = find_guid(&o.combined())
                        .unwrap_or_else(|| catalog::ULTIMATE_SCHEME_GUID.to_string());
                    match platform::run("powercfg", &["-setactive", guid.as_str()]) {
                        Ok(o2) => (
                            o2.ok,
                            format!("ultimate performance -> {}", first_line(&o2.combined())),
                        ),
                        Err(e) => (false, format!("powercfg -setactive -> {}", e)),
                    }
                }
                Err(e) => (false, format!("powercfg -> {}", e)),
            }
        }
        "power_reapply" => {
            let (ok, msg) = do_cmd(state, "powercfg", &["-setactive", "SCHEME_CURRENT"]);
            (ok, format!("scheme re-applied -> {}", first_line(&msg)))
        }
        "net_nagle_off" => net_nagle(state, revert),
        "net_nagle_reset" => net_nagle(state, true),
        "net_congestion" => net_congestion(state, false),
        "net_congestion_default" => net_congestion(state, true),
        "clean_temp" => clean_temp(),
        "ff_priority" => {
            let (pids, skipped) = player_pids();
            if pids.is_empty() {
                return (
                    false,
                    if skipped.is_empty() {
                        "no emulator window is running".to_string()
                    } else {
                        format!(
                            "waiting for the guest to settle  ·  {}",
                            skipped.join(", ")
                        )
                    },
                );
            }
            let mut ok = 0;
            let mut last = String::new();
            for (pid, name) in pids.iter() {
                match platform::set_priority(*pid, "High") {
                    Ok(m) => {
                        ok += 1;
                        last = format!("{} {}", name, m);
                    }
                    Err(e) => last = e,
                }
            }
            let note = if skipped.is_empty() {
                String::new()
            } else {
                format!("  ·  skipped {}", skipped.join(", "))
            };
            (
                ok > 0,
                format!("{} window(s) at HIGH  ·  {}{}", ok, last, note),
            )
        }
        "ff_priority_reset" => {
            let mut ok = 0;
            for (name, pid, _) in platform::processes().iter() {
                let lower = name.to_ascii_lowercase();
                let bare = lower.trim_end_matches(".exe").to_string();
                if FF_PLAYER.iter().any(|p| bare == *p)
                    || FF_PROC.iter().any(|p| bare.starts_with(p))
                {
                    if platform::set_priority(*pid, "Normal").is_ok() {
                        ok += 1;
                    }
                }
            }
            (ok > 0, format!("{} process(es) back to NORMAL", ok))
        }
        "ff_affinity" => {
            let Some(mask) = physical_mask() else {
                return (
                    false,
                    "fewer than four logical CPUs — pinning would starve the guest".to_string(),
                );
            };
            let (pids, skipped) = player_pids();
            if pids.is_empty() {
                return (
                    false,
                    if skipped.is_empty() {
                        "no emulator window is running".to_string()
                    } else {
                        format!(
                            "waiting for the guest to settle  ·  {}",
                            skipped.join(", ")
                        )
                    },
                );
            }
            let mut ok = 0;
            let mut last = String::new();
            for (pid, name) in pids.iter() {
                match platform::set_affinity(*pid, mask) {
                    Ok(_) => {
                        ok += 1;
                        last = format!("{} mask {:#x}", name, mask);
                    }
                    Err(e) => last = e,
                }
            }
            let note = if skipped.is_empty() {
                String::new()
            } else {
                format!("  ·  skipped {}", skipped.join(", "))
            };
            (
                ok > 0,
                format!("{} window(s) pinned  ·  {}{}", ok, last, note),
            )
        }
        "ff_affinity_reset" => {
            let cpus = platform::logical_cpus().min(std::mem::size_of::<usize>() * 8);
            let full: usize = if cpus >= std::mem::size_of::<usize>() * 8 {
                usize::MAX
            } else {
                (1usize << cpus) - 1
            };
            let mut ok = 0;
            for (name, pid, _) in platform::processes().iter() {
                let lower = name.to_ascii_lowercase();
                let bare = lower.trim_end_matches(".exe").to_string();
                if FF_PLAYER.iter().any(|p| bare == *p)
                    || FF_PROC.iter().any(|p| bare.starts_with(p))
                {
                    if platform::set_affinity(*pid, full).is_ok() {
                        ok += 1;
                    }
                }
            }
            (ok > 0, format!("{} process(es) back on all {} cores", ok, cpus))
        }
        "ff_repair" => {
            let (a, ma) = run_special(state, "ff_priority_reset", false);
            let (b, mb) = run_special(state, "ff_affinity_reset", false);
            let (c, mc) = run_special(state, "ff_gpu_pref_reset", false);
            (
                a || b || c,
                format!("priority: {}  ·  cores: {}  ·  gpu: {}", ma, mb, mc),
            )
        }
        "ff_boost" => {
            let (a, ma) = run_special(state, "ff_priority", false);
            let (b, mb) = run_special(state, "ff_affinity", false);
            (a || b, format!("{}  ·  {}", ma, mb))
        }
        "ff_gpu_pref" => {
            if simulated() {
                return (true, "simulated :: dedicated GPU preference".to_string());
            }
            match emulator_exe() {
                Some(path) => match platform::set_gpu_preference(&path, true) {
                    Ok(m) => (true, m),
                    Err(e) => (false, e),
                },
                None => (
                    false,
                    "no emulator found — install MSI App Player, BlueStacks, LDPlayer…".to_string(),
                ),
            }
        }
        "ff_gpu_pref_reset" => {
            if simulated() {
                return (true, "simulated :: GPU preference cleared".to_string());
            }
            match emulator_exe() {
                Some(path) => match platform::set_gpu_preference(&path, false) {
                    Ok(m) => (true, m),
                    Err(e) => (false, e),
                },
                None => (true, "no emulator found — nothing to clear".to_string()),
            }
        }
        "timer_hold" => {
            if simulated() {
                return (true, "simulated :: timer held at 0.5 ms".to_string());
            }
            match platform::hold_timer(true) {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            }
        }
        "timer_release" => {
            if simulated() {
                return (true, "simulated :: timer released".to_string());
            }
            match platform::hold_timer(false) {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            }
        }
        "restore_point" => {
            if simulated() {
                return (true, "simulated :: system checkpoint".to_string());
            }
            match platform::create_restore_point("VOID checkpoint") {
                Ok(m) => (true, m),
                Err(e) => (false, e),
            }
        }
        other => (false, format!("unknown special :: {}", other)),
    }
}

fn net_nagle(state: &mut State, revert: bool) -> (bool, String) {
    if simulated() {
        thread::sleep(Duration::from_millis(45));
        return (
            true,
            if revert {
                "simulated :: nagle values removed".to_string()
            } else {
                "simulated :: nagle disabled on all interfaces".to_string()
            },
        );
    }
    let key = "HKLM\\SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces";
    match platform::run("reg", &["query", key]) {
        Ok(o) => {
            let mut count = 0;
            let ops: [(&str, &str); 3] = [
                ("TcpAckFrequency", "1"),
                ("TCPNoDelay", "1"),
                ("TcpDelAckTicks", "0"),
            ];
            for line in o.combined().lines() {
                let sub = line.trim().to_string();
                if sub.starts_with("HKEY_LOCAL_MACHINE") {
                    for &(v, d) in ops.iter() {
                        if revert {
                            let _ = do_cmd_lenient(
                                state,
                                "reg",
                                &["delete", sub.as_str(), "/v", v, "/f"],
                            );
                        } else {
                            let _ = do_cmd_lenient(
                                state,
                                "reg",
                                &[
                                    "add",
                                    sub.as_str(),
                                    "/v",
                                    v,
                                    "/t",
                                    "REG_DWORD",
                                    "/d",
                                    d,
                                    "/f",
                                ],
                            );
                        }
                    }
                    count += 1;
                }
            }
            let verb = if revert { "cleared" } else { "disabled" };
            (count > 0, format!("nagle {} on {} interface(s)", verb, count))
        }
        Err(e) => (false, e),
    }
}

fn net_congestion(state: &mut State, revert: bool) -> (bool, String) {
    if revert {
        let (ok, msg) = do_cmd(
            state,
            "netsh",
            &[
                "int",
                "tcp",
                "set",
                "supplemental",
                "Internet",
                "congestionprovider=default",
            ],
        );
        if ok {
            return (true, format!("congestion provider -> default ({})", first_line(&msg)));
        }
        let (ok2, msg2) = do_cmd(
            state,
            "netsh",
            &[
                "int",
                "tcp",
                "set",
                "supplemental",
                "template=internet",
                "congestionprovider=default",
            ],
        );
        return (ok2, format!("congestion provider -> default ({})", first_line(&msg2)));
    }
    let tries: [&[&str]; 3] = [
        &[
            "int",
            "tcp",
            "set",
            "supplemental",
            "Internet",
            "congestionprovider=ctcp",
        ],
        &[
            "int",
            "tcp",
            "set",
            "supplemental",
            "template=internet",
            "congestionprovider=ctcp",
        ],
        &[
            "int",
            "tcp",
            "set",
            "supplemental",
            "Internet",
            "congestionprovider=bbr2",
        ],
    ];
    let mut last = String::from("no attempt made");
    for args in tries.iter() {
        let (ok, msg) = do_cmd(state, "netsh", *args);
        if ok {
            return (true, format!("congestion provider -> {}", msg));
        }
        last = msg;
    }
    (false, last)
}

fn execute(state: &mut State, action: &Action, revert: bool) -> (bool, String) {
    match action {
        Action::Reg(k, v, t, d) => {
            if simulated() {
                return (true, format!("simulated :: {}\\{} = {}", k, v, d));
            }
            let kind = platform::RegKind::from_tag(t);
            match platform::reg_write(k, v, kind, d) {
                Ok(verified) => (
                    true,
                    if verified {
                        format!("{}\\{} = {}  ·  verified", k, v, d)
                    } else {
                        format!("{}\\{} = {}  ·  written", k, v, d)
                    },
                ),
                Err(e) => (false, format!("{}\\{}  ·  {}", k, v, e)),
            }
        }
        Action::RegDel(k, v) => {
            if simulated() {
                return (true, format!("simulated :: delete {}\\{}", k, v));
            }
            match platform::reg_delete(k, v) {
                Ok(_) => (true, format!("{}\\{} removed", k, v)),
                Err(e) => (false, format!("{}\\{}  ·  {}", k, v, e)),
            }
        }
        Action::Cmd(p, args) => do_cmd(state, *p, *args),
        Action::Svc(name) => {
            if revert {
                let (a, m1) = do_cmd_lenient(state, "sc", &["config", *name, "start=", "auto"]);
                let (b, m2) = do_cmd_lenient(state, "sc", &["start", *name]);
                (a || b, format!("{} · {}", m1, m2))
            } else {
                let (a, m1) = do_cmd_lenient(state, "sc", &["config", *name, "start=", "disabled"]);
                let (b, m2) = do_cmd_lenient(state, "sc", &["stop", *name]);
                (a || b, format!("{} · {}", m1, m2))
            }
        }
        Action::SvcEnable(name) => {
            let (a, m1) = do_cmd_lenient(state, "sc", &["config", *name, "start=", "auto"]);
            let (b, m2) = do_cmd_lenient(state, "sc", &["start", *name]);
            (a || b, format!("{} · {}", m1, m2))
        }
        Action::Special(s) => run_special(state, s, revert),
    }
}

// ───────────────────────────── apply / revert ─────────────────────────────

fn apply_one(state: &mut State, t: &'static catalog::Tweak, revert: bool) -> Outcome {
    // The emulator guard. Turning memory integrity off flips the hypervisor the
    // next time Windows boots — and an Android instance that was created against
    // the old hypervisor can come back unable to start. That is not a risk worth
    // taking silently on a machine that has an emulator installed.
    if !revert && state.emu_guard && matches!(t.id, "kernel.vbs" | "kernel.timer" | "kernel.tsc") {
        if let Some(name) = emulator_installed() {
            let what = match t.id {
                "kernel.vbs" => "flips the hypervisor",
                "kernel.timer" => "locks the kernel tick to the platform clock, which hangs guests",
                _ => "forces the enhanced TSC policy, which desyncs a guest from its host clock",
            };
            return Outcome {
                id: t.id.to_string(),
                ok: false,
                msg: format!(
                    "blocked · {} is installed and this {}. Settings → Emulator guard overrides it.",
                    name, what
                ),
            };
        }
    }
    if !revert && state.auto_backup && t.risk == 2 && platform::is_windows() {
        if let Err(e) = platform::create_restore_point("VOID auto checkpoint") {
            state.push("err", &format!("auto checkpoint :: {}", e));
        }
    }
    let actions = if revert { t.revert } else { t.apply };
    let mut msgs: Vec<String> = Vec::new();
    let mut ok = true;
    for action in actions.iter() {
        let (aok, msg) = execute(state, action, revert);
        if !aok {
            ok = false;
        }
        msgs.push(msg);
    }
    if msgs.is_empty() {
        msgs.push("no action required".to_string());
    }
    Outcome {
        id: t.id.to_string(),
        ok,
        msg: msgs.join("  ·  "),
    }
}

pub fn apply_ids(state: &mut State, ids: &[String]) -> Vec<Outcome> {
    // The gate lives here, not in the window: every caller — a view, a profile,
    // match mode, an auto-switch — arrives through this function.
    if !crate::auth::gate_open() {
        return ids
            .iter()
            .map(|id| Outcome {
                id: id.clone(),
                ok: false,
                msg: "locked · sign in with Discord to apply".to_string(),
            })
            .collect();
    }
    let mut out: Vec<Outcome> = Vec::new();
    for id in ids.iter() {
        match find_tweak(id) {
            Some(t) => {
                let o = apply_one(state, t, false);
                if o.ok {
                    if simulated() {
                        // nothing was written to the machine — do not pretend
                    } else if t.once {
                        state.applied.remove(t.id);
                    } else {
                        state.applied.insert(t.id.to_string());
                    }
                    state.ops = state.ops.wrapping_add(1);
                }
                let level = if o.ok { "ok" } else { "err" };
                state.push(level, &format!("{} :: {}", t.name, o.msg));
                out.push(o);
            }
            None => {
                state.push("err", &format!("unknown tweak {}", id));
                out.push(Outcome {
                    id: id.clone(),
                    ok: false,
                    msg: "unknown tweak".to_string(),
                });
            }
        }
    }
    state.save();
    out
}

pub fn apply_profile(state: &mut State, pid: &str) -> Vec<Outcome> {
    match find_profile(pid) {
        Some(p) => {
            let ids: Vec<String> = p.tweaks.iter().map(|s| s.to_string()).collect();
            state.last_profile = p.id.to_string();
            state.push("sys", &format!("profile engaged :: {}", p.name));
            apply_ids(state, &ids)
        }
        None => {
            state.push("err", &format!("unknown profile {}", pid));
            vec![Outcome {
                id: pid.to_string(),
                ok: false,
                msg: "unknown profile".to_string(),
            }]
        }
    }
}

pub fn revert_ids(state: &mut State, ids: &[String]) -> Vec<Outcome> {
    let mut out: Vec<Outcome> = Vec::new();
    for id in ids.iter() {
        if let Some(t) = find_tweak(id) {
            let o = apply_one(state, t, true);
            state.applied.remove(t.id);
            let level = if o.ok { "ok" } else { "err" };
            state.push(level, &format!("reverted {} :: {}", t.name, o.msg));
            out.push(o);
        }
    }
    state.save();
    out
}

pub fn revert_all(state: &mut State) -> Vec<Outcome> {
    let ids: Vec<String> = state.applied.iter().cloned().collect();
    if ids.is_empty() {
        state.push("sys", "nothing to revert");
        return vec![Outcome {
            id: "*".to_string(),
            ok: true,
            msg: "nothing to revert".to_string(),
        }];
    }
    let out = revert_ids(state, &ids);
    state.applied.clear();
    state.save();
    out
}

pub fn restore_point(state: &mut State) -> Outcome {
    let (ok, msg) = run_special(state, "restore_point", false);
    state.push(
        if ok { "ok" } else { "err" },
        &format!("restore point :: {}", msg),
    );
    Outcome {
        id: "sys.restore".to_string(),
        ok,
        msg,
    }
}

// ───────────────────────────── serialisation ─────────────────────────────

pub fn catalog_json(state: &State) -> String {
    let mut tweaks: Vec<String> = Vec::new();
    for t in TWEAKS.iter() {
        let plan: Vec<String> = t
            .apply
            .iter()
            .map(|a| json::esc(&catalog::action_text(a)))
            .collect();
        let rev: Vec<String> = t
            .revert
            .iter()
            .map(|a| json::esc(&catalog::action_text(a)))
            .collect();
        tweaks.push(json::obj(vec![
            json::ks("id", t.id),
            json::ks("name", t.name),
            json::ks("cat", t.cat),
            json::ks("desc", t.desc),
            json::ks("why", t.why),
            json::ki("risk", t.risk as i64),
            json::kb("reboot", t.reboot),
            json::kb("once", t.once),
            json::kb("applied", state.applied.contains(t.id)),
            json::kv("plan", json::arr(plan)),
            json::kv("revplan", json::arr(rev)),
        ]));
    }

    let cats: Vec<String> = CATEGORIES
        .iter()
        .map(|&(id, label)| json::obj(vec![json::ks("id", id), json::ks("label", label)]))
        .collect();

    let profiles: Vec<String> = PROFILES
        .iter()
        .map(|p| {
            let ids: Vec<String> = p.tweaks.iter().map(|s| json::esc(s)).collect();
            json::obj(vec![
                json::ks("id", p.id),
                json::ks("name", p.name),
                json::ks("tagline", p.tagline),
                json::ks("note", p.note),
                json::ki("count", p.tweaks.len() as i64),
                json::kv("tweaks", json::arr(ids)),
            ])
        })
        .collect();

    json::obj(vec![
        json::kv("tweaks", json::arr(tweaks)),
        json::kv("categories", json::arr(cats)),
        json::kv("profiles", json::arr(profiles)),
        json::ks("version", VERSION),
    ])
}

pub fn state_json(state: &State, machine: &Machine) -> String {
    let applied: Vec<String> = state.applied.iter().map(|s| json::esc(s)).collect();
    json::obj(vec![
        json::ks("version", VERSION),
        json::ks("os", &machine.os),
        json::ks("host", &machine.host),
        json::ks("user", &machine.user),
        json::ks("cpu", &machine.cpu),
        json::ks("gpu", &machine.gpu),
        json::kb("admin", machine.admin),
        json::kb("simulated", machine.simulated),
        json::kb("native_engine", !simulated()),
        json::kv("applied", json::arr(applied)),
        json::kv(
            "settings",
            json::obj(vec![
                json::ki("volume", state.volume as i64),
                json::kb("muted", state.muted),
                json::kb("motion", state.motion),
                json::kb("auto_backup", state.auto_backup),
                json::ks("last_profile", &state.last_profile),
            ]),
        ),
        json::ki("uptime", platform::epoch_secs() - state.started),
        json::ki("ops", state.ops as i64),
        json::ki("total_tweaks", TWEAKS.len() as i64),
        json::ki("applied_count", state.applied.len() as i64),
        json::ks("now", &platform::now_stamp()),
    ])
}

pub fn log_json(state: &State) -> String {
    let items: Vec<String> = state
        .log
        .iter()
        .map(|l| {
            json::obj(vec![
                json::ks("at", &l.at),
                json::ks("level", &l.level),
                json::ks("msg", &l.msg),
            ])
        })
        .collect();
    json::arr(items)
}

pub fn outcomes_json(outcomes: &[Outcome]) -> String {
    let items: Vec<String> = outcomes
        .iter()
        .map(|o| {
            json::obj(vec![
                json::ks("id", &o.id),
                json::kb("ok", o.ok),
                json::ks("msg", &o.msg),
            ])
        })
        .collect();
    json::arr(items)
}

// ───────────────────────────── telemetry ─────────────────────────────

pub fn stats_json() -> String {
    // Two samples 120 ms apart so the CPU figure is a real delta, not a zero.
    let _ = sample();
    thread::sleep(Duration::from_millis(120));
    let s = sample();
    json::obj(vec![
        json::kf("cpu", s.cpu),
        json::kf("ram", s.ram),
        json::kf("ram_total", s.total_gb),
        json::kf("ram_used", s.used_gb),
        json::ki("uptime", s.uptime),
        json::ki("ts", platform::epoch_secs()),
    ])
}

fn stats_unix() -> (f64, f64, f64) {
    let first = read_proc_stat();
    thread::sleep(Duration::from_millis(120));
    let second = read_proc_stat();
    let cpu = match (first, second) {
        (Some((i1, t1)), Some((i2, t2))) => {
            let di = i2.saturating_sub(i1) as f64;
            let dt = t2.saturating_sub(t1) as f64;
            if dt > 0.0 {
                ((dt - di) / dt * 100.0).clamp(0.0, 100.0)
            } else {
                0.0
            }
        }
        _ => 0.0,
    };
    let (total, used) = mem_unix();
    (cpu, total, used)
}

fn read_proc_stat() -> Option<(u64, u64)> {
    let text = fs::read_to_string("/proc/stat").ok()?;
    let line = text.lines().next()?;
    let mut parts = line.split_whitespace();
    let _name = parts.next();
    let mut vals: Vec<u64> = Vec::new();
    for p in parts {
        if let Ok(v) = p.parse::<u64>() {
            vals.push(v);
        }
    }
    if vals.len() < 4 {
        return None;
    }
    let total: u64 = vals.iter().sum();
    let idle = vals[3] + vals.get(4).copied().unwrap_or(0);
    Some((idle, total))
}

fn mem_unix() -> (f64, f64) {
    let mut total = 0.0f64;
    let mut avail = 0.0f64;
    if let Ok(text) = fs::read_to_string("/proc/meminfo") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                total = kb_to_gb(rest);
            }
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail = kb_to_gb(rest);
            }
        }
    }
    let used = if total > avail { total - avail } else { 0.0 };
    (total, used)
}

fn kb_to_gb(raw: &str) -> f64 {
    let cleaned = raw.trim().trim_end_matches("kB").trim();
    cleaned.parse::<f64>().unwrap_or(0.0) / 1024.0 / 1024.0
}

/// Measures real sleep-precision jitter — works on every platform.
pub fn bench_json() -> String {
    let mut samples: Vec<f64> = Vec::with_capacity(120);
    for _ in 0..120 {
        let t0 = Instant::now();
        thread::sleep(Duration::from_millis(1));
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let n = samples.len() as f64;
    let avg = samples.iter().sum::<f64>() / n;
    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = sorted[sorted.len() / 2];
    let p95 = sorted[((sorted.len() as f64) * 0.95) as usize % sorted.len()];
    let max = sorted[sorted.len() - 1];
    let overshoot = (avg - 1.0).max(0.0);
    let score = (100.0 - overshoot * 6.0).clamp(0.0, 100.0);
    json::obj(vec![
        json::kf("avg_ms", avg),
        json::kf("p50_ms", p50),
        json::kf("p95_ms", p95),
        json::kf("max_ms", max),
        json::kf("jitter_ms", overshoot),
        json::kf("score", score),
        json::ki("samples", sorted.len() as i64),
        json::ki("ts", platform::epoch_secs()),
    ])
}

// ───────────────────── typed telemetry for the native UI ─────────────────────

#[derive(Clone, Copy, Default)]
pub struct Stats {
    pub cpu: f64,
    pub ram: f64,
    pub total_gb: f64,
    pub used_gb: f64,
    pub uptime: i64,
}

static CPU_IDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static CPU_TOTAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn sample() -> Stats {
    // Native counters only: no PowerShell, no sleeping on the UI thread.
    let (total_gb, used_gb) = platform::ram();
    let cpu = match platform::cpu_ticks() {
        Some((idle, total)) => {
            let prev_idle = CPU_IDLE.swap(idle, Ordering::Relaxed);
            let prev_total = CPU_TOTAL.swap(total, Ordering::Relaxed);
            if prev_total == 0 || total <= prev_total {
                0.0
            } else {
                let d_total = (total - prev_total) as f64;
                let d_idle = idle.saturating_sub(prev_idle) as f64;
                (((d_total - d_idle) / d_total) * 100.0).clamp(0.0, 100.0)
            }
        }
        None => 0.0,
    };
    Stats {
        cpu,
        ram: if total_gb > 0.0 {
            used_gb / total_gb * 100.0
        } else {
            0.0
        },
        total_gb,
        used_gb,
        uptime: platform::uptime_secs(),
    }
}

#[derive(Clone, Copy, Default)]
pub struct BenchResult {
    pub avg: f64,
    pub p50: f64,
    pub p95: f64,
    pub max: f64,
    pub jitter: f64,
    pub score: f64,
    pub samples: usize,
}

pub fn bench() -> BenchResult {
    let mut samples: Vec<f64> = Vec::with_capacity(120);
    for _ in 0..120 {
        let t0 = Instant::now();
        thread::sleep(Duration::from_millis(1));
        samples.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    let n = samples.len() as f64;
    let avg = samples.iter().sum::<f64>() / n;
    let mut sorted = samples.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p50 = sorted[sorted.len() / 2];
    let p95 = sorted[((sorted.len() as f64) * 0.95) as usize % sorted.len()];
    let max = sorted[sorted.len() - 1];
    let jitter = (avg - 1.0).max(0.0);
    BenchResult {
        avg,
        p50,
        p95,
        max,
        jitter,
        score: (100.0 - jitter * 6.0).clamp(0.0, 100.0),
        samples: sorted.len(),
    }
}

// ───────────────────────── android emulators (Free Fire) ─────────────────────

pub struct Emulator {
    pub name: &'static str,
    pub path: String,
    pub installed: bool,
    pub running: bool,
}

/// How an emulator is found: a name, the player executable inside its install
/// directory, and the places Windows records that directory.
pub struct EmuSpec {
    pub name: &'static str,
    pub exe: &'static str,
    /// Registry values that may hold the install directory.
    pub reg: &'static [(&'static str, &'static str)],
    /// Fallback install directories (with the exe appended).
    pub paths: &'static [&'static str],
}

pub const EMULATORS: &[EmuSpec] = &[
    EmuSpec {
        name: "MSI App Player 5",
        exe: "HD-Player.exe",
        reg: &[
            (r"HKLM\SOFTWARE\BlueStacks_msi5", "InstallDir"),
            (r"HKLM\SOFTWARE\BlueStacks_msi5", "DataDir"),
        ],
        paths: &[
            r"C:\Program Files\BlueStacks_msi5",
            r"C:\Program Files\MSI\MSI App Player",
            r"C:\Program Files (x86)\MSI\MSI App Player",
            r"C:\Program Files\BlueStacks_msi5_nxt",
        ],
    },
    EmuSpec {
        name: "MSI App Player 4",
        exe: "HD-Player.exe",
        reg: &[
            (r"HKLM\SOFTWARE\BlueStacks_msi2", "InstallDir"),
            (r"HKLM\SOFTWARE\WOW6432Node\BlueStacks_msi2", "InstallDir"),
        ],
        paths: &[
            r"C:\Program Files\BlueStacks_msi2",
            r"C:\Program Files (x86)\BlueStacks_msi2",
        ],
    },
    EmuSpec {
        name: "BlueStacks 5",
        exe: "HD-Player.exe",
        reg: &[
            (r"HKLM\SOFTWARE\BlueStacks_nxt", "InstallDir"),
            (r"HKLM\SOFTWARE\BlueStacks_nxt", "UserDefinedDir"),
        ],
        paths: &[r"C:\Program Files\BlueStacks_nxt"],
    },
    EmuSpec {
        name: "BlueStacks 4",
        exe: "HD-Player.exe",
        reg: &[
            (r"HKLM\SOFTWARE\BlueStacks", "InstallDir"),
            (r"HKLM\SOFTWARE\WOW6432Node\BlueStacks", "InstallDir"),
        ],
        paths: &[
            r"C:\Program Files\BlueStacks",
            r"C:\Program Files (x86)\BlueStacks",
        ],
    },
    EmuSpec {
        name: "LDPlayer 9",
        exe: "dnplayer.exe",
        reg: &[
            (r"HKLM\SOFTWARE\leidian\ldplayer9", "InstallDir"),
            (r"HKLM\SOFTWARE\WOW6432Node\leidian\ldplayer9", "InstallDir"),
        ],
        paths: &[
            r"C:\LDPlayer\LDPlayer9",
            r"D:\LDPlayer\LDPlayer9",
            r"C:\Program Files\LDPlayer\LDPlayer9",
        ],
    },
    EmuSpec {
        name: "LDPlayer 5",
        exe: "dnplayer.exe",
        reg: &[
            (r"HKLM\SOFTWARE\leidian\ldplayer", "InstallDir"),
            (r"HKLM\SOFTWARE\WOW6432Node\leidian\ldplayer", "InstallDir"),
        ],
        paths: &[r"C:\LDPlayer\LDPlayer5", r"C:\LDPlayer\LDPlayer4.0"],
    },
    EmuSpec {
        name: "MuMu Player 12",
        exe: "MuMuPlayer.exe",
        reg: &[
            (r"HKLM\SOFTWARE\Netease\MuMuPlayer-12.0", "InstallDir"),
            (r"HKLM\SOFTWARE\WOW6432Node\Netease\MuMuPlayer-12.0", "InstallDir"),
        ],
        paths: &[r"C:\Program Files\Netease\MuMuPlayer-12.0\shell"],
    },
    EmuSpec {
        name: "MuMu Player 6",
        exe: "NemuPlayer.exe",
        reg: &[(r"HKLM\SOFTWARE\Netease\MuMu", "InstallDir")],
        paths: &[r"C:\Program Files\Netease\MuMu\emulator\nemu\EmulatorShell"],
    },
    EmuSpec {
        name: "MEmu",
        exe: "MEmu.exe",
        reg: &[
            (r"HKLM\SOFTWARE\Microvirt\MEmu", "InstallPath"),
            (r"HKLM\SOFTWARE\WOW6432Node\Microvirt\MEmu", "InstallPath"),
        ],
        paths: &[r"C:\Program Files\Microvirt\MEmu"],
    },
    EmuSpec {
        name: "Nox",
        exe: "Nox.exe",
        reg: &[(r"HKLM\SOFTWARE\WOW6432Node\Nox", "InstallDir")],
        paths: &[r"C:\Program Files (x86)\Nox\bin", r"C:\Program Files\Nox\bin"],
    },
    EmuSpec {
        name: "GameLoop",
        exe: "GameLoop.exe",
        reg: &[
            (r"HKLM\SOFTWARE\Tencent\GameLoop", "InstallPath"),
            (r"HKLM\SOFTWARE\WOW6432Node\Tencent\GameLoop", "InstallPath"),
        ],
        paths: &[
            r"C:\Program Files\Tencent\GameLoop\Launcher",
            r"C:\Program Files (x86)\Tencent\GameLoop\Launcher",
            r"C:\Program Files\SmartGame\GameLoop",
        ],
    },
    EmuSpec {
        name: "Google Play Games",
        exe: "Bootstrapper.exe",
        reg: &[(r"HKCU\SOFTWARE\Google\Play Games", "InstallLocation")],
        paths: &[r"C:\Program Files\Google\Play Games"],
    },
];

fn join_exe(dir: &str, exe: &str) -> String {
    if dir.ends_with('\\') {
        format!("{}{}", dir, exe)
    } else {
        format!("{}\\{}", dir, exe)
    }
}

/// Best-effort install location for one emulator: registry → known paths →
/// Start-menu shortcut. Returns the launchable path.
fn emu_path(spec: &EmuSpec) -> Option<String> {
    for (key, value) in spec.reg.iter() {
        if let Some(dir) = platform::reg_read_string(key, value) {
            let dir = dir.trim().trim_end_matches('"').to_string();
            if dir.is_empty() {
                continue;
            }
            let candidate = if dir.to_ascii_lowercase().ends_with(".exe") {
                dir.clone()
            } else {
                join_exe(&dir, spec.exe)
            };
            if platform::exists(&candidate) {
                return Some(candidate);
            }
            // the player usually lives one level down (…\shell, …\Launcher)
            for sub in ["shell", "Launcher", "bin", "canvas"] {
                let deeper = format!("{}\\{}\\{}", dir.trim_end_matches('\\'), sub, spec.exe);
                if platform::exists(&deeper) {
                    return Some(deeper);
                }
            }
        }
    }
    for dir in spec.paths.iter() {
        let candidate = join_exe(dir, spec.exe);
        if platform::exists(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Path of the first installed emulator, for the GPU-preference special.
pub fn emulator_exe() -> Option<String> {
    for spec in EMULATORS.iter() {
        if let Some(path) = emu_path(spec) {
            return Some(path);
        }
    }
    None
}

pub fn emulators() -> Vec<Emulator> {
    // one process-table read, not nine PowerShell round-trips
    let running: Vec<String> = if platform::is_windows() {
        platform::processes()
            .into_iter()
            .map(|(n, _, _)| n.to_lowercase())
            .collect()
    } else {
        Vec::new()
    };
    EMULATORS
        .iter()
        .map(|spec| {
            let found = emu_path(spec);
            let needle = spec.exe.to_lowercase();
            Emulator {
                name: spec.name,
                path: found.clone().unwrap_or_default(),
                installed: found.is_some(),
                running: running.iter().any(|p| p == &needle || p.starts_with(&needle)),
            }
        })
        .collect()
}

/// Raise the priority of whichever emulator is currently running.
pub fn boost_emulator() -> Result<String, String> {
    let (pids, skipped) = player_pids();
    if pids.is_empty() {
        return Err(if skipped.is_empty() {
            "no emulator window is running".to_string()
        } else {
            format!(
                "the guest is still booting ({}). Let it settle, then boost.",
                skipped.join(", ")
            )
        });
    }
    let mut ok = 0;
    let mut err = String::new();
    for (pid, _) in pids.iter() {
        match platform::set_priority(*pid, "High") {
            Ok(_) => ok += 1,
            Err(e) => err = e,
        }
    }
    if ok == 0 {
        return Err(format!("nothing changed · {}", err));
    }
    Ok(format!(
        "{} window(s) at HIGH{} · never the VM service, never a booting guest",
        ok,
        if skipped.is_empty() {
            String::new()
        } else {
            format!(" · skipped {}", skipped.join(", "))
        }
    ))
}

/// Run one of the named specials from outside the engine (the repair button).
pub fn run_named(name: &str) -> (bool, String) {
    let mut state = State::bare();
    run_special(&mut state, name, false)
}

pub fn launch_emulator(path: &str) -> Result<String, String> {
    platform::spawn_detached(path).map(|_| "launcher started".to_string())
}

/// Games VOID recognises by process name, with the loadout it recommends.
/// Nothing is launched, injected or hooked — this is a name match on `tasklist`.
pub const GAMES: &[(&str, &str, &str)] = &[
    ("cs2.exe", "Counter-Strike 2", "competitive"),
    ("valorant.exe", "Valorant", "competitive"),
    ("valorant-win64-shipping.exe", "Valorant", "competitive"),
    ("r5apex.exe", "Apex Legends", "competitive"),
    ("rainbowsix.exe", "Rainbow Six Siege", "competitive"),
    ("fortniteclient-win64-shipping.exe", "Fortnite", "competitive"),
    ("modernwarfare.exe", "Call of Duty", "competitive"),
    ("destiny2.exe", "Destiny 2", "competitive"),
    ("gta5.exe", "GTA V", "gpu"),
    ("hd-player.exe", "Free Fire (BlueStacks)", "freefire"),
    ("dnplayer.exe", "Free Fire (LDPlayer)", "freefire"),
    ("mumuplayer.exe", "Free Fire (MuMu)", "freefire"),
    ("memu.exe", "Free Fire (MEmu)", "freefire"),
    ("gameloop.exe", "Free Fire (GameLoop)", "freefire"),
    ("javaw.exe", "Minecraft", "stream"),
    ("obs64.exe", "OBS Studio", "stream"),
];

/// (game, profile) for every recognised game currently in the process table.
pub fn running_games() -> Vec<(&'static str, &'static str)> {
    if !platform::is_windows() {
        return Vec::new();
    }
    let procs: Vec<String> = platform::processes()
        .into_iter()
        .map(|(n, _, _)| n.to_lowercase())
        .collect();
    let mut out: Vec<(&'static str, &'static str)> = Vec::new();
    for (exe, name, profile) in GAMES.iter() {
        if procs.iter().any(|p| p == exe) && !out.iter().any(|(n, _)| n == name) {
            out.push((name, profile));
        }
    }
    out
}

pub fn cpu_times() -> Vec<(String, u32, u64)> {
    platform::cpu_times()
}

/// (used %, used GB, total GB) of the system volume, when the OS reports it.
pub fn disk() -> Option<(f64, f64, f64)> {
    platform::disk()
}

/// The previous network sample, so the rate is a delta and not a guess.
static NET_PREV: std::sync::Mutex<Option<(u64, u64, std::time::Instant)>> =
    std::sync::Mutex::new(None);

/// (down Mbps, up Mbps) since the previous call.
pub fn net_rate() -> (f64, f64) {
    let Some((rx, tx)) = platform::net_counters() else {
        return (0.0, 0.0);
    };
    let now = std::time::Instant::now();
    let mut slot = match NET_PREV.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let rate = match *slot {
        Some((prx, ptx, at)) => {
            let dt = now.saturating_duration_since(at).as_secs_f64().max(0.05);
            let down = rx.saturating_sub(prx) as f64 * 8.0 / dt / 1_000_000.0;
            let up = tx.saturating_sub(ptx) as f64 * 8.0 / dt / 1_000_000.0;
            // a counter reset (adapter replug) shows as a huge spike; ignore it
            (
                if down > 100_000.0 { 0.0 } else { down },
                if up > 100_000.0 { 0.0 } else { up },
            )
        }
        None => (0.0, 0.0),
    };
    *slot = Some((rx, tx, now));
    rate
}

pub fn processes() -> Vec<(String, u32, f64)> {
    platform::processes()
}

pub fn boost_pid(pid: u32) -> Result<String, String> {
    platform::set_priority(pid, "High")
}

pub fn timer_resolution_ms() -> Option<f64> {
    platform::timer_resolution_ms()
}

pub fn power_plan() -> String {
    platform::power_plan()
}

/// Percentage of the catalog that this machine can actually run.
pub fn readiness(state: &State, machine: &Machine) -> f64 {
    if TWEAKS.is_empty() {
        return 0.0;
    }
    let weak_gpu = machine.gpu.to_lowercase().contains("730")
        || machine.gpu.to_lowercase().contains("intel");
    let mut possible = 0usize;
    let mut done = 0usize;
    for t in TWEAKS.iter() {
        possible += 1;
        if state.applied.contains(t.id) {
            done += 1;
        }
    }
    let _ = weak_gpu;
    done as f64 / possible as f64 * 100.0
}

// ───────────────────────────── jobs ─────────────────────────────
//
// Every write happens on a worker thread. The window keeps painting at 60 fps
// while a profile is being applied, and the UI shows exactly which step is
// running — no frozen frame, no "not responding", no white rectangle.

use std::sync::mpsc;
use std::sync::Arc;

#[derive(Clone)]
pub enum Work {
    Apply(Vec<String>),
    Revert(Vec<String>),
    Profile(&'static str),
    RevertAll,
    Restore,
    Bench,
    Ping,
}

impl Work {
    pub fn label(&self) -> String {
        match self {
            Work::Apply(ids) => format!("applying {}", ids.len()),
            Work::Revert(ids) => format!("reverting {}", ids.len()),
            Work::Profile(pid) => find_profile(pid)
                .map(|p| p.name.to_string())
                .unwrap_or_else(|| pid.to_string()),
            Work::RevertAll => "reverting everything".to_string(),
            Work::Restore => "system checkpoint".to_string(),
            Work::Bench => "sleep-jitter benchmark".to_string(),
            Work::Ping => "net lab · handshake RTT".to_string(),
        }
    }
}

pub enum JobMsg {
    Begin {
        label: String,
        total: usize,
    },
    Step {
        index: usize,
        total: usize,
        name: String,
        ok: bool,
        note: String,
    },
    Bench(BenchResult),
    Ping(Vec<PingRow>),
    Done {
        label: String,
        outcomes: Vec<Outcome>,
        state: State,
    },
}

pub struct Job {
    pub label: String,
    pub total: usize,
    pub done: usize,
    pub failed: usize,
    pub current: String,
    pub notes: Vec<(bool, String)>,
    rx: mpsc::Receiver<JobMsg>,
    finished: Option<(String, Vec<Outcome>, State)>,
}

impl Job {
    pub fn start(state: State, work: Work, repaint: Arc<dyn Fn() + Send + Sync>) -> Job {
        let (tx, rx) = mpsc::channel();
        let label = work.label();
        std::thread::spawn(move || {
            let mut st = state;
            let paint = {
                let r = repaint.clone();
                move || r()
            };
            match work {
                Work::Bench => {
                    let _ = tx.send(JobMsg::Begin {
                        label: "sleep-jitter benchmark".to_string(),
                        total: 1,
                    });
                    paint();
                    let result = bench();
                    let _ = tx.send(JobMsg::Bench(result));
                    let _ = tx.send(JobMsg::Begin {
                        label: "sleep-jitter benchmark".to_string(),
                        total: 1,
                    });
                    let _ = tx.send(JobMsg::Step {
                        index: 0,
                        total: 1,
                        name: "120 wake-ups sampled".to_string(),
                        ok: true,
                        note: "done".to_string(),
                    });
                    st.save();
                    let _ = tx.send(JobMsg::Done {
                        label: "sleep-jitter benchmark".to_string(),
                        outcomes: Vec::new(),
                        state: st.clone(),
                    });
                    paint();
                }
                Work::Ping => {
                    let total = PING_TARGETS.len();
                    let _ = tx.send(JobMsg::Begin {
                        label: "net lab · handshake RTT".to_string(),
                        total,
                    });
                    paint();
                    let mut rows: Vec<PingRow> = Vec::new();
                    for (i, (label, host, port)) in PING_TARGETS.iter().enumerate() {
                        let (rtts, lost) = platform::tcp_ping(host, *port, 4);
                        let row = PingRow::new(label, host, *port, rtts, lost);
                        let _ = tx.send(JobMsg::Step {
                            index: i,
                            total,
                            name: label.to_string(),
                            ok: row.ok,
                            note: if row.ok {
                                format!("{:.0} ms · jitter {:.1}", row.avg, row.jitter)
                            } else {
                                "unreachable".to_string()
                            },
                        });
                        rows.push(row);
                        paint();
                    }
                    let _ = tx.send(JobMsg::Ping(rows));
                    st.save();
                    let _ = tx.send(JobMsg::Done {
                        label: "net lab".to_string(),
                        outcomes: Vec::new(),
                        state: st.clone(),
                    });
                    paint();
                }
                Work::Restore => {
                    let _ = tx.send(JobMsg::Begin {
                        label: "system checkpoint".to_string(),
                        total: 1,
                    });
                    paint();
                    let o = restore_point(&mut st);
                    let _ = tx.send(JobMsg::Step {
                        index: 0,
                        total: 1,
                        name: "system checkpoint".to_string(),
                        ok: o.ok,
                        note: o.msg.clone(),
                    });
                    st.save();
                    let _ = tx.send(JobMsg::Done {
                        label: "system checkpoint".to_string(),
                        outcomes: vec![o],
                        state: st.clone(),
                    });
                    paint();
                }
                Work::Apply(ids) => {
                    run_list(&tx, &mut st, ids, false, "apply".to_string(), &paint);
                    st.save();
                    paint();
                }
                Work::Revert(ids) => {
                    run_list(&tx, &mut st, ids, true, "revert".to_string(), &paint);
                    st.save();
                    paint();
                }
                Work::Profile(pid) => {
                    let name = find_profile(pid)
                        .map(|p| p.name.to_string())
                        .unwrap_or_else(|| pid.to_string());
                    let ids: Vec<String> = find_profile(pid)
                        .map(|p| p.tweaks.iter().map(|s| s.to_string()).collect())
                        .unwrap_or_default();
                    st.last_profile = pid.to_string();
                    st.push("sys", &format!("profile engaged :: {}", name));
                    run_list(&tx, &mut st, ids, false, name, &paint);
                    st.save();
                    paint();
                }
                Work::RevertAll => {
                    let ids: Vec<String> = st.applied.iter().cloned().collect();
                    run_list(&tx, &mut st, ids, true, "revert all".to_string(), &paint);
                    st.applied.clear();
                    st.save();
                    paint();
                }
            }
        });
        Job {
            label,
            total: 0,
            done: 0,
            failed: 0,
            current: String::new(),
            notes: Vec::new(),
            rx,
            finished: None,
        }
    }

    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.done as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }

    /// Drain everything the worker has said so far.
    pub fn pump(&mut self) -> JobEvent {
        let mut event = JobEvent::default();
        loop {
            match self.rx.try_recv() {
                Ok(JobMsg::Begin { label, total }) => {
                    self.label = label;
                    self.total = total;
                }
                Ok(JobMsg::Step {
                    index,
                    total,
                    name,
                    ok,
                    note,
                }) => {
                    self.total = total;
                    self.done = index + 1;
                    self.current = name.clone();
                    if !ok {
                        self.failed += 1;
                    }
                    self.notes.push((ok, format!("{}  ·  {}", name, note)));
                    if self.notes.len() > 80 {
                        self.notes.remove(0);
                    }
                }
                Ok(JobMsg::Bench(b)) => event.bench = Some(b),
                Ok(JobMsg::Ping(rows)) => event.ping = Some(rows),
                Ok(JobMsg::Done {
                    label,
                    outcomes,
                    state,
                }) => {
                    self.label = label.clone();
                    self.done = self.total.max(self.done);
                    self.finished = Some((label, outcomes, state));
                }
                Err(_) => break,
            }
        }
        event
    }

    pub fn take_finished(&mut self) -> Option<(String, Vec<Outcome>, State)> {
        self.finished.take()
    }
}

/// What a worker produced during a `pump`.
#[derive(Default)]
pub struct JobEvent {
    pub bench: Option<BenchResult>,
    pub ping: Option<Vec<PingRow>>,
}

// ───────────────────────── net lab (handshake RTT) ─────────────────────────

pub struct PingRow {
    pub label: &'static str,
    pub host: &'static str,
    pub port: u16,
    pub avg: f64,
    pub jitter: f64,
    pub loss: f64,
    pub ok: bool,
}

impl PingRow {
    fn new(label: &'static str, host: &'static str, port: u16, rtts: Vec<f64>, lost: usize) -> PingRow {
        let total = rtts.len() + lost;
        let avg = if rtts.is_empty() {
            0.0
        } else {
            rtts.iter().sum::<f64>() / rtts.len() as f64
        };
        let jitter = if rtts.len() < 2 {
            0.0
        } else {
            let mut worst = 0.0f64;
            for w in rtts.windows(2) {
                worst = worst.max((w[1] - w[0]).abs());
            }
            // mean absolute deviation is steadier than max for 4 samples
            let mad = rtts.iter().map(|v| (v - avg).abs()).sum::<f64>() / rtts.len() as f64;
            mad.max(worst * 0.5)
        };
        let loss = if total == 0 {
            100.0
        } else {
            lost as f64 / total as f64 * 100.0
        };
        PingRow {
            label,
            host,
            port,
            avg,
            jitter,
            loss,
            ok: !rtts.is_empty(),
        }
    }
}

/// Endpoints that matter to the games this app tunes.
pub const PING_TARGETS: &[(&str, &str, u16)] = &[
    ("Cloudflare baseline", "1.1.1.1", 443),
    ("Valve / CS2", "api.steampowered.com", 443),
    ("Riot / Valorant", "auth.riotgames.com", 443),
    ("Epic / Fortnite", "launcher.epicgames.com", 443),
    ("Garena / Free Fire", "www.garena.com", 443),
];

// ───────────────────────── bench history ─────────────────────────

fn bench_log_path() -> std::path::PathBuf {
    platform::data_dir().join("bench.log")
}

/// Record a score so the monitor can show a trend across sessions.
pub fn record_bench(score: f64) {
    use std::io::Write;
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(bench_log_path())
    {
        let _ = writeln!(f, "{:.1} {}", score, platform::epoch_secs());
    }
}

/// Last 12 scores, oldest first.
pub fn bench_history() -> Vec<f64> {
    let text = match fs::read_to_string(bench_log_path()) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut out: Vec<f64> = text
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .filter_map(|v| v.parse::<f64>().ok())
        .collect();
    if out.len() > 12 {
        out.drain(0..out.len() - 12);
    }
    out
}

fn run_list(
    tx: &mpsc::Sender<JobMsg>,
    st: &mut State,
    ids: Vec<String>,
    revert: bool,
    label: String,
    paint: &dyn Fn(),
) {
    let total = ids.len();
    let _ = tx.send(JobMsg::Begin {
        label: label.clone(),
        total,
    });
    paint();
    let mut outcomes: Vec<Outcome> = Vec::new();
    for (i, id) in ids.iter().enumerate() {
        let t = find_tweak(id);
        let name = t.map(|t| t.name.to_string()).unwrap_or_else(|| id.clone());
        let o = match t {
            Some(t) => apply_one(st, t, revert),
            None => Outcome {
                id: id.clone(),
                ok: false,
                msg: "unknown tweak".to_string(),
            },
        };
        if revert {
            st.applied.remove(id.as_str());
        } else if o.ok {
            match t {
                Some(t) if t.once => {
                    st.applied.remove(id.as_str());
                }
                _ => {
                    st.applied.insert(id.clone());
                }
            }
            st.ops = st.ops.wrapping_add(1);
        }
        let level = if o.ok { "ok" } else { "err" };
        st.push(level, &format!("{} :: {}", name, o.msg));
        let _ = tx.send(JobMsg::Step {
            index: i,
            total,
            name,
            ok: o.ok,
            note: o.msg.clone(),
        });
        outcomes.push(o);
        paint();
    }
    let _ = tx.send(JobMsg::Done {
        label,
        outcomes,
        state: st.clone(),
    });
}

// ─────────────────────────── discord account ────────────────────────────────

/// A Discord account, once the user has authorised VOID against their app.
#[derive(Clone, Default)]
pub struct Account {
    pub name: String,
    pub display: String,
    pub id: String,
    pub avatar: String,
}

impl Account {
    pub fn handle(&self) -> String {
        if self.display.is_empty() {
            self.name.clone()
        } else {
            self.display.clone()
        }
    }

    /// Monogram used in the UI instead of downloading an avatar: first glyph of
    /// the display name, which keeps the app monochrome and dependency-free.
    pub fn monogram(&self) -> String {
        self.handle()
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into())
    }

    pub fn profile_url(&self) -> String {
        format!("https://discord.com/users/{}", self.id)
    }
}

fn url_encode(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn json_str(src: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\":\"", key);
    let i = src.find(&pat)? + pat.len();
    let rest = &src[i..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Exchange the one-time `code` Discord handed back over IPC for an access
/// token, then read the profile. Both calls go straight to discord.com over
/// WinHTTP — no server of ours sits in the middle.
pub fn oauth_account(client_id: &str, secret: &str, code: &str) -> Result<Account, String> {
    let form = format!(
        "client_id={}&client_secret={}&grant_type=authorization_code&code={}",
        url_encode(client_id),
        url_encode(secret),
        url_encode(code)
    );
    let (status, body) = platform::https(
        "POST",
        "discord.com",
        "/api/oauth2/token",
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some(&form),
    )?;

    if status != 200 {
        let why = json_str(&body, "error").unwrap_or_else(|| format!("http {}", status));
        return Err(format!("discord refused the exchange · {}", why));
    }
    let token = json_str(&body, "access_token").ok_or("no access_token in the reply")?;

    let (status, me) = platform::https(
        "GET",
        "discord.com",
        "/api/users/@me",
        &[
            ("Authorization", &format!("Bearer {}", token)),
            ("User-Agent", "VOID/1.0"),
        ],
        None,
    )?;
    if status != 200 {
        return Err(format!("profile request failed · http {}", status));
    }

    Ok(Account {
        name: json_str(&me, "username").unwrap_or_default(),
        display: json_str(&me, "global_name").unwrap_or_default(),
        id: json_str(&me, "id").unwrap_or_default(),
        avatar: json_str(&me, "avatar").unwrap_or_default(),
    })
}

// ───────────────────────────── pre-flight ────────────────────────────────────

/// One line of the pre-flight checklist. Every entry is derived from something
/// that was actually measured on this machine — never from a static list.
#[derive(Clone)]
pub struct Check {
    pub id: &'static str,
    pub label: &'static str,
    pub detail: String,
    pub ok: bool,
    /// Which profile fixes it, when a fix is mechanical.
    pub fix: Option<&'static str>,
}

/// The checklist states a machine must satisfy before a ranked session. It is
/// the same audit a coach would run by hand, expressed as data.
pub fn preflight(applied: usize, verified: usize, jitter: Option<f64>, ping: Option<f64>) -> Vec<Check> {
    let mut out: Vec<Check> = Vec::new();

    let admin = platform::is_admin();
    out.push(Check {
        id: "admin",
        label: "elevated",
        detail: if admin { "token is elevated".into() } else { "run as administrator".into() },
        ok: admin,
        fix: None,
    });

    let plan = power_plan();
    let plan_ok = {
        let p = plan.to_ascii_lowercase();
        p.contains("high performance") || p.contains("ultimate") || p.contains("ryzen") || p.contains("bitsum")
    };
    out.push(Check {
        id: "power",
        label: "power plan",
        detail: plan,
        ok: plan_ok,
        fix: Some("latency"),
    });

    let timer = timer_resolution_ms();
    out.push(Check {
        id: "timer",
        label: "kernel timer",
        detail: match timer {
            Some(t) if t <= 0.5 => format!("{:.3} ms · locked", t),
            Some(t) => format!("{:.3} ms", t),
            None => "unavailable".into(),
        },
        ok: matches!(timer, Some(t) if t <= 1.0),
        fix: Some("latency"),
    });

    let total = catalog::TWEAKS.len().max(1);
    let pct = applied as f32 / total as f32 * 100.0;
    out.push(Check {
        id: "coverage",
        label: "catalog",
        detail: format!("{} applied · {} verified", applied, verified),
        ok: pct >= 55.0,
        fix: Some("esports"),
    });

    out.push(Check {
        id: "jitter",
        label: "sleep jitter",
        detail: match jitter {
            Some(j) => format!("{:.2} ms", j),
            None => "not measured".into(),
        },
        ok: matches!(jitter, Some(j) if j <= 1.5),
        fix: None,
    });

    out.push(Check {
        id: "net",
        label: "handshake rtt",
        detail: match ping {
            Some(p) => format!("{:.0} ms", p),
            None => "net lab not run".into(),
        },
        ok: matches!(ping, Some(p) if p <= 70.0),
        fix: None,
    });

    let emu = emulators().iter().any(|e| e.installed);
    out.push(Check {
        id: "emulator",
        label: "emulator",
        detail: if emu { "found".into() } else { "none installed".into() },
        ok: emu,
        fix: None,
    });

    out
}

// ─────────────────────────────── guard ───────────────────────────────────────

/// The applied ids that no longer hold their value on the machine.
pub fn drift(state: &State) -> Vec<String> {
    if simulated() {
        return Vec::new();
    }
    state
        .applied
        .iter()
        .filter(|id| live_status(id) == Some(false))
        .cloned()
        .collect()
}

// ─────────────────────────── diagnostics report ──────────────────────────────

/// Write a self-contained markdown report next to the settings: what the
/// machine is, what VOID measured, what it changed and why. Returns the path.
pub fn export_report(state: &State, score: u32, checks: &[Check], jitter: Option<f64>, ping: Option<f64>) -> Result<std::path::PathBuf, String> {
    let m = Machine::detect();
    let mut body = String::new();
    body.push_str(&format!("# VOID {} — diagnostics\n\n", VERSION));
    body.push_str(&format!("generated {}\n\n", platform::now_stamp()));
    body.push_str(&format!("## machine\n\n- os: {}\n- host: {}\n- cpu: {}\n- gpu: {}\n- admin: {}\n", m.os, m.host, m.cpu, m.gpu, m.admin));
    body.push_str(&format!(
        "\n## score\n\n- void score: {}/100\n- timer: {:?} ms\n- sleep jitter: {:?}\n- handshake rtt: {:?}\n",
        score,
        timer_resolution_ms(),
        jitter,
        ping
    ));
    body.push_str("\n## pre-flight\n\n");
    for c in checks {
        body.push_str(&format!("- [{}] {} — {}\n", if c.ok { "x" } else { " " }, c.label, c.detail));
    }
    body.push_str(&format!(
        "\n## applied ({}/{})\n\n",
        state.applied.len(),
        catalog::TWEAKS.len()
    ));
    for id in state.applied.iter() {
        match find_tweak(id) {
            Some(t) => body.push_str(&format!("- `{}` {} — {}\n", t.id, t.name, t.why)),
            None => body.push_str(&format!("- `{}` (not in catalog)\n", id)),
        }
    }
    body.push_str("\n## journal (last 60)\n\n```\n");
    let skip = state.log.len().saturating_sub(60);
    for e in state.log.iter().skip(skip) {
        body.push_str(&format!("{} [{}] {}\n", e.at, e.level, e.msg));
    }
    body.push_str("```\n");

    let dir = platform::data_dir();
    let name = format!("void-report-{}.md", platform::epoch_secs());
    let path = dir.join(name);
    fs::write(&path, body).map_err(|e| format!("write {}: {}", path.display(), e))?;
    Ok(path)
}

/// Does the machine currently hold this tweak's value? `None` when the tweak is
/// not a plain registry write (services, bcdedit, power plans…).
pub fn live_status(id: &str) -> Option<bool> {
    let t = find_tweak(id)?;
    let mut seen = false;
    for a in t.apply.iter() {
        if let Action::Reg(k, v, ty, d) = a {
            seen = true;
            if !platform::reg_matches(k, v, platform::RegKind::from_tag(ty), d) {
                return Some(false);
            }
        }
    }
    if seen {
        Some(true)
    } else {
        None
    }
}

/// Current value of a tweak's first registry key, for the "current / target"
/// read-out in the optimizer rows.
pub fn current_value(id: &str) -> Option<String> {
    let t = find_tweak(id)?;
    for a in t.apply.iter() {
        if let Action::Reg(k, v, ty, _) = a {
            return match platform::RegKind::from_tag(ty) {
                platform::RegKind::Dword => platform::reg_read_u32(k, v).map(|n| n.to_string()),
                platform::RegKind::Str => platform::reg_read_string(k, v),
            };
        }
    }
    None
}
