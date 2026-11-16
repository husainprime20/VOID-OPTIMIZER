// VOID — application shell.
//
// Everything the UI needs lives in this process: the engine runs in-process, so
// there is no localhost socket, no fetch(), and therefore no way for the
// "engine offline" state to come back.
//
// Two rules shape this file:
//   1. The window never blocks. Every change to the machine happens on a worker
//      thread (`engine::Job`); the frame loop only reads progress.
//   2. Nothing on the launch path spawns a process. Machine identity, admin
//      state, power plan and telemetry are all read from the kernel.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{self, Align, Align2, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::audio::Ambient;
use crate::engine::{self, Job, Machine, State, Stats, Work};
use crate::theme::{self, *};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Dashboard,
    Optimize,
    Profiles,
    FreeFire,
    Network,
    Kernel,
    Monitor,
    Aim,
    Settings,
}

pub const VIEWS: [View; 9] = [
    View::Dashboard,
    View::Optimize,
    View::Profiles,
    View::FreeFire,
    View::Network,
    View::Kernel,
    View::Monitor,
    View::Aim,
    View::Settings,
];

impl View {
    pub fn title(self) -> &'static str {
        match self {
            View::Dashboard => "Dashboard",
            View::Optimize => "Optimizer",
            View::Profiles => "Profiles",
            View::FreeFire => "Free Fire",
            View::Network => "Network",
            View::Kernel => "Kernel",
            View::Monitor => "Monitor",
            View::Aim => "Aim Lab",
            View::Settings => "Settings",
        }
    }
    pub fn kicker(self) -> &'static str {
        match self {
            View::Dashboard => "command deck",
            View::Optimize => "the catalog",
            View::Profiles => "one-tap loadouts",
            View::FreeFire => "emulator tuning",
            View::Network => "latency lab",
            View::Kernel => "deep system",
            View::Monitor => "live telemetry",
            View::Aim => "sensitivity",
            View::Settings => "presets & engine",
        }
    }
    pub fn hint(self) -> &'static str {
        match self {
            View::Dashboard => "The state of the machine, the games it can see, and one button that fixes both.",
            View::Optimize => "Every change in the catalog, with its live value on this machine.",
            View::Profiles => "Curated loadouts. One press writes them; one press takes them back.",
            View::FreeFire => "Detect the emulator, pin its cores, raise its priority, then go.",
            View::Network => "Nagle, throttling index and queue shape — 40 ms versus 20 ms.",
            View::Kernel => "Timer resolution, TSC policy, HAGS, scheduler bias.",
            View::Monitor => "One-second telemetry, sleep-jitter benchmark, live process boost.",
            View::Aim => "Convert sensitivity between games and run the warm-up ladder.",
            View::Settings => "Sound, motion, safety rails and the exact machine report.",
        }
    }
}

#[derive(Clone)]
pub enum Action {
    None,
    Apply(Vec<String>),
    Revert(Vec<String>),
    RevertAll,
    Profile(&'static str),
    RestorePoint,
    FlushDns,
    CleanTemp,
    LaunchEmulator(String),
    BoostPid(u32),
    Bench,
    Ping,
    /// One press: esports set, timer hold, net lab, benchmark.
    MatchMode,
    /// 0 = Obsidian, 1 = Porcelain.
    Theme(u8),
    /// Write a diagnostics report next to the settings.
    ExportReport,
    /// Apply the matches of the whole pre-flight checklist.
    FixPreflight,
    /// Jump to a view (command palette).
    View(View),
}

pub struct Modal {
    pub kicker: String,
    pub title: String,
    pub body: String,
    pub plan: Vec<String>,
    pub ok: String,
    pub cancel: String,
    pub action: Action,
    pub info_only: bool,
    pub warn: bool,
}

impl Modal {
    pub fn info(kicker: &str, title: &str, body: &str) -> Modal {
        Modal {
            kicker: kicker.to_string(),
            title: title.to_string(),
            body: body.to_string(),
            plan: Vec::new(),
            ok: "Close".to_string(),
            cancel: String::new(),
            action: Action::None,
            info_only: true,
            warn: false,
        }
    }
}

pub struct Toast {
    pub kind: u8, // 0 ok · 1 info · 2 err
    pub msg: String,
    pub born: f64,
}

pub struct App {
    pub view: View,
    pub state: State,
    pub machine: Machine,
    pub stats: Stats,
    pub cpu_hist: Vec<f32>,
    pub ram_hist: Vec<f32>,
    pub search: String,
    pub cat: usize,
    pub risk: u8,
    pub expanded: Option<&'static str>,
    pub bench: Option<engine::BenchResult>,
    pub bench_prev: Option<engine::BenchResult>,
    pub emulators: Vec<engine::Emulator>,
    pub procs: Vec<(String, u32, f64)>,
    pub games: Vec<(&'static str, &'static str)>,
    pub games_at: f64,
    pub audio: Ambient,
    pub modal: Option<Modal>,
    pub toasts: Vec<Toast>,
    pub started: std::time::Instant,
    pub clock: String,
    pub polled_at: f64,
    pub procs_at: f64,
    pub dpi: f32,
    pub sens: f32,
    pub aim_locked: bool,
    pub selftest_report: Option<String>,

    /// Net lab results (handshake RTT per endpoint).
    pub ping: Vec<engine::PingRow>,
    pub ping_at: f64,
    /// Bench scores across sessions.
    pub bench_hist: Vec<f64>,
    /// Animated catalog coverage, eased toward the real value.
    pub cov: f32,
    /// Cached timer resolution in ms (0.0 = unknown).
    pub timer_ms: f32,
    /// Eased cpu / memory percentages for the meters.
    pub cpu_e: f32,
    pub ram_e: f32,
    /// Gliding rail indicator position.
    pub nav_y: f32,
    /// 0..1 sweep used as the "ignite" animation while a job runs.
    pub sweep: f32,
    /// When the current job started, for the sweep.
    pub job_born: f64,
    /// Queued work that runs after the current job finishes (match mode).
    pub chain: Vec<Work>,

