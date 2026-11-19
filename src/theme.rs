// VOID — design system: MONOCHROME GLASS.
//
// The language, in one line: a single near-black surface carrying white light —
// glass cards with hairline rims, a drifting black/white band field underneath,
// water standing on the glass, toggles that spring, and a dot for a cursor.
//
// Rules this file obeys (docs/UI-REFERENCE.md):
//   1. one accent: white. colour only carries meaning (mint done / amber warn /
//      rose error), never decoration.
//   2. glass, never flat fills: translucent ink + 1 px white rim + top highlight.
//   3. answers are soft: hover = glow and a 1 px lift, never a colour swap.
//   4. motion arrives: eased, staggered, never popped.
//   5. instrumentation type: tracked mono micro-labels.
//
// Every pixel in the app comes from here, so the whole surface stays consistent.

use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{
    self, Align2, Color32, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke,
    TextStyle, Ui, Vec2,
};

// ── palette ──────────────────────────────────────────────────────────────────
// Two themes, one ramp. Obsidian is near-black ink on near-black paper;
// Porcelain is the same drawing inverted — same alphas, same hairlines, so every
// primitive in this file works in both without knowing which is on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Dark,
    Light,
}

static MODE: AtomicU8 = AtomicU8::new(0);

pub fn mode() -> Mode {
    if MODE.load(Ordering::Relaxed) == 1 {
        Mode::Light
    } else {
        Mode::Dark
    }
}

pub fn set_mode(m: Mode) {
    MODE.store(if m == Mode::Light { 1 } else { 0 }, Ordering::Relaxed);
}

pub fn light() -> bool {
    MODE.load(Ordering::Relaxed) == 1
}

fn pick(dark: (u8, u8, u8), pale: (u8, u8, u8)) -> Color32 {
    let (r, g, b) = if light() { pale } else { dark };
    Color32::from_rgb(r, g, b)
}

/// Window.
pub fn bg() -> Color32 {
    pick((8, 8, 10), (233, 233, 237))
}
/// Card. Kept opaque for anything that must sit on its own (popups, tooltips).
pub fn ink() -> Color32 {
    pick((11, 11, 13), (255, 255, 255))
}
/// Raised surface.
pub fn ink2() -> Color32 {
    pick((17, 17, 20), (226, 226, 231))
}
/// Primary text.
pub fn txt() -> Color32 {
    pick((241, 241, 241), (17, 17, 20))
}
/// Secondary text.
pub fn dim() -> Color32 {
    pick((150, 152, 160), (86, 86, 94))
}
/// Tertiary text.
pub fn faint() -> Color32 {
    pick((124, 125, 133), (118, 118, 126))
}
/// Quaternary text / micro-labels.
pub fn ghost() -> Color32 {
    pick((104, 106, 114), (150, 150, 158))
}
/// Rail surface. A plane just behind the deck, so navigation reads as its own layer.
pub fn rail_bg() -> Color32 {
    pick((6, 6, 8), (224, 224, 229))
}
/// Hairline.
pub fn line() -> Color32 {
    pick((30, 30, 35), (212, 212, 218))
}
/// Stronger divider / hover rim.
pub fn line2() -> Color32 {
    pick((58, 58, 66), (198, 198, 194))
}
/// The accent: pure white in Obsidian, near-black ink in Porcelain.
pub fn white() -> Color32 {
    pick((255, 255, 255), (14, 14, 16))
}

// ── meaning ──────────────────────────────────────────────────────────────────
/// Done.
pub fn mint() -> Color32 {
    Color32::from_rgb(110, 231, 183)
}
/// Careful.
pub fn amber() -> Color32 {
    Color32::from_rgb(255, 204, 107)
}
/// Broke.
pub fn rose() -> Color32 {
    Color32::from_rgb(255, 122, 148)
}

/// A glass surface: translucent ink that lets the band field underneath live.
pub fn glass(alpha: u8) -> Color32 {
    if light() {
        Color32::from_rgba_unmultiplied(252, 252, 253, alpha)
    } else {
        Color32::from_rgba_unmultiplied(8, 8, 10, alpha)
    }
}

/// The five faces VOID is set in: IBM Plex Sans Light / Regular / Medium and
/// IBM Plex Mono Light / Regular. Subsets live in `assets/fonts`.
pub const FACE_DISPLAY: &str = "plex.display";
pub const FACE_TEXT: &str = "plex.text";
pub const FACE_LABEL: &str = "plex.label";
pub const FACE_MONO: &str = "plex.mono";

fn face(name: &str) -> FontFamily {
    FontFamily::Name(name.into())
}

pub fn a(color: Color32, alpha: u8) -> Color32 {
    if alpha >= 200 {
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
    } else {
        over(color, alpha)
    }
}

/// egui composites in linear light, so a veil authored the way a designer says it
/// ("10 % white") lands roughly three times too bright: 26/255 of white over black
/// displays as 35 % grey, not 10 %. This maps a *displayed* alpha to the linear
/// alpha that produces it. Everything subtle in this file is authored this way.
pub fn veil(srgb_alpha: u8) -> Color32 {
    over(Color32::WHITE, srgb_alpha)
}

/// `veil` for any colour.
pub fn over(color: Color32, srgb_alpha: u8) -> Color32 {
    let s = (srgb_alpha as f32 / 255.0).clamp(0.0, 1.0);
    let alpha = if srgb_alpha >= 200 {
        srgb_alpha as f32
    } else {
        s.powf(2.2) * 255.0
    };
    Color32::from_rgba_unmultiplied(
        color.r(),
        color.g(),
        color.b(),
        alpha.round().clamp(0.0, 255.0) as u8,
    )
}

