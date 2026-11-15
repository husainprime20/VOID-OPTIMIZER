// VOID — application shell.
//
// Everything the UI needs lives in this process: the engine runs in-process, so
// there is no localhost socket, no fetch(), and therefore no way for the
// "engine offline" state to come back.

use std::time::Duration;

use eframe::egui::{self, Align2, Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::audio::Ambient;
use crate::engine::{self, Machine, State, Stats};
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
            View::Optimize => "42 tweaks",
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
            View::Dashboard => "Everything worth knowing at a glance. Ignite applies the chosen profile in one motion.",
            View::Optimize => "The full catalog. Filter by lane, audit the plan before it touches the machine.",
            View::Profiles => "Curated loadouts for the games and hardware people actually play on.",
            View::FreeFire => "Detect your emulator, pin it high, and give the phone-client the frame budget it wants.",
            View::Network => "DNS, Nagle, throttling index and queue shape — the difference between 40 ms and 20 ms.",
            View::Kernel => "Where Windows stops being polite: timer resolution, core parking, syscall shaping.",
            View::Monitor => "One-second telemetry, sleep-jitter benchmark, and a live process list you can boost.",
            View::Aim => "Convert sens between games, lock a DPI, and run the warm-up ladder before queueing.",
            View::Settings => "Sound, motion, safety rails and the exact machine report the engine sees.",
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
    pub emulators: Vec<engine::Emulator>,
    pub procs: Vec<(String, u32, f64)>,
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
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::install(&cc.egui_ctx);
        let state = State::new();
        let machine = Machine::detect();
        let mut audio = Ambient::new(0, state.volume as f32 / 100.0);
        if !state.muted && state.volume > 0 {
            audio.play();
        }
        let stats = Stats::default();
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
            emulators: Vec::new(),
            procs: Vec::new(),
            audio,
            modal: None,
            toasts: Vec::new(),
            started: std::time::Instant::now(),
            clock: crate::platform::clock_label(),
            polled_at: -10.0,
            procs_at: -10.0,
            dpi: 800.0,
            sens: 90.0,
            aim_locked: false,
            selftest_report: None,
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

    pub fn toggle_tweak(&mut self, id: &'static str) {
        let on = self.state.applied.contains(id);
        let ids = vec![id.to_string()];
        let out = if on {
            engine::revert_ids(&mut self.state, &ids)
        } else {
            engine::apply_ids(&mut self.state, &ids)
        };
        let name = engine::find_tweak(id).map(|t| t.name).unwrap_or(id);
        match out.first() {
            Some(o) if o.ok => {
                if on {
                    self.toast(1, format!("{} reverted", name))
                } else {
                    self.toast(0, format!("{} applied", name))
                }
            }
            Some(o) => self.toast(2, format!("{} · {}", name, o.msg)),
            None => {}
        }
    }

    pub fn apply_set(&mut self, ids: Vec<String>, label: &str) {
        let sim = self.state.sim();
        let out = engine::apply_ids(&mut self.state, &ids);
        let ok = out.iter().filter(|o| o.ok).count();
        let bad = out.len().saturating_sub(ok);
        let suffix = if sim { " (simulated)" } else { "" };
        if bad == 0 {
            self.toast(0, format!("{label} · {ok} steps{suffix}"));
        } else {
            self.toast(2, format!("{label} · {ok} ok, {bad} failed"));
        }
    }

    pub fn revert_set(&mut self, ids: Vec<String>, label: &str) {
        let out = engine::revert_ids(&mut self.state, &ids);
        let ok = out.iter().filter(|o| o.ok).count();
        self.toast(1, format!("{label} reverted · {ok} steps"));
    }

    pub fn run_action(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Apply(ids) => self.apply_set(ids, "apply"),
            Action::Revert(ids) => self.revert_set(ids, "revert"),
            Action::RevertAll => {
                let out = engine::revert_all(&mut self.state);
                self.toast(1, format!("restored · {} steps", out.len()));
            }
            Action::Profile(pid) => {
                let name = engine::find_profile(pid).map(|p| p.name).unwrap_or(pid);
                if self.state.auto_backup {
                    let o = engine::restore_point(&mut self.state);
                    self.toast(
                        if o.ok { 1 } else { 2 },
                        format!("restore point · {}", o.msg),
                    );
                }
                let out = engine::apply_profile(&mut self.state, pid);
                let ok = out.iter().filter(|o| o.ok).count();
                self.toast(0, format!("{name} engaged · {ok} steps"));
                self.view = View::Dashboard;
            }
            Action::RestorePoint => {
                let o = engine::restore_point(&mut self.state);
                self.toast(if o.ok { 0 } else { 2 }, o.msg);
            }
            Action::FlushDns => {
                let ids = vec!["net.dns".to_string()];
                let _ = engine::apply_ids(&mut self.state, &ids);
                self.toast(0, "DNS cache flushed");
            }
            Action::CleanTemp => {
                let ids = vec!["disk.temp".to_string()];
                let _ = engine::apply_ids(&mut self.state, &ids);
                self.toast(0, "temp files cleared");
            }
            Action::LaunchEmulator(path) => match engine::launch_emulator(path) {
                Ok(m) => self.toast(0, m),
                Err(e) => self.toast(2, e),
            },
            Action::BoostPid(pid) => match engine::boost_pid(pid) {
                Ok(m) => self.toast(0, format!("priority raised · {}", m)),
                Err(e) => self.toast(2, format!("boost failed · {}", e)),
            },
            Action::Bench => {
                let r = engine::bench();
                self.state.push("sys", &format!("bench :: {:.0}", r.score));
                self.toast(0, format!("bench complete · {:.0}", r.score));
                self.bench = Some(r);
            }
        }
    }

    fn poll(&mut self, ctx: &egui::Context, now: f64) {
        let live = matches!(self.view, View::Dashboard | View::Monitor);
        if live && now - self.polled_at > 2.0 {
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
        if self.view == View::Monitor && now - self.procs_at > 6.0 {
            self.procs_at = now;
            self.procs = engine::processes();
        }
        if self.view == View::FreeFire && self.emulators.is_empty() {
            self.emulators = engine::emulators();
        }
        let tick = if self.state.motion { 40 } else { 900 };
        ctx.request_repaint_after(Duration::from_millis(tick));
        self.clock = crate::platform::clock_label();
        self.toasts
            .retain(|t| now - t.born < 4.6 && t.born <= now + 0.1);
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.modal.is_some() {
            self.modal = None;
        }
    }

    // ── chrome ──────────────────────────────────────────────────────────────
    fn rail(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 46.0), Sense::hover());
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y),
            Align2::LEFT_CENTER,
            "VOID",
            num_font(27.0),
            TXT,
        );
        // a small pulsing dot next to the wordmark
        let t = self.started.elapsed().as_secs_f32();
        let pulse = if self.state.motion {
            (t * 1.7).sin() * 0.5 + 0.5
        } else {
            1.0
        };
        ui.painter().circle_filled(
            Pos2::new(rect.right() - 4.0, rect.center().y),
            3.0,
            a(WHITE, (90.0 + 140.0 * pulse) as u8),
        );

        ui.add_space(2.0);
        ui.add(egui::Label::new(
            RichText::new(tracked("competitive optimizer"))
                .font(micro_font())
                .color(GHOST),
        ));
        ui.add_space(22.0);

        for (i, v) in VIEWS.iter().enumerate() {
            if self.nav_item(ui, *v, i + 1) {
                self.view = *v;
                self.expanded = None;
            }
            ui.add_space(2.0);
        }

        // bottom: sound + engine status
        let bottom = ui.available_height() - 96.0;
        if bottom > 0.0 {
            ui.add_space(bottom);
        }
        theme::hairline(ui);
        ui.add_space(10.0);

        let playing = self.audio.playing;
        if theme::chip_button(ui, if playing { "ambient on" } else { "ambient off" }, playing)
            .clicked()
        {
            self.audio.toggle();
            self.state.muted = !self.audio.playing;
            self.state.save();
        }
        ui.add_space(8.0);

        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 30.0), Sense::hover());
        let lvl = self.audio.level();
        let bars = 16;
        let bw = 3.0;
        for b in 0..bars {
            let bh = 4.0 + 16.0 * (((b as f32 * 0.7).sin() * 0.5 + 0.5) * lvl);
            let x = rect.left() + b as f32 * (bw + 3.0);
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(x, rect.bottom() - bh),
                    Pos2::new(x + bw, rect.bottom()),
                ),
                1.0,
                a(WHITE, if lvl > 0.01 { 120 } else { 22 }),
            );
        }
        ui.painter().text(
            Pos2::new(rect.right(), rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{}%", (self.audio.volume * 100.0).round() as i32),
            mono_font(10.5),
            FAINT,
        );
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::new(6.0, 6.0), Sense::hover());
            ui.painter().circle_filled(r.center(), 3.0, WHITE);
            ui.add(egui::Label::new(
                RichText::new(tracked("engine inline"))
                    .font(micro_font())
                    .color(FAINT),
            ));
        });
    }

    fn nav_item(&mut self, ui: &mut Ui, v: View, index: usize) -> bool {
        let h = 34.0;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
        let active = self.view == v;
        let t = ui.ctx().animate_bool(resp.id, active || resp.hovered());

        let bg = if active {
            a(WHITE, 10)
        } else {
            a(WHITE, (t * 6.0) as u8)
        };
        ui.painter().rect_filled(rect, 8.0, bg);
        if active {
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.left(), rect.top() + 9.0),
                    Pos2::new(rect.left() + 2.0, rect.bottom() - 9.0),
                ),
                1.0,
                WHITE,
            );
        }
        let fg = if active {
            TXT
        } else if resp.hovered() {
            DIM
        } else {
            FAINT
        };
        ui.painter().text(
            Pos2::new(rect.left() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            v.title(),
            label_font(),
            fg,
        );
        ui.painter().text(
            Pos2::new(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            format!("{:02}", index),
            mono_font(9.5),
            if active { DIM } else { GHOST },
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp.clicked()
    }

    fn topbar(&mut self, ui: &mut Ui) {
        let h = 30.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y - 8.0),
            Align2::LEFT_CENTER,
            tracked(self.view.kicker()),
            micro_font(),
            GHOST,
        );
        ui.painter().text(
            Pos2::new(rect.left(), rect.center().y + 8.0),
            Align2::LEFT_CENTER,
            self.view.title(),
            num_font(17.0),
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
        right -= 74.0;

        // admin state
        let admin_txt = if self.machine.admin { "admin" } else { "limited" };
        let galley =
            ui.painter()
                .layout_no_wrap(tracked(admin_txt), micro_font(), TXT);
        let w = galley.size().x + 18.0;
        let box_rect = Rect::from_center_size(
            Pos2::new(right - w / 2.0, rect.center().y),
            Vec2::new(w, 20.0),
        );
        let fill = if self.machine.admin {
            a(WHITE, 235)
        } else {
            a(WHITE, 8)
        };
        ui.painter().rect_filled(box_rect, 10.0, fill);
        if !self.machine.admin {
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
        right -= w + 8.0;

        if self.state.dry_run {
            let g = ui
                .painter()
                .layout_no_wrap(tracked("dry run"), micro_font(), TXT);
            let w = g.size().x + 18.0;
            let br = Rect::from_center_size(
                Pos2::new(right - w / 2.0, rect.center().y),
                Vec2::new(w, 20.0),
            );
            ui.painter().rect_filled(br, 10.0, a(WHITE, 8));
            ui.painter()
                .rect_stroke(br, 10.0, Stroke::new(1.0_f32, a(WHITE, 60)));
            ui.painter().text(
                br.center(),
                Align2::CENTER_CENTER,
                tracked("dry run"),
                micro_font(),
                TXT,
            );
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
            .fixed_pos(Pos2::new(screen.right() - 380.0, screen.bottom() - 120.0))
            .interactable(false)
            .show(ctx, |ui| {
                ui.set_width(356.0);
                for t in self.toasts.iter().rev() {
                    let age = now - t.born;
                    let appear = ((age / 0.18).min(1.0)) as f32;
                    let fade = if age > 3.8 {
                        (1.0 - (age - 3.8) / 0.8).max(0.0) as f32
                    } else {
                        1.0
                    };
                    let alpha = (appear * fade * 255.0) as u8;
                    if alpha == 0 {
                        continue;
                    }
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(356.0, 38.0), Sense::hover());
                    let fill = Color32::from_rgba_unmultiplied(14, 14, 17, (alpha as f32 * 0.96) as u8);
                    ui.painter().rect_filled(rect, 10.0, fill);
                    ui.painter().rect_stroke(
                        rect,
                        10.0,
                        Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(70, 70, 78, alpha)),
                    );
                    let dot = match t.kind {
                        0 => Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                        1 => Color32::from_rgba_unmultiplied(170, 170, 178, alpha),
                        _ => Color32::from_rgba_unmultiplied(120, 120, 128, alpha),
                    };
                    let label = match t.kind {
                        0 => "OK",
                        1 => "·",
                        _ => "FAIL",
                    };
                    ui.painter().circle_filled(
                        Pos2::new(rect.left() + 16.0, rect.center().y),
                        3.0,
                        dot,
                    );
                    ui.painter().text(
                        Pos2::new(rect.left() + 30.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{}  {}", label, t.msg),
                        label_font(),
                        Color32::from_rgba_unmultiplied(235, 235, 238, alpha),
                    );
                    ui.add_space(6.0);
                }
            });
    }

    fn draw_modal(&mut self, ctx: &egui::Context) {
        let Some(modal) = self.modal.as_ref() else {
            return;
        };
        let screen = ctx.screen_rect();
        let mut close = false;
        let mut confirm = false;

        egui::Area::new(egui::Id::new("void.modal.dim"))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let _ = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(190));
            });

        egui::Area::new(egui::Id::new("void.modal"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::new(screen.center().x - 290.0, screen.center().y - 210.0))
            .show(ctx, |ui| {
                ui.set_width(580.0);
                egui::Frame::none()
                    .fill(INK)
                    .stroke(Stroke::new(1.0_f32, LINE2))
                    .rounding(20.0)
                    .inner_margin(egui::Margin::same(26.0))
                    .show(ui, |ui| {
                        ui.set_width(528.0);
                        theme::micro(ui, &modal.kicker);
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(
                            RichText::new(&modal.title).font(num_font(23.0)).color(TXT),
                        ));
                        ui.add_space(10.0);
                        ui.add(egui::Label::new(
                            RichText::new(&modal.body).font(body_font()).color(DIM),
                        ));
                        if !modal.plan.is_empty() {
                            ui.add_space(14.0);
                            egui::Frame::none()
                                .fill(VOID)
                                .rounding(12.0)
                                .inner_margin(egui::Margin::same(14.0))
                                .show(ui, |ui| {
                                    ui.set_width(500.0);
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
            self.run_action(action);
        } else if close {
            self.modal = None;
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = ctx.input(|i| i.time);
        self.poll(ctx, now);

        // dust: slow specks behind everything, disabled with reduced motion
        if self.state.motion {
            let screen = ctx.screen_rect();
            egui::Area::new(egui::Id::new("void.dust"))
                .order(egui::Order::Background)
                .fixed_pos(screen.min)
                .interactable(false)
                .show(ctx, |ui| {
                    let p = ui.painter();
                    for i in 0..44 {
                        let seed = i as f32 * 12.9898;
                        let x = ((seed.sin() * 43758.5453).fract().abs() * screen.width()
                            + now as f32 * 6.0 * (0.4 + (i % 5) as f32 * 0.2))
                            % screen.width();
                        let y = ((seed.cos() * 17345.234).fract().abs() * screen.height()
                            + now as f32 * 9.0 * (0.3 + (i % 3) as f32 * 0.3))
                            % screen.height();
                        let fade = ((i as f32 * 0.37).sin() * 0.5 + 0.5) * 18.0 + 5.0;
                        p.circle_filled(
                            Pos2::new(screen.left() + x, screen.top() + y),
                            if i % 7 == 0 { 1.4 } else { 0.9 },
                            a(WHITE, fade as u8),
                        );
                    }
                });
        }

        egui::SidePanel::left("void.rail")
            .exact_width(212.0)
            .resizable(false)
            .frame(
                egui::Frame::none()
                    .fill(Color32::from_rgb(7, 7, 9))
                    .inner_margin(egui::Margin::symmetric(16.0, 16.0)),
            )
            .show(ctx, |ui| {
                self.rail(ui);
            });

        egui::TopBottomPanel::top("void.top")
            .exact_height(64.0)
            .frame(
                egui::Frame::none()
                    .fill(VOID)
                    .inner_margin(egui::Margin::symmetric(26.0, 16.0)),
            )
            .show(ctx, |ui| {
                self.topbar(ui);
                let (r, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), 1.0),
                    Sense::hover(),
                );
                ui.painter().rect_filled(r, 0.0, LINE);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::none()
                    .fill(VOID)
                    .inner_margin(egui::Margin::symmetric(26.0, 18.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(self.view.hint()).font(body_font()).color(FAINT),
                    ));
                });
                ui.add_space(16.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        crate::views::dispatch(self, ui);
                        ui.add_space(40.0);
                    });
            });

        self.draw_toasts(ctx, now);
        self.draw_modal(ctx);
    }
}
