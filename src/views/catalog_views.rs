// VOID — catalog-driven views: optimizer, profiles, network, kernel.

use eframe::egui::{self, Align, Layout, RichText, Sense, Ui, Vec2};

use crate::app::{Action, App, Modal};
use crate::engine::{self, catalog};
use crate::theme::{self, *};
use crate::views::{confirm_profile, risk_pill, section, tweak_row};

// ── optimizer ────────────────────────────────────────────────────────────────

pub fn optimize(app: &mut App, ui: &mut Ui) {
    ui.horizontal(|ui| {
        // search
        egui::Frame::none()
            .fill(INK)
            .stroke(egui::Stroke::new(1.0_f32, LINE))
            .rounding(9.0)
            .inner_margin(egui::Margin::symmetric(12.0, 7.0))
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.search)
                        .hint_text("search the catalog")
                        .frame(false)
                        .desired_width(220.0)
                        .font(theme::label_font()),
                );
            });
        ui.add_space(10.0);
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if theme::button(ui, "apply filtered", true).clicked() {
                let ids = filtered_ids(app);
                if ids.is_empty() {
                    app.toast(2, "nothing matches that filter");
                } else {
                    let m = Modal {
                        kicker: "bulk apply".into(),
                        title: format!("Apply {} tweaks?", ids.len()),
                        body: "Each entry is applied in order and logged. Anything that fails is reported, not retried."
                            .to_string(),
                        plan: ids
                            .iter()
                            .filter_map(|id| engine::find_tweak(id).map(|t| t.name.to_string()))
                            .take(24)
                            .collect(),
                        ok: "Apply".into(),
                        cancel: "Cancel".into(),
                        action: Action::Apply(ids.iter().map(|s| s.to_string()).collect()),
                        info_only: false,
                        warn: false,
                    };
                    app.modal = Some(m);
                }
            }
        });
    });

    ui.add_space(12.0);

    // category chips
    ui.horizontal_wrapped(|ui| {
        if theme::chip_button(ui, "all", app.cat == 0).clicked() {
            app.cat = 0;
        }
        for (i, (_, label)) in catalog::CATEGORIES.iter().enumerate() {
            if theme::chip_button(ui, label, app.cat == i + 1).clicked() {
                app.cat = if app.cat == i + 1 { 0 } else { i + 1 };
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal_wrapped(|ui| {
        for (i, label) in ["any risk", "safe", "moderate", "advanced"].iter().enumerate() {
            if theme::chip_button(ui, label, app.risk == i as u8).clicked() {
                app.risk = i as u8;
            }
        }
        ui.add_space(10.0);
        let ids = filtered_ids(app);
        theme::micro(ui, &format!("{} visible · {} applied total", ids.len(), app.state.applied.len()));
    });

    ui.add_space(12.0);

    let ids = filtered_ids(app);
    if ids.is_empty() {
        theme::card(ui, |ui| {
            ui.add(egui::Label::new(
                RichText::new("No tweak matches that combination. Clear the search or pick another lane.")
                    .font(body_font())
                    .color(FAINT),
            ));
        });
        return;
    }

    let mut last_cat = "";
    for id in ids {
        if let Some(t) = engine::find_tweak(id) {
            if t.cat != last_cat {
                last_cat = t.cat;
                let label = catalog::CATEGORIES
                    .iter()
                    .find(|(c, _)| *c == t.cat)
                    .map(|(_, l)| *l)
                    .unwrap_or(t.cat);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    theme::micro(ui, label);
                    let n = catalog::TWEAKS.iter().filter(|x| x.cat == t.cat).count();
                    theme::micro(ui, &format!("· {}", n));
                });
                ui.add_space(6.0);
            }
            tweak_row(app, ui, t);
        }
    }
}

fn filtered_ids(app: &App) -> Vec<&'static str> {
    let needle = app.search.trim().to_lowercase();
    let cat: Option<&str> = if app.cat == 0 {
        None
    } else {
        catalog::CATEGORIES.get(app.cat - 1).map(|(c, _)| *c)
    };
    catalog::TWEAKS
        .iter()
        .filter(|t| match cat {
            Some(c) => t.cat == c,
            None => true,
        })
        .filter(|t| match app.risk {
            0 => true,
            n => t.risk == n - 1,
        })
        .filter(|t| {
            needle.is_empty()
                || t.name.to_lowercase().contains(&needle)
                || t.desc.to_lowercase().contains(&needle)
                || t.id.contains(&needle)
        })
        .map(|t| t.id)
        .collect()
}

// ── profiles ─────────────────────────────────────────────────────────────────

pub fn profiles(app: &mut App, ui: &mut Ui) {
    let w = ui.available_width();
    let gap = 12.0;
    let cw = (w - gap) * 0.5;
    let profiles = catalog::PROFILES;
    let mut i = 0;
    while i < profiles.len() {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for _ in 0..2 {
                if i >= profiles.len() {
                    break;
                }
                let p = &profiles[i];
                ui.allocate_ui_with_layout(
                    Vec2::new(cw, 0.0),
                    Layout::top_down(Align::MIN),
                    |ui| {
                        ui.set_width(cw);
                        profile_card(app, ui, p);
                    },
                );
                i += 1;
            }
        });
        ui.add_space(gap);
    }
}