/// Shorten a machine string so it can never blow up a fixed column.
pub fn clip(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        out.push('\u{2026}');
    }
    out
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// cubic-bezier(.16,1,.3,1) — the entrance curve.
pub fn ease(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

// ── type ─────────────────────────────────────────────────────────────────────
/// Tracked micro-label (plex medium, uppercase, letterspaced by `tracked`).
pub fn micro_font() -> FontId {
    FontId::new(9.5, face(FACE_LABEL))
}
/// Interface text.
pub fn label_font() -> FontId {
    FontId::new(12.5, face(FACE_LABEL))
}
/// Reading text.
pub fn body_font() -> FontId {
    FontId::new(13.5, face(FACE_TEXT))
}
/// Big numbers — Plex Sans Light at display sizes.
pub fn num_font(size: f32) -> FontId {
    FontId::new(size, face(FACE_DISPLAY))
}
/// Tabular data.
pub fn mono_font(size: f32) -> FontId {
    FontId::new(size, face(FACE_MONO))
}
/// Wordmark / section titles.
pub fn title_font(size: f32) -> FontId {
    FontId::new(size, face(FACE_LABEL))
}

/// "SETTINGS" → "S E T T I N G S" — the tracking that gives the app its calm.
pub fn tracked(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for (i, ch) in s.chars().enumerate() {
        if ch == ' ' {
            out.push_str("  ");
            continue;
        }
        if i > 0 {
            out.push(' ');
        }
        out.extend(ch.to_uppercase());
    }
    out
}

// ── style install ────────────────────────────────────────────────────────────
fn register_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, bytes: &'static [u8]| {
        fonts
            .font_data
            .insert(name.to_owned(), egui::FontData::from_static(bytes).into());
    };
    add(FACE_DISPLAY, include_bytes!("../assets/fonts/plex-sans-light.ttf"));
    add(FACE_TEXT, include_bytes!("../assets/fonts/plex-sans-regular.ttf"));
    add(FACE_LABEL, include_bytes!("../assets/fonts/plex-sans-medium.ttf"));
    add(FACE_MONO, include_bytes!("../assets/fonts/plex-mono-regular.ttf"));
    add(
        "plex.mono.light",
        include_bytes!("../assets/fonts/plex-mono-light.ttf"),
    );

    let family = |list: Vec<&str>| -> Vec<String> { list.into_iter().map(|s| s.to_string()).collect() };
    fonts.families.insert(
        face(FACE_DISPLAY),
        family(vec![FACE_DISPLAY, FACE_TEXT, "NotoEmoji-Regular"]),
    );
    fonts.families.insert(
        face(FACE_TEXT),
        family(vec![FACE_TEXT, FACE_DISPLAY, "NotoEmoji-Regular"]),
    );
    fonts.families.insert(
        face(FACE_LABEL),
        family(vec![FACE_LABEL, FACE_TEXT, "NotoEmoji-Regular"]),
    );
    fonts.families.insert(
        face(FACE_MONO),
        family(vec![FACE_MONO, "plex.mono.light", FACE_TEXT]),
    );

    // Default families point at Plex too, so any stray widget still matches.
    if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
        list.insert(0, FACE_TEXT.to_string());
        list.insert(0, FACE_DISPLAY.to_string());
    }
    if let Some(list) = fonts.families.get_mut(&FontFamily::Monospace) {
        list.insert(0, FACE_MONO.to_string());
    }
    ctx.set_fonts(fonts);
}

pub fn install(ctx: &egui::Context) {
    register_fonts(ctx);
    restyle(ctx);
}

/// Re-derive the widget style from the active palette. Cheap; used when the
/// theme flips so native widgets (scrollbars, text edits) invert with the rest.
pub fn restyle(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (TextStyle::Heading, title_font(21.0)),
        (TextStyle::Body, body_font()),
        (TextStyle::Button, label_font()),
        (TextStyle::Small, micro_font()),
        (TextStyle::Monospace, mono_font(11.5)),
    ]
    .into();

    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.interact_size.y = 26.0;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = true;

    let mut v = if light() {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.override_text_color = Some(dim());
    v.panel_fill = bg();
    v.window_fill = ink();
    v.extreme_bg_color = ink();
    v.faint_bg_color = ink2();
    v.window_rounding = 24.0.into();
    v.menu_rounding = 16.0.into();
    v.selection.bg_fill = a(white(), 40);
    v.selection.stroke = Stroke::new(1.0_f32, txt());
    v.widgets.noninteractive.bg_fill = ink();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, line());
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, dim());
    v.widgets.noninteractive.rounding = 12.0.into();
    v.widgets.inactive.bg_fill = a(white(), 8);
    v.widgets.inactive.weak_bg_fill = a(white(), 8);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, a(white(), 26));
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, txt());
    v.widgets.inactive.rounding = 12.0.into();
    v.widgets.hovered.bg_fill = a(white(), 16);
    v.widgets.hovered.weak_bg_fill = a(white(), 16);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, a(white(), 70));
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, white());
    v.widgets.hovered.rounding = 12.0.into();
    v.widgets.active.bg_fill = a(white(), 22);
    v.widgets.active.weak_bg_fill = a(white(), 22);
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, white());
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, white());
    v.widgets.active.rounding = 12.0.into();
    style.visuals = v;

    ctx.set_style(style);
}

// ── primitives ───────────────────────────────────────────────────────────────
pub fn micro(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(tracked(text)).font(micro_font()).color(faint()),
    ));
}

pub fn hairline(ui: &mut Ui) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, line());
}

pub fn split_hairline(ui: &mut Ui, inset: f32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 1.0), Sense::hover());
    let r = Rect::from_min_max(
        Pos2::new(rect.left() + inset, rect.top()),
        Pos2::new(rect.right(), rect.bottom()),
    );
    ui.painter().rect_filled(r, 0.0, line());
}

/// The glass recipe: translucent ink, 1 px white rim, 1 px top highlight.
pub fn glass_frame(pad: f32, rounding: f32) -> egui::Frame {
    egui::Frame::none()
        .fill(glass(if light() { 224 } else { 222 }))
        .stroke(Stroke::new(1.0_f32, rim()))
        .rounding(rounding)
        .inner_margin(egui::Margin::same(pad))
}

fn top_highlight(p: &egui::Painter, r: Rect, rounding: f32) {
    p.line_segment(
        [
            Pos2::new(r.left() + rounding, r.top() + 0.5),
            Pos2::new(r.right() - rounding, r.top() + 0.5),
        ],
        Stroke::new(
            1.0_f32,
            if light() {
                a(Color32::BLACK, 26)
            } else {
                a(Color32::WHITE, 54)
            },
        ),
    );
}

fn spotlight(ui: &Ui, r: Rect, rounding: f32) {
    let Some(pos) = ui.ctx().pointer_latest_pos() else {
        return;
    };
    if !r.contains(pos) {
        return;
    }
    let p = ui.painter().with_clip_rect(r);
    for (rad, alpha) in [(170.0_f32, 5u8), (110.0, 5), (56.0, 6)] {
        p.circle_filled(pos, rad, a(white(), alpha));
    }
    let _ = rounding;
}

/// A card: glass, hairline rim, top highlight, and a spotlight under the pointer.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_styled(ui, 18.0, 16.0, add)
}

