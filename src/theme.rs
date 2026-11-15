// VOID — design system.
//
// One palette, one type scale, one set of primitives. Every pixel in the app
// comes from here so the whole surface stays monochrome and consistent.

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke,
    TextStyle, Ui, Vec2,
};

// ── palette ──────────────────────────────────────────────────────────────────
pub const VOID: Color32 = Color32::from_rgb(5, 5, 6); // window
pub const INK: Color32 = Color32::from_rgb(11, 11, 13); // card
pub const INK2: Color32 = Color32::from_rgb(16, 16, 19); // raised
pub const TXT: Color32 = Color32::from_rgb(244, 244, 246);
pub const DIM: Color32 = Color32::from_rgb(152, 152, 159);
pub const FAINT: Color32 = Color32::from_rgb(108, 108, 116);
pub const GHOST: Color32 = Color32::from_rgb(74, 74, 82);
pub const LINE: Color32 = Color32::from_rgb(28, 28, 32);
pub const LINE2: Color32 = Color32::from_rgb(44, 44, 50);
pub const WHITE: Color32 = Color32::from_rgb(255, 255, 255);

pub fn a(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

// ── type ─────────────────────────────────────────────────────────────────────
pub fn micro_font() -> FontId {
    FontId::new(9.5, FontFamily::Proportional)
}
pub fn label_font() -> FontId {
    FontId::new(12.0, FontFamily::Proportional)
}
pub fn body_font() -> FontId {
    FontId::new(13.0, FontFamily::Proportional)
}
pub fn num_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}
pub fn mono_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// "SETTINGS" → "S E T T I N G S" — the tracking that gives the app its calm.
pub fn tracked(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && ch != ' ' {
            out.push(' ');
        }
        out.extend(ch.to_uppercase());
    }
    out
}

// ── style install ────────────────────────────────────────────────────────────
pub fn install(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (TextStyle::Heading, FontId::new(22.0, FontFamily::Proportional)),
        (TextStyle::Body, body_font()),
        (TextStyle::Button, label_font()),
        (TextStyle::Small, micro_font()),
        (TextStyle::Monospace, mono_font(11.5)),
    ]
    .into();

    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;

    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(DIM);
    v.panel_fill = VOID;
    v.window_fill = VOID;
    v.extreme_bg_color = INK;
    v.faint_bg_color = INK2;
    v.window_rounding = 16.0.into();
    v.menu_rounding = 12.0.into();
    v.selection.bg_fill = a(WHITE, 40);
    v.selection.stroke = Stroke::new(1.0_f32, TXT);
    v.widgets.noninteractive.bg_fill = INK;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, DIM);
    v.widgets.inactive.bg_fill = INK2;
    v.widgets.inactive.weak_bg_fill = INK2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, LINE2);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, TXT);
    v.widgets.hovered.bg_fill = INK2;
    v.widgets.hovered.weak_bg_fill = INK2;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, a(WHITE, 90));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, WHITE);
    v.widgets.active.bg_fill = INK2;
    v.widgets.active.weak_bg_fill = INK2;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, WHITE);
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, WHITE);
    style.visuals = v;

    ctx.set_style(style);
}

// ── primitives ───────────────────────────────────────────────────────────────
pub fn micro(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(tracked(text)).font(micro_font()).color(FAINT),
    ));
}

pub fn hairline(ui: &mut Ui) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, LINE);
}

pub fn split_hairline(ui: &mut Ui, inset: f32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    let r = Rect::from_min_max(
        Pos2::new(rect.left() + inset, rect.top()),
        Pos2::new(rect.right(), rect.bottom()),
    );
    ui.painter().rect_filled(r, 0.0, LINE);
}

/// A card: hairline border, ink fill, hover-brightening edge.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_styled(ui, 18.0, 16.0, add)
}

pub fn card_styled<R>(
    ui: &mut Ui,
    pad: f32,
    rounding: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let frame = egui::Frame::none()
        .fill(INK)
        .stroke(Stroke::new(1.0_f32, LINE))
        .rounding(rounding)
        .inner_margin(egui::Margin::same(pad));

    frame.show(ui, add).inner
}

