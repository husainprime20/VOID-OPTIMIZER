//! Platform layer: process execution, privilege checks, machine identity.
//!
//! Design rule for this file: **nothing on the launch path may spawn a
//! process.** Everything the window needs to draw its first frame (CPU name,
//! GPU name, OS build, admin state, power plan, telemetry) is read straight
//! from kernel32 / advapi32 / registry, so the window appears in milliseconds
//! instead of waiting on three PowerShell CIM queries.
//!
//! Subprocesses still exist for the things Windows only exposes through a
//! tool (`reg`, `powercfg`, `bcdedit`, `sc`, `netsh`, `fsutil`), and every one
//! of them is created with CREATE_NO_WINDOW.

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
pub fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

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

// ── registry ────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RegKind {
    Str,
    Dword,
}

impl RegKind {
    pub fn tag(self) -> &'static str {
        match self {
            RegKind::Str => "REG_SZ",
            RegKind::Dword => "REG_DWORD",
        }
    }
    pub fn from_tag(tag: &str) -> RegKind {
        if tag.eq_ignore_ascii_case("REG_DWORD") {
            RegKind::Dword
        } else {
            RegKind::Str
        }
    }
}

/// Write a registry value. `path` is `HIVE\sub\key` (HKCU / HKLM / HKCR / HKU
/// / HKCC). Returns `Ok(true)` when the value was read back exactly as written.
pub fn reg_write(path: &str, name: &str, kind: RegKind, data: &str) -> Result<bool, String> {
    #[cfg(windows)]
    {
        win::reg_write(path, name, kind, data)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, name, kind, data);
        Err("registry writes are Windows-only".to_string())
    }
}

/// Delete a registry value. A value that does not exist is not an error.
pub fn reg_delete(path: &str, name: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::reg_delete(path, name)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, name);
        Err("registry writes are Windows-only".to_string())
    }
}

pub fn reg_read_string(path: &str, name: &str) -> Option<String> {
    #[cfg(windows)]
    {
        win::reg_string(path, name)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, name);
        None
    }
}

pub fn reg_read_u32(path: &str, name: &str) -> Option<u32> {
    #[cfg(windows)]
    {
        win::reg_u32(path, name)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, name);
        None
    }
}

/// Does the machine currently hold this exact value?
pub fn reg_matches(path: &str, name: &str, kind: RegKind, data: &str) -> bool {
    match kind {
        RegKind::Dword => {
            let want: u32 = data.trim().parse().unwrap_or(0);
            reg_read_u32(path, name) == Some(want)
        }
        RegKind::Str => match reg_read_string(path, name) {
            Some(have) => have.trim_end_matches('\0') == data,
            None => false,
        },
    }
}

// ── privileges & identity ───────────────────────────────────────────────────

