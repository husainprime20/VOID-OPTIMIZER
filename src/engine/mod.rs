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
use std::time::{Duration, Instant};

pub const VERSION: &str = "0.2.1";

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

pub struct LogEntry {
    pub at: String,
    pub level: String,
    pub msg: String,
}

pub struct State {
    pub dry_run: bool,
    pub applied: BTreeSet<String>,
    pub log: Vec<LogEntry>,
    pub volume: u8,
    pub muted: bool,
    pub motion: bool,
    pub auto_backup: bool,
    pub last_profile: String,
    pub started: i64,
    pub ops: u32,
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
    pub fn new() -> State {
        let mut s = State {
            dry_run: false,
            applied: BTreeSet::new(),
            log: Vec::new(),
            volume: 34,
            muted: false,
            motion: true,
            auto_backup: true,
            last_profile: String::new(),
            started: platform::epoch_secs(),
            ops: 0,
        };
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

    /// True when nothing may actually be changed on this machine.
    pub fn sim(&self) -> bool {
        self.dry_run || !platform::is_windows()
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
                    "muted" => self.muted = truthy(v),
                    "motion" => self.motion = truthy(v),
                    "auto_backup" => self.auto_backup = truthy(v),
                    "dry_run" => self.dry_run = truthy(v),
                    "last_profile" => self.last_profile = v.to_string(),
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
            "volume={}\nmuted={}\nmotion={}\nauto_backup={}\ndry_run={}\nlast_profile={}\napplied={}\n",
            self.volume,
            if self.muted { 1 } else { 0 },
            if self.motion { 1 } else { 0 },
            if self.auto_backup { 1 } else { 0 },
            if self.dry_run { 1 } else { 0 },
            self.last_profile,
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
            "muted" => self.muted = truthy(value),
            "motion" => self.motion = truthy(value),
            "auto_backup" => self.auto_backup = truthy(value),
            "dry_run" => self.dry_run = truthy(value),
            _ => {}
        }
        self.save();
    }
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

fn do_cmd(state: &State, program: &str, args: &[&str]) -> (bool, String) {
    let shown = format!("{} {}", program, args.join(" "));
    if state.sim() {
        thread::sleep(Duration::from_millis(26));
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
fn do_cmd_lenient(state: &State, program: &str, args: &[&str]) -> (bool, String) {
    let shown = format!("{} {}", program, args.join(" "));
    if state.sim() {
        thread::sleep(Duration::from_millis(18));
        return (true, format!("simulated :: {}", shown));
    }
    match platform::run(program, args) {
        Ok(_) => (true, shown),
        Err(e) => (true, format!("{} -> {}", shown, e)),
    }
}

fn do_ps(state: &State, script: &str, label: &str) -> (bool, String) {
    if state.sim() {
        thread::sleep(Duration::from_millis(70));
        return (true, format!("simulated :: {}", label));
    }
    match platform::powershell(script) {
        Ok(o) => {
            let out = first_line(&o.combined());
            if o.ok {
                (true, format!("{} :: {}", label, out))
            } else {
                (false, format!("{} -> {}", label, out))
            }
        }
        Err(e) => (false, format!("{} -> {}", label, e)),
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

const FF_PROC: &str = "'HD-Player','Bluestacks','BlueStacksApp','LdVBoxHeadless','dnplayer','MEmu','MEmuHeadless','Nox','NoxVMHandle'";

// ───────────────────────────── specials ─────────────────────────────

fn run_special(state: &mut State, name: &str, revert: bool) -> (bool, String) {
    match name {
        "power_ultimate" => {
            if state.sim() {
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
        "clean_temp" => do_ps(
            state,
            r#"$p=@($env:TEMP,"$env:WINDIR\Temp"); $sum=0; foreach($d in $p){ if(Test-Path $d){ try { $sum += (Get-ChildItem -LiteralPath $d -Recurse -Force -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum } catch {}; Get-ChildItem -LiteralPath $d -Force -ErrorAction SilentlyContinue | Remove-Item -Recurse -Force -ErrorAction SilentlyContinue } }; "{0:N0} MB purged" -f ($sum/1MB)"#,
            "temp purge",
        ),
        "ff_priority" => do_ps(
            state,
            &format!(
                r#"$c=0; Get-Process -Name {} -ErrorAction SilentlyContinue | ForEach-Object {{ try {{ $_.PriorityClass='High'; $c++ }} catch {{}} }}; "$c process(es) promoted to HIGH""#,
                FF_PROC
            ),
            "emulator priority",
        ),
        "ff_affinity" => do_ps(
            state,
            &format!(
                r#"$cores=(Get-CimInstance Win32_Processor | Measure-Object -Property NumberOfCores -Sum).Sum; if(-not $cores -or $cores -lt 1){{$cores=4}}; if($cores -gt 16){{$cores=16}}; $mask=0; for($i=0;$i -lt $cores;$i++){{ $mask=$mask -bor ([int64]1 -shl (2*$i)) }}; $c=0; Get-Process -Name {} -ErrorAction SilentlyContinue | ForEach-Object {{ try {{ $_.ProcessorAffinity=[IntPtr]$mask; $c++ }} catch {{}} }}; "cores=$cores pinned=$c""#,
                FF_PROC
            ),
            "emulator affinity",
        ),
        "ff_boost" => {
            let (a, ma) = run_special(state, "ff_priority", false);
            let (b, mb) = run_special(state, "ff_affinity", false);
            let (c, mc) = do_ps(
                state,
                &format!(
                    r#"$n=0; Get-Process -Name {} -ErrorAction SilentlyContinue | ForEach-Object {{ try {{ $_.MinWorkingSet=[IntPtr]0; $n++ }} catch {{}} }}; "working set unlocked on $n""#,
                    FF_PROC
                ),
                "working set",
            );
            (a || b || c, format!("{} · {} · {}", ma, mb, mc))
        }
        "restore_point" => do_ps(
            state,
            "Checkpoint-Computer -Description 'VOID restore point' -RestorePointType MODIFY_SETTINGS; 'restore point created'",
            "system restore point",
        ),
        other => (false, format!("unknown special :: {}", other)),
    }
}

fn net_nagle(state: &mut State, revert: bool) -> (bool, String) {
    if state.sim() {
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
        Action::Reg(k, v, t, d) => do_cmd(
            state,
            "reg",
            &["add", *k, "/v", *v, "/t", *t, "/d", *d, "/f"],
        ),
        Action::RegDel(k, v) => do_cmd_lenient(state, "reg", &["delete", *k, "/v", *v, "/f"]),
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
    if !revert && state.auto_backup && t.risk == 2 && platform::is_windows() && !state.dry_run {
        let _ = do_ps(
            state,
            "Checkpoint-Computer -Description 'VOID auto' -RestorePointType MODIFY_SETTINGS",
            "auto restore point",
        );
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
    let mut out: Vec<Outcome> = Vec::new();
    for id in ids.iter() {
        match find_tweak(id) {
            Some(t) => {
                let o = apply_one(state, t, false);
                if o.ok {
                    if state.sim() {
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
        json::kb("dry_run", state.dry_run),
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
    let (cpu, total_gb, used_gb) = if platform::is_windows() {
        stats_windows()
    } else {
        stats_unix()
    };
    json::obj(vec![
        json::kf("cpu", cpu),
        json::kf("ram", if total_gb > 0.0 { used_gb / total_gb * 100.0 } else { 0.0 }),
        json::kf("ram_total", total_gb),
        json::kf("ram_used", used_gb),
        json::ki("uptime", platform::uptime_secs()),
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

fn stats_windows() -> (f64, f64, f64) {
    let script = "$os=Get-CimInstance Win32_OperatingSystem; $cpu=(Get-CimInstance Win32_Processor | Measure-Object -Property LoadPercentage -Average).Average; if(-not $cpu){$cpu=0}; \"$cpu|$($os.TotalVisibleMemorySize)|$($os.FreePhysicalMemory)\"";
    if let Ok(o) = platform::powershell(script) {
        let line = o.stdout.trim().to_string();
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() >= 3 {
            let cpu = parts[0].trim().parse::<f64>().unwrap_or(0.0);
            let total = parts[1].trim().parse::<f64>().unwrap_or(0.0) / 1024.0 / 1024.0;
            let free = parts[2].trim().parse::<f64>().unwrap_or(0.0) / 1024.0 / 1024.0;
            let used = if total > free { total - free } else { 0.0 };
            return (cpu, total, used);
        }
    }
    (0.0, 0.0, 0.0)
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

pub fn sample() -> Stats {
    let (cpu, total_gb, used_gb) = if platform::is_windows() {
        stats_windows()
    } else {
        stats_unix()
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
    pub path: &'static str,
    pub installed: bool,
    pub running: bool,
}

pub const EMULATORS: &[(&str, &str, &str)] = &[
    ("BlueStacks 5", r"C:\Program Files\BlueStacks_nxt\HD-Player.exe", "HD-Player"),
    ("BlueStacks 4", r"C:\Program Files\BlueStacks\HD-Player.exe", "HD-Player"),
    ("LDPlayer 9", r"C:\LDPlayer\LDPlayer9\dnplayer.exe", "dnplayer"),
    ("LDPlayer 4", r"C:\LDPlayer\LDPlayer4.0\dnplayer.exe", "dnplayer"),
    ("MuMu Player 12", r"C:\Program Files\Netease\MuMuPlayer-12.0\shell\MuMuPlayer.exe", "MuMuPlayer"),
    ("MEmu", r"C:\Program Files\Microvirt\MEmu\MEmu.exe", "MEmu"),
    ("Nox", r"C:\Program Files (x86)\Nox\bin\Nox.exe", "Nox"),
    ("GameLoop", r"C:\Program Files\Tencent\GameLoop\Launcher\GameLoop.exe", "GameLoop"),
    ("Google Play Games", r"C:\Program Files\Google\Play Games\Bootstrapper.exe", "Bootstrapper"),
];

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
        .map(|(name, path, exe)| {
            let needle = format!("{}.exe", exe.to_lowercase());
            Emulator {
                name,
                path,
                installed: platform::exists(path),
                running: running.iter().any(|p| p == &needle || p.starts_with(&needle)),
            }
        })
        .collect()
}

/// Raise the priority of whichever emulator is currently running.
pub fn boost_emulator() -> Result<String, String> {
    let procs = platform::processes();
    for (name, path, exe) in EMULATORS.iter() {
        let needle = format!("{}.exe", exe.to_lowercase());
        if let Some((pname, pid, _)) = procs
            .iter()
            .find(|(p, _, _)| p.to_lowercase() == needle || p.to_lowercase().starts_with(&needle))
        {
            platform::set_priority(*pid, "High")
                .map(|_| format!("{} boosted · pid {} · high", name, pid))
                .map_err(|e| format!("{} ({}) · {}", name, pname, e))?;
            let _ = path;
            return Ok(format!("{} boosted · pid {}", name, pid));
        }
    }
    Err("no emulator is running".to_string())
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