pub fn card_styled<R>(
    ui: &mut Ui,
    pad: f32,
    rounding: f32,
    add: impl FnOnce(&mut Ui) -> R,
) -> R {
    let out = glass_frame(pad, rounding).show(ui, add);
    let r = out.response.rect;
    top_highlight(ui.painter(), r, rounding);
    // hover: the rim brightens — the whole answer, no colour change
    let t = ui.ctx().animate_bool(out.response.id, out.response.hovered());
    if t > 0.01 {
        let hot = if light() {
            a(ink_light(), (96.0 + 70.0 * t) as u8)
        } else {
            a(white(), (34.0 + 40.0 * t) as u8)
        };
        ui.painter()
            .rect_stroke(r, rounding, Stroke::new(1.0_f32, hot));
    }
    spotlight(ui, r, rounding);
    out.inner
}

/// Card that lifts its rim when the pointer is inside it.
pub fn hot_card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_styled(ui, 18.0, 18.0, add)
}

/// A round glow behind a shape: three expanding strokes, alphas falling off.
fn glow(p: &egui::Painter, rect: Rect, rounding: f32, color: Color32, strength: f32) {
    if strength <= 0.01 {
        return;
    }
    for (grow, alpha) in [(1.0_f32, 34.0_f32), (3.0, 16.0), (6.0, 8.0)] {
        let r = rect.expand(grow);
        p.rect_stroke(
            r,
            rounding + grow,
            Stroke::new(1.2_f32, a(color, (alpha * strength) as u8)),
        );
    }
}

pub fn pill(ui: &mut Ui, text: &str, lit: bool) {
    let font = micro_font();
    let galley = ui.painter().layout_no_wrap(tracked(text), font.clone(), txt());
    let pad = Vec2::new(9.0, 5.0);
    let size = galley.size() + pad * 2.0;
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    if lit {
        ui.painter().rect_filled(rect, size.y * 0.5, a(white(), 238));
        glow(ui.painter(), rect, size.y * 0.5, white(), 0.5);
    } else {
        ui.painter().rect_filled(rect, size.y * 0.5, a(white(), 8));
        ui.painter()
            .rect_stroke(rect, size.y * 0.5, Stroke::new(1.0_f32, a(white(), 26)));
    }
    let fg = if lit { bg() } else { dim() };
    ui.painter()
        .text(rect.center(), Align2::CENTER_CENTER, tracked(text), font, fg);
}

/// Primary / ghost button. Primary is a white slab that glows and shimmers;
/// ghost is glass that brightens. Both lift a pixel on hover.
pub fn button(ui: &mut Ui, text: &str, primary: bool) -> Response {
    let font = label_font();
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), txt());
    let h = 34.0;
    let w = galley.size().x + 40.0;
    let (slot, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::click());
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    let lift = 1.0 * t;
    let rect = slot.translate(Vec2::new(0.0, -lift));

    if primary {
        let base = a(white(), (236.0 + 19.0 * t) as u8);
        ui.painter().rect_filled(rect, 12.0, base);
        glow(ui.painter(), rect, 12.0, white(), t * 0.9);
        // light sweep across the face while hovered
        if t > 0.02 {
            let time = ui.input(|i| i.time) as f32;
            let phase = (time * 0.55) % 1.0;
            let band = 46.0;
            let cx = rect.left() - band + (rect.width() + band * 2.0) * phase;
            let clip = ui.painter().with_clip_rect(rect.shrink(2.0));
            let pts = vec![
                Pos2::new(cx, rect.top()),
                Pos2::new(cx + band, rect.top()),
                Pos2::new(cx + band - 18.0, rect.bottom()),
                Pos2::new(cx - 18.0, rect.bottom()),
            ];
            clip.add(egui::Shape::convex_polygon(
                pts,
                a(Color32::WHITE, (22.0 * t) as u8),
                Stroke::NONE,
            ));
            ui.ctx().request_repaint();
        }
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            text,
            font,
            if light() { Color32::WHITE } else { bg() },
        );
    } else {
        ui.painter()
            .rect_filled(rect, 12.0, a(white(), (8.0 + 9.0 * t) as u8));
        ui.painter().rect_stroke(
            rect,
            12.0,
            Stroke::new(1.0_f32, a(white(), (26.0 + 44.0 * t) as u8)),
        );
        glow(ui.painter(), rect, 12.0, white(), t * 0.45);
        ui.painter()
            .text(rect.center(), Align2::CENTER_CENTER, text, font, txt());
    }

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Small pill control used in headers and cards.
pub fn chip_button(ui: &mut Ui, text: &str, armed: bool) -> Response {
    let font = micro_font();
    let galley = ui.painter().layout_no_wrap(
        tracked(text),
        font.clone(),
        if armed { bg() } else { dim() },
    );
    let size = galley.size() + Vec2::new(20.0, 13.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    if armed {
        ui.painter().rect_filled(rect, size.y * 0.5, a(white(), 238));
        glow(ui.painter(), rect, size.y * 0.5, white(), 0.45);
    } else {
        ui.painter()
            .rect_filled(rect, size.y * 0.5, a(white(), (5.0 + 10.0 * t) as u8));
        ui.painter().rect_stroke(
            rect,
            size.y * 0.5,
            Stroke::new(1.0_f32, a(white(), (22.0 + 40.0 * t) as u8)),
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        tracked(text),
        font,
        if armed { bg() } else { dim() },
    );
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Segmented control (the sliding indicator from the kit). Returns the clicked
/// index, if any.
pub fn segmented(ui: &mut Ui, items: &[&str], sel: usize, width: f32) -> Option<usize> {
    let h = 32.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, h), Sense::hover());
    ui.painter().rect_filled(rect, 14.0, a(white(), 7));
    ui.painter()
        .rect_stroke(rect, 14.0, Stroke::new(1.0_f32, a(white(), 22)));

    let inner = rect.shrink(4.0);
    let seg_w = inner.width() / items.len().max(1) as f32;
    let target = inner.left() + seg_w * sel as f32;
    let x = ui
        .ctx()
        .animate_value_with_time(ui.id().with("seg-x"), target, 0.16);
    let ind = Rect::from_min_size(Pos2::new(x, inner.top()), Vec2::new(seg_w, inner.height()));
    // On porcelain the indicator is ink and the label goes white; on obsidian the
    // indicator is a thin white veil and the label is already white.
    if light() {
        ui.painter().rect_filled(ind, 10.0, a(ink_light(), 232));
        ui.painter().rect_stroke(
            ind,
            10.0,
            Stroke::new(1.0_f32, a(ink_light(), 250)),
        );
    } else {
        ui.painter().rect_filled(ind, 10.0, a(white(), 26));
        ui.painter()
            .rect_stroke(ind, 10.0, Stroke::new(1.0_f32, a(white(), 46)));
    }

    let mut hit = None;
    for (i, label) in items.iter().enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(inner.left() + seg_w * i as f32, inner.top()),
            Vec2::new(seg_w, inner.height()),
        );
        let r = ui.interact(cell, ui.id().with(("seg", i)), Sense::click());
        if r.clicked() {
            hit = Some(i);
        }
        let on = i == sel;
        let alpha = if on {
            255
        } else if r.hovered() {
            210
        } else {
            130
        };
        let fg = if on {
            // the indicator is a slab in both modes, so the label is always white
            Color32::WHITE
        } else {
            a(if light() { txt() } else { Color32::WHITE }, alpha)
        };
        ui.painter().text(
            cell.center(),
            Align2::CENTER_CENTER,
            tracked(label),
            micro_font(),
            fg,
        );
        if r.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }
    hit
}

