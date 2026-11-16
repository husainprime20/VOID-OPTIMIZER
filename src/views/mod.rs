// VOID — views.

pub mod catalog_views;
pub mod live;

use eframe::egui::{self, Align, Layout, Pos2, Rect, RichText, Stroke, Ui, Vec2};

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
        .fill(if on { INK2 } else { INK })
        .stroke(Stroke::new(1.0_f32, if on { a(WHITE, 55) } else { LINE }))
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
                if theme::chip_button(ui, if open { "hide" } else { "plan" }, open).clicked() {
                    plan_toggle = true;
                }
                ui.add_space(6.0);
                ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(t.name).font(label_font()).color(TXT),
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
                        RichText::new(t.desc).font(body_font()).color(FAINT),
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
                LINE,
            );
            ui.add_space(10.0);
            ui.add(egui::Label::new(
                RichText::new(t.why).font(body_font()).color(DIM),
            ));
            ui.add_space(8.0);
            for a in t.apply.iter() {
                ui.add(egui::Label::new(
                    RichText::new(format!("-> {}", catalog::action_text(a)))
                        .font(mono_font(10.5))
                        .color(FAINT),
                ));
            }
            for a in t.revert.iter() {
                ui.add(egui::Label::new(
                    RichText::new(format!("<- {}", catalog::action_text(a)))
                        .font(mono_font(10.5))
                        .color(GHOST),
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
                RichText::new(value).font(num_font(28.0)).color(TXT),
            ));
            if !unit.is_empty() {
                ui.add(egui::Label::new(
                    RichText::new(unit).font(label_font()).color(FAINT),
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
    let coverage = if total > 0 {
        applied as f32 / total as f32 * 100.0
    } else {
        0.0
    };

    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 200.0).max(320.0));
                theme::micro(ui, "catalog coverage");
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:.0}", coverage)).font(num_font(64.0)).color(TXT),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("%").font(num_font(24.0)).color(FAINT),
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
                    .color(FAINT),
                ));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if theme::button(ui, "ignite", true).clicked() {
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
                                RichText::new(theme::clip(&v, 30))
                                    .font(mono_font(10.5))
                                    .color(DIM),
                            ));
                        });
                        ui.add_space(30.0);
                    }
                });
            });
            ui.add_space(12.0);
            theme::ring(ui, coverage, 168.0, "readiness");
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
                    format!("{:.0}", app.stats.cpu),
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
                            RichText::new(v).font(num_font(28.0)).color(TXT),
                        ));
                        ui.add(egui::Label::new(
                            RichText::new(u).font(label_font()).color(FAINT),
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
                            .color(TXT),
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
                    "Quiet. VOID re-reads the process table every ten seconds and offers the matching loadout the moment a game starts.",
                )
                .font(body_font())
                .color(FAINT),
            ));
        } else {
            for (name, profile) in games {
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), egui::Sense::hover());
                    ui.painter().circle_filled(dot.center(), 3.0, WHITE);
                    ui.add(egui::Label::new(
                        RichText::new(name).font(label_font()).color(TXT),
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
                            .color(FAINT),
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
                            RichText::new(at).font(mono_font(10.0)).color(GHOST),
                        ));
                        ui.add(egui::Label::new(
                            RichText::new(msg).font(mono_font(10.5)).color(DIM),
                        ));
                    });
                }
            });
        });
    });

    ui.add_space(14.0);
    section(ui, "quick actions", |ui| {
        ui.horizontal_wrapped(|ui| {
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
                        RichText::new("Ambient bed").font(label_font()).color(TXT),
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
                        .color(FAINT),
                ));
                ui.add_space(14.0);

                let mut vol = app.audio.volume;
                ui.horizontal(|ui| {
                    theme::micro(ui, "volume");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.add(egui::Label::new(
                            RichText::new(format!("{}", (vol * 100.0).round() as i32))
                                .font(mono_font(11.0))
                                .color(DIM),
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
                if let Some(err) = &app.audio.error {
                    ui.add_space(8.0);
                    let short: String = err.chars().take(78).collect();
                    ui.add(egui::Label::new(
                        RichText::new(format!("audio: {}", short))
                            .font(mono_font(10.0))
                            .color(GHOST),
                    ));
                }
            });

            ui.add_space(12.0);

            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "behaviour");
                ui.add_space(10.0);
                let mut rows: Vec<(&str, bool, String)> = Vec::new();
                rows.push(("Reduced motion", !app.state.motion, "stops dust, pulses and easing".into()));
                rows.push(("Checkpoint before advanced", app.state.auto_backup, "srclient checkpoint, ~0.2 s".into()));
                for (label, on, hint) in rows {
                    ui.horizontal(|ui| {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let mut v = on;
                            if theme::toggle(ui, &mut v).changed() {
                                match label {
                                    "Reduced motion" => app.state.motion = !app.state.motion,
                                    _ => app.state.auto_backup = !app.state.auto_backup,
                                }
                                app.state.save();
                                app.toast(1, format!("{} toggled", label.to_lowercase()));
                            }
                            ui.with_layout(Layout::top_down(Align::LEFT), |ui| {
                                ui.add(egui::Label::new(
                                    RichText::new(label).font(label_font()).color(TXT),
                                ));
                                ui.add(egui::Label::new(
                                    RichText::new(hint).font(micro_font()).color(FAINT),
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
                    .color(GHOST),
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
                .color(FAINT),
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
            .color(GHOST),
        ));
    });
}