pub fn is_admin() -> bool {
    #[cfg(windows)]
    {
        win::is_elevated()
    }
    #[cfg(not(windows))]
    {
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

/// CPU model string. Registry read on Windows (~0.1 ms), no subprocess.
pub fn cpu_name() -> String {
    #[cfg(windows)]
    {
        if let Some(name) = win::reg_string(
            r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
            "ProcessorNameString",
        ) {
            let name = name.trim().to_string();
            if !name.is_empty() {
                return name;
            }
        }
    }
    if let Ok(text) = std::fs::read_to_string("/proc/cpuinfo") {
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

/// Primary display adapter. Reads the display class key directly.
pub fn gpu_name() -> String {
    #[cfg(windows)]
    {
        if let Some(name) = win::gpu_name() {
            return name;
        }
    }
    "Unknown GPU".to_string()
}

/// "Windows 11 Pro 26100.2314"
pub fn os_build() -> String {
    #[cfg(windows)]
    {
        if let Some(s) = win::os_build() {
            return s;
        }
    }
    if let Ok(text) = std::fs::read_to_string("/etc/os-release") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                return rest.trim_matches('"').to_string();
            }
        }
    }
    os_name().to_string()
}

// ── clock ───────────────────────────────────────────────────────────────────

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

// ── windows native surface ──────────────────────────────────────────────────

#[cfg(windows)]
mod win {
    use std::ffi::c_void;

    type Handle = isize;

    const HKEY_CLASSES_ROOT: Handle = 0x8000_0000u32 as i32 as Handle;
    const HKEY_CURRENT_USER: Handle = 0x8000_0001u32 as i32 as Handle;
    const HKEY_LOCAL_MACHINE: Handle = 0x8000_0002u32 as i32 as Handle;
    const HKEY_USERS: Handle = 0x8000_0003u32 as i32 as Handle;
    const HKEY_CURRENT_CONFIG: Handle = 0x8000_0005u32 as i32 as Handle;

    const KEY_ALL_ACCESS: u32 = 0x000F_003F;
    const REG_SZ: u32 = 1;
    const REG_DWORD: u32 = 4;
    const RRF_RT_REG_SZ: u32 = 0x0000_0002;
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

    #[link(name = "advapi32")]
    extern "system" {
        fn RegCreateKeyExW(
            key: Handle,
            sub: *const u16,
            reserved: u32,
            class: *const u16,
            options: u32,
            sam: u32,
            sa: *const c_void,
            out: *mut Handle,
            disposition: *mut u32,
        ) -> i32;
        fn RegOpenKeyExW(key: Handle, sub: *const u16, options: u32, sam: u32, out: *mut Handle)
            -> i32;
        fn RegSetValueExW(key: Handle, name: *const u16, reserved: u32, ty: u32, data: *const u8, cb: u32)
            -> i32;
        fn RegDeleteValueW(key: Handle, name: *const u16) -> i32;
        fn RegCloseKey(key: Handle) -> i32;
        fn RegGetValueW(
            key: Handle,
            sub: *const u16,
            value: *const u16,
            flags: u32,
            ty: *mut u32,
            data: *mut c_void,
            cb: *mut u32,
        ) -> i32;
        fn OpenProcessToken(process: Handle, access: u32, out: *mut Handle) -> i32;
        fn GetTokenInformation(
            token: Handle,
            class: u32,
            info: *mut c_void,
            len: u32,
            ret: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn CloseHandle(h: Handle) -> i32;
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        fn SetPriorityClass(process: Handle, class: u32) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        fn Process32FirstW(snap: Handle, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snap: Handle, entry: *mut ProcessEntry32W) -> i32;
        fn K32GetProcessMemoryInfo(process: Handle, counters: *mut ProcessMemoryCounters, cb: u32)
            -> i32;
        fn GetTickCount64() -> u64;
        fn GetActiveProcessorCount(group: u16) -> u32;
        fn SetProcessAffinityMask(process: Handle, mask: usize) -> i32;
    }

    #[link(name = "powrprof")]
    extern "system" {
        fn PowerGetActiveScheme(root: *mut c_void, guid: *mut *mut u8) -> u32;
    }

    #[link(name = "srclient")]
    extern "system" {
        fn SRSetRestorePointW(info: *const RestorePointInfoW, status: *mut StateMgrStatus) -> i32;
    }

    const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    const PROCESS_SET_INFORMATION: u32 = 0x0200;
    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const TOKEN_QUERY: u32 = 0x0008;
    const TOKEN_ELEVATION_CLASS: u32 = 20;
    const MAX_PATH: usize = 260;
    const INVALID_HANDLE: Handle = -1;

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        pid: u32,
        default_heap: usize,
        module_id: u32,
        threads: u32,
        parent: u32,
        pri_class: i32,
        flags: u32,
        exe: [u16; MAX_PATH],
    }

    #[repr(C)]
    #[derive(Default)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged: usize,
        quota_paged: usize,
        quota_peak_nonpaged: usize,
        quota_nonpaged: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }

    #[repr(C)]
    struct RestorePointInfoW {
        event_type: u32,
        restore_pt_type: u32,
        sequence: i64,
        description: [u16; MAX_PATH],
    }

    #[repr(C)]
    struct StateMgrStatus {
        status: u32,
        sequence: i64,
        bmid: [u16; MAX_PATH],
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn hive(path: &str) -> Option<(Handle, Vec<u16>)> {
        let (root, rest) = match path.split_once('\\') {
            Some(v) => v,
            None => (path, ""),
        };
        let key = match root.to_ascii_uppercase().as_str() {
            "HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
            "HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
            "HKCR" | "HKEY_CLASSES_ROOT" => HKEY_CLASSES_ROOT,
            "HKU" | "HKEY_USERS" => HKEY_USERS,
            "HKCC" | "HKEY_CURRENT_CONFIG" => HKEY_CURRENT_CONFIG,
            _ => return None,
        };
        Some((key, wide(rest)))
    }

    /// (code, message) for a Win32 error.
    fn err(what: &str, code: i32) -> Result<bool, String> {
        let text = match code {
            5 => "access denied — run VOID as Administrator".to_string(),
            2 => "key or value not found".to_string(),
            _ => format!("windows error {}", code),
        };
        Err(format!("{}: {}", what, text))
    }

    pub fn reg_write(
        path: &str,
        name: &str,
        kind: super::RegKind,
        data: &str,
    ) -> Result<bool, String> {
        let (root, sub) = match hive(path) {
            Some(v) => v,
            None => return Err(format!("unrecognised registry path `{}`", path)),
        };
        let mut key: Handle = 0;
        let mut disp: u32 = 0;
        let rc = unsafe {
            RegCreateKeyExW(
                root,
                sub.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_ALL_ACCESS,
                std::ptr::null(),
                &mut key,
                &mut disp,
            )
        };
        if rc != 0 {
            return err(&format!("open {}\\{}", path, name), rc);
        }

        let val = wide(name);
        let (bytes, ty): (Vec<u8>, u32) = match kind {
            super::RegKind::Dword => {
                let n: u32 = data.trim().parse().unwrap_or(0);
                (n.to_le_bytes().to_vec(), REG_DWORD)
            }
            super::RegKind::Str => {
                let w = wide(data);
                (w.iter().flat_map(|u| u.to_le_bytes()).collect::<Vec<u8>>(), REG_SZ)
            }
        };
        let cb = bytes.len() as u32;
        let rc = unsafe {
            RegSetValueExW(key, val.as_ptr(), 0, ty, bytes.as_ptr() as *const u8, cb)
        };
        unsafe {
            RegCloseKey(key);
        }
        if rc != 0 {
            return err(&format!("write {}\\{}", path, name), rc);
        }
        Ok(super::reg_matches(path, name, kind, data))
    }

    pub fn reg_delete(path: &str, name: &str) -> Result<(), String> {
        let (root, sub) = match hive(path) {
            Some(v) => v,
            None => return Err(format!("unrecognised registry path `{}`", path)),
        };
        let mut key: Handle = 0;
        let rc = unsafe { RegOpenKeyExW(root, sub.as_ptr(), 0, KEY_ALL_ACCESS, &mut key) };
        if rc != 0 {
            if rc == 2 {
                return Ok(());
            }
            return err(&format!("open {}\\{}", path, name), rc).map(|_| ());
        }
        let val = wide(name);
        let rc = unsafe { RegDeleteValueW(key, val.as_ptr()) };
        unsafe {
            RegCloseKey(key);
        }
        if rc == 0 || rc == 2 {
            Ok(())
        } else {
            err(&format!("delete {}\\{}", path, name), rc).map(|_| ())
        }
    }

    pub fn reg_string(path: &str, name: &str) -> Option<String> {
        let (root, sub) = hive(path)?;
        let val = wide(name);
        let mut buf = vec![0u16; 512];
        let mut cb = (buf.len() * 2) as u32;
        let mut ty: u32 = 0;
        let rc = unsafe {
            RegGetValueW(
                root,
                sub.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_SZ,
                &mut ty,
                buf.as_mut_ptr() as *mut c_void,
                &mut cb,
            )
        };
        if rc != 0 {
            return None;
        }
        let len = (cb as usize / 2).min(buf.len());
        let end = buf[..len].iter().position(|c| *c == 0).unwrap_or(len);
        Some(String::from_utf16_lossy(&buf[..end]))
    }

    pub fn reg_u32(path: &str, name: &str) -> Option<u32> {
        let (root, sub) = hive(path)?;
        let val = wide(name);
        let mut out: u32 = 0;
        let mut cb: u32 = 4;
        let mut ty: u32 = 0;
        let rc = unsafe {
            RegGetValueW(
                root,
                sub.as_ptr(),
                val.as_ptr(),
                RRF_RT_REG_DWORD,
                &mut ty,
                &mut out as *mut u32 as *mut c_void,
                &mut cb,
            )
        };
        if rc == 0 {
            Some(out)
        } else {
            None
        }
    }

    pub fn is_elevated() -> bool {
        unsafe {
            let mut token: Handle = 0;
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }
            let mut elevated: u32 = 0;
            let mut ret: u32 = 0;
            let ok = GetTokenInformation(
                token,
                TOKEN_ELEVATION_CLASS,
                &mut elevated as *mut u32 as *mut c_void,
                4,
                &mut ret,
            );
            CloseHandle(token);
            ok != 0 && elevated == 1
        }
    }

    pub fn gpu_name() -> Option<String> {
        let base = r"SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}";
        let mut first: Option<String> = None;
        for i in 0..12 {
            let sub = format!("{}\\{:04}", base, i);
            if let Some(desc) = reg_string(&sub, "DriverDesc") {
                let desc = desc.trim().to_string();
                if desc.is_empty() {
                    continue;
                }
                if desc.to_ascii_lowercase().contains("basic display") {
                    if first.is_none() {
                        first = Some(desc);
                    }
                    continue;
                }
                return Some(desc);
            }
        }
        first
    }

    pub fn os_build() -> Option<String> {
        let key = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
        let product = reg_string(key, "ProductName").unwrap_or_else(|| "Windows".to_string());
        let build = reg_string(key, "CurrentBuild").unwrap_or_default();
        let ubr = reg_u32(key, "UBR");
        let mut name = product.trim().to_string();
        if let Ok(n) = build.trim().parse::<u32>() {
            if n >= 22000 {
                name = name.replace("Windows 10", "Windows 11");
            }
        }
        if name.is_empty() {
            name = "Windows".to_string();
        }
        match (build.trim().is_empty(), ubr) {
            (true, _) => Some(name),
            (false, Some(u)) => Some(format!("{} {}.{}", name, build.trim(), u)),
            (false, None) => Some(format!("{} {}", name, build.trim())),
        }
    }

    pub fn power_plan_guid() -> Option<String> {
        let mut ptr: *mut u8 = std::ptr::null_mut();
        let rc = unsafe { PowerGetActiveScheme(std::ptr::null_mut(), &mut ptr) };
        if rc != 0 || ptr.is_null() {
            return None;
        }
        // GUID bytes: data1 (4, LE), data2 (2, LE), data3 (2, LE), 8 raw bytes
        let b = unsafe { std::slice::from_raw_parts(ptr, 16) };
        let d1 = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
        let d2 = u16::from_le_bytes([b[4], b[5]]);
        let d3 = u16::from_le_bytes([b[6], b[7]]);
        let mut s = format!("{:08x}-{:04x}-{:04x}-", d1, d2, d3);
        for (i, byte) in b[8..16].iter().enumerate() {
            if i == 2 {
                s.push('-');
            }
            s.push_str(&format!("{:02x}", byte));
        }
        Some(s.to_ascii_lowercase())
    }

    pub fn create_restore_point(desc: &str) -> Result<String, String> {
        let mut info = RestorePointInfoW {
            // BEGIN_SYSTEM_CHANGE / MODIFY_SETTINGS
            event_type: 100,
            restore_pt_type: 0,
            sequence: 0,
            description: [0u16; MAX_PATH],
        };
        for (i, unit) in desc.encode_utf16().take(MAX_PATH - 1).enumerate() {
            info.description[i] = unit;
        }
        let mut status = StateMgrStatus {
            status: 0,
            sequence: 0,
            bmid: [0u16; MAX_PATH],
        };
        let ok = unsafe { SRSetRestorePointW(&info, &mut status) };
        if status.status != 0 {
            let hint = match status.status {
                0x8007_0000u32 => "system protection is turned off for this drive",
                _ => "Windows refused to create the checkpoint",
            };
            return Err(format!("{} ({:#x})", hint, status.status as u32));
        }
        if ok == 0 {
            return Err("Windows refused to create the checkpoint".to_string());
        }
        Ok(format!("checkpoint {} created", status.sequence))
    }

    pub fn priorities(pid: u32, class: &str) -> Result<String, String> {
        let want = match class.to_ascii_lowercase().as_str() {
            "realtime" => 0x0000_0100u32,
            "high" => 0x0000_0080,
            "abovenormal" => 0x0000_8000,
            "below" | "belownormal" => 0x0000_4000,
            "idle" | "low" => 0x0000_0040,
            _ => 0x0000_0020,
        };
        unsafe {
            let handle = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
                0,
                pid,
            );
            if handle == 0 || handle == INVALID_HANDLE {
                return Err("cannot open the process (it may need Administrator)".to_string());
            }
            let ok = SetPriorityClass(handle, want);
            CloseHandle(handle);
            if ok == 0 {
                return Err("Windows refused the priority change".to_string());
            }
        }
        Ok(format!("pid {} -> {}", pid, class))
    }

    pub fn procs() -> Vec<(String, u32, f64)> {
        let mut out: Vec<(String, u32, f64)> = Vec::new();
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == 0 || snap == INVALID_HANDLE {
                return out;
            }
            let mut entry: ProcessEntry32W = std::mem::zeroed();
            entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
            let mut more = Process32FirstW(snap, &mut entry) != 0;
            while more {
                let len = entry.exe.iter().position(|c| *c == 0).unwrap_or(MAX_PATH);
                let name = String::from_utf16_lossy(&entry.exe[..len]);
                if !name.is_empty() && entry.pid > 0 {
                    let mut counters = ProcessMemoryCounters::default();
                    counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
                    let mut mb = 0.0;
                    let handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, entry.pid);
                    if handle != 0 && handle != INVALID_HANDLE {
                        if K32GetProcessMemoryInfo(
                            handle,
                            &mut counters,
                            std::mem::size_of::<ProcessMemoryCounters>() as u32,
                        ) != 0
                        {
                            mb = counters.working_set as f64 / (1024.0 * 1024.0);
                        }
                        CloseHandle(handle);
                    }
                    out.push((name, entry.pid, mb));
                }
                more = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
        out.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        out
    }

    pub fn set_affinity(pid: u32, mask: usize) -> Result<(), String> {
        unsafe {
            let handle = OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_SET_INFORMATION,
                0,
                pid,
            );
            if handle == 0 || handle == INVALID_HANDLE {
                return Err("cannot open the process (it may need Administrator)".to_string());
            }
            let ok = SetProcessAffinityMask(handle, mask);
            CloseHandle(handle);
            if ok == 0 {
                return Err("Windows refused the affinity change".to_string());
            }
        }
        Ok(())
    }

    pub fn cpus() -> usize {
        let n = unsafe { GetActiveProcessorCount(0xFFFF) };
        if n == 0 {
            1
        } else {
            n as usize
        }
    }

    pub fn uptime() -> i64 {
        (unsafe { GetTickCount64() } / 1000) as i64
    }
}