/// Toggle switch: 36×20, light track, ink knob, spring travel.
pub fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let scale = ui.ctx().pixels_per_point().clamp(1.0, 2.0);
    let (slot, mut resp) = ui.allocate_exact_size(Vec2::new(40.0, 26.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool(resp.id, *on);
    let pressed = resp.is_pointer_button_down_on();
    let track_w = 36.0;
    let track = Rect::from_center_size(
        Pos2::new(slot.center().x - 2.0, slot.center().y),
        Vec2::new(track_w, 20.0),
    );

    let hover = ui.ctx().animate_bool(resp.id.with("h"), resp.hovered());
    if *on {
        let slab = if light() {
            a(ink_light(), 238)
        } else {
            a(Color32::from_rgb(238, 239, 243), 242)
        };
        ui.painter().rect_filled(track, 10.0, slab);
        glow(ui.painter(), track, 10.0, white(), 0.35 + 0.25 * hover);
    } else {
        ui.painter()
            .rect_filled(track, 10.0, a(white(), (13.0 + 9.0 * hover) as u8));
        ui.painter()
            .rect_stroke(track, 10.0, Stroke::new(1.0_f32, rim()));
    }

    let knob_w = if pressed { 18.0 } else { 14.0 };
    let cx = lerp(track.left() + 3.0 + 7.0, track.right() - 3.0 - 7.0, t);
    let knob = if t > 0.5 {
        if light() {
            Color32::WHITE
        } else {
            Color32::from_rgb(14, 14, 17)
        }
    } else {
        a(ink_light(), (150.0 + 60.0 * hover) as u8)
    };
    let knob_rect = Rect::from_center_size(
        Pos2::new(cx, track.center().y),
        Vec2::new(knob_w, 14.0),
    );
    ui.painter().rect_filled(knob_rect, 7.0, knob);
    if !*on && !light() {
        ui.painter().circle_filled(
            Pos2::new(knob_rect.center().x, knob_rect.top() + 4.0),
            2.0,
            a(Color32::WHITE, 40),
        );
    }
    let _ = scale;

    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp
}

/// Horizontal slider, 0..1: 5 px filled track and a glowing white thumb.
pub fn slider(ui: &mut Ui, value: &mut f32, width: f32) -> Response {
    let (slot, mut resp) = ui.allocate_exact_size(Vec2::new(width, 22.0), Sense::click_and_drag());
    let track = Rect::from_center_size(slot.center(), Vec2::new(slot.width(), 5.0));
    let hover = ui.ctx().animate_bool(resp.id, resp.hovered() || resp.dragged());

    ui.painter().rect_filled(track, 2.5, a(white(), 28));
    let v = value.clamp(0.0, 1.0);
    let filled = Rect::from_min_max(
        track.left_top(),
        Pos2::new(track.left() + track.width() * v, track.bottom()),
    );
    ui.painter().rect_filled(filled, 2.5, a(white(), 232));

    let cx = track.left() + track.width() * v;
    let c = Pos2::new(cx, slot.center().y);
    // glow: concentric rings, brightest against the thumb
    for (rad, alpha) in [(14.0_f32, 10.0_f32), (11.0, 16.0), (8.5, 30.0)] {
        ui.painter()
            .circle_filled(c, rad, a(white(), (alpha * (0.55 + 0.45 * hover)) as u8));
    }
    ui.painter().circle_filled(c, 7.0, a(white(), 248));
    ui.painter()
        .circle_stroke(c, 7.0, Stroke::new(1.0_f32, a(Color32::BLACK, 26)));

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
        ui.ctx().request_repaint();
    }
    resp
}

// ── the signatures ───────────────────────────────────────────────────────────
/// The band field: a 760 px black↔white gradient sheared −32°, drifting left
/// forever. Painted behind everything, never over text.
pub fn bands(p: &egui::Painter, rect: Rect, now: f64) {
    bands_scaled(p, rect, now, 1.0);
}

pub fn bands_scaled(p: &egui::Painter, rect: Rect, now: f64, scale: f32) {
    if rect.width() < 8.0 || rect.height() < 8.0 {
        return;
    }
    // Park the noise outside the deck, then only paint what can be seen.
    let p = p.with_clip_rect(rect);
    let period = 760.0_f32;
    let shear = rect.height() * 0.62;
    let step = 5.0_f32;
    let drift = ((now as f32) * 16.0) % period;
    let top_alpha = if light() { 34.0 } else { 42.0 } * scale;
    let ink_col = if light() { Color32::BLACK } else { Color32::WHITE };

    let mut x = -shear - step;
    while x < rect.width() + step {
        let u = ((x + drift) % period + period) % period / period;
        let v = band_at(u);
        if v > 0.01 {
            let alpha = (v.powf(1.45) * top_alpha) as u8;
            if alpha > 1 {
                let x0 = rect.left() + x;
                let x1 = x0 + step + 0.6;
                let pts = vec![
                    Pos2::new(x0, rect.top()),
                    Pos2::new(x1, rect.top()),
                    Pos2::new(x1 + shear, rect.bottom()),
                    Pos2::new(x0 + shear, rect.bottom()),
                ];
                p.add(egui::Shape::convex_polygon(
                    pts,
                    a(ink_col, alpha),
                    Stroke::NONE,
                ));
            }
        }
        x += step;
    }
}