    /// The one worker job allowed at a time.
    pub job: Option<Job>,
    /// Last finished job summary, shown in the rail.
    pub last_job: String,
    /// Live read-back of every registry tweak on this machine.
    pub live: HashMap<&'static str, bool>,
    pub live_at: f64,
    /// Set when the view changed, drives the wipe.
    pub view_at: f64,
    /// Discord presence worker.
    pub rpc: crate::rpc::Rpc,
    /// Pre-flight checklist, recomputed on a slow tick.
    pub checks: Vec<engine::Check>,
    pub checks_at: f64,
    /// Guard tick: re-asserts drifted tweaks.
    pub guard_at: f64,
    /// Command palette.
    pub palette: bool,
    pub palette_q: String,
    /// Audio starts on frame two so the window is never waiting on WASAPI.
    pub frames: u32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let state = State::new();
        let presence_on = state.presence;
        // CI can force a palette so both themes get rendered (see preview.yml)
        let theme_idx = match std::env::var("VOID_THEME").unwrap_or_default().as_str() {
            "light" => 1,
            "dark" => 0,
            _ => state.theme,
        };
        theme::set_mode(if theme_idx == 1 {
            theme::Mode::Light
        } else {
            theme::Mode::Dark
        });
        theme::restyle(&cc.egui_ctx);
        let machine = Machine::detect();
        let mut audio = Ambient::new(0, state.volume as f32 / 100.0);
        audio.set_intensity(state.intensity as f32 / 100.0);
        // prime the CPU counters so the first painted frame has a real delta
        let primed = engine::sample();
        let stats = if primed.total_gb > 0.0 {
            primed
        } else {
            Stats::default()
        };
        // Lets CI render a specific view head-lessly (see .github/workflows/preview.yml)
        let view = match std::env::var("VOID_VIEW").unwrap_or_default().as_str() {
            "optimize" => View::Optimize,
            "profiles" => View::Profiles,
            "freefire" => View::FreeFire,
            "network" => View::Network,
            "kernel" => View::Kernel,
            "monitor" => View::Monitor,
            "aim" => View::Aim,
            "settings" => View::Settings,
            _ => View::Dashboard,
        };
        Self {
            view,
            state,
            machine,
            stats,
            cpu_hist: vec![0.0; 60],
            ram_hist: vec![0.0; 60],
            search: String::new(),
            cat: 0,
            risk: 0,
            expanded: None,
            bench: None,
            bench_prev: None,
            emulators: Vec::new(),
            procs: Vec::new(),
            games: Vec::new(),
            games_at: -20.0,
            audio,
            modal: None,
            toasts: Vec::new(),
            started: std::time::Instant::now(),
            clock: crate::platform::clock_label(),
            polled_at: -10.0,
            procs_at: -10.0,
            dpi: 800.0,
            sens: 0.9,
            aim_locked: false,
            selftest_report: None,
            ping: Vec::new(),
            ping_at: -10.0,
            bench_hist: engine::bench_history(),
            cov: 0.0,
            timer_ms: 0.0,
            cpu_e: 0.0,
            ram_e: 0.0,
            nav_y: -1.0,
            sweep: 1.0,
            job_born: 0.0,
            chain: Vec::new(),
            job: None,
            last_job: String::new(),
            live: HashMap::new(),
            live_at: -10.0,
            view_at: -10.0,
            rpc: crate::rpc::Rpc::start(presence_on),
            checks: Vec::new(),
            checks_at: -10.0,
            guard_at: 0.0,
            palette: false,
            palette_q: String::new(),
            frames: 0,
        }
    }

    // ── plumbing ────────────────────────────────────────────────────────────
    pub fn toast(&mut self, kind: u8, msg: impl Into<String>) {
        self.toasts.push(Toast {
            kind,
            msg: msg.into(),
            born: self.started.elapsed().as_secs_f64(),
        });
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Is this tweak actually live on the machine right now?
    pub fn live_state(&self, id: &str) -> Option<bool> {
        self.live.get(id).copied()
    }

    fn start(&mut self, ctx: &egui::Context, work: Work) {
        if self.job.is_some() {
            self.toast(1, "one job at a time — let the current one finish");
            return;
        }
        let repaint_ctx = ctx.clone();
        let repaint: Arc<dyn Fn() + Send + Sync> = Arc::new(move || repaint_ctx.request_repaint());
        self.job_born = self.started.elapsed().as_secs_f64();
        self.sweep = 0.0;
        self.job = Some(Job::start(self.state.clone(), work, repaint));
    }

    pub fn set_view(&mut self, view: View, now: f64) {
        if self.view != view {
            self.view = view;
            self.view_at = now;
            self.expanded = None;
        }
    }

    pub fn toggle_tweak(&mut self, ctx: &egui::Context, id: &'static str) {
        let on = self.state.applied.contains(id);
        let ids = vec![id.to_string()];
        if on {
            self.start(ctx, Work::Revert(ids));
        } else {
            self.start(ctx, Work::Apply(ids));
        }
    }

    pub fn revert_set(&mut self, ctx: &egui::Context, ids: Vec<String>) {
        if ids.is_empty() {
            self.toast(1, "nothing applied");
            return;
        }
        self.start(ctx, Work::Revert(ids));
    }

    pub fn apply_set(&mut self, ctx: &egui::Context, ids: Vec<String>) {
        if ids.is_empty() {
            self.toast(1, "nothing selected");
            return;
        }
        self.start(ctx, Work::Apply(ids));
    }

    pub fn run_action(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::None => {}
            Action::Apply(ids) => self.apply_set(ctx, ids),
            Action::Revert(ids) => self.revert_set(ctx, ids),
            Action::RevertAll => {
                if self.state.applied.is_empty() {
                    self.toast(1, "nothing applied");
                } else {
                    self.start(ctx, Work::RevertAll);
                }
            }
            Action::Profile(pid) => self.start(ctx, Work::Profile(pid)),
            Action::RestorePoint => self.start(ctx, Work::Restore),
            Action::FlushDns => self.start(ctx, Work::Apply(vec!["net.dns".to_string()])),
            Action::CleanTemp => self.start(ctx, Work::Apply(vec!["disk.temp".to_string()])),
            Action::LaunchEmulator(path) => match engine::launch_emulator(&path) {
                Ok(m) => self.toast(0, m),
                Err(e) => self.toast(2, e),
            },
            Action::BoostPid(pid) => match engine::boost_pid(pid) {
                Ok(m) => self.toast(0, format!("priority raised · {}", m)),
                Err(e) => self.toast(2, format!("boost failed · {}", e)),
            },
            Action::Bench => self.start(ctx, Work::Bench),
            Action::Ping => self.start(ctx, Work::Ping),
            Action::View(v) => {
                let now = self.started.elapsed().as_secs_f64();
                self.set_view(v, now);
            }
            Action::Theme(n) => {
                self.state.theme = n.min(1);
                self.state.save();
                theme::set_mode(if self.state.theme == 1 {
                    theme::Mode::Light
                } else {
                    theme::Mode::Dark
                });
                theme::restyle(ctx);
                self.toast(
                    0,
                    if self.state.theme == 1 {
                        "porcelain — light theme"
                    } else {
                        "obsidian — dark theme"
                    },
                );
            }
            Action::ExportReport => {
                let (score, _) = self.score();
                let checks = self.checks.clone();
                let jitter = self.bench.as_ref().map(|b| b.jitter);
                let rows: Vec<&engine::PingRow> = self.ping.iter().filter(|r| r.ok).collect();
                let ping = if rows.is_empty() {
                    None
                } else {
                    Some(rows.iter().map(|r| r.avg).sum::<f64>() / rows.len() as f64)
                };
                match engine::export_report(&self.state, score, &checks, jitter, ping) {
                    Ok(p) => self.toast(0, format!("report · {}", theme::clip(&p.display().to_string(), 70))),
                    Err(e) => self.toast(2, e),
                }
            }
            Action::FixPreflight => {
                let mut ids: Vec<String> = Vec::new();
                for c in self.checks.iter().filter(|c| !c.ok) {
                    if let Some(profile) = c.fix {
                        if let Some(p) = engine::catalog::PROFILES.iter().find(|p| p.id == profile) {
                            for id in p.tweaks.iter() {
                                let sid = id.to_string();
                                if !self.state.applied.contains(&sid) && !ids.contains(&sid) {
                                    ids.push(sid);
                                }
                            }
                        }
                    }
                }
                if ids.is_empty() {
                    self.toast(1, "nothing actionable — measure jitter and net lab first");
                } else {
                    self.apply_set(ctx, ids);
                }
            }
            Action::MatchMode => {
                self.chain = vec![Work::Ping, Work::Bench];
                self.toast(1, "match mode · arming".to_string());
                self.start(ctx, Work::Profile("esports"));
            }
        }
    }

