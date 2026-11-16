// VOID — live views: free fire, monitor, aim lab.

use eframe::egui::{self, Align, Layout, RichText, Sense, Ui, Vec2};

use crate::app::{Action, App};
use crate::engine;
use crate::theme::{self, *};

// ── free fire ────────────────────────────────────────────────────────────────

pub fn freefire(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "android emulator");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::chip_button(ui, "rescan", false).clicked() {
                    app.emulators = engine::emulators();
                    app.toast(1, format!("{} emulators scanned", app.emulators.len()));
                }
                if theme::chip_button(ui, "boost emulator", false).clicked() {
                    match engine::boost_emulator() {
                        Ok(m) => app.toast(0, m),
                        Err(e) => app.toast(2, e),
                    }
                }
            });
        });
        ui.add_space(10.0);

        if app.emulators.is_empty() {
            app.emulators = engine::emulators();
        }

        let rows: Vec<(String, bool, bool, String)> = app
            .emulators
            .iter()
            .map(|e| (e.name.to_string(), e.installed, e.running, e.path.clone()))
            .collect();

        if rows.is_empty() {
            ui.add(egui::Label::new(
                RichText::new("No emulator found on this machine.").font(body_font()).color(FAINT),
            ));
        }

        for (name, installed, running, path) in rows {
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                let col = if running {
                    WHITE
                } else if installed {
                    a(WHITE, 110)
                } else {
                    a(WHITE, 30)
                };
                ui.painter().circle_filled(dot.center(), 3.0, col);
                ui.add(egui::Label::new(
                    RichText::new(&name).font(label_font()).color(if installed { TXT } else { GHOST }),
                ));
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(theme::clip(&path, 58))
                        .font(mono_font(9.5))
                        .color(GHOST),
                ));
                ui.add_space(8.0);
                theme::pill(
                    ui,
                    if running {
                        "running"
                    } else if installed {
                        "installed"
                    } else {
                        "not found"
                    },
                    running,
                );
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if installed && theme::chip_button(ui, "launch", false).clicked() {
                        app.run_action(ui.ctx(), Action::LaunchEmulator(path.clone()));
                    }
                });
            });
            ui.add_space(4.0);
        }

        ui.add_space(8.0);
        ui.add(egui::Label::new(
            RichText::new(
                "VOID tunes the host, never the game. Free Fire's anti-cheat only ever sees an emulator doing what it always does — just on a quieter machine.",
            )
            .font(micro_font())
            .color(GHOST),
        ));
    });

    ui.add_space(14.0);

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let w = ui.available_width();
        let half = (w - 12.0) * 0.5;
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "dial-in");
                ui.add_space(8.0);
                for (k, v) in [
                    ("graphics", "smooth · 60 fps cap"),
                    ("instance", "1 core per instance pin"),
                    ("resolution", "1280×720 @ 240 dpi"),
                    ("process", "high priority, above normal"),
                    ("network", "wifi jammer off, nagle off"),
                    ("input", "raw mouse, accel off"),
                ] {
                    theme::kv_row(ui, k, v, false);
                }
            });
        });
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "why it works");
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "The phone client was written for a 2 GHz mid-range SoC. On a desktop it is throttled by the emulator's scheduler, not by the CPU. Unparking cores, disabling HPET coalescing and pinning the emulator process removes the 5–15 ms frame spikes that feel like \"lag\" but are actually stalls.",
                    )
                    .font(body_font())
                    .color(DIM),
                ));
                ui.add_space(10.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "If the emulator still stutters, drop graphics to Smooth before adding more tweaks — the engine is already clean at that point.",
                    )
                    .font(micro_font())
                    .color(FAINT),
                ));
            });
        });
    });

    ui.add_space(14.0);
    theme::micro(ui, "free fire lane");
    ui.add_space(8.0);
    crate::views::by_category(app, ui, "freefire");
}

// ── monitor ──────────────────────────────────────────────────────────────────

