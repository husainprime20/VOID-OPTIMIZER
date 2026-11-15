//! Platform layer: process execution, privilege checks, machine identity.
//!
//! Everything here is `std` only. Windows-specific behaviour is reached purely
//! through `cfg!(windows)` branches, so the crate builds on Linux/macOS too
//! (where VOID runs in a clearly-labelled simulation mode for UI work).

use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Out {
    pub ok: bool,
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    pub fn combined(&self) -> String {
        let mut s = self.stdout.trim().to_string();
        let err = self.stderr.trim();
        if !err.is_empty() {
            if !s.is_empty() {
                s.push('\n');
            }
            s.push_str(err);
        }
        s
    }
}

/// CREATE_NO_WINDOW — keeps console helpers from flashing on screen.
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

/// Run a program and capture its output. Never panics.
pub fn run(program: &str, args: &[&str]) -> Result<Out, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    hide_console(&mut cmd);
    match cmd.output() {
        Ok(out) => Ok(Out {
            ok: out.status.success(),
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        }),
        Err(e) => Err(format!("{}: {}", program, e)),
    }
}

/// Run an inline PowerShell script (Windows only in practice).
pub fn powershell(script: &str) -> Result<Out, String> {
    run(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
    )
}

pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

pub fn os_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "Windows"
    } else if cfg!(target_os = "linux") {
        "Linux"
    } else if cfg!(target_os = "macos") {
        "macOS"
    } else {
        "Unknown"
    }
}

pub fn is_admin() -> bool {
    if cfg!(windows) {
        match run("net", &["session"]) {
            Ok(o) => o.ok,
            Err(_) => false,
        }
    } else {
        match run("id", &["-u"]) {
            Ok(o) => o.stdout.trim() == "0",
            Err(_) => false,
        }
    }
}

pub fn hostname() -> String {
    env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "void".to_string())
}

pub fn username() -> String {
    env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "player".to_string())
}

pub fn data_dir() -> PathBuf {
    let base = if cfg!(windows) {
        env::var("APPDATA").unwrap_or_else(|_| ".".to_string())
    } else {
        env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            format!("{}/.local/share", home)
        })
    };
    let dir = PathBuf::from(base).join("VOID");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn cpu_name() -> String {
    if cfg!(windows) {
        if let Ok(o) = powershell("(Get-CimInstance Win32_Processor | Select-Object -First 1).Name") {
            let name = o.stdout.trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    } else if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
        for line in text.lines() {
            if line.starts_with("model name") {
                if let Some(v) = line.split(':').nth(1) {
                    return v.trim().to_string();
                }
            }
        }
    }
    "Unknown CPU".to_string()
}

