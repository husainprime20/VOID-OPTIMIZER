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

use eframe::egui::{self, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

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

pub enum Action {
    None,
    Apply(Vec<String>),
    Revert(Vec<String>),
    RevertAll,
    Profile(&'static str),
    RestorePoint,
    FlushDns,
    CleanTemp,
    LaunchEmulator(&'static str),
    BoostPid(u32),
    Bench,
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

    /// The one worker job allowed at a time.
    pub job: Option<Job>,
    /// Last finished job summary, shown in the rail.
    pub last_job: String,
    /// Live read-back of every registry tweak on this machine.
    pub live: HashMap<&'static str, bool>,
    pub live_at: f64,
    /// Set when the view changed, drives the wipe.
    pub view_at: f64,
    /// Audio starts on frame two so the window is never waiting on WASAPI.
    pub frames: u32,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let state = State::new();
        let machine = Machine::detect();
        let audio = Ambient::new(0, state.volume as f32 / 100.0);
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
            job: None,
            last_job: String::new(),
            live: HashMap::new(),
            live_at: -10.0,
            view_at: -10.0,
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
            Action::LaunchEmulator(path) => match engine::launch_emulator(path) {
                Ok(m) => self.toast(0, m),
                Err(e) => self.toast(2, e),
            },
            Action::BoostPid(pid) => match engine::boost_pid(pid) {
                Ok(m) => self.toast(0, format!("priority raised · {}", m)),
                Err(e) => self.toast(2, format!("boost failed · {}", e)),
            },
            Action::Bench => self.start(ctx, Work::Bench),
        }
    }

    /// Drain the worker and fold the result back into the shell.
    fn pump_job(&mut self) {
        let mut bench_result: Option<engine::BenchResult> = None;
        let mut finished: Option<(String, Vec<engine::Outcome>, State)> = None;
        if let Some(job) = self.job.as_mut() {
            if let Some(b) = job.pump() {
                bench_result = Some(b);
            }
            finished = job.take_finished();
        }
        if let Some(b) = bench_result {
            if let Some(prev) = self.bench {
                self.bench_prev = Some(prev);
            }
            self.bench = Some(b);
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
        }
    }

    fn poll(&mut self, ctx: &egui::Context, now: f64) {
        let live = matches!(self.view, View::Dashboard | View::Monitor);
        if live && now - self.polled_at > 1.0 {
            self.polled_at = now;
            self.stats = engine::sample();
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
            TXT,
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
            a(WHITE, (70.0 + 150.0 * pulse) as u8),
        );
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y + 9.0),
            Align2::LEFT_CENTER,
            engine::VERSION,
            mono_font(9.5),
            GHOST,
        );
        ui.painter().text(
            Pos2::new(rect.right() - 3.0, rect.center().y + 9.0),
            Align2::RIGHT_CENTER,
            "native",
            mono_font(9.5),
            GHOST,
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
                        .color(if playing { TXT } else { FAINT }),
                ));
                ui.add(
                    egui::Label::new(
                        RichText::new(crate::audio::PRESETS[self.audio.preset].0)
                            .font(mono_font(10.5))
                            .color(GHOST),
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
        let bars = 22;
        let step = mrect.width() / bars as f32;
        for b in 0..bars {
            let energy = (((b as f32 * 0.7).sin() * 0.5 + 0.5) * lvl).max(0.03);
            let bh = 2.0 + 15.0 * energy;
            let x = mrect.left() + b as f32 * step;
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(x, mrect.bottom() - bh),
                    Pos2::new(x + 3.0, mrect.bottom()),
                ),
                1.0,
                a(WHITE, if lvl > 0.01 { 165 } else { 30 }),
            );
        }

        ui.add_space(12.0);
        let (srect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
        let dot = if self.busy() { DIM } else { WHITE };
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
            FAINT,
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
            ui.painter().rect_filled(rect, 7.0, a(WHITE, 9));
            let bar = Rect::from_min_max(
                Pos2::new(rect.left(), rect.top() + 8.0),
                Pos2::new(rect.left() + 1.5, rect.bottom() - 8.0),
            );
            ui.painter().rect_filled(bar, 1.0, WHITE);
        } else if t > 0.01 {
            ui.painter().rect_filled(rect, 7.0, a(WHITE, (t * 5.0) as u8));
        }

        let fg = if active {
            TXT
        } else if resp.hovered() {
            DIM
        } else {
            FAINT
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
            if active { DIM } else { GHOST },
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
            GHOST,
        );
        ui.painter().text(
            Pos2::new(rect.left(), rect.top() + 22.0 + slide * 1.6),
            Align2::LEFT_TOP,
            self.view.title(),
            num_font(25.0),
            TXT,
        );

        let mut right = rect.right();

        // clock
        ui.painter().text(
            Pos2::new(right, rect.center().y),
            Align2::RIGHT_CENTER,
            &self.clock,
            mono_font(11.0),
            FAINT,
        );
        right -= 78.0;

        // admin chip
        let admin_txt = if self.machine.admin { "admin" } else { "limited" };
        let galley = ui
            .painter()
            .layout_no_wrap(tracked(admin_txt), micro_font(), TXT);
        let w = galley.size().x + 18.0;
        let box_rect = Rect::from_center_size(
            Pos2::new(right - w / 2.0, rect.center().y),
            Vec2::new(w, 20.0),
        );
        if self.machine.admin {
            ui.painter().rect_filled(box_rect, 10.0, a(WHITE, 232));
        } else {
            ui.painter().rect_filled(box_rect, 10.0, a(WHITE, 8));
            ui.painter()
                .rect_stroke(box_rect, 10.0, Stroke::new(1.0_f32, LINE2));
        }
        ui.painter().text(
            box_rect.center(),
            Align2::CENTER_CENTER,
            tracked(admin_txt),
            micro_font(),
            if self.machine.admin { VOID } else { DIM },
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
                GHOST,
            );
            ui.painter().text(
                Pos2::new(right, rect.top() + 15.0),
                Align2::RIGHT_TOP,
                value.clone(),
                num_font(21.0),
                TXT,
            );
            right -= 66.0;
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
            .circle_filled(Pos2::new(rect.left(), rect.center().y), 3.0, a(WHITE, 120));
        ui.painter().circle_stroke(
            Pos2::new(rect.left(), rect.center().y),
            6.0 + pulse * 2.0,
            Stroke::new(1.0_f32, a(WHITE, (70.0 * (1.0 - pulse)) as u8)),
        );
        ui.painter().text(
            Pos2::new(rect.left() + 18.0, rect.center().y),
            Align2::LEFT_CENTER,
            job.label.clone(),
            label_font(),
            TXT,
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
            FAINT,
        );
        let cur = theme::clip(&job.current, 60);
        ui.painter().text(
            Pos2::new(rect.center().x - 20.0, rect.center().y),
            Align2::CENTER_CENTER,
            cur,
            mono_font(10.5),
            GHOST,
        );
        // progress hairline
        let track = Rect::from_min_max(
            Pos2::new(rect.left(), rect.bottom() - 1.0),
            Pos2::new(rect.right(), rect.bottom()),
        );
        ui.painter().rect_filled(track, 0.0, a(WHITE, 14));
        let fill = Rect::from_min_max(
            track.left_top(),
            Pos2::new(track.left() + track.width() * p.max(0.02), track.bottom()),
        );
        ui.painter().rect_filled(fill, 0.0, a(WHITE, 200));
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
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(376.0, 40.0), Sense::hover());
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
                    .fill(INK)
                    .stroke(Stroke::new(1.0_f32, LINE2))
                    .rounding(16.0)
                    .inner_margin(egui::Margin::same(26.0))
                    .show(ui, |ui| {
                        ui.set_width(548.0);
                        ui.add(egui::Label::new(
                            RichText::new(tracked(&modal.kicker))
                                .font(micro_font())
                                .color(GHOST),
                        ));
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(
                            RichText::new(&modal.title).font(num_font(24.0)).color(TXT),
                        ));
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(
                            RichText::new(&modal.body).font(body_font()).color(DIM),
                        ));
                        if !modal.plan.is_empty() {
                            ui.add_space(14.0);
                            egui::Frame::none()
                                .fill(VOID)
                                .rounding(10.0)
                                .inner_margin(egui::Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(520.0);
                                    for line in modal.plan.iter() {
                                        ui.add(egui::Label::new(
                                            RichText::new(format!("· {}", line))
                                                .font(mono_font(11.0))
                                                .color(DIM),
                                        ));
                                    }
                                });
                        }
                        if modal.warn {
                            ui.add_space(12.0);
                            ui.add(egui::Label::new(
                                RichText::new(tracked("this can restart a service"))
                                    .font(micro_font())
                                    .color(FAINT),
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
        self.pump_job();

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
                            a(WHITE, fade as u8),
                        );
                    }
                });
        }

        egui::SidePanel::left("void.rail")
            .exact_width(236.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(6, 6, 8))
                    .inner_margin(egui::Margin::symmetric(18.0, 18.0)),
            )
            .show(ctx, |ui| {
                self.rail(ui, now);
            });

        egui::TopBottomPanel::top("void.top")
            .exact_height(84.0)
            .frame(
                egui::Frame::none()
                    .fill(VOID)
                    .inner_margin(egui::Margin::symmetric(30.0, 16.0)),
            )
            .show(ctx, |ui| {
                self.topbar(ui, now);
                let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
                ui.painter().rect_filled(r, 0.0, LINE);
            });

        if self.busy() {
            egui::TopBottomPanel::top("void.job")
                .exact_height(41.0)
                .frame(
                    egui::Frame::none()
                        .fill(Color32::from_rgb(8, 8, 10))
                        .inner_margin(egui::Margin::symmetric(30.0, 10.0)),
                )
                .show(ctx, |ui| {
                    self.job_strip(ui);
                });
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(VOID)
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
                        a(WHITE, ((1.0 - e) * 150.0) as u8),
                    );
                } else {
                    ui.add_space(1.0);
                }

                ui.add(egui::Label::new(
                    RichText::new(self.view.hint()).font(body_font()).color(FAINT),
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
    }
}