// ── shell / elevation ───────────────────────────────────────────────────────

#[cfg(windows)]
mod shell {
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
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Re-launch this executable elevated. Returns true when a new elevated
/// process was started (the caller must then exit).
#[cfg(windows)]
pub fn elevate() -> bool {
    let exe = match env::current_exe() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let file = wide(&exe.to_string_lossy());
    let op = wide("runas");
    let rc = unsafe {
        shell::ShellExecuteW(
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
            shell::MessageBoxW(0, t.as_ptr(), c.as_ptr(), 0x0000_0010 | 0x0000_0040);
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
    let st = unsafe { shell::NtQueryTimerResolution(&mut a, &mut b, &mut cur) };
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

/// Active Windows power plan — resolved from the live GUID, no subprocess.
pub fn power_plan() -> String {
    #[cfg(windows)]
    {
        if let Some(guid) = win::power_plan_guid() {
            let known = match guid.as_str() {
                "381b4222-f694-41f0-9685-ff5bb260df2e" => Some("Balanced"),
                "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" => Some("High performance"),
                "a1841308-3541-4fab-bc81-f71556f20b4a" => Some("Power saver"),
                "e9a42b02-d5df-448d-aa00-03f14749eb61" => Some("Ultimate Performance"),
                _ => None,
            };
            if let Some(name) = known {
                return name.to_string();
            }
            let key = format!(
                r"SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes\{}",
                guid
            );
            if let Some(name) = reg_read_string(&key, "FriendlyName") {
                let name = name.trim().to_string();
                if !name.is_empty() && !name.starts_with('@') {
                    return name;
                }
            }
            return format!("custom ({})", &guid[..8]);
        }
        "unknown".to_string()
    }
    #[cfg(not(windows))]
    {
        "n/a".to_string()
    }
}

/// (image name, pid, working set in MB), sorted by memory. Native on Windows.
pub fn processes() -> Vec<(String, u32, f64)> {
    #[cfg(windows)]
    {
        win::procs()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

pub fn is_process_running(exe: &str) -> bool {
    let want = exe.trim_end_matches(".exe").to_ascii_lowercase();
    processes()
        .iter()
        .any(|(name, _, _)| name.trim_end_matches(".exe").to_ascii_lowercase() == want)
}

/// Raise a process' priority class natively (no PowerShell, no lag).
pub fn set_priority(pid: u32, class: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        win::priorities(pid, class)
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, class);
        Err("process priority is Windows-only".to_string())
    }
}

/// Pin a process to a CPU mask.
pub fn set_affinity(pid: u32, mask: usize) -> Result<(), String> {
    #[cfg(windows)]
    {
        win::set_affinity(pid, mask)
    }
    #[cfg(not(windows))]
    {
        let _ = (pid, mask);
        Err("process affinity is Windows-only".to_string())
    }
}

/// Logical processors on this machine.
pub fn logical_cpus() -> usize {
    #[cfg(windows)]
    {
        win::cpus()
    }
    #[cfg(not(windows))]
    {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    }
}

/// System checkpoint through srclient.dll — instant, unlike Checkpoint-Computer.
pub fn create_restore_point(desc: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        win::create_restore_point(desc)
    }
    #[cfg(not(windows))]
    {
        let _ = desc;
        Err("system restore is Windows-only".to_string())
    }
}

#[cfg(windows)]
pub fn uptime_secs() -> i64 {
    win::uptime()
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
        pub fn GetSystemTimes(
            idle: *mut super::FileTime,
            kernel: *mut super::FileTime,
            user: *mut super::FileTime,
        ) -> i32;
        pub fn GlobalMemoryStatusEx(mem: *mut super::MemStatusEx) -> i32;
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
    let mut vals = line
        .split_whitespace()
        .skip(1)
        .filter_map(|p| p.parse::<u64>().ok());
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
                total = rest
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    / 1024.0
                    / 1024.0;
            }
            if let Some(rest) = line.strip_prefix("MemAvailable:") {
                avail = rest
                    .trim()
                    .trim_end_matches("kB")
                    .trim()
                    .parse::<f64>()
                    .unwrap_or(0.0)
                    / 1024.0
                    / 1024.0;
            }
        }
    }
    let used = if total > avail { total - avail } else { 0.0 };
    (total, used)
}
