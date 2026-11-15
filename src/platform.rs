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

pub fn uptime_secs() -> i64 {
    if cfg!(windows) {
        if let Ok(o) = powershell(
            "[int]((Get-Date) - (Get-CimInstance Win32_OperatingSystem).LastBootUpTime).TotalSeconds",
        ) {
            if let Ok(v) = o.stdout.trim().parse::<f64>() {
                return v as i64;
            }
        }
        0
    } else if let Ok(text) = std::fs::read_to_string("/proc/uptime") {
        let first = text.split_whitespace().next().unwrap_or("0");
        first.parse::<f64>().unwrap_or(0.0) as i64
    } else {
        0
    }
}

/// Seconds since the unix epoch.
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