/// Card that lifts its border when the pointer is inside it.
pub fn hot_card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let frame = egui::Frame::none()
        .fill(INK)
        .stroke(Stroke::new(1.0_f32, LINE))
        .rounding(18.0)
        .inner_margin(egui::Margin::same(18.0));
    let inner = frame.show(ui, add);
    if ui.rect_contains_pointer(inner.response.rect) {
        ui.painter().rect_stroke(
            inner.response.rect,
            18.0,
            Stroke::new(1.0_f32, LINE2),
        );
    }
    inner.inner
}

pub fn pill(ui: &mut Ui, text: &str, lit: bool) {
    let font = micro_font();
    let galley = ui.painter().layout_no_wrap(tracked(text), font.clone(), TXT);
    let pad = Vec2::new(9.0, 5.0);
    let size = galley.size() + pad * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    let fill = if lit { a(WHITE, 235) } else { a(WHITE, 8) };
    let fg = if lit { VOID } else { DIM };
    ui.painter().rect_filled(rect, size.y * 0.5, fill);
    if !lit {
        ui.painter()
            .rect_stroke(rect, size.y * 0.5, Stroke::new(1.0_f32, LINE2));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        tracked(text),
        font,
        fg,
    );
}

/// Primary / ghost button. Primary = solid white with ink text.
pub fn button(ui: &mut Ui, text: &str, primary: bool) -> Response {
    let font = label_font();
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), TXT);
    let h = 32.0;
    let w = galley.size().x + 34.0;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let hover = if resp.hovered() { 1.0 } else { 0.0 };
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    let _ = hover;

    if primary {
        let base = a(WHITE, (235.0 - t * 15.0) as u8);
        ui.painter().rect_filled(rect, 9.0, base);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font,
            if resp.hovered() { Color32::BLACK } else { VOID },
        );
    } else {
        let fill = a(WHITE, (6.0 + t * 8.0) as u8);
        ui.painter().rect_filled(rect, 9.0, fill);
        ui.painter().rect_stroke(
            rect,
            9.0,
            Stroke::new(1.0_f32, if t > 0.02 { LINE2 } else { LINE }),
        );
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, text, font, TXT);
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Small square/round chain control used in the header and in cards.
pub fn chip_button(ui: &mut Ui, text: &str, armed: bool) -> Response {
    let font = micro_font();
    let galley = ui.painter().layout_no_wrap(
        tracked(text),
        font.clone(),
        if armed { VOID } else { DIM },
    );
    let size = galley.size() + Vec2::new(18.0, 12.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    let fill = if armed {
        a(WHITE, 235)
    } else {
        a(WHITE, (5.0 + t * 9.0) as u8)
    };
    ui.painter().rect_filled(rect, size.y * 0.5, fill);
    if !armed {
        ui.painter()
            .rect_stroke(rect, size.y * 0.5, Stroke::new(1.0_f32, LINE));
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        tracked(text),
        font,
        if armed { VOID } else { DIM },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Toggle switch: 40×22, animated knob.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 22.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let track = if *on {
        a(WHITE, 235)
    } else {
        a(WHITE, 10)
    };
    ui.painter().rect_filled(rect, 11.0, track);
    if !*on {
        ui.painter()
            .rect_stroke(rect, 11.0, Stroke::new(1.0_f32, LINE2));
    }
    let r = 8.0;
    let cx = lerp(rect.left() + r + 3.0, rect.right() - r - 3.0, t);
    let knob = if t > 0.5 { VOID } else { a(WHITE, 170) };
    ui.painter().circle_filled(Pos2::new(cx, rect.center().y), r, knob);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Horizontal slider, 0..1, drawn by hand so it stays hairline-thin.
pub fn slider(ui: &mut Ui, value: &mut f32, width: f32) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, 20.0), Sense::click_and_drag());
    let track = Rect::from_center_size(
        rect.center(),
        Vec2::new(rect.width(), 2.0),
    );
    ui.painter().rect_filled(track, 1.0, a(WHITE, 26));
    let filled = Rect::from_min_max(
        track.left_top(),
        Pos2::new(
            track.left() + track.width() * value.clamp(0.0, 1.0),
            track.bottom(),
        ),
    );
    ui.painter().rect_filled(filled, 1.0, a(WHITE, 210));
    let cx = track.left() + track.width() * value.clamp(0.0, 1.0);
    ui.painter()
        .circle_filled(Pos2::new(cx, rect.center().y), 5.0, WHITE);

    if resp.dragged() || resp.is_pointer_button_down_on() {
        if let Some(p) = resp.interact_pointer_pos() {
            let t = ((p.x - track.left()) / track.width()).clamp(0.0, 1.0);
            if (*value - t).abs() > f32::EPSILON {
                *value = t;
                resp.mark_changed();
            }
        }
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    resp
}

/// Animated readiness ring.
pub fn ring(ui: &mut Ui, pct: f32, size: f32, caption: &str) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size * 0.5 - 10.0;
    let p = ui.painter();
    p.circle_stroke(c, r, Stroke::new(1.0_f32, LINE));

    let sweep = (pct.clamp(0.0, 100.0) / 100.0) * std::f32::consts::TAU;
    let steps = 96;
    let mut prev: Option<Pos2> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        if t * std::f32::consts::TAU > sweep {
            break;
        }
        let ang = -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU;
        let pt = Pos2::new(c.x + r * ang.cos(), c.y + r * ang.sin());
        if let Some(q) = prev {
            p.line_segment([q, pt], Stroke::new(2.0_f32, a(WHITE, 230)));
        }
        prev = Some(pt);
    }
    p.text(
        c,
        Align2::CENTER_CENTER,
        format!("{:.0}", pct),
        num_font(size * 0.30),
        TXT,
    );
    let cap = Pos2::new(c.x, c.y + size * 0.15);
    p.text(cap, Align2::CENTER_CENTER, tracked(caption), micro_font(), FAINT);
}