pub fn gpu_name() -> String {
    if cfg!(windows) {
        if let Ok(o) = powershell(
            "(Get-CimInstance Win32_VideoController | Select-Object -First 1).Name",
        ) {
            let name = o.stdout.trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    "Unknown GPU".to_string()
}

pub fn os_build() -> String {
    if cfg!(windows) {
        if let Ok(o) = powershell(
            "$v=Get-CimInstance Win32_OperatingSystem; \"$($v.Caption) $($v.Version)\"",
        ) {
            let v = o.stdout.trim().to_string();
            if !v.is_empty() {
                return v;
            }
        }
    } else if let Ok(text) = std::fs::read_to_string("/etc/os-release") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    format!("{}", os_name())
}

#[cfg(windows)]
pub fn uptime_secs() -> i64 {
    (unsafe { kern::GetTickCount64() } / 1000) as i64
}

#[cfg(not(windows))]
pub fn uptime_secs() -> i64 {
    match std::fs::read_to_string("/proc/uptime") {
        Ok(text) => text
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0) as i64,
        Err(_) => 0,
    }
}

pub fn epoch_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `2026-11-15 18:22:31` in UTC, without pulling in a date crate.
pub fn now_stamp() -> String {
    let secs = epoch_secs();
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        y,
        m,
        d,
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

pub fn clock_label() -> String {
    let secs = epoch_secs();
    let sod = secs.rem_euclid(86400);
    format!("{:02}:{:02}:{:02}", sod / 3600, (sod % 3600) / 60, sod % 60)
}

/// Best-effort: open `url` in a chrome-less app window, falling back to the
/// user's default browser.
pub fn open_window(url: &str) {
    if cfg!(windows) {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for exe in candidates.iter() {
            if std::path::Path::new(*exe).exists() {
                let flag = format!("--app={}", url);
                let size = "--window-size=1480,940".to_string();
                if let Ok(child) = Command::new(*exe).arg(&flag).arg(&size).spawn() {
                    let _ = child;
                    return;
                }
            }
        }
        let _ = run("cmd", &["/c", "start", "", url]);
    } else {
        for exe in ["chromium", "google-chrome", "chromium-browser"].iter() {
            let flag = format!("--app={}", url);
            if let Ok(child) = Command::new(*exe).arg(&flag).spawn() {
                let _ = child;
                return;
            }
        }
        let _ = run("xdg-open", &[url]);
    }
}

// ── native shell integration ─────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    #[link(name = "shell32")]
    extern "system" {
        pub fn ShellExecuteW(
            hwnd: isize,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
    #[link(name = "user32")]
    extern "system" {
        pub fn MessageBoxW(hwnd: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
    }
    #[link(name = "ntdll")]
    extern "system" {
        pub fn NtQueryTimerResolution(min: *mut u32, max: *mut u32, cur: *mut u32) -> i32;
    }
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// Re-launch this executable elevated. Returns true when a new elevated
/// process was started (the caller must then exit).
#[cfg(windows)]
pub fn elevate() -> bool {
    if is_admin() {
        return false;
    }
    let exe = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let file = wide(&exe.to_string_lossy());
    let op = wide("runas");
    let rc = unsafe {
        win::ShellExecuteW(
            0,
            op.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    // >32 means success; 5 (SE_ERR_ACCESSDENIED) means the user declined UAC.
    rc > 32
}

#[cfg(not(windows))]
pub fn elevate() -> bool {
    false
}

/// Native modal dialog — used for fatal errors where no window exists yet.
pub fn message_box(title: &str, text: &str) {
    #[cfg(windows)]
    {
        let t = wide(text);
        let c = wide(title);
        unsafe {
            win::MessageBoxW(0, t.as_ptr(), c.as_ptr(), 0x0000_0010 | 0x0000_0040);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{}: {}", title, text);
    }
}

/// Launch a program with no console and no wait.
pub fn spawn_detached(path: &str) -> Result<(), String> {
    let mut cmd = Command::new(path);
    hide_console(&mut cmd);
    match cmd.spawn() {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{}: {}", path, e)),
    }
}

pub fn exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

/// Current system timer resolution in milliseconds (Windows: 15.6 / 1.0 / 0.5).
#[cfg(windows)]
pub fn timer_resolution_ms() -> Option<f64> {
    let mut a = 0u32;
    let mut b = 0u32;
    let mut cur = 0u32;
    let st = unsafe { win::NtQueryTimerResolution(&mut a, &mut b, &mut cur) };
    if st == 0 && cur > 0 {
        Some(cur as f64 / 10_000.0)
    } else {
        None
    }
}

#[cfg(not(windows))]
pub fn timer_resolution_ms() -> Option<f64> {
    None
}

/// Active Windows power plan, e.g. "Balanced" / "High performance".
pub fn power_plan() -> String {
    if !cfg!(windows) {
        return "n/a".to_string();
    }
    match run("powercfg", &["/getactivescheme"]) {
        Ok(o) => {
            let s = o.combined();
            if let (Some(i), Some(j)) = (s.find('('), s.rfind(')')) {
                if j > i + 1 {
                    return s[i + 1..j].trim().to_string();
                }
            }
            "unknown".to_string()
        }
        Err(_) => "unknown".to_string(),
    }
}

/// `tasklist` -> (image name, pid, working set in MB), sorted by memory.
pub fn processes() -> Vec<(String, u32, f64)> {
    let mut out: Vec<(String, u32, f64)> = Vec::new();
    if !cfg!(windows) {
        return out;
    }
    if let Ok(o) = run("tasklist", &["/fo", "csv", "/nh"]) {
        for line in o.stdout.lines() {
            let cols: Vec<&str> = line
                .trim()
                .trim_matches('"')
                .split("\",\"")
                .map(|s| s.trim_matches('"'))
                .collect();
            if cols.len() < 5 {
                continue;
            }
            let name = cols[0].to_string();
            let pid: u32 = cols[1].parse().unwrap_or(0);
            let mem: f64 = cols[4]
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect::<String>()
                .parse::<f64>()
                .unwrap_or(0.0)
                / 1024.0;
            if pid > 0 && !name.is_empty() {
                out.push((name, pid, mem));
            }
        }
    }
    out.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    out
}

pub fn is_process_running(exe: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let script = format!(
        "if (Get-Process -Name '{}' -ErrorAction SilentlyContinue) {{ 'yes' }} else {{ 'no' }}",
        exe.trim_end_matches(".exe")
    );
    matches!(powershell(&script), Ok(o) if o.combined().contains("yes"))
}

/// Raise a process' priority class without needing a handle from Rust.
pub fn set_priority(pid: u32, class: &str) -> Result<String, String> {
    let script = format!(
        "try {{ $p = Get-Process -Id {} -ErrorAction Stop; $p.PriorityClass = '{}'; 'ok' }} catch {{ 'fail ' + $_.Exception.Message }}",
        pid, class
    );
    match powershell(&script) {
        Ok(o) => {
            let s = o.combined();
            if s.contains("ok") {
                Ok(format!("{} -> {}", pid, class))
            } else {
                Err(s)
            }
        }
        Err(e) => Err(e),
    }
}

// ── instant telemetry (no process spawn, no PowerShell) ─────────────────────

#[cfg(windows)]
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FileTime {
    low: u32,
    high: u32,
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct MemStatusEx {
    length: u32,
    memory_load: u32,
    total_phys: u64,
    avail_phys: u64,
    total_page: u64,
    avail_page: u64,
    total_virtual: u64,
    avail_virtual: u64,
    avail_extended: u64,
}

#[cfg(windows)]
mod kern {
    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetSystemTimes(idle: *mut super::FileTime, kernel: *mut super::FileTime, user: *mut super::FileTime) -> i32;
        pub fn GlobalMemoryStatusEx(mem: *mut super::MemStatusEx) -> i32;
        pub fn GetTickCount64() -> u64;
    }
}

/// (idle, total) CPU ticks. Cheap on every platform — the caller keeps the delta.
#[cfg(windows)]
pub fn cpu_ticks() -> Option<(u64, u64)> {
    let mut idle = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let ok = unsafe { kern::GetSystemTimes(&mut idle, &mut kernel, &mut user) };
    if ok == 0 {
        return None;
    }
    let f = |t: FileTime| ((t.high as u64) << 32) | t.low as u64;
    // kernel time already contains idle time
    Some((f(idle), f(kernel) + f(user)))
}

#[cfg(not(windows))]
pub fn cpu_ticks() -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/stat").ok()?;
    let line = text.lines().next()?;
    let mut vals = line.split_whitespace().skip(1).filter_map(|p| p.parse::<u64>().ok());
    let _user = vals.next()?;
    let _nice = vals.next()?;
    let _system = vals.next()?;
    let idle = vals.next()?;
    let iowait = vals.next().unwrap_or(0);
    let mut total: u64 = _user + _nice + _system + idle + iowait;
    for v in vals {
        total = total.saturating_add(v);
    }
    Some((idle + iowait, total))
}

/// (total GB, used GB)
#[cfg(windows)]
pub fn ram() -> (f64, f64) {
    let mut m = MemStatusEx::default();
    m.length = std::mem::size_of::<MemStatusEx>() as u32;
    let ok = unsafe { kern::GlobalMemoryStatusEx(&mut m) };
    if ok == 0 {
        return (0.0, 0.0);
    }
    let gb = 1024.0 * 1024.0 * 1024.0;
    let total = m.total_phys as f64 / gb;
    let used = (m.total_phys.saturating_sub(m.avail_phys)) as f64 / gb;
    (total, used)
}

#[cfg(not(windows))]
pub fn ram() -> (f64, f64) {
    let mut total = 0.0f64;
    let mut avail = 0.0f64;
    if let Ok(text) = std::fs::read_to_string("/proc/meminfo") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                total = rest.trim().trim_end_matches("kB").trim().parse::<f64>().unwrap_or(0.0)
                    / 1024.0
                    / 1024.0;
            }
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail = rest.trim().trim_end_matches("kB").trim().parse::<f64>().unwrap_or(0.0)
                    / 1024.0
                    / 1024.0;
            }
        }
    }
    let used = if total > avail { total - avail } else { 0.0 };
    (total, used)
}