    /// Drain the worker and fold the result back into the shell.
    fn pump_job(&mut self, cc: &egui::Context) {
        let mut finished: Option<(String, Vec<engine::Outcome>, State)> = None;
        if let Some(job) = self.job.as_mut() {
            let event = job.pump();
            if let Some(b) = event.bench {
                if let Some(prev) = self.bench {
                    self.bench_prev = Some(prev);
                }
                engine::record_bench(b.score);
                self.bench_hist = engine::bench_history();
                self.bench = Some(b);
            }
            if let Some(rows) = event.ping {
                self.ping = rows;
                self.ping_at = self.started.elapsed().as_secs_f64();
            }
            finished = job.take_finished();
        }
        if let Some((label, outcomes, state)) = finished {
            let failed = outcomes.iter().filter(|o| !o.ok).count();
            let ok = outcomes.len().saturating_sub(failed);
            self.state = state;
            self.last_job = if outcomes.is_empty() {
                label.clone()
            } else {
                format!("{} · {}/{} ok", label, ok, outcomes.len())
            };
            if failed == 0 {
                if !outcomes.is_empty() {
                    self.toast(0, format!("{} · {} step(s) applied", label, ok));
                } else {
                    self.toast(0, format!("{} done", label));
                }
            } else {
                let first = outcomes
                    .iter()
                    .find(|o| !o.ok)
                    .map(|o| o.msg.clone())
                    .unwrap_or_default();
                self.toast(
                    2,
                    format!("{} failed · {}", failed, theme::clip(&first, 84)),
                );
            }
            self.job = None;
            self.live_at = -10.0;
            if label.to_lowercase().contains("benchmark") {
                self.view = View::Monitor;
            }
            // match mode: the queued steps run back to back
            if let Some(next) = self.chain.first().cloned() {
                self.chain.remove(0);
                let ctx = cc.clone();
                self.start(&ctx, next);
            }
        }
    }

    // ── presence ────────────────────────────────────────────────────────────

    fn presence_tick(&mut self) {
        if !self.state.presence {
            return;
        }
        let (applied, verified) = self.tally();
        let total = engine::catalog::TWEAKS.len();
        let (score, _) = self.score();
        let mood = if self.busy() { "matchmode" } else { "idle" };
        let details = if self.busy() {
            format!("{} · {}/{} steps", theme::clip(&self.last_job, 60), applied, total)
        } else {
            format!("{}/{} tuned · {}% · score {}", applied, total, (applied * 100 / total.max(1)), score)
        };
        let state = format!(
            "{} · {} verified · {}",
            self.view.title(),
            verified,
            engine::VERSION
        );
        self.rpc.set(crate::rpc::Presence {
            details,
            state,
            large_text: format!("VOID {} — competitive optimizer", engine::VERSION),
            start: self.state.started,
            mood: mood.to_string(),
        });
    }

    // ── guard ───────────────────────────────────────────────────────────────

    /// Every 20 s, read the applied tweaks back. Anything that drifted is
    /// re-applied without asking — that is the point of a guard.
    fn guard_tick(&mut self, ctx: &egui::Context, now: f64) {
        if !self.state.guard || self.busy() {
            return;
        }
        if now - self.guard_at < 20.0 {
            return;
        }
        self.guard_at = now;
        if self.state.applied.is_empty() {
            return;
        }
        let drifted = engine::drift(&self.state);
        if drifted.is_empty() {
            return;
        }
        let n = drifted.len();
        self.toast(1, format!("guard · {} drifted · re-asserting", n));
        let ctx = ctx.clone();
        self.start(&ctx, engine::Work::Apply(drifted));
    }

    // ── pre-flight ──────────────────────────────────────────────────────────

    fn checks_tick(&mut self, now: f64) {
        if now - self.checks_at < 4.0 && !self.checks.is_empty() {
            return;
        }
        self.checks_at = now;
        let (applied, verified) = self.tally();
        let jitter = self.bench.as_ref().map(|b| b.jitter);
        let rows: Vec<&engine::PingRow> = self.ping.iter().filter(|r| r.ok).collect();
        let ping = if rows.is_empty() {
            None
        } else {
            Some(rows.iter().map(|r| r.avg).sum::<f64>() / rows.len() as f64)
        };
        self.checks = engine::preflight(applied, verified, jitter, ping);
    }