/// The 0..1 luminance of the band pattern at `u` (0..1 across one period).
fn band_at(u: f32) -> f32 {
    const STOPS: [(f32, f32); 17] = [
        (0.00, 0.03),
        (0.14, 0.04),
        (0.22, 0.12),
        (0.28, 0.54),
        (0.33, 0.95),
        (0.37, 1.00),
        (0.42, 1.00),
        (0.48, 0.77),
        (0.53, 0.35),
        (0.60, 0.05),
        (0.68, 0.03),
        (0.72, 0.17),
        (0.76, 0.86),
        (0.785, 1.00),
        (0.82, 0.61),
        (0.87, 0.10),
        (0.94, 0.03),
    ];
    let u = u.clamp(0.0, 1.0);
    for w in STOPS.windows(2) {
        let (u0, v0) = w[0];
        let (u1, v1) = w[1];
        if u >= u0 && u <= u1 {
            let t = if (u1 - u0).abs() < 1e-6 {
                0.0
            } else {
                (u - u0) / (u1 - u0)
            };
            return v0 + (v1 - v0) * t;
        }
    }
    STOPS[STOPS.len() - 1].1
}

/// Water standing on the glass: a lens sprite per drop — contact shadow,
/// refracted top, bright caustic at the bottom, one specular highlight — and a
/// few drops that slide, leaving a shrinking trail. Deterministic, no state.
pub fn droplets(p: &egui::Painter, rect: Rect, now: f64, density: f32) {
    if rect.width() < 40.0 || rect.height() < 40.0 || density <= 0.0 {
        return;
    }
    let p = p.with_clip_rect(rect);
    let area = rect.width() * rect.height();
    let count = ((area / 5200.0) * density).clamp(28.0, 260.0) as usize;
    let t = now as f32;
    let col = if light() { Color32::BLACK } else { Color32::WHITE };

    for i in 0..count {
        let s = i as f32;
        let h1 = hash(s * 1.37 + 11.0);
        let h2 = hash(s * 2.71 + 5.0);
        let h3 = hash(s * 0.53 + 23.0);
        let r = lerp(0.9, 3.4, h3 * h3);
        let base_x = rect.left() + h1 * rect.width();
        let slide = h2 > 0.86;
        let y = if slide {
            let speed = lerp(16.0, 52.0, h3);
            let span = rect.height() + 90.0;
            let travel = (h2 * span + t * speed) % span;
            rect.top() - 40.0 + travel
        } else {
            rect.top() + h2 * rect.height()
        };
        let x = base_x + if slide {
            (t * 0.6 + h1 * 7.0).sin() * 3.0
        } else {
            0.0
        };
        if y < rect.top() - 6.0 || y > rect.bottom() + 6.0 {
            continue;
        }
        let pulse = if slide {
            1.0
        } else {
            0.82 + 0.18 * ((t * 0.7 + h1 * 6.28).sin() * 0.5 + 0.5)
        };
        lens(&p, Pos2::new(x, y), r, col, pulse);
        if slide {
            // the trail: three shrinking ghosts behind the drop
            for k in 1..4 {
                let frac = k as f32 / 4.0;
                lens(
                    &p,
                    Pos2::new(x - 0.6 * k as f32, y - r * (1.6 + 2.1 * frac)),
                    r * (0.55 - 0.10 * k as f32).max(0.3),
                    col,
                    0.45 - 0.08 * k as f32,
                );
            }
        }
    }
}

fn lens(p: &egui::Painter, c: Pos2, r: f32, col: Color32, k: f32) {
    if r < 0.3 {
        return;
    }
    // contact shadow, offset like a real drop standing off the surface
    p.circle_filled(
        Pos2::new(c.x + r * 0.16, c.y + r * 0.26),
        r * 1.08,
        a(Color32::BLACK, (16.0 * k) as u8),
    );
    // body: a lens, so the middle is clearer than the rim
    p.circle_filled(c, r * 0.98, a(col, (10.0 * k) as u8));
    p.circle_filled(c, r * 0.72, a(col, (8.0 * k) as u8));
    // refracted dark on top, caustic light at the bottom
    p.circle_filled(
        Pos2::new(c.x, c.y - r * 0.34),
        r * 0.52,
        a(Color32::BLACK, (10.0 * k) as u8),
    );
    p.circle_filled(
        Pos2::new(c.x, c.y + r * 0.42),
        r * 0.40,
        a(col, (26.0 * k) as u8),
    );
    p.circle_stroke(c, r * 0.98, Stroke::new(1.0_f32, a(col, (26.0 * k) as u8)));
    // specular
    p.circle_filled(
        Pos2::new(c.x - r * 0.32, c.y - r * 0.38),
        (r * 0.17).max(0.6),
        a(Color32::WHITE, (150.0 * k) as u8),
    );
}

fn hash(n: f32) -> f32 {
    let x = (n * 12.9898).sin() * 43758.5453;
    x.fract().abs()
}

/// The brand mark, drawn as vectors: a hexagon with the three crossing cuts.
pub fn mark(p: &egui::Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    let pts: Vec<Pos2> = (0..6)
        .map(|i| {
            let ang = -std::f32::consts::FRAC_PI_2
                + i as f32 * std::f32::consts::TAU / 6.0;
            Pos2::new(c.x + r * ang.cos(), c.y + r * ang.sin())
        })
        .collect();
    let mut outline = pts.clone();
    outline.push(pts[0]);
    p.add(egui::Shape::line(outline, Stroke::new(1.6_f32, color)));
    let faint = a(color, 110);
    for (i, j) in [(0usize, 3usize), (1, 4), (2, 5)] {
        p.line_segment([pts[i], pts[j]], Stroke::new(1.0_f32, faint));
    }
}

// ── data display ─────────────────────────────────────────────────────────────
/// Animated readiness ring, mint once it is actually done.
pub fn ring(ui: &mut Ui, pct: f32, size: f32, caption: &str) {
    ring_sweep(ui, pct, size, caption, 1.0)
}

