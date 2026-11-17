// VOID — views.

pub mod catalog_views;
pub mod live;

use eframe::egui::{self, Align, Layout, Pos2, Rect, RichText, Sense, Stroke, Ui, Vec2};

use crate::app::{Action, App, Modal, View};
use crate::engine::{self, catalog};
use crate::theme::{self, *};

pub fn dispatch(app: &mut App, ui: &mut Ui) {
    match app.view {
        View::Dashboard => dashboard(app, ui),
        View::Optimize => catalog_views::optimize(app, ui),
        View::Profiles => catalog_views::profiles(app, ui),
        View::FreeFire => live::freefire(app, ui),
        View::Network => catalog_views::network(app, ui),
        View::Kernel => catalog_views::kernel(app, ui),
        View::Monitor => live::monitor(app, ui),
        View::Aim => live::aim(app, ui),
        View::Settings => settings(app, ui),
    }
}

// ── shared pieces ────────────────────────────────────────────────────────────

pub fn risk_pill(ui: &mut Ui, risk: u8) {
    let (text, lit) = match risk {
        0 => ("safe", false),
        1 => ("moderate", false),
        _ => ("advanced", false),
    };
    theme::pill(ui, text, lit);
}

pub fn section<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    theme::micro(ui, label);
    ui.add_space(8.0);
    let r = add(ui);
    ui.add_space(16.0);
    r
}

pub fn count_badge(ui: &mut Ui, n: usize, total: usize) {
    theme::pill(ui, &format!("{}/{}", n, total), false);
}

/// One tweak as a card. Returns nothing; mutates app state through actions.
pub fn tweak_row(app: &mut App, ui: &mut Ui, t: &'static catalog::Tweak) {
    let on = app.state.applied.contains(t.id);
    let open = app.expanded == Some(t.id);
    let mut flip = false;
    let mut plan_toggle = false;

    let frame = egui::Frame::none()
        .fill(if on { ink2() } else { ink() })
        .stroke(Stroke::new(1.0_f32, if on { a(white(), 55) } else { line() }))
        .rounding(14.0)
        .inner_margin(egui::Margin::symmetric(16.0, 13.0));

    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let mut v = on;
                if theme::toggle(ui, &mut v).changed() {
                    flip = true;
                }
                ui.add_space(10.0);
                if theme::chip_button(ui, if open { "hide" } else { "details" }, open).clicked() {
                    plan_toggle = true;
                }
                ui.add_space(6.0);
                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(t.name).font(label_font()).color(txt()),
                        ));
                        ui.add_space(8.0);
                        risk_pill(ui, t.risk);
                        if t.once {
                            theme::pill(ui, "one-shot", false);
                        }
                        if t.reboot {
                            theme::pill(ui, "reboot", false);
                        }
                        match app.live_state(t.id) {
                            Some(true) => theme::pill(ui, "live", true),
                            Some(false) if on => theme::pill(ui, "drift", false),
                            _ => {}
                        }
                    });
                    ui.add_space(4.0);
                    ui.add(egui::Label::new(
                        RichText::new(t.desc).font(body_font()).color(faint()),
                    ));
                });
            });
        });

        if open {
            ui.add_space(12.0);
            ui.painter().rect_filled(
                Rect::from_min_max(
                    Pos2::new(ui.min_rect().left(), ui.cursor().top()),
                    Pos2::new(ui.min_rect().right(), ui.cursor().top() + 1.0),
                ),
                0.0,
                line(),
            );
            ui.add_space(10.0);
            ui.add(egui::Label::new(
                RichText::new(t.why).font(body_font()).color(dim()),
            ));
            ui.add_space(8.0);
            for a in t.apply.iter() {
                ui.add(egui::Label::new(
                    RichText::new(format!("-> {}", catalog::action_text(a)))
                        .font(mono_font(10.5))
                        .color(faint()),
                ));
            }
            for a in t.revert.iter() {
                ui.add(egui::Label::new(
                    RichText::new(format!("<- {}", catalog::action_text(a)))
                        .font(mono_font(10.5))
                        .color(ghost()),
                ));
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if theme::button(ui, if on { "revert now" } else { "apply now" }, false).clicked() {
                    flip = true;
                }
                ui.add_space(8.0);
                theme::micro(ui, &format!("id {}", t.id));
            });
        }
    });

    if plan_toggle {
        app.expanded = if open { None } else { Some(t.id) };
    }
    if flip {
        app.toggle_tweak(ui.ctx(), t.id);
    }
    ui.add_space(6.0);
}

