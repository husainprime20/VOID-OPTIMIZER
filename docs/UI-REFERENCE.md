# UI reference — the house interface language

**This file is the standard.** Every app, every session: build the interface in this language
unless the user says otherwise. The working implementation lives in
[`docs/ui-kit/`](ui-kit/) (`ui.css`, `ui.js`, `index.html` = live showcase); this page is the
spec those files obey.

Origin: the *Boot Wallpaper Patcher* desktop app (frameless pywebview window, HTML/CSS/JS).

---

## 1. What it is, in one line

A **single near-black surface** carrying **white light** — glass cards, hairline rims, a
drifting black-and-white band wallpaper underneath, running water on the glass, a glowing dot
for a cursor, and motion that settles into place instead of popping in.

## 2. Non-negotiables

1. **Monochrome.** The only "accent" is white. Colour is reserved for meaning:
   mint `rgb(110,231,183)` = done, amber `rgb(255,204,107)` = warning,
   rose `rgb(255,122,148)` = error / destructive hover.
2. **Glass, not panels.** Translucent black + `backdrop-filter: blur(18px) saturate(1.3)` +
   1 px white rim at 10 % + a 1 px top highlight at 16 %.
3. **Soft answers.** Hover = glow + 1 px lift + slightly lighter glass. Never a colour swap,
   never a bounce over 4 px.
4. **Motion arrives.** Entrance `0.8 s` `cubic-bezier(.16,1,.3,1)`, children staggered `90 ms`.
   Interactions `cubic-bezier(.22,1.28,.36,1)` (spring). Nothing snaps.
5. **Instrumentation type.** Micro-labels are uppercase mono, `10 px`, `1.7 px` tracking,
   ghost grey. Body is a geometric sans (Plus Jakarta Sans → Poppins → Inter → Segoe UI).
6. **Legibility beats atmosphere.** Rain, wallpaper and glows always sit *behind* content,
   clipped to the panel. Never draw an effect across text.
7. **Always reversible.** Every dialog closes on X, scrim click and `Esc`; every animation has a
   `reduce-motion` off-switch; settings are remembered.

## 3. Tokens

```
surface   bg #08080a   panel rgba(19,19,19,.88)   glass linear-gradient(135deg,rgba(0,0,0,.40),rgba(0,0,0,.26))
ink       txt #f1f1f1   txt-2 #d1d3d9   dim #9698a0   faint #8f8f8f   ghost #6e7078
light     acc rgb(255,255,255)   acc-2 rgb(205,208,216)
meaning   mint 110,231,183   amber 255,204,107   rose 255,122,148
lines     rgba(255,255,255,.10) / .16
shape     card 16 · field 12 · button 12–14 · dialog 24 · pill 99 · icon button 11
motion    ease cubic-bezier(.16,1,.3,1) · spring cubic-bezier(.22,1.28,.36,1) · std cubic-bezier(.4,0,.2,1)
```

Depth recipe for any glass element:

```css
background: linear-gradient(135deg, rgba(0,0,0,.40), rgba(0,0,0,.26)), rgba(0,0,0,.12);
box-shadow: inset 0 0 0 1px rgba(255,255,255,.10),
            inset 0 1px 0 rgba(255,255,255,.16),
            0 8px 22px -10px rgba(0,0,0,.70);
backdrop-filter: blur(18px) saturate(1.3);
```

## 4. Signatures (the parts people remember)