/// Ring plus an "ignite" scanline: `sweep` runs 0 -> 1 while a job is applying.
pub fn ring_sweep(ui: &mut Ui, pct: f32, size: f32, caption: &str, sweep: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let c = rect.center();
    let r = size * 0.5 - 10.0;
    let p = ui.painter();
    p.circle_stroke(c, r, Stroke::new(1.0_f32, a(white(), 30)));
    p.circle_filled(c, r - 1.0, a(white(), if light() { 10 } else { 5 }));

    let done = pct >= 99.9;
    let arc_col = if done { mint() } else { a(white(), 235) };
    let arc = (pct.clamp(0.0, 100.0) / 100.0) * std::f32::consts::TAU;
    let steps = 96;
    let mut prev: Option<Pos2> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        if t * std::f32::consts::TAU > arc {
            break;
        }
        let ang = -std::f32::consts::FRAC_PI_2 + t * std::f32::consts::TAU;
        let pt = Pos2::new(c.x + r * ang.cos(), c.y + r * ang.sin());
        if let Some(q) = prev {
            p.line_segment([q, pt], Stroke::new(2.0_f32, arc_col));
        }
        prev = Some(pt);
    }
    if done {
        // a completed ring carries its own light
        for (grow, alpha) in [(2.0_f32, 26u8), (5.0, 12)] {
            p.circle_stroke(c, r + grow, Stroke::new(1.0_f32, a(mint(), alpha)));
        }
    }
    p.text(
        c,
        Align2::CENTER_CENTER,
        format!("{:.0}", pct),
        num_font(size * 0.30),
        txt(),
    );
    let cap = Pos2::new(c.x, c.y + size * 0.15);
    p.text(cap, Align2::CENTER_CENTER, tracked(caption), micro_font(), faint());

    if sweep < 1.0 {
        let y = rect.top() + 8.0 + (size - 16.0) * sweep;
        let fade = (1.0 - sweep) * 200.0;
        p.line_segment(
            [
                Pos2::new(rect.left() + 6.0, y),
                Pos2::new(rect.right() - 6.0, y),
            ],
            Stroke::new(1.0_f32, a(white(), fade as u8)),
        );
        p.circle_filled(
            Pos2::new(rect.right() - 6.0, y),
            2.0,
            a(white(), (fade + 40.0).min(255.0) as u8),
        );
    }
}

/// Sparkline with a soft fill under the curve.
pub fn spark(ui: &mut Ui, values: &[f32], size: Vec2, max: f32) {
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
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

    // A flat (or near-flat) series would tessellate into a degenerate polygon,
    // which epaint fills as a solid block — so only fill when there is a shape.
    let top = pts.iter().fold(f32::INFINITY, |m, p| m.min(p.y));
    let bottom = pts.iter().fold(f32::NEG_INFINITY, |m, p| m.max(p.y));
    if bottom - top > 2.0 {
        let mut poly = pts.clone();
        poly.push(Pos2::new(rect.right(), rect.bottom()));
        poly.push(Pos2::new(rect.left(), rect.bottom()));
        ui.painter()
            .add(egui::Shape::convex_polygon(poly, a(white(), 10), Stroke::NONE));
    }
    ui.painter()
        .add(egui::Shape::line(pts, Stroke::new(1.4_f32, a(white(), 220))));
}

/// Big number + caption block.
pub fn metric(ui: &mut Ui, label: &str, value: &str, unit: &str) {
    ui.vertical(|ui| {
        micro(ui, label);
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.add(egui::Label::new(
                RichText::new(value).font(num_font(30.0)).color(txt()),
            ));
            if !unit.is_empty() {
                ui.add(egui::Label::new(
                    RichText::new(unit).font(label_font()).color(faint()),
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
        faint(),
    );
    ui.painter().text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        value,
        mono_font(11.5),
        if accent { txt() } else { dim() },
    );
}

pub fn heading(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text).font(num_font(26.0)).color(txt()),
    ));
}

pub fn sub(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(
        RichText::new(text).font(body_font()).color(faint()),
    ));
}

// ── the cursor ───────────────────────────────────────────────────────────────
/// A white dot with a ring that lags behind. The OS cursor is hidden by the
/// app while this is on, so the pointer you see is the one VOID draws.
pub fn cursor_overlay(ctx: &egui::Context, now: f64) {
    let Some(pos) = ctx.pointer_latest_pos() else {
        return;
    };
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("void.cursor"));
    let p = ctx.layer_painter(layer);
    let down = ctx.input(|i| i.pointer.any_down());
    let t = now as f32;
    // the ring breathes while the button is held, and settles when released
    let r = if down { 11.0 } else { 17.0 + (t * 2.2).sin() * 0.6 };
    let halo = if down { 40.0 } else { 18.0 };

    let ink = ink_light();
    p.circle_filled(pos, r, over(ink, halo as u8));
    p.circle_stroke(pos, r, Stroke::new(1.5_f32, over(ink, 232)));
    p.circle_filled(pos, 3.0, ink);
}

/// Draw a rounded glass tile with the brand mark inside — the app icon, drawn.
pub fn brand_tile(p: &egui::Painter, rect: Rect, strength: f32) {
    let ink = ink_light();
    p.rect_filled(rect, 10.0, over(ink, 10));
    p.rect_stroke(rect, 10.0, Stroke::new(1.0_f32, over(ink, 34)));
    p.line_segment(
        [
            Pos2::new(rect.left() + 10.0, rect.top() + 0.6),
            Pos2::new(rect.right() - 10.0, rect.top() + 0.6),
        ],
        Stroke::new(1.0_f32, over(ink, 30)),
    );
    mark(p, rect.shrink(rect.width() * 0.24), over(ink, 236));
    if strength > 0.01 {
        glow(p, rect, 10.0, ink, strength);
    }
}

/// A 1 px contour: white in Obsidian, ink in Porcelain.
pub fn rim() -> Color32 {
    if light() {
        a(ink_light(), 96)
    } else {
        a(white(), 34)
    }
}

/// The light that draws *on* the surface: white in Obsidian, ink in Porcelain.
pub fn ink_light() -> Color32 {
    if light() {
        Color32::from_rgb(20, 20, 24)
    } else {
        Color32::WHITE
    }
}

// ── instruments ──────────────────────────────────────────────────────────────
// The reference for this block is a telemetry deck: one instrument per
// resource, each with its own hue, a big reading, and a live area chart that
// glows against the glass. The hue *is* the meaning — red is the CPU, blue is
// memory, violet is the volume, green is the wire — so the monochrome rule
// still holds everywhere else on the page.

pub fn hue_cpu() -> Color32 {
    pick((255, 92, 92), (206, 54, 54))
}
pub fn hue_mem() -> Color32 {
    pick((86, 141, 255), (46, 100, 206))
}
pub fn hue_disk() -> Color32 {
    pick((168, 122, 255), (122, 74, 206))
}
pub fn hue_net() -> Color32 {
    pick((62, 219, 132), (20, 148, 88))
}

/// The instrument surface: a hair more opaque than a glass card, so a chart
/// never has to compete with the band field behind it.
pub fn surface(ui: &Ui, r: Rect, rounding: f32) {
    let fill = if light() {
        Color32::from_rgba_unmultiplied(255, 255, 255, 240)
    } else {
        Color32::from_rgba_unmultiplied(12, 12, 15, 240)
    };
    ui.painter().rect_filled(r, rounding, fill);
    ui.painter()
        .rect_stroke(r, rounding, Stroke::new(1.0_f32, rim()));
    top_highlight(ui.painter(), r, rounding);
    spotlight(ui, r, rounding);
}