pub fn monitor(app: &mut App, ui: &mut Ui) {
    let w = ui.available_width();
    let gap = 12.0;
    let third = (w - gap * 2.0) / 3.0;

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = gap;

        for (label, value, unit, hist) in [
            (
                "cpu load",
                format!("{:.0}", app.stats.cpu),
                "%",
                app.cpu_hist.clone(),
            ),
            (
                "memory",
                format!("{:.0}", app.stats.ram),
                "%",
                app.ram_hist.clone(),
            ),
        ] {
            ui.allocate_ui_with_layout(
                Vec2::new(third, 0.0),
                Layout::top_down(Align::Min),
                |ui| {
                    ui.set_width(third);
                    theme::card(ui, |ui| {
                        ui.set_width(ui.available_width());
                        theme::micro(ui, label);
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(
                                RichText::new(value).font(num_font(34.0)).color(TXT),
                            ));
                            ui.add(egui::Label::new(
                                RichText::new(unit).font(label_font()).color(FAINT),
                            ));
                        });
                        ui.add_space(6.0);
                        theme::spark(ui, &hist, Vec2::new(ui.available_width(), 56.0), 100.0);
                    });
                },
            );
        }

        ui.allocate_ui_with_layout(
            Vec2::new(third, 0.0),
            Layout::top_down(Align::Min),
            |ui| {
                ui.set_width(third);
                theme::card(ui, |ui| {
                    ui.set_width(ui.available_width());
                    theme::micro(ui, "memory detail");
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(format!("{:.1}", app.stats.used_gb))
                                .font(num_font(34.0))
                                .color(TXT),
                        ));
                        ui.add(egui::Label::new(
                            RichText::new(format!("of {:.1} GB", app.stats.total_gb))
                                .font(label_font())
                                .color(FAINT),
                        ));
                    });
                    ui.add_space(8.0);
                    theme::kv_row(
                        ui,
                        "uptime",
                        &format!(
                            "{}h {:02}m",
                            app.stats.uptime / 3600,
                            (app.stats.uptime % 3600) / 60
                        ),
                        false,
                    );
                    theme::kv_row(ui, "timer", &format!(
                        "{}",
                        engine::timer_resolution_ms()
                            .map(|v| format!("{:.3} ms", v))
                            .unwrap_or_else(|| "n/a".into())
                    ), false);
                });
            },
        );
    });

    ui.add_space(14.0);

    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "sleep jitter benchmark");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, "run", false).clicked() {
                    app.run_action(ui.ctx(), Action::Bench);
                }
            });
        });
        ui.add_space(10.0);
        match app.bench {
            Some(b) => {
                let w = ui.available_width();
                let cw = (w - 4.0 * 8.0) / 5.0;
                ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (k, v, unit) in [
                        ("avg", format!("{:.2}", b.avg), "ms"),
                        ("p50", format!("{:.2}", b.p50), "ms"),
                        ("p95", format!("{:.2}", b.p95), "ms"),
                        ("max", format!("{:.2}", b.max), "ms"),
                        ("score", format!("{:.0}", b.score), ""),
                    ] {
                        ui.allocate_ui_with_layout(
                            Vec2::new(cw, 0.0),
                            Layout::top_down(Align::Min),
                            |ui| {
                                ui.set_width(cw);
                                theme::micro(ui, k);
                                ui.horizontal(|ui| {
                                    ui.add(egui::Label::new(
                                        RichText::new(v).font(num_font(22.0)).color(TXT),
                                    ));
                                    ui.add(egui::Label::new(
                                        RichText::new(unit).font(micro_font()).color(FAINT),
                                    ));
                                });
                            },
                        );
                    }
                });
                ui.add_space(6.0);
                let delta = match app.bench_prev {
                    Some(p) => format!(" · {:+.2} ms vs previous run", b.jitter - p.jitter),
                    None => String::new(),
                };
                theme::micro(
                    ui,
                    &format!(
                        "{} samples · one sleep(1ms) each · overshoot = jitter{}",
                        b.samples, delta
                    ),
                );
                if app.bench_hist.len() >= 2 {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        theme::micro(ui, "trend");
                        ui.add_space(8.0);
                        theme::spark(
                            ui,
                            &app.bench_hist.iter().map(|v| *v as f32).collect::<Vec<f32>>(),
                            Vec2::new(220.0, 26.0),
                            100.0,
                        );
                        ui.add_space(8.0);
                        ui.add(egui::Label::new(
                            RichText::new(format!(
                                "last {} runs · best {:.0}",
                                app.bench_hist.len(),
                                app.bench_hist
                                    .iter()
                                    .cloned()
                                    .fold(0.0f64, f64::max)
                            ))
                            .font(micro_font())
                            .color(GHOST),
                        ));
                    });
                }
            }
            None => {
                ui.add(egui::Label::new(
                    RichText::new("No measurement yet. Run it with the process idle.")
                        .font(body_font())
                        .color(FAINT),
                ));
            }
        }
    });

    ui.add_space(14.0);

    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "processes · by memory");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::chip_button(ui, "refresh", false).clicked() {
                    app.procs = engine::processes();
                    app.procs_at = app.started.elapsed().as_secs_f64();
                }
            });
        });
        ui.add_space(8.0);

        let rows: Vec<(String, u32, f64)> = app.procs.iter().take(12).cloned().collect();
        if rows.is_empty() {
            let msg = if cfg!(windows) {
                "Reading the process table…"
            } else {
                "The process table is Windows-only. This build is running in simulation mode."
            };
            ui.add(egui::Label::new(
                RichText::new(msg).font(body_font()).color(FAINT),
            ));
        }
        for (name, pid, mem) in rows {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(&name).font(mono_font(11.5)).color(DIM),
                ));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::chip_button(ui, "boost", false).clicked() {
                        app.run_action(ui.ctx(), Action::BoostPid(pid));
                    }
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:.0} mb", mem))
                            .font(mono_font(10.5))
                            .color(FAINT),
                    ));
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(
                        RichText::new(format!("{}", pid)).font(mono_font(10.5)).color(GHOST),
                    ));
                });
            });
        }
        ui.add_space(6.0);
        theme::micro(ui, "boost raises priority to high — no process is ever killed");
    });
}