| Signature | Behaviour |
|---|---|
| **Band wallpaper** | 760 px repeating black↔white gradient on a −32° field, drifting left forever at 22 s per cycle; brightness/speed user-controlled. |
| **Rain on glass** | Canvas sprite per drop: contact shadow, refracted dark top, bright caustic bottom, specular highlight. Drops grow, merge when they touch, then slide in stop-and-go bursts leaving a thin trail, shrinking as they go. |
| **Cursor** | Native cursor hidden. 6 px white dot (`mix-blend-mode: difference`) + 34 px ring that lags on a spring, stretches along travel direction, swells to 40 px with a 20 % white fill over interactive elements, and emits a 12 px wave on click. |
| **Primary button** | Full-width glass slab, shimmer sweep on hover, radial ripple from the click point, slow pulse while busy, then the label fades and a dark tick draws itself (button floods mint). |
| **Switch** | 36×20; off = white 10 % track with white knob; on = near-white track with **ink** knob and a white glow. Knob widens while pressed. |
| **Slider** | 5 px track filled white up to the value, 16 px white thumb with a layered white glow, value repeated in a mono chip. |
| **Tabs** | Segmented glass control whose indicator *slides* (0.5 s) to the active tab; panels fade up with a 5 px→0 blur. |
| **Modal** | `scale(.90)` + `blur(10px)` → `scale(1)` + `blur(0)` on a spring, contents staggered 60 ms apart, scrim blurs the app behind. |
| **Success** | Full-screen celebration: 264° mint ring draws itself, tick draws, concentric rings pulse, rotating rays, three blurred blobs, one confetti burst + drifting star sparks. Then everything holds still. |
| **Progress** | 3 px light line across the top: indeterminate comet while working, floods mint and fades when finished. |
| **Toast** | Bottom-right, glass pill, rises 14 px + fades, auto-dismiss 2.6 s. |
| **Log** | Timestamp (mono, grey) + message; each line slides in from +22 px; `ok/warn/err/dim` tint the *message only*. |
| **Title bar** | Frameless strip: glass mark tile, name with an occasional specular shine sweep, then glass icon buttons (close turns rose and rotates its X on hover). |

## 5. Layout

- Frameless window with a custom 44 px title bar; the app fills the window and rounds with
  `--app-r` (default 14 px).
- One centred column, 22–34 px gutters, 14 px gaps between cards. Nothing wider than ~700 px.
- Order: head (eyebrow / title / one-line purpose) → inputs → primary action → log/output →
  meta. The primary action sits **after** the inputs, never in a title bar.
- Cards have an uppercase mono title row with an optional status dot on the right.

## 6. Copy

- Sentence case, present tense, no exclamation marks: *"Ready. Every line arrives from the right."*
- Labels name the thing (*Amount*, *Drop size*), helpers explain the consequence
  (*"Replaces the picture."*), never repeat the label.
- Errors state what happened and what to do next, in one line.
- No emoji anywhere. No icon-only actions without a `title` / `aria-label`.

## 7. Implementation notes

- Zero dependencies, zero build. `ui.css` + `ui.js`; behaviour is opt-in via data attributes.
- Everything animates on `transform` / `opacity` / `filter` only — never layout.
- Effects are canvas layers at `z-index` 1 (rain) and 0 (wallpaper); content is `z-index` 2.
- Persist user choices through `UIKit.store` (namespaced `localStorage`) or the host app's
  settings file.
- Porting to a native toolkit (egui, Qt, WPF): reproduce the *rules* and the tokens, not the
  CSS. The mapping table in [`ui-kit/README.md`](ui-kit/README.md#porting-it-to-a-non-web-surface)
  shows the egui equivalent of each piece.

## 8. Checklist before shipping a screen

- [ ] One accent; colour appears only as meaning.
- [ ] Every surface is glass with a 1 px rim (no flat fills).
- [ ] Every control answers on hover, and the answer is a glow/lift.
- [ ] Entrances are staggered; nothing pops without easing.
- [ ] Micro-labels are uppercase mono with tracking.
- [ ] Rain/wallpaper/glow never crosses text.
- [ ] `Esc`, X and scrim all close dialogs.
- [ ] `reduce-motion` (setting or `prefers-reduced-motion`) is honoured.
- [ ] Light theme? If the app needs one, it is the same language inverted: `#f4f4f6`
      surface, ink glass, black light, same radii, same motion. Never a second design.