/// A one-stroke glyph, drawn from vectors — no icon font, no bitmaps, and it
/// inherits whatever colour the caller is using.
pub fn glyph(p: &egui::Painter, c: Pos2, kind: &str, color: Color32, s: f32) {
    let w = Stroke::new(1.3_f32, color);
    let box_ = |x: f32, y: f32, ww: f32, hh: f32| {
        Rect::from_center_size(Pos2::new(c.x + x * s, c.y + y * s), Vec2::new(ww * s, hh * s))
    };
    match kind {
        "cpu" => {
            p.rect_stroke(box_(0.0, 0.0, 9.0, 9.0), 2.0, w);
            p.rect_filled(box_(0.0, 0.0, 3.0, 3.0), 0.8, color);
            for k in [-3.0_f32, 0.0, 3.0] {
                p.line_segment(
                    [
                        Pos2::new(c.x + k * s, c.y - 4.5 * s),
                        Pos2::new(c.x + k * s, c.y - 6.5 * s),
                    ],
                    w,
                );
                p.line_segment(
                    [
                        Pos2::new(c.x + k * s, c.y + 4.5 * s),
                        Pos2::new(c.x + k * s, c.y + 6.5 * s),
                    ],
                    w,
                );
            }
        }
        "mem" => {
            for (i, dx) in [-4.0_f32, 0.0, 4.0].iter().enumerate() {
                let h = [4.0_f32, 8.0, 6.0][i];
                p.line_segment(
                    [
                        Pos2::new(c.x + dx * s, c.y + 5.0 * s),
                        Pos2::new(c.x + dx * s, c.y + (5.0 - h) * s),
                    ],
                    w,
                );
            }
            p.line_segment(
                [
                    Pos2::new(c.x - 6.0 * s, c.y + 6.0 * s),
                    Pos2::new(c.x + 6.0 * s, c.y + 6.0 * s),
                ],
                w,
            );
        }
        "disk" => {
            p.rect_stroke(box_(0.0, 0.0, 12.0, 9.0), 2.5, w);
            p.line_segment(
                [
                    Pos2::new(c.x - 5.0 * s, c.y + 2.0 * s),
                    Pos2::new(c.x + 5.0 * s, c.y + 2.0 * s),
                ],
                w,
            );
            p.circle_filled(Pos2::new(c.x + 3.5 * s, c.y + 3.5 * s), 0.9 * s, color);
        }
        "net" => {
            for (i, r) in [2.5_f32, 5.0, 7.5].iter().enumerate() {
                let alpha = if i == 0 { 255u8 } else { 185u8 };
                p.circle_stroke(
                    Pos2::new(c.x, c.y + 4.0 * s),
                    r * s,
                    Stroke::new(1.2_f32, a(color, alpha)),
                );
            }
            p.circle_filled(Pos2::new(c.x, c.y + 4.0 * s), 1.2 * s, color);
        }
        "speed" => {
            p.add(egui::Shape::line(
                vec![
                    Pos2::new(c.x - 4.5 * s, c.y - 6.0 * s),
                    Pos2::new(c.x + 1.0 * s, c.y - 1.0 * s),
                    Pos2::new(c.x - 1.0 * s, c.y + 1.0 * s),
                    Pos2::new(c.x + 4.5 * s, c.y + 6.0 * s),
                ],
                w,
            ));
        }
        "stack" => {
            for i in 0..3 {
                let y = -3.5 + i as f32 * 3.5;
                p.line_segment(
                    [
                        Pos2::new(c.x - 5.5 * s, c.y + y * s),
                        Pos2::new(c.x + 5.5 * s, c.y + y * s),
                    ],
                    Stroke::new(1.3_f32, a(color, 255 - i as u8 * 55)),
                );
            }
        }
        "crosshair" => {
            p.circle_stroke(c, 5.5 * s, w);
            p.circle_filled(c, 1.2 * s, color);
            for (dx, dy) in [(0.0_f32, -1.0_f32), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
                p.line_segment(
                    [
                        Pos2::new(c.x + dx * 5.5 * s, c.y + dy * 5.5 * s),
                        Pos2::new(c.x + dx * 7.5 * s, c.y + dy * 7.5 * s),
                    ],
                    w,
                );
            }
        }
        "chip" => {
            p.rect_stroke(box_(0.0, 0.0, 8.0, 8.0), 1.5, w);
            for k in [-2.0_f32, 2.0] {
                p.line_segment(
                    [
                        Pos2::new(c.x + k * s, c.y - 4.0 * s),
                        Pos2::new(c.x + k * s, c.y - 6.0 * s),
                    ],
                    w,
                );
                p.line_segment(
                    [
                        Pos2::new(c.x + k * s, c.y + 4.0 * s),
                        Pos2::new(c.x + k * s, c.y + 6.0 * s),
                    ],
                    w,
                );
            }
        }
        "pulse" => {
            p.add(egui::Shape::line(
                vec![
                    Pos2::new(c.x - 7.0 * s, c.y),
                    Pos2::new(c.x - 3.0 * s, c.y),
                    Pos2::new(c.x - 1.0 * s, c.y - 5.0 * s),
                    Pos2::new(c.x + 1.5 * s, c.y + 5.0 * s),
                    Pos2::new(c.x + 3.5 * s, c.y),
                    Pos2::new(c.x + 7.0 * s, c.y),
                ],
                w,
            ));
        }
        "target" => {
            p.circle_stroke(c, 6.5 * s, w);
            p.circle_stroke(c, 3.2 * s, w);
            p.circle_filled(c, 1.1 * s, color);
        }
        "gear" => {
            p.circle_stroke(c, 4.5 * s, w);
            for i in 0..8 {
                let a0 = i as f32 * std::f32::consts::TAU / 8.0;
                let (sn, cs) = a0.sin_cos();
                p.line_segment(
                    [
                        Pos2::new(c.x + cs * 5.5 * s, c.y + sn * 5.5 * s),
                        Pos2::new(c.x + cs * 7.2 * s, c.y + sn * 7.2 * s),
                    ],
                    w,
                );
            }
        }
        "home" => {
            p.add(egui::Shape::line(
                vec![
                    Pos2::new(c.x - 6.0 * s, c.y - 0.5 * s),
                    Pos2::new(c.x, c.y - 6.0 * s),
                    Pos2::new(c.x + 6.0 * s, c.y - 0.5 * s),
                ],
                w,
            ));
            p.rect_stroke(box_(0.0, 2.5, 8.0, 6.0), 1.0, w);
        }
        _ => {
            p.circle_stroke(c, 5.0 * s, w);
        }
    }
}