pub fn tweak_stack(app: &mut App, ui: &mut Ui, ids: &[&'static str]) {
    for id in ids {
        if let Some(t) = engine::find_tweak(id) {
            tweak_row(app, ui, t);
        }
    }
}

pub fn by_category(app: &mut App, ui: &mut Ui, cat: &'static str) {
    let ids: Vec<&'static str> = catalog::TWEAKS
        .iter()
        .filter(|t| t.cat == cat)
        .map(|t| t.id)
        .collect();
    tweak_stack(app, ui, &ids);
}

fn stat_card(ui: &mut Ui, label: &str, value: String, unit: &str, hist: &[f32], max: f32) {
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(98.0);
        theme::micro(ui, label);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(
                RichText::new(value).font(num_font(28.0)).color(txt()),
            ));
            if !unit.is_empty() {
                ui.add(egui::Label::new(
                    RichText::new(unit).font(label_font()).color(faint()),
                ));
            }
        });
        ui.add_space(6.0);
        theme::spark(ui, hist, Vec2::new(ui.available_width(), 34.0), max);
    });
}

// ── dashboard ────────────────────────────────────────────────────────────────

pub fn dashboard(app: &mut App, ui: &mut Ui) {
    let total = catalog::TWEAKS.len();
    let applied = app.state.applied.len();
    // eased, so the ring slides instead of snapping
    let coverage = app.cov;

    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 200.0).max(320.0));
                theme::micro(ui, "catalog coverage");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:.0}", coverage)).font(num_font(64.0)).color(txt()),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("%").font(num_font(24.0)).color(faint()),
                    ));
                });
                ui.add_space(2.0);
                ui.add(egui::Label::new(
                    RichText::new(format!(
                        "{} applied · {} verified on this machine · {} profiles",
                        applied,
                        app.tally().1,
                        catalog::PROFILES.len()
                    ))
                    .font(body_font())
                    .color(faint()),
                ));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if theme::button(ui, "match mode", true).clicked() {
                        app.run_action(ui.ctx(), Action::MatchMode);
                    }
                    ui.add_space(8.0);
                    if theme::chip_button(ui, "esports set", false).clicked() {
                        let m = confirm_profile(app, "esports");
                        app.modal = Some(m);
                    }
                    ui.add_space(8.0);
                    if theme::chip_button(ui, "ignite", false).clicked() {
                        let m = confirm_profile(app, "competitive");
                        app.modal = Some(m);
                    }
                    ui.add_space(8.0);
                    if theme::button(ui, "revert everything", false).clicked() {
                        app.modal = Some(Modal {
                            kicker: "safety".into(),
                            title: "Revert every applied change?".into(),
                            body: "Every entry in the applied stack is restored to its pre-VOID value."
                                .into(),
                            plan: vec![
                                "registry values are deleted or reset to defaults".into(),
                                "services are re-enabled and started".into(),
                                "power plan returns to Balanced".into(),
                            ],
                            ok: "Revert all".into(),
                            cancel: "Keep changes".into(),
                            action: Action::RevertAll,
                            info_only: false,
                            warn: false,
                        });
                    }
                });

                ui.add_space(18.0);
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    let machine = [
                        ("host", app.machine.host.clone()),
                        ("cpu", app.machine.cpu.clone()),
                        ("gpu", app.machine.gpu.clone()),
                    ];
                    for (k, v) in machine {
                        ui.vertical(|ui| {
                            theme::micro(ui, k);
                            ui.add_space(2.0);
                            ui.add(egui::Label::new(
                                RichText::new(theme::clip(&v, 34))
                                    .font(mono_font(10.5))
                                    .color(dim()),
                            ));
                        });
                        ui.add_space(30.0);
                    }
                });
            });
            ui.add_space(12.0);
            theme::ring_sweep(ui, coverage, 168.0, "readiness", app.sweep);
        });
    });

    ui.add_space(14.0);

    let cols = 4;
    let w = ui.available_width();
    let gap = 10.0;
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.allocate_ui_with_layout(
            Vec2::new(cw, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(cw);
                stat_card(
                    ui,
                    "cpu load",
                    format!("{:.0}", app.cpu_e),
                    "%",
                    &app.cpu_hist,
                    100.0,
                );
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(cw, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(cw);
                stat_card(
                    ui,
                    "memory",
                    format!("{:.0}", app.stats.ram),
                    "%",
                    &app.ram_hist,
                    100.0,
                );
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(cw, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(cw);
                theme::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(98.0);
                    theme::micro(ui, "sleep jitter");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        let (v, u) = match &app.bench {
                            Some(b) => (format!("{:.2}", b.jitter), "ms"),
                            None => ("—".to_string(), ""),
                        };
                        ui.add(egui::Label::new(
                            RichText::new(v).font(num_font(28.0)).color(txt()),
                        ));
                        ui.add(egui::Label::new(
                            RichText::new(u).font(label_font()).color(faint()),
                        ));
                    });
                    ui.add_space(6.0);
                    if theme::button(ui, "run bench", false).clicked() {
                        app.run_action(ui.ctx(), Action::Bench);
                    }
                });
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(cw, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(cw);
                theme::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_min_height(98.0);
                    theme::micro(ui, "session");
                    ui.add_space(4.0);
                    let up = app.started.elapsed().as_secs();
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:02}:{:02}", up / 60, up % 60))
                            .font(num_font(28.0))
                            .color(txt()),
                    ));
                    ui.add_space(6.0);
                    theme::micro(
                        ui,
                        &format!("{} ops · {} in journal", app.state.ops, app.state.log.len()),
                    );
                });
            },
        );
    });

    ui.add_space(14.0);

    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "game watch");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::chip_button(ui, "scan", false).clicked() {
                    app.games = engine::running_games();
                    app.toast(
                        1,
                        format!("{} recognised", app.games.len()),
                    );
                }
                theme::pill(
                    ui,
                    if app.games.is_empty() { "watching" } else { "live" },
                    !app.games.is_empty(),
                );
            });
        });
        ui.add_space(10.0);
        let games: Vec<(&'static str, &'static str)> = app.games.clone();
        if games.is_empty() {
            ui.add(egui::Label::new(
                RichText::new(
                    "Quiet. VOID re-reads the process table every few seconds and offers the matching loadout the moment a game starts.",
                )
                .font(body_font())
                .color(faint()),
            ));
        } else {
            for (name, profile) in games {
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.0, white());
                    ui.add(egui::Label::new(
                        RichText::new(name).font(label_font()).color(txt()),
                    ));
                    ui.add_space(8.0);
                    theme::pill(ui, profile, false);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::chip_button(ui, "engage", false).clicked() {
                            let m = confirm_profile(app, profile);
                            app.modal = Some(m);
                        }
                    });
                });
                ui.add_space(4.0);
            }
        }
    });

    ui.add_space(14.0);

    let w = ui.available_width();
    let half = (w - 12.0) * 0.5;
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(148.0);
                ui.horizontal(|ui| {
                    theme::micro(ui, "applied stack");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        count_badge(ui, applied, total);
                    });
                });
                ui.add_space(10.0);
                if app.state.applied.is_empty() {
                    ui.add(egui::Label::new(
                        RichText::new("Nothing applied yet. Engines are on standby.")
                            .font(body_font())
                            .color(faint()),
                    ));
                } else {
                    let ids: Vec<String> = app.state.applied.iter().cloned().collect();
                    for id in ids.iter().take(7) {
                        let name = engine::find_tweak(id).map(|t| t.name).unwrap_or(id.as_str());
                        theme::kv_row(ui, name, "applied", false);
                    }
                    if ids.len() > 7 {
                        theme::micro(ui, &format!("+{} more", ids.len() - 7));
                    }
                }
            });
        });
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(148.0);
                ui.horizontal(|ui| {
                    theme::micro(ui, "journal");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::chip_button(ui, "bench", false).clicked() {
                            app.run_action(ui.ctx(), Action::Bench);
                        }
                    });
                });
                ui.add_space(8.0);
                let lines: Vec<(String, String)> = app
                    .state
                    .log
                    .iter()
                    .rev()
                    .take(9)
                    .map(|l| (l.at.clone(), format!("[{}] {}", l.level, l.msg)))
                    .collect();
                for (at, msg) in lines.iter().rev() {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(at).font(mono_font(10.0)).color(ghost()),
                        ));
                        ui.add(egui::Label::new(
                            RichText::new(msg).font(mono_font(10.5)).color(dim()),
                        ));
                    });
                }
            });
        });
    });

    ui.add_space(14.0);

    // ── pre-flight: the audit a coach would run by hand ──
    let checks: Vec<(String, String, bool)> = app
        .checks
        .iter()
        .map(|c| (c.label.to_string(), c.detail.clone(), c.ok))
        .collect();
    let failing = app.checks.iter().filter(|c| !c.ok).count();
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "pre-flight");
            ui.add_space(10.0);
            let badge = if failing == 0 {
                "cleared".to_string()
            } else {
                format!("{} open", failing)
            };
            theme::pill(ui, &badge, failing == 0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if failing > 0 && theme::button(ui, "fix the gaps", false).clicked() {
                    app.run_action(ui.ctx(), Action::FixPreflight);
                }
                ui.add_space(8.0);
                if theme::chip_button(ui, "net lab", false).clicked() {
                    app.run_action(ui.ctx(), Action::Ping);
                }
            });
        });
        ui.add_space(8.0);
        let per_row = 4;
        for chunk in checks.chunks(per_row) {
            ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                let gap = 10.0;
                let cw = (ui.available_width() - gap * (per_row as f32 - 1.0)) / per_row as f32;
                ui.spacing_mut().item_spacing.x = gap;
                for (label, detail, ok) in chunk {
                    ui.allocate_ui_with_layout(
                        Vec2::new(cw, 0.0),
                        Layout::top_down(Align::Min),
                        |ui| {
                            ui.set_width(cw);
                            ui.horizontal(|ui| {
                                let (dot, _) =
                                    ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                                if *ok {
                                    ui.painter().circle_filled(dot.center(), 3.0, white());
                                } else {
                                    ui.painter().circle_stroke(
                                        dot.center(),
                                        3.0,
                                        Stroke::new(1.0_f32, faint()),
                                    );
                                }
                                ui.add(egui::Label::new(
                                    RichText::new(tracked(label))
                                        .font(micro_font())
                                        .color(if *ok { faint() } else { dim() }),
                                ));
                            });
                            ui.add(egui::Label::new(
                                RichText::new(theme::clip(detail, 28))
                                    .font(mono_font(9.5))
                                    .color(if *ok { ghost() } else { faint() }),
                            ));
                        },
                    );
                }
            });
            ui.add_space(8.0);
        }
    });

    ui.add_space(14.0);

    // ── the VOID score: one measured number ──
    let (score, parts) = app.score();
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            ui.vertical(|ui| {
                ui.set_width(168.0);
                theme::micro(ui, "void score");
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:03}", score))
                            .font(num_font(48.0))
                            .color(txt()),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("/100").font(micro_font()).color(ghost()),
                    ));
                });
                ui.add(egui::Label::new(
                    RichText::new(match score {
                        0..=39 => "cold",
                        40..=64 => "warming",
                        65..=84 => "sharp",
                        _ => "match ready",
                    })
                    .font(label_font())
                    .color(faint()),
                ));
            });
            ui.add_space(30.0);
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 8.0).max(240.0));
                for (label, v, detail) in parts {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(tracked(label))
                                .font(micro_font())
                                .color(faint()),
                        ));
                        ui.add_space(8.0);
                        let (rect, _) = ui.allocate_exact_size(
                            Vec2::new((ui.available_width() - 78.0).max(60.0), 6.0),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(rect, 3.0, a(white(), 14));
                        let filled = egui::Rect::from_min_size(
                            rect.min,
                            Vec2::new(rect.width() * v.clamp(0.0, 1.0), rect.height()),
                        );
                        ui.painter().rect_filled(filled, 3.0, a(white(), 225));
                        ui.add(egui::Label::new(
                            RichText::new(detail).font(mono_font(10.0)).color(dim()),
                        ));
                    });
                    ui.add_space(5.0);
                }
                ui.add(egui::Label::new(
                    RichText::new(
                        "Measured from your machine: applied share, kernel timer, sleep jitter, handshake RTT. Nothing is scored until it has been read back.",
                    )
                    .font(micro_font())
                    .color(ghost()),
                ));
            });
        });
    });

    ui.add_space(14.0);
    section(ui, "quick actions", |ui| {
        ui.horizontal_wrapped(|ui| {
            if theme::chip_button(ui, "match mode", false).clicked() {
                app.run_action(ui.ctx(), Action::MatchMode);
            }
            if theme::chip_button(ui, "net lab", false).clicked() {
                app.run_action(ui.ctx(), Action::Ping);
            }
            if theme::chip_button(ui, "flush dns", false).clicked() {
                app.run_action(ui.ctx(), Action::FlushDns);
            }
            if theme::chip_button(ui, "clean temp", false).clicked() {
                app.run_action(ui.ctx(), Action::CleanTemp);
            }
            if theme::chip_button(ui, "restore point", false).clicked() {
                app.run_action(ui.ctx(), Action::RestorePoint);
            }
            if theme::chip_button(ui, "verify now", false).clicked() {
                app.live_at = -10.0;
                app.toast(0, "re-reading every value from Windows");
            }
            let mut motion = app.state.motion;
            if theme::chip_button(ui, "motion", motion).clicked() {
                motion = !motion;
                app.state.motion = motion;
                app.state.save();
            }
        });
    });
}

