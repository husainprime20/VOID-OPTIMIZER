# Monochrome Glass — the house UI kit

The interface language used for every app from here on: one dark surface, white light,
black glass, and motion that *arrives* instead of appearing.

Drop-in, dependency-free: `ui.css` + `ui.js`, plain HTML. No build step, no framework.
Open `index.html` to see every component live.

```html
<link rel="stylesheet" href="ui.css">
<script src="ui.js" defer></script>

<div id="wallpaper"><i></i></div>          <!-- drifting band field -->
<div class="app">
  <canvas id="rain"></canvas>              <!-- raindrops on the glass -->
  <header class="titlebar"> … </header>
  <main class="view"> … cards … </main>
</div>
```

`ui.js` mounts itself on `DOMContentLoaded` and wires every opt-in attribute
(`data-switch`, `data-key`, `input.rg`, `.tabs`, `data-open`/`data-close`, `.btn` ripples).

## The rules

| # | Rule | Why |
|---|------|-----|
| 1 | **One accent: white.** Colour is meaning only — mint `#6ee7b7` done, amber `#ffcc6b` warning, rose `#ff7a94` error. | A monochrome surface makes colour carry information instead of decoration. |
| 2 | **Glass, never a flat panel.** Translucent black + `backdrop-filter: blur(18px)` + a 1 px light rim. | Depth without borders, and the wallpaper stays alive underneath. |
| 3 | **Answer softly.** Hover = glow + 1 px lift. Never a colour swap. | The surface is calm; interaction is a breath, not a highlight. |
| 4 | **Motion arrives.** Entrance `0.8 s` with `cubic-bezier(.16,1,.3,1)`, staggered `90 ms`. Interaction is springy `cubic-bezier(.22,1.28,.36,1)`. | Things should feel *placed*, never popped. |
| 5 | **Uppercase mono micro-labels.** `10 px`, `1.7 px` tracking, ghost grey. | Reads as instrumentation, not marketing. |
| 6 | **Never put text on the moving layer without contrast.** Rain and wallpaper sit behind the content. | Legibility is not negotiable. |
| 7 | **Reset is always available.** Every dialog has an X and `Esc`; every animation respects `reduce-motion`. | Confidence, not entrapment. |

## Tokens

```
surface   #08080a        panel rgba(19,19,19,.88)      glass rgba(0,0,0,.40)→.26
ink       #f1f1f1        dim #9698a0    faint #8f8f8f   ghost #6e7078
light     white 255      accent-2 205,208,216
meaning   mint 110,231,183   amber 255,204,107   rose 255,122,148
shape     16 cards · 12 fields · 12–14 buttons · 24 dialogs · 99 pills
motion    --ease cubic-bezier(.16,1,.3,1)   --spring cubic-bezier(.22,1.28,.36,1)
type      Plus Jakarta Sans → Poppins → Inter → Segoe UI Variable
```

## Components

| Component | Markup | Behaviour |
|---|---|---|
| App shell | `.app` | lives inside the wallpaper, `--app-r` radius, `appIn` settle |
| Title bar | `.titlebar` → `.tb-brand` + `.btn-icon` | draggable strip, close goes rose on hover |
| Card | `.card` | glass + rim + cursor spotlight (`--mx/--my/--sa`) |
| Switch | `[data-switch]` → `.switch > i` | light track, ink knob, springs 36 px wide |
| Slider | `input.rg` + `.slider-head em[data-out]` | glowing white thumb, `--p` fill, live value chip |
| Field | `.field` | glass well, focuses with a white halo |
| Buttons | `.btn.primary` / `.btn.ghost` | shimmer sweep, click ripple, busy pulse, tick on success |
| Tabs | `.tabs` + `.tab-ind` | indicator slides on a 0.5 s ease |
| Modal | `.modal` + `.scrim` | scale .90 → 1 with a 10 px→0 blur, content staggers in |
| Success | `.success` | ring draw, rays, blobs, confetti burst, then quiet |
| Toast | `.toast` | 2.6 s, slides up, auto-dismisses |
| Progress | `.progress` | 3 px light line, indeterminate → mint |
| Log | `.list .row.{ok,warn,err,dim}` | lines slide in from the right |
| Cursor | `#cursor-dot` + `#cursor-ring` | white dot, lagging ring, stretches with travel, wave on click |

## JS surface

```js
UIKit.mount({ brightness: .9 })     // cursor, wallpaper, all bindings
UIKit.rain(canvas)                  // .tick(dt, {l,t,w,h}) each frame; .amount .size .slide .speed
UIKit.progress(el).run() / .finish()
UIKit.busy(btn, true)   UIKit.flash(btn, 'done' | 'error')
UIKit.toast('saved', 'ok')
UIKit.success.open([['Live wallpaper', 'boot.mp4']]) / .close()
UIKit.store.get(k, d) / .set(k, v)  // namespaced localStorage
```

## Porting it to a non-web surface

The language is style, not CSS. Mapping to a native toolkit (this repo's own app is
`egui`/Rust):

| Kit | egui |
|---|---|
| `--bg` / panel | `Visuals::dark()` with `window_fill = #0b0b0d`, `panel_fill` translucent |
| glass rim | `Frame::new().fill(...).stroke(Stroke::new(1.0, white.a(26)))` |
| switch | 36×20 rounded rect; track `white.a(240)` when on, knob `#0c0c0f` |
| glow | `Stroke` with alpha + a 2 px outer ring at 12 % white |
| `--ease` | `ctx.animate_value_with_time(id, target, 0.22)` with your own curve for entrances |
| rain / wallpaper | a `Painter` layer *behind* the content widgets, clipped to the panel |
| micro-label | `FontId::monospace(10.0)` + manual letter spacing |

Keep the numbered rules above and the look survives the port; reproduce only the CSS and
it won't.