/// Catmull-Rom: one sample in, a smooth curve out. Charts must not stair-step.
fn catmull(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let t2 = t * t;
    let t3 = t2 * t;
    let f = |a: f32, b: f32, c: f32, d: f32| {
        0.5 * ((2.0 * b) + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    Pos2::new(
        f(p0.x, p1.x, p2.x, p3.x),
        f(p0.y, p1.y, p2.y, p3.y),
    )
}

/// A live area chart: gradient fill (mesh, so the alpha fades with height), a
/// glowing stroke, and a bright head where the newest sample lands.
pub fn area_chart(p: &egui::Painter, rect: Rect, values: &[f32], color: Color32, max: f32) {
    if values.len() < 2 || rect.width() < 8.0 || rect.height() < 6.0 {
        return;
    }
    let max = if max <= 0.001 { 1.0 } else { max };
    let n = values.len();
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.left() + rect.width() * (i as f32 / (n - 1) as f32);
            let y = rect.bottom() - (v / max).clamp(0.0, 1.0) * rect.height() * 0.88;
            Pos2::new(x, y)
        })
        .collect();

    let mut line: Vec<Pos2> = Vec::with_capacity(n * 4);
    for i in 0..n - 1 {
        let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
        let p1 = pts[i];
        let p2 = pts[i + 1];
        let p3 = if i + 2 < n { pts[i + 2] } else { p2 };
        for s in 0..4 {
            line.push(catmull(p0, p1, p2, p3, s as f32 / 4.0));
        }
    }
    if let Some(last) = pts.last() {
        line.push(*last);
    }

    // fill: two vertices per column, colour at the curve, transparent at the floor
    let mut mesh = egui::epaint::Mesh::default();
    let top = a(color, 64);
    let floor = a(color, 0);
    for v in line.iter() {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: *v,
            uv: egui::epaint::WHITE_UV,
            color: top,
        });
        mesh.vertices.push(egui::epaint::Vertex {
            pos: Pos2::new(v.x, rect.bottom()),
            uv: egui::epaint::WHITE_UV,
            color: floor,
        });
    }
    for i in 0..line.len().saturating_sub(1) {
        let k = (i * 2) as u32;
        mesh.indices.extend_from_slice(&[k, k + 1, k + 2, k + 1, k + 3, k + 2]);
    }
    p.add(egui::Shape::mesh(mesh));

    // glow: three widening strokes, then the line itself
    for (w, al) in [(6.0_f32, 16u8), (3.5, 30), (2.0, 60)] {
        p.add(egui::Shape::line(
            line.clone(),
            Stroke::new(w, a(color, al)),
        ));
    }
    p.add(egui::Shape::line(
        line.clone(),
        Stroke::new(1.5_f32, a(color, 245)),
    ));

    // the head: where "now" is
    if let Some(head) = line.last() {
        p.circle_filled(*head, 5.0, a(color, 34));
        p.circle_filled(*head, 2.6, a(color, 255));
        p.circle_filled(*head, 1.2, Color32::WHITE);
    }

    // floor grid: two hairlines, so the chart has a baseline to sit on
    p.line_segment(
        [
            Pos2::new(rect.left(), rect.bottom()),
            Pos2::new(rect.right(), rect.bottom()),
        ],
        Stroke::new(1.0_f32, a(white(), 14)),
    );
}

/// One instrument card: badge + name + reading on top, live chart under it,
/// average along the floor. Everything is painted, so the card keeps its exact
/// geometry at any width.
pub fn metric_card(
    ui: &mut Ui,
    kind: &str,
    title: &str,
    sub: &str,
    value: String,
    unit: &str,
    color: Color32,
    values: &[f32],
    max: f32,
    footer: &str,
    footer_v: String,
    height: f32,
) {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, height), Sense::hover());
    surface(ui, rect, 14.0);
    let t = ui.ctx().animate_bool(resp.id, resp.hovered());
    if t > 0.01 {
        let hot = if light() {
            a(ink_light(), (96.0 + 60.0 * t) as u8)
        } else {
            a(white(), (30.0 + 34.0 * t) as u8)
        };
        ui.painter()
            .rect_stroke(rect, 14.0, Stroke::new(1.0_f32, hot));
    }

    let p = ui.painter();
    let pad = 16.0;
    // badge
    let badge = Rect::from_min_size(
        Pos2::new(rect.left() + pad, rect.top() + pad),
        Vec2::splat(26.0),
    );
    p.rect_filled(badge, 8.0, a(color, 30));
    p.rect_stroke(badge, 8.0, Stroke::new(1.0_f32, a(color, 70)));
    glyph(p, badge.center(), kind, color, 0.85);

    // name + subtitle
    p.text(
        Pos2::new(badge.right() + 10.0, badge.center().y - 5.0),
        Align2::LEFT_CENTER,
        title,
        FontId::new(12.5, label_font().family),
        txt(),
    );
    p.text(
        Pos2::new(badge.right() + 10.0, badge.center().y + 7.0),
        Align2::LEFT_CENTER,
        sub,
        mono_font(9.0),
        ghost(),
    );

    // reading, right aligned
    let unit_w = if unit.is_empty() {
        0.0
    } else {
        ui.painter()
            .layout_no_wrap(unit.to_string(), label_font(), faint())
            .size()
            .x
            + 4.0
    };
    p.text(
        Pos2::new(rect.right() - pad - unit_w, badge.center().y),
        Align2::RIGHT_CENTER,
        value,
        num_font(21.0),
        txt(),
    );
    if !unit.is_empty() {
        p.text(
            Pos2::new(rect.right() - pad, badge.center().y + 1.0),
            Align2::RIGHT_CENTER,
            unit,
            mono_font(10.0),
            faint(),
        );
    }

    // chart
    let chart = Rect::from_min_max(
        Pos2::new(rect.left() + pad, badge.bottom() + 16.0),
        Pos2::new(rect.right() - pad, rect.bottom() - 30.0),
    );
    area_chart(p, chart, values, color, max);

    // floor: average
    p.line_segment(
        [
            Pos2::new(rect.left() + pad, rect.bottom() - 26.0),
            Pos2::new(rect.right() - pad, rect.bottom() - 26.0),
        ],
        Stroke::new(1.0_f32, a(white(), 12)),
    );
    p.text(
        Pos2::new(rect.left() + pad, rect.bottom() - 13.0),
        Align2::LEFT_CENTER,
        footer,
        mono_font(9.5),
        ghost(),
    );
    p.text(
        Pos2::new(rect.right() - pad, rect.bottom() - 13.0),
        Align2::RIGHT_CENTER,
        footer_v,
        mono_font(10.0),
        dim(),
    );
}
