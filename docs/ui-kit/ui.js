/* ============================================================================
   MONOCHROME GLASS  ·  ui.js
   ----------------------------------------------------------------------------
   Dependency-free behaviour for ui.css. Everything is opt-in through markup:

     <div class="card" data-switch data-key="rain">          … toggle row
     <input type="range" class="rg" data-key="amount" data-out="amount">
     <div class="tabs"><i class="tab-ind"></i><button class="tab" data-tab="a">
     <button class="modal-x" data-close>                     … close a modal
     <button data-open="settings">                           … open one

   Global:  UIKit.toast('saved', 'ok')            notifications
            UIKit.progress(el).run() / .finish()
            UIKit.busy(btn, true|false)           button busy state
            UIKit.flash(btn, 'error'|'done')      feedback pulse
            UIKit.success.open() / .close()       celebratory dialog
   ========================================================================== */
(() => {
  "use strict";

  const $  = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const clamp = (v, a, b) => Math.min(b, Math.max(a, v));
  const store = {
    get(k, d) { try { const v = localStorage.getItem("uikit:" + k); return v === null ? d : JSON.parse(v); } catch { return d; } },
    set(k, v) { try { localStorage.setItem("uikit:" + k, JSON.stringify(v)); } catch {} }
  };
  const HOT = 'button,.btn,.sw,.tg,[data-switch],.tab,.tab-btn,.chip,input[type="range"],.presets button,.btn-icon,[role="switch"]';

  /* ── cursor: a white dot with a ring that lags behind and stretches ───── */
  function cursor(opts = {}) {
    if (matchMedia("(pointer: coarse)").matches) return;
    const dot = document.getElementById("cursor-dot") || Object.assign(document.createElement("div"), { id: "cursor-dot" });
    const ring = document.getElementById("cursor-ring") || Object.assign(document.createElement("div"), { id: "cursor-ring" });
    document.body.append(dot, ring);

    let tx = -100, ty = -100, x = -100, y = -100, px = -100, py = -100;
    let ang = 0, stretch = 0, last = 0, seen = false, raf = 0;

    const show = v => { dot.classList.toggle("is-on", v); ring.classList.toggle("is-on", v); };

    addEventListener("mousemove", e => {
      tx = e.clientX; ty = e.clientY;
      if (!seen) { x = px = tx; y = py = ty; seen = true; }
      dot.style.transform = `translate3d(${tx}px,${ty}px,0)`;
      show(true);
      const t = e.target;
      const hot = !!(t && t.closest && t.closest(HOT));
      dot.classList.toggle("is-hot", hot);
      ring.classList.toggle("is-hot", hot);
      ring.classList.toggle("is-slider", !!(t && t.matches && t.matches('input[type="range"]')));
      if (!raf) { last = 0; raf = requestAnimationFrame(tick); }
    }, true);

    function tick(now) {
      const dt = last ? Math.min(.05, (now - last) / 1000) : .016; last = now;
      const k = 1 - Math.exp(-dt * 16);
      x += (tx - x) * k; y += (ty - y) * k;
      const vx = x - px, vy = y - py; px = x; py = y;
      const sp = Math.hypot(vx, vy);
      if (sp > .6) {
        const na = Math.atan2(vy, vx);
        let da = na - ang; da = Math.atan2(Math.sin(da), Math.cos(da));
        ang += da * .35;
      }
      stretch += (Math.min(sp / 26, .38) - stretch) * (1 - Math.exp(-dt * 14));
      const calm = document.documentElement.classList.contains("reduce-motion");
      ring.style.transform =
        `translate3d(${x.toFixed(2)}px,${y.toFixed(2)}px,0) rotate(${ang.toFixed(3)}rad) ` +
        `scale(${(calm ? 1 : 1 + stretch).toFixed(3)},${(calm ? 1 : 1 - stretch * .45).toFixed(3)})`;
      if (Math.abs(tx - x) > .05 || Math.abs(ty - y) > .05 || stretch > .004) raf = requestAnimationFrame(tick);
      else { raf = 0; last = 0; }
    }

    document.addEventListener("mouseleave", () => show(false));
    document.addEventListener("mouseenter", () => show(true));
    addEventListener("mousedown", e => {
      const w = document.createElement("div");
      w.className = "cursor-wave"; w.style.left = e.clientX + "px"; w.style.top = e.clientY + "px";
      document.body.appendChild(w);
      w.addEventListener("animationend", () => w.remove());
    }, true);

    if (opts.hideNative !== false) document.documentElement.style.cursor = "none";
  }

  /* ── wallpaper: the drifting black / white band field ─────────────────── */
  function wallpaper(el, { speed = 1, brightness = .9 } = {}) {
    if (!el) return {};
    el.style.setProperty("--wp-o", brightness);
    const band = $("i", el) || el.appendChild(document.createElement("i"));
    let anim = null;
    try {
      anim = band.animate(
        [{ transform: "translate3d(0,0,0)" }, { transform: "translate3d(-760px,0,0)" }],
        { duration: 22000, iterations: Infinity }
      );
      anim.playbackRate = speed;
    } catch {}
    return {
      set speed(v) { if (anim) anim.playbackRate = v; },
      set brightness(v) { el.style.setProperty("--wp-o", v); },
      show(v) { el.style.display = v ? "" : "none"; }
    };
  }

  /* ── rain on glass: lens sprite, merging drops, stop-and-go sliding ───── */
  function rain(canvas, opts = {}) {
    const ctx = canvas.getContext("2d");
    const o = Object.assign({ amount: 100, size: 100, slide: 100, speed: 100, bounds: null }, opts);
    const SPR = makeSprite();
    const rnd = (a, b) => a + Math.random() * (b - a);
    const drops = [];
    const box = { l: 0, t: 0, w: 0, h: 0 };
    let mergeT = 0, enabled = true;

    function makeSprite() {
      const S = 144, c = S / 2, R = 48;
      const cv = document.createElement("canvas"); cv.width = cv.height = S;
      const g = cv.getContext("2d");
      let q = g.createRadialGradient(c + 5, c + 10, R * .55, c + 5, c + 10, R * 1.3);
      q.addColorStop(0, "rgba(0,0,0,.22)"); q.addColorStop(1, "rgba(0,0,0,0)");
      g.fillStyle = q; g.beginPath(); g.arc(c + 5, c + 10, R * 1.3, 0, 6.2832); g.fill();
      g.save(); g.beginPath(); g.arc(c, c, R, 0, 6.2832); g.clip();
      q = g.createRadialGradient(c - 10, c - 14, 2, c, c, R);
      q.addColorStop(0, "rgba(234,234,234,.20)"); q.addColorStop(.55, "rgba(183,183,183,.05)");
      q.addColorStop(.86, "rgba(162,162,162,.10)"); q.addColorStop(1, "rgba(234,234,234,.30)");
      g.fillStyle = q; g.fillRect(0, 0, S, S);
      q = g.createLinearGradient(0, c - R, 0, c + R * .2);
      q.addColorStop(0, "rgba(11,11,11,.34)"); q.addColorStop(1, "rgba(11,11,11,0)");
      g.fillStyle = q; g.fillRect(0, 0, S, S);
      q = g.createRadialGradient(c + R * .12, c + R * .6, 0, c + R * .12, c + R * .6, R * .75);
      q.addColorStop(0, "rgba(224,224,224,.42)"); q.addColorStop(1, "rgba(224,224,224,0)");
      g.fillStyle = q; g.fillRect(0, 0, S, S);
      g.restore();
      g.lineWidth = 1.6; g.strokeStyle = "rgba(239,239,239,.30)";
      g.beginPath(); g.arc(c, c, R - .8, 0, 6.2832); g.stroke();
      g.save(); g.translate(c - R * .36, c - R * .42); g.rotate(-.7);
      g.shadowColor = "rgba(255,255,255,.9)"; g.shadowBlur = 7; g.fillStyle = "rgba(255,255,255,.92)";
      g.beginPath(); g.ellipse(0, 0, R * .24, R * .13, 0, 0, 6.2832); g.fill(); g.restore();
      g.fillStyle = "rgba(255,255,255,.55)";
      g.beginPath(); g.arc(c + R * .32, c + R * .42, R * .07, 0, 6.2832); g.fill();
      return cv;
    }

    const mk = (x, y, r, extra) => {
      const d = Object.assign({
        x, y, r, v: 0, g: false, age: 0, fade: .35, crit: rnd(4.2, 6.6), grow: rnd(.04, .22),
        life: Infinity, mv: false, go: 0, seed: rnd(0, 6.28)
      }, extra || {});
      drops.push(d); return d;
    };
    function seed(pre) {
      const q = Math.random();
      const r = q < .68 ? rnd(.8, 2.0) : q < .94 ? rnd(2.0, 4.0) : rnd(4.0, 6.0);
      const d = mk(box.l + Math.random() * box.w, box.t + Math.random() * box.h, r);
      if (r < 1.8) d.life = rnd(7, 22);
      if (r >= d.crit) d.crit = r + 1.5;
      if (pre) d.age = 1;
    }
    const startSlide = d => { d.g = true; d.mv = true; d.go = rnd(.5, 1.2); d.v = Math.max(d.v, 8); };
    const stopSlide = d => { d.g = false; d.v = 0; d.mv = false; d.crit = d.r + rnd(2.5, 4); };

    function step(dt) {
      const slideK = o.slide / 100;
      const want = Math.round(box.w * box.h / 4200 * o.amount / 100);
      if (!drops.length && box.w > 100) {
        for (let i = 0; i < want * .85; i++) seed(true);
        let n = 0;
        for (const d of drops) if (slideK > 0 && n < 4 && d.r > 2.2 && d.r < d.crit) { d.r = d.crit + .05; n++; }
      }
      let still = 0; for (const d of drops) if (!d.g) still++;
      if (still < want && Math.random() < dt * 40) seed(false);

      for (let i = drops.length - 1; i >= 0; i--) {
        const d = drops[i];
        d.age += dt;
        if (!d.g) {
          if (d.life !== Infinity) {
            d.life -= dt;
            if (d.life < 0) { d.r -= dt * 1.1; if (d.r < .3) { drops.splice(i, 1); continue; } }
          } else {
            d.r += d.grow * dt * .5 * slideK;
            if (slideK > 0 && d.r >= d.crit) startSlide(d);
          }
          continue;
        }
        d.go -= dt;
        if (d.go <= 0) { d.mv = !d.mv; d.go = d.mv ? rnd(.35, 1.3) : rnd(.12, .6); }
        const target = d.mv ? (26 + d.r * 13) * o.speed / 100 : 0;
        d.v += (target - d.v) * (1 - Math.exp(-dt * (d.mv ? 5 : 9)));
        const dy = d.v * dt;
        d.y += dy;
        d.x += Math.sin(d.y * .045 + d.seed) * dy * .12;
        d.r -= dy * .0042;
        if (d.r < 2.1) { stopSlide(d); continue; }
        if (d.y - d.r > box.t + box.h + 10) drops.splice(i, 1);
      }

      mergeT += dt;
      if (mergeT > .1) {                       // overlapping drops become one bigger drop
        mergeT = 0;
        for (let i = drops.length - 1; i >= 0; i--) {
          const a = drops[i]; if (!a) continue;
          for (let j = i - 1; j >= 0; j--) {
            const b = drops[j];
            if (a.age < .3 || b.age < .3) continue;
            const dx = a.x - b.x, dy = a.y - b.y, lim = (a.r + b.r) * .75;
            if (dx * dx + dy * dy > lim * lim) continue;
            const big = (a.g && !b.g) || (a.g === b.g && a.r >= b.r) ? a : b;
            const small = big === a ? b : a;
            const ab = big.r * big.r, as = small.r * small.r * .95;
            if (!big.g) {
              big.x = (big.x * ab + small.x * as) / (ab + as);
              big.y = (big.y * ab + small.y * as) / (ab + as);
            } else { big.v += 16; big.mv = true; big.go = Math.max(big.go, rnd(.4, .9)); }
            big.r = Math.sqrt(ab + as);
            big.life = (big.life === Infinity || small.life === Infinity) ? Infinity : Math.max(big.life, small.life);
            if (big.life === Infinity && big.r < 1.8) big.life = rnd(7, 14);
            if (big.age < 1) big.age = 1;
            drops.splice(small === a ? i : j, 1);
            break;
          }
        }
      }
    }

    function draw() {
      ctx.imageSmoothingQuality = "high";
      for (const pass of [false, true]) {
        for (const d of drops) {
          if (d.g !== pass) continue;
          const s = Math.min(1, d.age / d.fade);
          const e = 1 - Math.pow(1 - s, 3);
          const sc = d.r / 48 * (.35 + .65 * e) * o.size / 100;
          let sx = 1, sy = 1;
          if (d.g) { sy = 1 + Math.min(d.v / 110, .55); sx = 1 - (sy - 1) * .3; }
          const w = 144 * sc * sx, h = 144 * sc * sy;
          ctx.globalAlpha = e;
          ctx.drawImage(SPR, d.x - w / 2, d.y - h / 2, w, h);
        }
      }
      ctx.globalAlpha = 1;
    }

    return {
      set amount(v) { o.amount = v; }, set size(v) { o.size = v; },
      set slide(v) { o.slide = v; },   set speed(v) { o.speed = v; },
      get enabled() { return enabled; }, set enabled(v) { enabled = v; if (!v) drops.length = 0; },
      /** call each frame with the canvas's own pixel rect in CSS px + dt seconds */
      tick(dt, bounds) {
        if (!enabled) return;
        if (bounds) Object.assign(box, bounds);
        step(Math.min(dt, .05));
        draw();
      }
    };
  }

  /* ── small behaviours ─────────────────────────────────────────────────── */
  function bindSwitches(root = document) {
    $$("[data-switch]", root).forEach(el => {
      const key = el.dataset.key;
      const row = el.closest("[data-row]") || el;
      const apply = v => {
        row.classList.toggle("is-on", v);
        el.classList.toggle("is-on", v);
        el.setAttribute("aria-checked", String(v));
        const off = el.dataset.disables;
        if (off) $(off, document)?.classList.toggle("is-off", !v);
        el.dispatchEvent(new CustomEvent("switch:change", { bubbles: true, detail: { on: v, key } }));
      };
      let v = el.classList.contains("is-on");
      if (key) v = store.get(key, v);
      apply(v);
      const flip = () => { v = !v; if (key) store.set(key, v); apply(v); };
      el.addEventListener("click", e => {
        if (e.target.closest("input,button:not([data-switch]),a")) return;
        flip();
      });
      el.addEventListener("keydown", e => {
        if (e.key === " " || e.key === "Enter") { e.preventDefault(); flip(); }
      });
    });
  }

  function bindSliders(root = document) {
    const list = $$('input[type="range"]', root);
    list.forEach(el => {
      const key = el.dataset.key;
      const out = () => el.parentElement.querySelector("[data-out]") || $(`[data-out="${key}"]`, root);
      const fmt = v => (el.dataset.unit ? v + " " + el.dataset.unit : v);
      const paint = () => {
        const p = (el.value - el.min) / (el.max - el.min) * 100;
        el.style.setProperty("--p", p + "%");
        const o = out(); if (o) o.textContent = fmt(el.value);
      };
      if (key) {
        const saved = store.get(key, null);
        if (saved !== null) el.value = clamp(saved, +el.min, +el.max);
      }
      paint();
      el.addEventListener("input", () => {
        paint();
        if (key) store.set(key, +el.value);
        el.dispatchEvent(new CustomEvent("slide", { bubbles: true, detail: { value: +el.value, key } }));
      });
    });
  }

  function bindTabs(root = document) {
    $$(".tabs", root).forEach(tabs => {
      const ind = $(".tab-ind", tabs);
      const move = () => {
        const on = $(".tab.is-on", tabs);
        if (!on || !ind) return;
        ind.style.width = on.offsetWidth + "px";
        ind.style.transform = `translateX(${on.offsetLeft}px)`;
      };
      tabs.addEventListener("click", e => {
        const tab = e.target.closest(".tab");
        if (!tab) return;
        $$(".tab", tabs).forEach(t => t.classList.toggle("is-on", t === tab));
        const scope = tabs.closest("[data-tabs-scope]") || document;
        $$("[data-panel]", scope).forEach(p => p.classList.toggle("is-on", p.dataset.panel === tab.dataset.tab));
        move();
        tabs.dispatchEvent(new CustomEvent("tab:change", { bubbles: true, detail: { tab: tab.dataset.tab } }));
      });
      move();
      addEventListener("resize", move);
      new ResizeObserver(move).observe(tabs);
    });
  }

  function bindRipples() {
    document.addEventListener("pointerdown", e => {
      const b = e.target.closest(".btn");
      if (!b) return;
      const r = b.getBoundingClientRect();
      const d = 2 * Math.max(r.width, r.height) * 1.05;
      const s = document.createElement("span");
      s.className = "ripple";
      s.style.cssText = `left:${e.clientX - r.left}px;top:${e.clientY - r.top}px;width:${d}px;height:${d}px`;
      b.appendChild(s);
      s.addEventListener("animationend", ev => { if (ev.animationName === "rippleS") s.remove(); });
    });
  }

  function bindSpotlight() {
    $$(".card").forEach(card => {
      const st = { a: 0, ta: 0, x: 0, y: 0, tx: 0, ty: 0, seen: false };
      card.addEventListener("pointermove", e => {
        const r = card.getBoundingClientRect();
        st.tx = e.clientX - r.left; st.ty = e.clientY - r.top;
        st.ta = 1;
        if (!st.seen) { st.x = st.tx; st.y = st.ty; st.seen = true; }
      });
      card.addEventListener("pointerleave", () => { st.ta = 0; });
      let prev = performance.now();
      const loop = now => {
        const dt = Math.min((now - prev) / 1000, .05); prev = now;
        st.a += (st.ta - st.a) * (1 - Math.exp(-dt * 7));
        st.x += (st.tx - st.x) * (1 - Math.exp(-dt * 12));
        st.y += (st.ty - st.y) * (1 - Math.exp(-dt * 12));
        card.style.setProperty("--sa", st.a.toFixed(3));
        card.style.setProperty("--mx", st.x.toFixed(1) + "px");
        card.style.setProperty("--my", st.y.toFixed(1) + "px");
        requestAnimationFrame(loop);
      };
      requestAnimationFrame(loop);
    });
  }

  function bindModals() {
    /* a dialog is a host element ([data-modal] or .success): toggling .is-open on the host
       opens the inner .modal / .success surface; a .scrim child dims what is behind. */
    const host = el => (el && (el.closest("[data-modal]") || el.closest(".modal, .success") || el));
    const open = id => {
      const w = document.getElementById(id);
      if (!w) return;
      const h = host(w) || w;
      h.classList.add("is-open");
      h.setAttribute("aria-hidden", "false");
      w.classList.add("is-open");
      $(".scrim", h)?.classList.add("is-open");
      $("[data-autofocus]", h)?.focus({ preventScroll: true });
    };
    const close = target => {
      const h = host(target);
      if (!h) return;
      h.classList.remove("is-open");
      h.setAttribute("aria-hidden", "true");
      $(".modal", h)?.classList.remove("is-open");
      if (h.classList.contains("success") || h.classList.contains("modal")) h.classList.remove("is-open");
      $(".scrim", h)?.classList.remove("is-open");
      $$(".modal.is-open", h).forEach(m => m.classList.remove("is-open"));
    };
    document.addEventListener("click", e => {
      const o = e.target.closest("[data-open]");
      if (o) { open(o.dataset.open); return; }
      const c = e.target.closest("[data-close]");
      if (c) { close(c); return; }
      if (e.target.classList.contains("scrim")) {
        $(".scrim.is-open")?.classList.remove("is-open");
        e.target.closest("[data-modal]")?.classList.remove("is-open");
        e.target.parentElement?.querySelectorAll(".modal.is-open").forEach(m => m.classList.remove("is-open"));
      }
    });
    addEventListener("keydown", e => {
      if (e.key !== "Escape") return;
      const hosts = new Set($$(".modal.is-open, .success.is-open, [data-modal].is-open").map(host));
      hosts.forEach(close);
    });
    return { open, close };
  }

  const toast = (msg, kind = "") => {
    let box = $(".toasts");
    if (!box) { box = document.createElement("div"); box.className = "toasts"; document.body.appendChild(box); }
    const t = document.createElement("div");
    t.className = "toast " + kind;
    t.textContent = msg;
    box.appendChild(t);
    setTimeout(() => { t.classList.add("out"); setTimeout(() => t.remove(), 400); }, 2600);
  };

  const progress = el => ({
    run() { el.classList.remove("is-fin", "is-out"); el.classList.add("is-run"); },
    finish() {
      el.classList.remove("is-run"); el.classList.add("is-fin");
      setTimeout(() => el.classList.add("is-out"), 1000);
      setTimeout(() => el.classList.remove("is-fin", "is-out"), 1600);
    }
  });

  const busy = (btn, on) => {
    if (!btn) return;
    btn.classList.toggle("is-busy", !!on);
    btn.disabled = !!on;
  };
  const flash = (btn, kind) => {
    if (!btn) return;
    if (kind === "done") {
      btn.classList.remove("is-done"); void btn.offsetWidth; btn.classList.add("is-done");
      setTimeout(() => btn.classList.remove("is-done"), 1800);
    } else {
      btn.classList.remove("is-error"); void btn.offsetWidth; btn.classList.add("is-error");
      btn.addEventListener("animationend", () => btn.classList.remove("is-error"), { once: true });
    }
  };

  /* success dialog with rays, blobs and a confetti burst */
  const success = (() => {
    let parts = [], sparks = [], raf = 0, t0 = 0, prev = 0, open = false;
    const el = () => $(".success");
    const cv = () => $("canvas", el());
    function frame(now) {
      const c = cv(); if (!c) return;
      const ctx = c.getContext("2d");
      const dt = Math.min((now - prev) / 1000, .05); prev = now;
      const t = (now - t0) / 1000;
      ctx.clearRect(0, 0, innerWidth, innerHeight);
      for (const s of sparks) {
        s.y -= s.v * dt;
        if (s.y < -10) { s.y = innerHeight + 10; s.x = Math.random() * innerWidth; }
        const a = Math.max(0, Math.sin(t * s.f + s.p)) * .85 * Math.min(1, t / .8);
        ctx.fillStyle = `rgba(227,227,227,${a})`;
        star(ctx, s.x, s.y, s.s);
      }
      parts = parts.filter(p => p.life < p.max);
      for (const p of parts) {
        p.life += dt; p.vy += 900 * dt; p.vx *= 1 - 1.4 * dt; p.vy *= 1 - .6 * dt;
        p.x += p.vx * dt; p.y += p.vy * dt; p.r += p.vr * dt;
        const a = Math.min(1, (p.max - p.life) / .6);
        ctx.save(); ctx.translate(p.x, p.y); ctx.rotate(p.r);
        ctx.fillStyle = `rgba(${p.c},${a})`;
        if (p.sq) ctx.fillRect(-p.s / 2, -p.s / 4, p.s, p.s / 2);
        else { ctx.beginPath(); ctx.arc(0, 0, p.s / 2.2, 0, 6.2832); ctx.fill(); }
        ctx.restore();
      }
      if (open) raf = requestAnimationFrame(frame);
    }
    function star(ctx, x, y, r) {
      ctx.beginPath(); ctx.moveTo(x, y - r);
      ctx.quadraticCurveTo(x, y, x + r, y); ctx.quadraticCurveTo(x, y, x, y + r);
      ctx.quadraticCurveTo(x, y, x - r, y); ctx.quadraticCurveTo(x, y, x, y - r);
      ctx.fill();
    }
    return {
      open(chips = []) {
        const m = el(); if (!m) return;
        const c = cv(), dpr = devicePixelRatio || 1;
        c.width = innerWidth * dpr; c.height = innerHeight * dpr;
        c.getContext("2d").setTransform(dpr, 0, 0, dpr, 0, 0);

        const box = $(".chips", m);
        if (box) {
          box.textContent = "";
          for (const [label, value] of chips) {
            const d = document.createElement("div");
            d.className = "chip";
            d.innerHTML = '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12.5l4.5 4.5L19 7"/></svg><b></b><span></span>';
            d.children[1].textContent = label;
            d.children[2].textContent = value;
            box.appendChild(d);
          }
          box.style.display = box.children.length ? "" : "none";
        }
        m.classList.add("is-open");
        m.setAttribute("aria-hidden", "false");
        open = true;
        const ring = $(".ringbox", m).getBoundingClientRect();
        const cx = ring.left + ring.width / 2, cy = ring.top + ring.height / 2;
        const cols = ["255,255,255", "226,228,234", "110,231,183", "255,255,255", "190,192,200"];
        parts = Array.from({ length: 90 }, () => {
          const a = -Math.PI / 2 + (Math.random() - .5) * 2.6;
          const v = 260 + Math.random() * 620;
          return { x: cx, y: cy, vx: Math.cos(a) * v, vy: Math.sin(a) * v,
            r: Math.random() * 6.28, vr: (Math.random() - .5) * 12, s: 3 + Math.random() * 5,
            c: cols[(Math.random() * cols.length) | 0], sq: Math.random() < .5, life: 0,
            max: 1.8 + Math.random() * 1.2 };
        });
        sparks = Array.from({ length: 26 }, () => ({ x: Math.random() * innerWidth, y: Math.random() * innerHeight,
          s: 2 + Math.random() * 4, v: 6 + Math.random() * 14, p: Math.random() * 6.28, f: .8 + Math.random() * 1.6 }));
        prev = t0 = performance.now();
        cancelAnimationFrame(raf); raf = requestAnimationFrame(frame);
      },
      close() {
        const m = el(); if (!m) return;
        m.classList.remove("is-open"); m.setAttribute("aria-hidden", "true");
        open = false;
        setTimeout(() => { const c = cv(); c && c.getContext("2d").clearRect(0, 0, innerWidth, innerHeight); }, 450);
      }
    };
  })();

  /* app-level wallpaper + rain loop shared by both layers */
  let wallpaperCtl = null;
  function mount(opts = {}) {
    if (document.documentElement.dataset.kit === "on") return;
    document.documentElement.dataset.kit = "on";
    if (opts.cursor !== false) cursor(opts);
    wallpaperCtl = wallpaper($("#wallpaper"), opts);
    bindSwitches(); bindSliders(); bindTabs(); bindRipples(); bindSpotlight(); bindModals();
    if (opts.reduceMotion) document.documentElement.classList.toggle("reduce-motion", !!opts.reduceMotion);
  }

  window.UIKit = { mount, cursor, wallpaper, rain, toast, progress, busy, flash, success,
    get wallpaperCtl() { return wallpaperCtl; }, store };
  document.addEventListener("DOMContentLoaded", () => mount(window.UIKIT_OPTS || {}), { once: true });
})();