// ── aim lab ──────────────────────────────────────────────────────────────────

pub fn aim(app: &mut App, ui: &mut Ui) {
    let edpi = app.dpi * app.sens;
    let cm360 = if edpi > 0.0 {
        360.0 / (edpi * 0.022) * 2.54
    } else {
        0.0
    };

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let w = ui.available_width();
        let half = (w - 12.0) * 0.5;

        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "effective dpi");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:.0}", edpi)).font(num_font(52.0)).color(TXT),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("eDPI").font(label_font()).color(FAINT),
                    ));
                });
                ui.add_space(2.0);
                ui.add(egui::Label::new(
                    RichText::new(format!("{:.1} cm / 360°", cm360))
                        .font(body_font())
                        .color(DIM),
                ));
                ui.add_space(16.0);

                stepper(ui, "dpi", &mut app.dpi, 50.0, 100.0, 25600.0);
                ui.add_space(8.0);
                stepper(ui, "sensitivity", &mut app.sens, 0.05, 0.05, 5.0);

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    let mut lock = app.aim_locked;
                    if theme::chip_button(ui, "lock dpi", lock).clicked() {
                        lock = !lock;
                        app.aim_locked = lock;
                        app.toast(1, if lock { "dpi locked" } else { "dpi unlocked" });
                    }
                    if theme::chip_button(ui, "reset", false).clicked() {
                        app.dpi = 800.0;
                        app.sens = 0.9;
                    }
                });
            });
        });

        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "same feel in other games");
                ui.add_space(8.0);
                for (game, fov) in [
                    ("CS2 / CSGO", 1.0),
                    ("Valorant", 0.314),
                    ("Apex Legends", 1.0),
                    ("Overwatch 2", 0.93),
                    ("Free Fire (emu)", 1.4),
                ] {
                    let s = if app.sens > 0.0 {
                        app.sens * fov
                    } else {
                        0.0
                    };
                    theme::kv_row(ui, game, &format!("{:.2}", s), false);
                }
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "Conversion is proportional to horizontal FOV. Set the target game's sensitivity so the 360° distance matches — that is what your hand learned.",
                    )
                    .font(micro_font())
                    .color(GHOST),
                ));
            });
        });
    });

    ui.add_space(14.0);

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let w = ui.available_width();
        let half = (w - 12.0) * 0.5;
        ui.allocate_ui_with_layout(Vec2::new(half, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(half);
            theme::card(ui, |ui| {
                ui.set_width(ui.available_width());
                theme::micro(ui, "warm-up ladder");
                ui.add_space(8.0);
                for (i, (name, detail)) in [
                    ("100 flicks · wide", "large-angle, 40 cm arcs, no rush"),
                    ("60 flicks · micro", "5 cm corrections, wrist only"),
                    ("90 s tracking", "hold a corner, never stop moving"),
                ]
                .iter()
                .enumerate()
                {
                    ui.horizontal(|ui| {
                        ui.add(egui::Label::new(
                            RichText::new(format!("0{}", i + 1))
                                .font(mono_font(11.0))
                                .color(GHOST),
                        ));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(
                                RichText::new(*name).font(label_font()).color(TXT),
                            ));
                            ui.add(egui::Label::new(
                                RichText::new(*detail).font(micro_font()).color(FAINT),
                            ));
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
                theme::micro(ui, "input lane");
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "Sensitivity is only half of it. Apply the input lane below and Windows stops resampling the mouse — the crosshair moves exactly where the sensor says.",
                    )
                    .font(body_font())
                    .color(DIM),
                ));
            });
        });
    });

    ui.add_space(14.0);
    theme::micro(ui, "mouse & keyboard");
    ui.add_space(8.0);
    crate::views::by_category(app, ui, "input");
}

fn stepper(ui: &mut Ui, label: &str, value: &mut f32, step: f32, min: f32, max: f32) {
    ui.horizontal(|ui| {
        theme::micro(ui, label);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::chip_button(ui, "+", false).clicked() {
                *value = (*value + step).min(max);
            }
            ui.add_space(4.0);
            let shown = if step < 1.0 {
                format!("{:.2}", *value)
            } else {
                format!("{:.0}", *value)
            };
            ui.add(egui::Label::new(
                RichText::new(shown).font(mono_font(13.0)).color(TXT),
            ));
            ui.add_space(4.0);
            if theme::chip_button(ui, "-", false).clicked() {
                *value = (*value - step).max(min);
            }
        });
    });
}