/// Sparkline with a soft fill under the curve.
pub fn spark(ui: &mut Ui, values: &[f32], size: Vec2, max: f32) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, 0.0, a(WHITE, 3));
    if values.len() < 2 {
        return;
    }
    let max = if max <= 0.001 { 1.0 } else { max };
    let n = values.len();
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            Pos2::new(
                rect.left() + rect.width() * (i as f32 / (n - 1) as f32),
                rect.bottom() - rect.height() * (v / max).clamp(0.0, 1.0) * 0.92,
            )
        })
        .collect();

    let mut poly = pts.clone();
    poly.push(Pos2::new(rect.right(), rect.bottom()));
    poly.push(Pos2::new(rect.left(), rect.bottom()));
    ui.painter()
        .add(egui::Shape::convex_polygon(poly, a(WHITE, 10), Stroke::NONE));
    ui.painter()
        .add(egui::Shape::line(pts, Stroke::new(1.2_f32, a(WHITE, 205))));
}

/// Big number + caption block.
pub fn metric(ui: &mut Ui, label: &str, value: &str, unit: &str) {
    ui.vertical(|ui| {
        micro(ui, label);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(
                RichText::new(value).font(num_font(30.0)).color(TXT),
            ));
            if !unit.is_empty() {
                ui.add(egui::Label::new(
                    RichText::new(unit).font(label_font()).color(FAINT),
                ));
            }
        });
    });
}

/// Key/value hairline row, used across every info grid.
pub fn kv_row(ui: &mut Ui, key: &str, value: &str, accent: bool) {
    let h = 26.0;
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    ui.painter().text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        tracked(key),
        micro_font(),
        FAINT,
    );
    ui.painter().text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        mono_font(11.5),
        if accent { TXT } else { DIM },
    );
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text).font(num_font(26.0)).color(TXT),
    ));
}

pub fn sub(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text).font(body_font()).color(FAINT),
    ));
}