pub fn confirm_profile(app: &App, pid: &'static str) -> Modal {
    let p = match engine::find_profile(pid) {
        Some(p) => p,
        None => {
            return Modal::info("profiles", "Unknown profile", "That loadout is not in the catalog.");
        }
    };
    let plan: Vec<String> = p
        .tweaks
        .iter()
        .map(|id| {
            engine::find_tweak(id)
                .map(|t| t.name.to_string())
                .unwrap_or_else(|| id.to_string())
        })
        .collect();
    let already = p.tweaks.iter().filter(|id| app.state.applied.contains(**id)).count();
    Modal {
        kicker: "engage profile".into(),
        title: p.name.to_string(),
        body: format!(
            "{} — {} steps.{} Everything is re-read from the registry afterwards, so what you see is what Windows holds.",
            p.note,
            plan.len(),
            if already > 0 {
                format!(" {} already applied.", already)
            } else {
                String::new()
            }
        ),
        plan,
        ok: "Engage".into(),
        cancel: "Cancel".into(),
        action: Action::Profile(pid),
        info_only: false,
        warn: false,
    }
}

// ── settings ─────────────────────────────────────────────────────────────────

pub fn settings(app: &mut App, ui: &mut Ui) {
    let w = ui.available_width();
    let half = (w - 12.0) * 0.5;

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);

            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "sound");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new("Ambient bed").font(label_font()).color(txt()),
                    ));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let mut on = app.audio.playing;
                        if theme::toggle(ui, &mut on).changed() {
                            app.audio.toggle();
                            app.state.muted = !app.audio.playing;
                            app.state.save();
                        }
                    });
                });
                ui.add_space(4.0);
                ui.add(egui::Label::new(
                    RichText::new(crate::audio::PRESETS[app.audio.preset].1)
                        .font(body_font())
                        .color(faint()),
                ));
                ui.add_space(14.0);

                let mut vol = app.audio.volume;
                ui.horizontal(|ui| {
                    theme::micro(ui, "volume");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(egui::Label::new(
                            RichText::new(format!("{}", (vol * 100.0).round() as i32))
                                .font(mono_font(11.0))
                                .color(dim()),
                        ));
                    });
                });
                if theme::slider(ui, &mut vol, ui.available_width()).changed() {
                    app.audio.set_volume(vol);
                    app.state.volume = (vol * 100.0).round() as u8;
                    app.state.save();
                }
                ui.add_space(14.0);
                theme::micro(ui, "preset");
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    for (i, (name, _)) in crate::audio::PRESETS.iter().enumerate() {
                        if theme::chip_button(ui, name, app.audio.preset == i).clicked() {
                            app.audio.set_preset(i);
                            if !app.audio.playing {
                                app.audio.play();
                            }
                        }
                    }
                });
                ui.add_space(12.0);
                let mut inten = app.audio.intensity;
                ui.horizontal(|ui| {
                    theme::micro(ui, "intensity");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(egui::Label::new(
                            RichText::new(format!("{}", (inten * 100.0).round() as i32))
                                .font(mono_font(11.0))
                                .color(dim()),
                        ));
                        ui.add_space(10.0);
                        let w = (ui.available_width() - 40.0).max(80.0);
                        if theme::slider(ui, &mut inten, w).changed() {
                            app.audio.set_intensity(inten);
                            app.state.intensity = (inten * 100.0).round() as u8;
                            app.state.save();
                        }
                    });
                });
                ui.add_space(4.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "Intensity drives the pulse depth and the bell density. Trauma is the heaviest bed.",
                    )
                    .font(micro_font())
                    .color(ghost()),
                ));

                if let Some(err) = &app.audio.error {
                    ui.add_space(8.0);
                    let short: String = err.chars().take(96).collect::<String>();
                    ui.add(egui::Label::new(
                        RichText::new(format!("audio: {}", short))
                            .font(mono_font(10.0))
                            .color(ghost()),
                    ));
                }
            });

            ui.add_space(12.0);

            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "behaviour");
                ui.add_space(10.0);
                // theme
                ui.horizontal(|ui| {
                    theme::micro(ui, "theme");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::chip_button(ui, "porcelain", theme::light()).clicked() {
                            app.run_action(ui.ctx(), Action::Theme(1));
                        }
                        if theme::chip_button(ui, "obsidian", !theme::light()).clicked() {
                            app.run_action(ui.ctx(), Action::Theme(0));
                        }
                    });
                });
                ui.add_space(12.0);
                let mut rows: Vec<(&str, bool, String)> = Vec::new();
                rows.push(("Reduced motion", !app.state.motion, "stops dust, pulses and easing".into()));
                rows.push(("Checkpoint before advanced", app.state.auto_backup, "srclient checkpoint, ~0.2 s".into()));
                rows.push((
                    "Discord presence",
                    app.state.presence,
                    if app.rpc.connected() {
                        "connected · shows your score".into()
                    } else {
                        "waiting for the Discord client".to_string()
                    },
                ));
                rows.push((
                    "Void guard",
                    app.state.guard,
                    "re-asserts applied edits that drift".into(),
                ));
                rows.push((
                    "Rain",
                    app.state.rain,
                    "ambient streaks behind the deck".into(),
                ));
                rows.push((
                    "Auto-loadout",
                    app.state.auto_switch,
                    "arms a title's set when it launches".into(),
                ));
                for (label, on, hint) in rows {
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut v = on;
                            if theme::toggle(ui, &mut v).changed() {
                                match label {
                                    "Reduced motion" => app.state.motion = !app.state.motion,
                                    "Discord presence" => {
                                        app.state.presence = !app.state.presence;
                                        app.rpc = crate::rpc::Rpc::start(app.state.presence);
                                    }
                                    "Void guard" => app.state.guard = !app.state.guard,
                                    "Rain" => app.state.rain = !app.state.rain,
                                    "Auto-loadout" => {
                                        app.state.auto_switch = !app.state.auto_switch
                                    }
                                    _ => app.state.auto_backup = !app.state.auto_backup,
                                }
                                app.state.save();
                                app.toast(1, format!("{} toggled", label.to_lowercase()));
                            }
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.add(egui::Label::new(
                                    RichText::new(label).font(label_font()).color(txt()),
                                ));
                                ui.add(egui::Label::new(
                                    RichText::new(hint).font(micro_font()).color(faint()),
                                ));
                            });
                        });
                    });
                    ui.add_space(6.0);
                }
            });
        });

        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);

            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    theme::micro(ui, "discord");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::chip_button(ui, "reconnect", false).clicked() {
                            app.rpc.reconnect();
                            app.toast(1, "discord · reconnecting");
                        }
                        if theme::chip_button(ui, "refresh", false).clicked() {
                            app.rpc.refresh_user();
                        }
                    });
                });
                ui.add_space(8.0);
                let who = app.rpc.who();
                let linked = app.rpc.connected();
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                    if linked {
                        ui.painter().circle_filled(dot.center(), 3.0, white());
                    } else {
                        ui.painter()
                            .circle_stroke(dot.center(), 3.0, Stroke::new(1.0_f32, faint()));
                    }
                    ui.add(egui::Label::new(
                        RichText::new(if linked { "connected" } else { "waiting for the client" })
                            .font(label_font())
                            .color(if linked { txt() } else { faint() }),
                    ));
                });
                ui.add_space(4.0);
                let account = if who.name.is_empty() {
                    match &who.error {
                        Some(e) => format!("account · {}", e),
                        None => "account · not linked yet".to_string(),
                    }
                } else {
                    format!("account · {}", who.name)
                };
                ui.add(egui::Label::new(
                    RichText::new(theme::clip(&account, 52))
                        .font(mono_font(10.5))
                        .color(dim()),
                ));
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(format!("client id · {}", crate::app::RPC_CLIENT_ID))
                        .font(mono_font(10.0))
                        .color(ghost()),
                ));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if theme::chip_button(ui, "copy id", false).clicked() {
                        ui.ctx().copy_text(crate::app::RPC_CLIENT_ID.to_string());
                        app.toast(0, "client id copied");
                    }
                    if theme::chip_button(ui, "presence", app.state.presence).clicked() {
                        app.state.presence = !app.state.presence;
                        app.rpc = crate::rpc::Rpc::start(app.state.presence);
                        app.state.save();
                    }
                });
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "Presence runs over the local Discord pipe. The app asks once for permission inside Discord — allow it and your tag appears here.",
                    )
                    .font(micro_font())
                    .color(ghost()),
                ));
            });

            ui.add_space(12.0);

            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "engine");
                ui.add_space(10.0);
                theme::kv_row(ui, "version", engine::VERSION, true);
                theme::kv_row(ui, "os", &app.machine.os, false);
                theme::kv_row(ui, "host", &app.machine.host, false);
                theme::kv_row(ui, "cpu", &app.machine.cpu, false);
                theme::kv_row(ui, "gpu", &app.machine.gpu, false);
                theme::kv_row(
                    ui,
                    "admin",
                    if app.machine.admin { "yes" } else { "no" },
                    app.machine.admin,
                );
                theme::kv_row(
                    ui,
                    "timer",
                    &match engine::timer_resolution_ms() {
                        Some(v) => format!("{:.3} ms", v),
                        None => "n/a".to_string(),
                    },
                    false,
                );
                theme::kv_row(ui, "power plan", &engine::power_plan(), false);
                theme::kv_row(ui, "tweaks", &format!("{}", catalog::TWEAKS.len()), false);
                theme::kv_row(
                    ui,
                    "applied",
                    &format!("{}", app.state.applied.len()),
                    false,
                );
            });

            ui.add_space(12.0);

            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "safety & maintenance");
                ui.add_space(10.0);
                if theme::chip_button(ui, "export report", false).clicked() {
                    app.run_action(ui.ctx(), Action::ExportReport);
                }
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    if theme::chip_button(ui, "restore point", false).clicked() {
                        app.run_action(ui.ctx(), Action::RestorePoint);
                    }
                    if theme::chip_button(ui, "revert all", false).clicked() {
                        app.run_action(ui.ctx(), Action::RevertAll);
                    }
                    if theme::chip_button(ui, "open journal", false).clicked() {
                        let path = crate::platform::data_dir().join("void.log");
                        match crate::platform::spawn_detached(&path.to_string_lossy()) {
                            Ok(_) => app.toast(1, format!("journal · {}", path.display())),
                            Err(e) => app.toast(2, format!("journal · {} · {}", path.display(), e)),
                        }
                    }
                });
                ui.add_space(10.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "VOID only edits documented Windows configuration. No game files, no injection, no anti-cheat contact.",
                    )
                    .font(micro_font())
                    .color(ghost()),
                ));
            });
        });
    });

    ui.add_space(16.0);
    section(ui, "about", |ui| {
        ui.add(egui::Label::new(
            RichText::new(format!(
                "VOID {} — native desktop build. Engine in-process, no localhost bridge, no browser runtime.",
                engine::VERSION
            ))
                .font(body_font())
                .color(faint()),
        ));
        ui.add_space(6.0);
        ui.add(egui::Label::new(
            RichText::new(format!(
                "window drawn by egui · audio by rodio · {} · {}",
                app.machine.os,
                if app.machine.simulated {
                    "simulation mode (non-Windows host)"
                } else {
                    "native windows mode"
                }
            ))
            .font(micro_font())
            .color(ghost()),
        ));
    });
}
