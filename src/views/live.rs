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
                RichText::new("No emulator found on this machine.").font(body_font()).color(faint()),
            ));
        }

        // Installed emulators get a row each; the rest collapse into one line. The
        // safety card below is the point of this page — it should not sit under a
        // screenful of "not found".
        let (found, missing): (Vec<_>, Vec<_>) = rows.into_iter().partition(|r| r.1);
        for (name, installed, running, path) in found {
            ui.horizontal(|ui| {
                let (dot, _) = ui.allocate_exact_size(Vec2::new(8.0, 8.0), Sense::hover());
                let col = if running {
                    white()
                } else if installed {
                    a(white(), 110)
                } else {
                    a(white(), 30)
                };
                ui.painter().circle_filled(dot.center(), 3.0, col);
                ui.add(egui::Label::new(
                    RichText::new(&name).font(label_font()).color(if installed { txt() } else { ghost() }),
                ));
                ui.add_space(8.0);
                ui.add(egui::Label::new(
                    RichText::new(theme::clip(&path, 58))
                        .font(mono_font(9.5))
                        .color(ghost()),
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

        if !missing.is_empty() {
            ui.add_space(2.0);
            ui.add(egui::Label::new(
                RichText::new(format!(
                    "{} not installed · {}",
                    missing.len(),
                    missing
                        .iter()
                        .map(|r| r.0.as_str())
                        .collect::<Vec<&str>>()
                        .join(" · ")
                ))
                .font(micro_font())
                .color(faint()),
            ));
        }

        ui.add_space(8.0);
        ui.add(egui::Label::new(
            RichText::new(
                "VOID tunes the host, never the game. Free Fire's anti-cheat only ever sees an emulator doing what it always does — just on a quieter machine.",
            )
            .font(micro_font())
            .color(ghost()),
        ));
    });

    // ── emulator safety: what VOID will and will not touch ──────────────────
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "emulator safety");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::chip_button(ui, "repair", false).clicked() {
                    app.run_action(ui.ctx(), Action::RepairEmulator);
                }
                theme::pill(
                    ui,
                    if app.state.emu_guard {
                        "guard on"
                    } else {
                        "guard off"
                    },
                    app.state.emu_guard,
                );
            });
        });
        ui.add_space(10.0);
        let guarded = if app.state.emu_guard {
            "memory-integrity and hypervisor toggles are refused"
        } else {
            "hypervisor toggles are ALLOWED - an instance can stop booting"
        };
        for (k, v) in [
            (
                "touches",
                "the player window only - VM service, ADB bridge and hypervisor hosts are never touched".to_string(),
            ),
            (
                "booting",
                "a guest younger than 90 s is left alone, whatever you press".to_string(),
            ),
            ("guard", guarded.to_string()),
            (
                "repair",
                "restores Normal priority, every core, and no forced GPU".to_string(),
            ),
        ] {
            theme::kv_row(ui, k, &v, false);
        }

        // Boot-level state: the only changes that survive a restart. This is
        // what "it will not boot any more" actually looks like.
        let boot: Vec<&engine::BootCheck> = app.boot.iter().filter(|c| c.id != "verdict").collect();
        if !boot.is_empty() {
            ui.add_space(4.0);
            for c in boot {
                let bad = !c.ok;
                theme::kv_row(
                    ui,
                    c.label,
                    &c.detail,
                    bad,
                );
            }
            if app.boot.iter().any(|c| !c.ok) {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if theme::chip_button(ui, "repair boot", true).clicked() {
                        app.run_action(ui.ctx(), Action::RepairBoot);
                    }
                    ui.add(egui::Label::new(
                        RichText::new(
                            "puts the BCD tick, TSC policy, hypervisor launch type and memory integrity back to Windows defaults",
                        )
                        .font(micro_font())
                        .color(ghost()),
                    ));
                });
            }
        }
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let mut g = app.state.emu_guard;
            if theme::toggle(ui, &mut g).changed() {
                app.state
                    .set_setting("emu_guard", if g { "1" } else { "0" });
                app.toast(
                    if g { 0 } else { 1 },
                    if g {
                        "emulator guard on"
                    } else {
                        "emulator guard off - hypervisor tweaks allowed"
                    },
                );
            }
            ui.add_space(8.0);
            ui.add(egui::Label::new(
                RichText::new("Emulator guard")
                    .font(label_font())
                    .color(txt()),
            ));
            ui.add_space(6.0);
            ui.add(egui::Label::new(
                RichText::new("refuses the changes that brick instances")
                    .font(micro_font())
                    .color(ghost()),
            ));
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
                theme::micro(ui, "detected");
                ui.add_space(8.0);
                let found = app
                    .emulators
                    .iter()
                    .find(|e| e.installed)
                    .map(|e| e.name.to_string())
                    .unwrap_or_else(|| "none installed".to_string());
                let running = app.emulators.iter().filter(|e| e.running).count();
                let player = app
                    .emulators
                    .iter()
                    .find(|e| e.running)
                    .map(|e| e.path.clone())
                    .unwrap_or_default();
                let rows = [
                    ("emulator", found),
                    ("instances", format!("{} running", running)),
                    (
                        "player",
                        if player.is_empty() {
                            "-".to_string()
                        } else {
                            theme::clip(&player, 34)
                        },
                    ),
                    (
                        "cores",
                        format!(
                            "{} logical · affinity keeps one of each pair",
                            crate::platform::logical_cpus()
                        ),
                    ),
                    (
                        "guard",
                        if app.state.emu_guard {
                            "on".to_string()
                        } else {
                            "off".to_string()
                        },
                    ),
                    (
                        "hypervisor",
                        if crate::platform::is_windows() {
                            "read, never changed".to_string()
                        } else {
                            "n/a".to_string()
                        },
                    ),
                ];
                for (k, v) in rows {
                    theme::kv_row(ui, k, &v, false);
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
                    .color(dim()),
                ));
                ui.add_space(10.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "If the emulator still stutters, drop graphics to Smooth before adding more tweaks — the engine is already clean at that point.",
                    )
                    .font(micro_font())
                    .color(faint()),
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
                                RichText::new(value).font(num_font(34.0)).color(txt()),
                            ));
                            ui.add(egui::Label::new(
                                RichText::new(unit).font(label_font()).color(faint()),
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
                                .color(txt()),
                        ));
                        ui.add(egui::Label::new(
                            RichText::new(format!("of {:.1} GB", app.stats.total_gb))
                                .font(label_font())
                                .color(faint()),
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
                                        RichText::new(v).font(num_font(22.0)).color(txt()),
                                    ));
                                    ui.add(egui::Label::new(
                                        RichText::new(unit).font(micro_font()).color(faint()),
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
                            .color(ghost()),
                        ));
                    });
                }
            }
            None => {
                ui.add(egui::Label::new(
                    RichText::new("No measurement yet. Run it with the process idle.")
                        .font(body_font())
                        .color(faint()),
                ));
            }
        }
    });

    ui.add_space(14.0);

    // ── cpu attribution ──
    // Nothing else in this category names the process that stole the frame.
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "cpu attribution · last sample");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::chip_button(ui, "boost the top one", false).clicked() {
                    if let Some((name, pid, _)) = app.thieves.first().cloned() {
                        app.run_action(ui.ctx(), Action::BoostPid(pid));
                        app.toast(1, format!("boosting {} · pid {}", theme::clip(&name, 24), pid));
                    }
                }
            });
        });
        ui.add_space(4.0);
        ui.add(egui::Label::new(
            RichText::new(
                "Kernel + user time per process, differenced between two samples. This is how VOID names what actually took the frame.",
            )
            .font(micro_font())
            .color(ghost()),
        ));
        ui.add_space(10.0);

        if app.thieves.is_empty() {
            ui.add(egui::Label::new(
                RichText::new("Sampling… this card fills in on the monitor view.")
                    .font(body_font())
                    .color(faint()),
            ));
        }
        let top = app
            .thieves
            .iter()
            .map(|(_, _, p)| *p)
            .fold(0.0f32, f32::max)
            .max(1.0);
        for (name, _pid, pct) in app.thieves.iter().take(7) {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(theme::clip(name, 28))
                        .font(mono_font(10.5))
                        .color(if *pct > 8.0 { txt() } else { dim() }),
                ));
                ui.add_space(8.0);
                let (rect, _) =
                    ui.allocate_exact_size(Vec2::new((ui.available_width() - 74.0).max(60.0), 5.0), Sense::hover());
                ui.painter().rect_filled(rect, 2.5, a(white(), 12));
                let w = (pct / top).clamp(0.02, 1.0) * rect.width();
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
                    2.5,
                    a(white(), if *pct > 8.0 { 225 } else { 150 }),
                );
                ui.add(egui::Label::new(
                    RichText::new(format!("{:.1}%", pct)).font(mono_font(10.0)).color(faint()),
                ));
            });
            ui.add_space(4.0);
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
                RichText::new(msg).font(body_font()).color(faint()),
            ));
        }
        for (name, pid, mem) in rows {
            ui.horizontal(|ui| {
                ui.add(egui::Label::new(
                    RichText::new(&name).font(mono_font(11.5)).color(dim()),
                ));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::chip_button(ui, "boost", false).clicked() {
                        app.run_action(ui.ctx(), Action::BoostPid(pid));
                    }
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(
                        RichText::new(format!("{:.0} mb", mem))
                            .font(mono_font(10.5))
                            .color(faint()),
                    ));
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(
                        RichText::new(format!("{}", pid)).font(mono_font(10.5)).color(ghost()),
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
                        RichText::new(format!("{:.0}", edpi)).font(num_font(52.0)).color(txt()),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("eDPI").font(label_font()).color(faint()),
                    ));
                });
                ui.add_space(2.0);
                ui.add(egui::Label::new(
                    RichText::new(format!("{:.1} cm / 360°", cm360))
                        .font(body_font())
                        .color(dim()),
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
                    .color(ghost()),
                ));
            });
        });
    });

    ui.add_space(14.0);


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
                                .color(ghost()),
                        ));
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(
                                RichText::new(*name).font(label_font()).color(txt()),
                            ));
                            ui.add(egui::Label::new(
                                RichText::new(*detail).font(micro_font()).color(faint()),
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
                    .color(dim()),
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
                RichText::new(shown).font(mono_font(13.0)).color(txt()),
            ));
            ui.add_space(4.0);
            if theme::chip_button(ui, "-", false).clicked() {
                *value = (*value - step).max(min);
            }
        });
    });
}