    /// The VOID score: one number for how ready this machine is to compete.
    /// Every component is measured, nothing is guessed, and a component with
    /// no measurement is dropped instead of being scored as zero.
    pub fn score(&self) -> (u32, Vec<(&'static str, f32, String)>) {
        let mut parts: Vec<(&'static str, f32, f32, String)> = Vec::new();

        let total = engine::catalog::TWEAKS.len().max(1);
        let ready = self.state.applied.len() as f32 / total as f32;
        parts.push(("readiness", ready, 0.30, format!("{}/{}", self.state.applied.len(), total)));

        if self.timer_ms > 0.0 {
            let v = (1.0 - ((self.timer_ms - 0.5) / 14.5)).clamp(0.0, 1.0);
            parts.push(("timer", v, 0.22, format!("{:.3} ms", self.timer_ms)));
        }
        if let Some(b) = self.bench.as_ref() {
            let v = (1.0 - (b.jitter / 3.0)).clamp(0.0, 1.0) as f32;
            parts.push(("jitter", v, 0.22, format!("{:.2} ms", b.jitter)));
        }
        let rows: Vec<&engine::PingRow> = self.ping.iter().filter(|r| r.ok).collect();
        if !rows.is_empty() {
            let avg = rows.iter().map(|r| r.avg).sum::<f64>() / rows.len() as f64;
            let v = (1.0 - ((avg as f32 - 10.0) / 150.0)).clamp(0.0, 1.0);
            parts.push(("net", v, 0.26, format!("{:.0} ms", avg)));
        }

        let weight: f32 = parts.iter().map(|p| p.2).sum();
        let score = if weight > 0.0 {
            parts.iter().map(|p| p.1 * p.2).sum::<f32>() / weight * 100.0
        } else {
            0.0
        };
        (
            score.round().clamp(0.0, 100.0) as u32,
            parts
                .into_iter()
                .map(|(l, v, _w, d)| (l, v, d))
                .collect::<Vec<(&'static str, f32, String)>>(),
        )
    }

    fn poll(&mut self, ctx: &egui::Context, now: f64) {
        let live = matches!(self.view, View::Dashboard | View::Monitor);
        if live && now - self.polled_at > 1.0 {
            self.polled_at = now;
            self.stats = engine::sample();
            self.timer_ms = engine::timer_resolution_ms().unwrap_or(0.0) as f32;
            self.cpu_hist.push(self.stats.cpu as f32);
            self.ram_hist.push(self.stats.ram as f32);
            if self.cpu_hist.len() > 80 {
                self.cpu_hist.remove(0);
            }
            if self.ram_hist.len() > 80 {
                self.ram_hist.remove(0);
            }
        }
        if self.view == View::Dashboard && now - self.games_at > 6.0 {
            self.games_at = now;
            self.games = engine::running_games();
        }
        if self.view == View::Monitor && now - self.procs_at > 5.0 {
            self.procs_at = now;
            self.procs = engine::processes();
        }
        if self.view == View::FreeFire && self.emulators.is_empty() {
            self.emulators = engine::emulators();
        }
        // Read every registry tweak back from Windows (cheap: ~2 us each).
        if now - self.live_at > 1.5 && self.job.is_none() {
            self.live_at = now;
            self.live.clear();
            for t in engine::catalog::TWEAKS.iter() {
                if let Some(v) = engine::live_status(t.id) {
                    self.live.insert(t.id, v);
                }
            }
        }
        let tick = if self.state.motion { 33 } else { 900 };
        ctx.request_repaint_after(Duration::from_millis(tick));
        self.clock = crate::platform::clock_label();
        self.toasts.retain(|t| now - t.born < 4.6);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.modal.is_some() {
            self.modal = None;
        }
    }

    /// Applied / verified counts for the dashboard hero.
    pub fn tally(&self) -> (usize, usize) {
        let applied = self.state.applied.len();
        let verified = self
            .state
            .applied
            .iter()
            .filter(|id| self.live.get(id.as_str()) == Some(&true))
            .count();
        (applied, verified)
    }

    // ── chrome ──────────────────────────────────────────────────────────────
    fn rail(&mut self, ui: &mut Ui, now: f64) {
        ui.add_space(4.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 40.0), Sense::hover());
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y - 2.0),
            Align2::LEFT_BOTTOM,
            "VOID",
            title_font(25.0),
            txt(),
        );
        // breathing dot
        let t = self.started.elapsed().as_secs_f32();
        let pulse = if self.state.motion {
            (t * 1.5).sin() * 0.5 + 0.5
        } else {
            1.0
        };
        ui.painter().circle_filled(
            Pos2::new(rect.right() - 3.0, rect.center().y - 6.0),
            2.5,
            a(white(), (70.0 + 150.0 * pulse) as u8),
        );
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y + 9.0),
            Align2::LEFT_CENTER,
            engine::VERSION,
            mono_font(9.5),
            ghost(),
        );
        ui.painter().text(
            Pos2::new(rect.right() - 3.0, rect.center().y + 9.0),
            Align2::RIGHT_CENTER,
            "native",
            mono_font(9.5),
            ghost(),
        );

        ui.add_space(14.0);
        theme::hairline(ui);
        ui.add_space(12.0);

        for (i, v) in VIEWS.iter().enumerate() {
            if self.nav_item(ui, *v, i + 1, now) {
                self.set_view(*v, now);
            }
            ui.add_space(1.0);
        }
        if self.nav_y > 0.0 {
            let x = ui.min_rect().left();
            let bar = Rect::from_min_max(
                Pos2::new(x, self.nav_y),
                Pos2::new(x + 1.5, self.nav_y + 16.0),
            );
            ui.painter().rect_filled(bar, 1.0, white());
        }

        // ── bottom: sound, engine state ──
        let need = 214.0;
        let space = ui.available_height() - need;
        if space > 0.0 {
            ui.add_space(space);
        }
        theme::hairline(ui);
        ui.add_space(12.0);

        let playing = self.audio.playing;
        ui.horizontal(|ui| {
            let mut p = playing;
            if theme::toggle(ui, &mut p).changed() {
                self.audio.toggle();
                self.state.muted = !self.audio.playing;
                self.state.save();
            }
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(tracked(if playing { "ambient" } else { "silent" }))
                        .font(micro_font())
                        .color(if playing { txt() } else { faint() }),
                ));
                ui.add(
                    egui::Label::new(
                        RichText::new(crate::audio::PRESETS[self.audio.preset].0)
                            .font(mono_font(10.5))
                            .color(ghost()),
                    ),
                );
            });
        });
        ui.add_space(8.0);
        let mut vol = self.audio.volume;
        if theme::slider(ui, &mut vol, ui.available_width() - 2.0).changed() {
            self.audio.set_volume(vol);
            self.state.volume = (vol * 100.0).round() as u8;
            self.state.save();
        }
        ui.add_space(8.0);

        // level meter
        let (mrect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 20.0), Sense::hover());
        let lvl = self.audio.level();
        if lvl > 0.01 {
            let bars = 22;
            let step = mrect.width() / bars as f32;
            for b in 0..bars {
                let energy = ((b as f32 * 0.7).sin() * 0.5 + 0.5) * lvl;
                let bh = 3.0 + 15.0 * energy;
                let x = mrect.left() + b as f32 * step;
                ui.painter().rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x, mrect.bottom() - bh),
                        Pos2::new(x + 3.0, mrect.bottom()),
                    ),
                    1.0,
                    a(white(), 165),
                );
            }
        } else {
            // silence: a single hairline instead of a row of dead bars
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(mrect.left(), mrect.center().y),
                    Pos2::new(mrect.right(), mrect.center().y + 1.0),
                ),
                0.0,
                a(white(), 14),
            );
        }

        ui.add_space(12.0);
        let (srect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
        let dot = if self.busy() { dim() } else { white() };
        ui.painter()
            .circle_filled(Pos2::new(srect.left() + 3.0, srect.center().y), 2.5, dot);
        let msg = if self.busy() {
            "working".to_string()
        } else if self.state.applied.is_empty() {
            "awaiting orders".to_string()
        } else {
            format!("{} live", self.state.applied.len())
        };
        ui.painter().text(
            Pos2::new(srect.left() + 12.0, srect.center().y),
            Align2::LEFT_CENTER,
            tracked(&msg),
            micro_font(),
            faint(),
        );
    }

    fn nav_item(&mut self, ui: &mut Ui, v: View, index: usize, now: f64) -> bool {
        let h = 32.0;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        let active = self.view == v;
        let t = ui.ctx().animate_bool(resp.id, active || resp.hovered());
        let _ = now;

        if active {
            ui.painter().rect_filled(rect, 7.0, a(white(), 9));
            // the indicator glides between items instead of snapping
            let ty = rect.top() + 8.0;
            if self.nav_y < 0.0 || !self.state.motion {
                self.nav_y = ty;
            } else {
                self.nav_y += (ty - self.nav_y) * 0.22;
                if (ty - self.nav_y).abs() < 0.3 {
                    self.nav_y = ty;
                }
            }
        } else if t > 0.01 {
            ui.painter().rect_filled(rect, 7.0, a(white(), (t * 5.0) as u8));
        }

        let fg = if active {
            txt()
        } else if resp.hovered() {
            dim()
        } else {
            faint()
        };
        ui.painter().text(
            Pos2::new(rect.left() + 13.0, rect.center().y),
            Align2::LEFT_CENTER,
            v.title(),
            label_font(),
            fg,
        );
        ui.painter().text(
            Pos2::new(rect.right() - 10.0, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{:02}", index),
            mono_font(9.0),
            if active { dim() } else { ghost() },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }

    fn topbar(&mut self, ui: &mut Ui, now: f64) {
        let h = 56.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
        let age = (now - self.view_at) as f32;
        let appear = (age / 0.22).clamp(0.0, 1.0);
        let slide = (1.0 - appear) * 8.0;

        ui.painter().text(
            Pos2::new(rect.left(), rect.top() + 8.0 + slide),
            Align2::LEFT_TOP,
            tracked(self.view.kicker()),
            micro_font(),
            ghost(),
        );
        ui.painter().text(
            Pos2::new(rect.left(), rect.top() + 22.0 + slide * 1.6),
            Align2::LEFT_TOP,
            self.view.title(),
            num_font(25.0),
            txt(),
        );

        let mut right = rect.right();

        // clock
        ui.painter().text(
            Pos2::new(right, rect.center().y),
            Align2::RIGHT_CENTER,
            &self.clock,
            mono_font(11.0),
            faint(),
        );
        right -= 78.0;

        // admin chip
        let admin_txt = if self.machine.admin { "admin" } else { "limited" };
        let galley = ui
            .painter()
            .layout_no_wrap(tracked(admin_txt), micro_font(), txt());
        let w = galley.size().x + 18.0;
        let box_rect = Rect::from_center_size(
            Pos2::new(right - w / 2.0, rect.center().y),
            Vec2::new(w, 20.0),
        );
        if self.machine.admin {
            ui.painter().rect_filled(box_rect, 10.0, a(white(), 232));
        } else {
            ui.painter().rect_filled(box_rect, 10.0, a(white(), 8));
            ui.painter()
                .rect_stroke(box_rect, 10.0, Stroke::new(1.0_f32, line2()));
        }
        ui.painter().text(
            box_rect.center(),
            Align2::CENTER_CENTER,
            tracked(admin_txt),
            micro_font(),
            if self.machine.admin { bg() } else { dim() },
        );
        right -= w + 18.0;

        // live readouts, right to left — label above, number under it
        let timer = match engine::timer_resolution_ms() {
            Some(v) => format!("{:.1}", v),
            None => "--".to_string(),
        };
        let readouts: [(&str, String); 3] = [
            ("timer", timer),
            ("ram", format!("{:.0}", self.stats.ram)),
            ("cpu", format!("{:.0}", self.stats.cpu)),
        ];
        for (label, value) in readouts.iter() {
            ui.painter().text(
                Pos2::new(right, rect.top() + 2.0),
                Align2::RIGHT_TOP,
                tracked(label),
                micro_font(),
                ghost(),
            );
            ui.painter().text(
                Pos2::new(right, rect.top() + 15.0),
                Align2::RIGHT_TOP,
                value.clone(),
                num_font(21.0),
                txt(),
            );
            right -= 66.0;
        }

        // palette + theme controls, left of the readouts
        right -= 12.0;
        let mut clicked_theme = false;
        let chip_w = 54.0;
        let theme_rect = Rect::from_center_size(
            Pos2::new(right - chip_w * 0.5, rect.center().y),
            Vec2::new(chip_w, 22.0),
        );
        let theme_resp = ui.interact(
            theme_rect,
            egui::Id::new("void.themebtn"),
            Sense::click(),
        );
        let hovered = theme_resp.hovered();
        ui.painter().rect_filled(
            theme_rect,
            11.0,
            a(white(), if hovered { 22 } else { 10 }),
        );
        ui.painter().text(
            theme_rect.center(),
            Align2::CENTER_CENTER,
            tracked(if theme::light() { "light" } else { "dark" }),
            micro_font(),
            if hovered { txt() } else { faint() },
        );
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if theme_resp.clicked() {
            clicked_theme = true;
        }
        right -= chip_w + 8.0;

        let pal_rect = Rect::from_center_size(
            Pos2::new(right - 26.0, rect.center().y),
            Vec2::new(52.0, 22.0),
        );
        let pal_resp = ui.interact(pal_rect, egui::Id::new("void.palbtn"), Sense::click());
        ui.painter().rect_filled(
            pal_rect,
            11.0,
            a(white(), if pal_resp.hovered() { 22 } else { 10 }),
        );
        ui.painter().text(
            pal_rect.center(),
            Align2::CENTER_CENTER,
            "⌘K",
            mono_font(10.0),
            if pal_resp.hovered() { txt() } else { ghost() },
        );
        if pal_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if pal_resp.clicked() {
            self.palette = !self.palette;
            self.palette_q.clear();
        }

        if clicked_theme {
            let next = if theme::light() { 0 } else { 1 };
            let ctx = ui.ctx().clone();
            self.run_action(&ctx, Action::Theme(next));
        }
    }

    fn job_strip(&mut self, ui: &mut Ui) {
        let h = 40.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
        let Some(job) = self.job.as_ref() else {
            return;
        };
        let p = job.progress();
        let t = self.started.elapsed().as_secs_f32();
        let pulse = (t * 4.0).sin() * 0.5 + 0.5;

        ui.painter()
            .circle_filled(Pos2::new(rect.left(), rect.center().y), 3.0, a(white(), 120));
        ui.painter().circle_stroke(
            Pos2::new(rect.left(), rect.center().y),
            6.0 + pulse * 2.0,
            Stroke::new(1.0_f32, a(white(), (70.0 * (1.0 - pulse)) as u8)),
        );
        ui.painter().text(
            Pos2::new(rect.left() + 18.0, rect.center().y),
            Align2::LEFT_CENTER,
            job.label.clone(),
            label_font(),
            txt(),
        );
        let done_txt = if job.total > 1 {
            format!("{:02}/{:02}", job.done, job.total)
        } else {
            String::new()
        };
        ui.painter().text(
            Pos2::new(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            done_txt,
            mono_font(10.5),
            faint(),
        );
        let cur = theme::clip(&job.current, 60);
        ui.painter().text(
            Pos2::new(rect.center().x - 20.0, rect.center().y),
            Align2::CENTER_CENTER,
            cur,
            mono_font(10.5),
            ghost(),
        );
        // progress hairline
        let track = Rect::from_min_max(
            Pos2::new(rect.left(), rect.bottom() - 1.0),
            Pos2::new(rect.right(), rect.bottom()),
        );
        ui.painter().rect_filled(track, 0.0, a(white(), 14));
        let fill = Rect::from_min_max(
            track.left_top(),
            Pos2::new(track.left() + track.width() * p.max(0.02), track.bottom()),
        );
        ui.painter().rect_filled(fill, 0.0, a(white(), 200));
        // a shimmer that travels along the filled part of the hairline
        if self.state.motion && fill.width() > 12.0 {
            let phase = (t * 0.55) % 1.0;
            let sx = fill.left() + fill.width() * phase;
            let glow = Rect::from_min_max(
                Pos2::new((sx - 26.0).max(fill.left()), fill.top() - 1.0),
                Pos2::new((sx + 26.0).min(fill.right()), fill.bottom() + 1.0),
            );
            ui.painter().rect_filled(glow, 0.0, a(white(), 60));
        }
    }

    // ── overlays ────────────────────────────────────────────────────────────
    fn draw_toasts(&mut self, ctx: &egui::Context, now: f64) {
        if self.toasts.is_empty() {
            return;
        }
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("void.toasts"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(screen.right() - 400.0, screen.bottom() - 130.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_width(376.0);
                for t in self.toasts.iter().rev() {
                    let age = now - t.born;
                    let appear = ((age / 0.16).min(1.0)) as f32;
                    let fade = if age > 3.8 {
                        (1.0 - (age - 3.8) / 0.8).max(0.0) as f32
                    } else {
                        1.0
                    };
                    let alpha = (appear * fade * 255.0) as u8;
                    if alpha == 0 {
                        continue;
                    }
                    // slide in from the right, fade in fast
                    ui.add_space((1.0 - appear) * 8.0);
                    let (mut rect, _) =
                        ui.allocate_exact_size(Vec2::new(376.0, 40.0), Sense::hover());
                    rect = rect.translate(Vec2::new(-(1.0 - appear) * 26.0, 0.0));
                    let life = (1.0 - (age - 3.8).max(0.0) / 0.8).clamp(0.0, 1.0) as f32;
                    let fill =
                        Color32::from_rgba_unmultiplied(13, 13, 16, (alpha as f32 * 0.97) as u8);
                    ui.painter().rect_filled(rect, 10.0, fill);
                    ui.painter().rect_stroke(
                        rect,
                        10.0,
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(64, 64, 72, alpha)),
                    );
                    let dot = match t.kind {
                        0 => Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        1 => Color32::from_rgba_unmultiplied(160, 160, 168, alpha),
                        _ => Color32::from_rgba_unmultiplied(96, 96, 104, alpha),
                    };
                    ui.painter().circle_filled(
                        Pos2::new(rect.left() + 16.0, rect.center().y),
                        3.0,
                        dot,
                    );
                    // life hairline: the toast visibly runs out of time
                    let track = Rect::from_min_max(
                        Pos2::new(rect.left() + 12.0, rect.bottom() - 2.0),
                        Pos2::new(rect.right() - 12.0, rect.bottom() - 1.0),
                    );
                    ui.painter()
                        .rect_filled(track, 0.0, a(white(), (14.0 * appear * fade) as u8));
                    ui.painter().rect_filled(
                        Rect::from_min_max(
                            track.left_top(),
                            Pos2::new(track.left() + track.width() * life, track.bottom()),
                        ),
                        0.0,
                        a(white(), (110.0 * appear * fade) as u8),
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + 30.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        theme::clip(&t.msg, 64),
                        label_font(),
                        Color32::from_rgba_unmultiplied(238, 238, 241, alpha),
                    );
                    ui.add_space(6.0);
                }
            });
    }

    /// Everything the palette can reach, ranked by prefix match first.
    fn palette_hits(&self) -> Vec<(String, String, Action)> {
        let q = self.palette_q.trim().to_lowercase();
        let hit = |label: &str| q.is_empty() || label.to_lowercase().contains(&q);

        let mut out: Vec<(String, String, Action)> = Vec::new();

        for v in VIEWS.iter() {
            if hit(v.title()) {
                out.push((v.title().to_string(), "view".into(), Action::View(*v)));
            }
        }

        let actions: Vec<(&str, &str, Action)> = vec![
            (
                "Match mode",
                "esports set → net lab → bench",
                Action::MatchMode,
            ),
            ("Net lab", "handshake RTT to 5 endpoints", Action::Ping),
            ("Benchmark sleep jitter", "120 wake-ups, scored", Action::Bench),
            (
                if self.state.theme == 1 {
                    "Obsidian · dark theme"
                } else {
                    "Porcelain · light theme"
                },
                "flip the whole deck",
                Action::Theme(if self.state.theme == 1 { 0 } else { 1 }),
            ),
            (
                "Export diagnostics report",
                "markdown · machine + score + journal",
                Action::ExportReport,
            ),
            (
                "Fix pre-flight",
                "apply every failing check",
                Action::FixPreflight,
            ),
            ("Restore point", "system checkpoint", Action::RestorePoint),
            ("Flush DNS", "clear the resolver cache", Action::FlushDns),
            ("Clean temp files", "user + windows temp", Action::CleanTemp),
            ("Revert everything", "undo every applied change", Action::RevertAll),
        ];
        for (label, sub, action) in actions {
            if hit(label) {
                out.push((label.to_string(), sub.to_string(), action));
            }
        }

        for t in engine::catalog::TWEAKS.iter() {
            if out.len() >= 9 {
                break;
            }
            if q.is_empty() || t.name.to_lowercase().contains(&q) || t.id.contains(&q) {
                let applied = self.state.applied.contains(t.id);
                out.push((
                    t.name.to_string(),
                    format!("{} · {}", t.id, if applied { "applied" } else { "off" }),
                    if applied {
                        Action::Revert(vec![t.id.to_string()])
                    } else {
                        Action::Apply(vec![t.id.to_string()])
                    },
                ));
            }
        }

        out.truncate(9);
        out
    }

    /// Ctrl/⌘+K — one box that reaches every view, action and tweak.
    fn draw_palette(&mut self, ctx: &egui::Context) {
        if !self.palette {
            return;
        }
        let screen = ctx.screen_rect();
        let width = 620.0;
        let pos = Pos2::new(screen.center().x - width * 0.5, screen.top() + 140.0);

        // dim the deck behind the palette
        egui::Area::new(egui::Id::new("void.pal.dim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(false)
            .show(ctx, |ui| {
                let _ = ui.allocate_exact_size(screen.size(), Sense::hover());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(if theme::light() { 90 } else { 150 }));
            });

        let mut run: Option<Action> = None;
        let mut close = false;

        egui::Area::new(egui::Id::new("void.pal"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ctx, |ui| {
                ui.set_width(width);
                egui::Frame::none()
                    .fill(ink())
                    .stroke(Stroke::new(1.0_f32, line2()))
                    .rounding(14.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.set_width(width - 32.0);
                        ui.horizontal(|ui| {
                            theme::micro(ui, "command");
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.add(egui::Label::new(
                                    RichText::new("esc").font(mono_font(9.5)).color(ghost()),
                                ));
                            });
                        });
                        ui.add_space(2.0);
                        let edit = ui.add(
                            egui::TextEdit::singleline(&mut self.palette_q)
                                .hint_text("jump to a view, run an action, apply a tweak…")
                                .desired_width(f32::INFINITY)
                                .font(body_font())
                                .frame(false)
                                .text_color(txt()),
                        );
                        if self.frames < 400 || edit.gained_focus() {
                            edit.request_focus();
                        }
                        ui.add_space(4.0);
                        let (r, _) = ui.allocate_exact_size(
                            Vec2::new(ui.available_width(), 1.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(r, 0.0, line());
                        ui.add_space(6.0);

                        let hits = self.palette_hits();
                        if hits.is_empty() {
                            ui.add(egui::Label::new(
                                RichText::new("no match").font(body_font()).color(faint()),
                            ));
                        }
                        for (i, (label, sub, action)) in hits.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(
                                Vec2::new(ui.available_width(), 30.0),
                                Sense::click(),
                            );
                            if resp.hovered() {
                                ui.painter().rect_filled(rect, 7.0, a(white(), 10));
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            ui.painter().text(
                                Pos2::new(rect.left() + 10.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                theme::clip(label, 44),
                                label_font(),
                                if resp.hovered() { txt() } else { dim() },
                            );
                            ui.painter().text(
                                Pos2::new(rect.left() + 300.0, rect.center().y),
                                Align2::LEFT_CENTER,
                                theme::clip(sub, 44),
                                mono_font(9.5),
                                ghost(),
                            );
                            if resp.clicked() {
                                run = Some(action.clone());
                                close = true;
                            }
                            let _ = i;
                        }

                        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Some((_, _, action)) = hits.first() {
                                run = Some(action.clone());
                                close = true;
                            }
                        }
                        ui.add_space(6.0);
                        theme::micro(ui, "enter runs the first hit · ctrl+k closes");
                    });
            });

        if close {
            self.palette = false;
            self.palette_q.clear();
        }
        if let Some(action) = run {
            self.run_action(ctx, action);
        }
    }

    fn draw_modal(&mut self, ctx: &egui::Context) {
        if self.modal.is_none() {
            return;
        }
        let screen = ctx.screen_rect();
        let mut close = false;
        let mut confirm = false;
        let modal = match self.modal.as_ref() {
            Some(m) => m,
            None => return,
        };

        egui::Area::new(egui::Id::new("void.modal.dim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let _ = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(198));
            });

        egui::Area::new(egui::Id::new("void.modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(
                screen.center().x - 300.0,
                screen.center().y - 200.0,
            ))
            .show(ctx, |ui| {
                ui.set_width(600.0);
                egui::Frame::none()
                    .fill(ink())
                    .stroke(Stroke::new(1.0_f32, line2()))
                    .rounding(16.0)
                    .inner_margin(egui::Margin::same(26.0))
                    .show(ui, |ui| {
                        ui.set_width(548.0);
                        ui.add(egui::Label::new(
                            RichText::new(tracked(&modal.kicker))
                                .font(micro_font())
                                .color(ghost()),
                        ));
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(
                            RichText::new(&modal.title).font(num_font(24.0)).color(txt()),
                        ));
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(
                            RichText::new(&modal.body).font(body_font()).color(dim()),
                        ));
                        if !modal.plan.is_empty() {
                            ui.add_space(14.0);
                            egui::Frame::none()
                                .fill(bg())
                                .rounding(10.0)
                                .inner_margin(egui::Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(520.0);
                                    for line in modal.plan.iter() {
                                        ui.add(egui::Label::new(
                                            RichText::new(format!("· {}", line))
                                                .font(mono_font(11.0))
                                                .color(dim()),
                                        ));
                                    }
                                });
                        }
                        if modal.warn {
                            ui.add_space(12.0);
                            ui.add(egui::Label::new(
                                RichText::new(tracked("this can restart a service"))
                                    .font(micro_font())
                                    .color(faint()),
                            ));
                        }
                        ui.add_space(20.0);
                        ui.horizontal(|ui| {
                            if modal.info_only {
                                if theme::button(ui, &modal.ok, true).clicked() {
                                    close = true;
                                }
                            } else {
                                if theme::button(ui, &modal.ok, true).clicked() {
                                    confirm = true;
                                }
                                ui.add_space(8.0);
                                if theme::button(ui, &modal.cancel, false).clicked() {
                                    close = true;
                                }
                            }
                        });
                    });
            });

        if confirm {
            let action = match self.modal.take() {
                Some(m) => m.action,
                None => Action::None,
            };
            self.run_action(ctx, action);
        } else if close {
            self.modal = None;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.frames = self.frames.wrapping_add(1);
        // Audio is started on the second frame: the window is already on screen,
        // so a slow WASAPI device can never delay the first paint.
        if self.frames == 2 && !self.state.muted && self.state.volume > 0 {
            self.audio.play();
        }
        self.poll(ctx, now);
        self.pump_job(ctx);

        // ── keyboard ──
        let (mod_key, k_press, esc, enter, num) = ctx.input(|i| {
            (
                i.modifiers.command,
                i.key_pressed(egui::Key::K),
                i.key_pressed(egui::Key::Escape),
                i.key_pressed(egui::Key::Enter),
                (1..=9).find(|n| i.key_pressed(match n {
                    1 => egui::Key::Num1,
                    2 => egui::Key::Num2,
                    3 => egui::Key::Num3,
                    4 => egui::Key::Num4,
                    5 => egui::Key::Num5,
                    6 => egui::Key::Num6,
                    7 => egui::Key::Num7,
                    8 => egui::Key::Num8,
                    _ => egui::Key::Num9,
                })),
            )
        });
        if mod_key && k_press {
            self.palette = !self.palette;
            self.palette_q.clear();
        } else if esc {
            self.palette = false;
            self.modal = None;
        } else if mod_key && !self.palette {
            if let Some(n) = num {
                if let Some(v) = VIEWS.get(n as usize - 1) {
                    self.set_view(*v, now);
                }
            }
        }
        let _ = enter;

        // eased coverage ring — it slides to the new value, never jumps
        let (applied, _) = self.tally();
        let target = if engine::catalog::TWEAKS.is_empty() {
            0.0
        } else {
            applied as f32 / engine::catalog::TWEAKS.len() as f32 * 100.0
        };
        self.cov += (target - self.cov) * 0.10;
        let cpu = self.stats.cpu as f32;
        let ram = self.stats.ram as f32;
        self.cpu_e += (cpu - self.cpu_e) * 0.09;
        self.ram_e += (ram - self.ram_e) * 0.09;
        // ignite sweep while a job is running
        if self.job.is_some() {
            let age = self.started.elapsed().as_secs_f64() - self.job_born;
            self.sweep = (age as f32 / 0.9).min(1.0);
        } else if self.sweep < 1.0 {
            self.sweep = (self.sweep + 0.06).min(1.0);
        }

        self.presence_tick();
        self.guard_tick(ctx, now);
        self.checks_tick(now);

        // dust: slow specks behind everything, disabled with reduced motion
        if self.state.motion {
            let screen = ctx.screen_rect();
            egui::Area::new(egui::Id::new("void.dust"))
                .order(egui::Order::Background)
                .fixed_pos(screen.min)
                .interactable(false)
                .show(ctx, |ui| {
                    let p = ui.painter();
                    for i in 0..40 {
                        let seed = i as f32 * 12.9898;
                        let x = ((seed.sin() * 43758.5453).fract().abs() * screen.width()
                            + now as f32 * 5.0 * (0.4 + (i % 5) as f32 * 0.2))
                            % screen.width();
                        let y = ((seed.cos() * 17345.234).fract().abs() * screen.height()
                            + now as f32 * 7.0 * (0.3 + (i % 3) as f32 * 0.3))
                            % screen.height();
                        let fade = ((i as f32 * 0.37).sin() * 0.5 + 0.5) * 16.0 + 4.0;
                        p.circle_filled(
                            Pos2::new(screen.left() + x, screen.top() + y),
                            if i % 7 == 0 { 1.3 } else { 0.8 },
                            a(white(), fade as u8),
                        );
                    }
                });
        }

        egui::SidePanel::left("void.rail")
            .exact_width(236.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(theme::rail_bg())
                    .inner_margin(egui::Margin::symmetric(18.0, 18.0)),
            )
            .show(ctx, |ui| {
                self.rail(ui, now);
            });

        egui::TopBottomPanel::top("void.top")
            .exact_height(84.0)
            .frame(
                egui::Frame::none()
                    .fill(bg())
                    .inner_margin(egui::Margin::symmetric(30.0, 16.0)),
            )
            .show(ctx, |ui| {
                self.topbar(ui, now);
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(r, 0.0, line());
            });

        if self.busy() {
            egui::TopBottomPanel::top("void.job")
                .exact_height(41.0)
                .frame(
                    egui::Frame::none()
                        .fill(theme::ink2())
                        .inner_margin(egui::Margin::symmetric(30.0, 10.0)),
                )
                .show(ctx, |ui| {
                    self.job_strip(ui);
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(bg())
                    .inner_margin(egui::Margin::symmetric(30.0, 20.0)),
            )
            .show(ctx, |ui| {
                // wipe on view change: a hairline that sweeps across the top
                let age = (now - self.view_at) as f32;
                if age < 0.45 {
                    let e = (age / 0.45).clamp(0.0, 1.0);
                    let w = ui.available_width();
                    let (r, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
                    ui.painter().rect_filled(
                        Rect::from_min_size(r.min, Vec2::new(w * e, 1.0)),
                        0.0,
                        a(white(), ((1.0 - e) * 150.0) as u8),
                    );
                } else {
                    ui.add_space(1.0);
                }

                ui.add(egui::Label::new(
                    RichText::new(self.view.hint()).font(body_font()).color(faint()),
                ));
                ui.add_space(18.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        crate::views::dispatch(self, ui);
                        ui.add_space(48.0);
                    });
            });

        self.draw_toasts(ctx, now);
        self.draw_modal(ctx);
        self.draw_palette(ctx);
    }
}