fn profile_card(app: &mut App, ui: &mut Ui, p: &'static catalog::Profile) {
    let active = app.state.last_profile == p.id;
    let frame = egui::Frame::none()
        .fill(INK)
        .stroke(egui::Stroke::new(1.0_f32, if active { a(WHITE, 70) } else { LINE }))
        .rounding(16.0)
        .inner_margin(egui::Margin::same(18.0));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            theme::micro(ui, "profile");
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if active {
                    theme::pill(ui, "engaged", true);
                }
                theme::pill(ui, &format!("{} steps", p.tweaks.len()), false);
            });
        });
        ui.add_space(8.0);
        ui.add(egui::Label::new(
            RichText::new(p.name).font(num_font(21.0)).color(TXT),
        ));
        ui.add_space(2.0);
        ui.add(egui::Label::new(
            RichText::new(p.tagline).font(body_font()).color(DIM),
        ));
        ui.add_space(8.0);
        ui.add(egui::Label::new(
            RichText::new(p.note).font(micro_font()).color(FAINT),
        ));
        ui.add_space(14.0);
        ui.horizontal_wrapped(|ui| {
            if theme::button(ui, "engage", true).clicked() {
                let m = confirm_profile(app, p.id);
                app.modal = Some(m);
            }
            if theme::chip_button(ui, "recipe", false).clicked() {
                let plan: Vec<String> = p
                    .tweaks
                    .iter()
                    .filter_map(|id| engine::find_tweak(id).map(|t| format!("{} · {}", t.name, t.id)))
                    .collect();
                app.modal = Some(Modal {
                    kicker: "recipe".into(),
                    title: p.name.to_string(),
                    body: p.note.to_string(),
                    plan,
                    ok: "Close".into(),
                    cancel: String::new(),
                    action: Action::None,
                    info_only: true,
                    warn: false,
                });
            }
        });
    });
}

// ── network ──────────────────────────────────────────────────────────────────

pub fn network(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 250.0).max(320.0));
                theme::micro(ui, "sleep jitter");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let v = app
                        .bench
                        .as_ref()
                        .map(|b| format!("{:.2}", b.jitter))
                        .unwrap_or_else(|| "—".into());
                    ui.add(egui::Label::new(
                        RichText::new(v).font(num_font(38.0)).color(TXT),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("ms").font(label_font()).color(FAINT),
                    ));
                });
                ui.add_space(2.0);
                ui.add(egui::Label::new(
                    RichText::new(
                        "Timer resolution decides how fast the OS can wake a thread. Under 1 ms is competitive territory.",
                    )
                    .font(micro_font())
                    .color(FAINT),
                ));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if theme::button(ui, "measure", false).clicked() {
                        app.run_action(Action::Bench);
                    }
                    if theme::chip_button(ui, "flush dns", false).clicked() {
                        app.run_action(Action::FlushDns);
                    }
                    if theme::chip_button(ui, "reset tcp", false).clicked() {
                        let ids = vec!["net.nagle".to_string()];
                        app.revert_set(ids, "nagle");
                    }
                });
            });
            ui.add_space(14.0);
            {
                let w = 220.0;
                let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 92.0), Sense::hover());
                // a queue-shape sketch: tight bars = good, spread = bad
                let jitter = app.bench.as_ref().map(|b| b.jitter).unwrap_or(0.5);
                let spread = (jitter * 26.0).clamp(0.6, 9.0) as f32;
                for i in 0..28 {
                    let h = 12.0 + ((i as f32 * 1.7).sin().abs() * spread * 6.0);
                    let x = rect.left() + i as f32 * (rect.width() / 28.0);
                    let bar = egui::Rect::from_min_max(
                        egui::Pos2::new(x, rect.bottom() - h),
                        egui::Pos2::new(x + rect.width() / 28.0 - 2.0, rect.bottom()),
                    );
                    ui.painter().rect_filled(bar, 1.5, a(WHITE, 60 + (i as u8 % 5) * 20));
                }
            }
        });
    });

    ui.add_space(14.0);
    section(ui, "network lane", |ui| {
        crate::views::by_category(app, ui, "net");
    });
}

// ── kernel ───────────────────────────────────────────────────────────────────

pub fn kernel(app: &mut App, ui: &mut Ui) {
    theme::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_width((ui.available_width() - 250.0).max(320.0));
                theme::micro(ui, "timer resolution");
                ui.add_space(4.0);
                let v = match engine::timer_resolution_ms() {
                    Some(v) => format!("{:.3}", v),
                    None => "n/a".to_string(),
                };
                ui.horizontal(|ui| {
                    ui.add(egui::Label::new(
                        RichText::new(v).font(num_font(34.0)).color(TXT),
                    ));
                    ui.add(egui::Label::new(
                        RichText::new("ms").font(label_font()).color(FAINT),
                    ));
                });
                ui.add_space(4.0);
                ui.add(egui::Label::new(
                    RichText::new(format!("active power plan · {}", engine::power_plan()))
                        .font(micro_font())
                        .color(FAINT),
                ));
            });
            if theme::chip_button(ui, "restore point", false).clicked() {
                app.run_action(Action::RestorePoint);
            }
        });
        ui.add_space(10.0);
        ui.add(egui::Label::new(
            RichText::new(
                "Kernel lane changes scheduler and timer policy. Every entry is reversible from the same switch that applies it; if Windows restarts a service afterwards, that is expected.",
            )
            .font(micro_font())
            .color(GHOST),
        ));
    });

    ui.add_space(14.0);
    section(ui, "kernel & scheduler", |ui| {
        crate::views::by_category(app, ui, "kernel");
    });
    section(ui, "power & thermals", |ui| {
        crate::views::by_category(app, ui, "power");
    });
    section(ui, "storage & i/o", |ui| {
        crate::views::by_category(app, ui, "storage");
    });
}

// keeps `risk_pill` imported for the shared helpers re-exported above
#[allow(dead_code)]
fn _keep(ui: &mut Ui) {
    risk_pill(ui, 0);
}
