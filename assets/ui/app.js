/* ══════════════════════════════════════════════════════════════
   VOID — client
   no framework · no build step · no dependencies
   ══════════════════════════════════════════════════════════════ */
(() => {
'use strict';

const $  = (s, r = document) => r.querySelector(s);
const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
const esc = s => String(s == null ? '' : s).replace(/[&<>"']/g, c =>
  ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

const app = {
  catalog: { tweaks: [], categories: [], profiles: [] },
  state: null,
  log: [],
  stats: [],
  jitter: [],
  bench: null,
  view: 'dashboard',
  filter: { cat: 'all', risk: 3, q: '' },
  selected: new Set(),
  audio: { on: false, vol: 34, preset: 'pad', uisfx: true },
  aim: { dpi: 800, sens: 95, lock: false, edpi: 760 },
};

const store = {
  get(k, d) { try { const v = localStorage.getItem('void.' + k); return v === null ? d : JSON.parse(v); } catch (e) { return d; } },
  set(k, v) { try { localStorage.setItem('void.' + k, JSON.stringify(v)); } catch (e) {} },
};

/* ═══════════════════ api ═══════════════════ */
const api = {
  async req(path, method) {
    const r = await fetch(path, { method, cache: 'no-store', headers: { 'Accept': 'application/json' } });
    if (!r.ok) throw new Error(path + ' → ' + r.status);
    return r.json();
  },
  get(p) { return this.req(p, 'GET'); },
  post(p) { return this.req(p, 'POST'); },
};

/* ═══════════════════ toasts ═══════════════════ */
function toast(kind, title, body) {
  const el = document.createElement('div');
  el.className = 'toast ' + (kind === 'err' ? 'err' : '');
  el.innerHTML = '<span class="tk">' + esc(kind === 'err' ? 'fail' : 'ok') + '</span>' +
    '<div><b>' + esc(title) + '</b>' + (body ? '<div style="color:var(--faint);margin-top:2px">' + esc(body) + '</div>' : '') + '</div>';
  $('#toasts').appendChild(el);
  setTimeout(() => { el.classList.add('is-out'); setTimeout(() => el.remove(), 450); }, 4600);
}

/* ═══════════════════ modal ═══════════════════ */
let modalResolve = null;
function confirmModal({ kicker = 'confirm', title, body, code, ok = 'run' }) {
  return new Promise(resolve => {
    modalResolve = resolve;
    $('#modal-kicker').textContent = kicker;
    $('#modal-title').textContent = title;
    $('#modal-body').textContent = body || '';
    const pre = $('#modal-code');
    pre.textContent = code || '';
    pre.style.display = code ? 'block' : 'none';
    $('#modal-ok').textContent = ok;
    $('#modal').classList.add('is-open');
  });
}
function closeModal(v) {
  $('#modal').classList.remove('is-open');
  if (modalResolve) { modalResolve(v); modalResolve = null; }
}

/* ═══════════════════ ambient engine ═══════════════════ */
const Sound = {
  ctx: null, master: null, nodes: [], playing: false, preset: 'pad',
  ensure() {
    if (this.ctx) return true;
    const AC = window.AudioContext || window.webkitAudioContext;
    if (!AC) return false;
    this.ctx = new AC();
    this.master = this.ctx.createGain();
    this.master.gain.value = 0;
    this.master.connect(this.ctx.destination);
    return true;
  },
  gainFor(v) { return Math.pow(clamp(v, 0, 100) / 100, 1.7) * 0.6; },
  build(preset) {
    if (!this.ctx) return;
    this.preset = preset;
    this.stopNodes();
    const ctx = this.ctx;
    if (preset === 'rain') {
      const len = Math.floor(ctx.sampleRate * 4);
      const buf = ctx.createBuffer(1, len, ctx.sampleRate);
      const d = buf.getChannelData(0);
      let last = 0;
      for (let i = 0; i < len; i++) {
        const w = Math.random() * 2 - 1;
        last = (last + 0.02 * w) / 1.02;
        d[i] = last * 3.4;
      }
      const src = ctx.createBufferSource(); src.buffer = buf; src.loop = true;
      const bp = ctx.createBiquadFilter(); bp.type = 'bandpass'; bp.frequency.value = 780; bp.Q.value = 0.55;
      const lp = ctx.createBiquadFilter(); lp.type = 'lowpass'; lp.frequency.value = 2400;
      const g = ctx.createGain(); g.gain.value = 0.55;
      src.connect(bp); bp.connect(lp); lp.connect(g); g.connect(this.master);
      src.start();
      // slow swell
      const lfo = ctx.createOscillator(); lfo.frequency.value = 0.031;
      const lg = ctx.createGain(); lg.gain.value = 0.16;
      lfo.connect(lg); lg.connect(g.gain); lfo.start();
      this.nodes = [src, bp, lp, g, lfo, lg];
      return;
    }
    const spec = preset === 'deep'
      ? { freqs: [41.2, 61.74, 82.41, 110.0], type: 'sine', cut: 340, q: 0.9, det: 6, lfo: 0.021 }
      : { freqs: [110.0, 164.81, 220.0, 329.63], type: 'sine', cut: 900, q: 0.4, det: 9, lfo: 0.037 };
    const filter = ctx.createBiquadFilter();
    filter.type = 'lowpass'; filter.frequency.value = spec.cut; filter.Q.value = spec.q;
    filter.connect(this.master);
    const lfo = ctx.createOscillator(); lfo.frequency.value = spec.lfo;
    const lfoGain = ctx.createGain(); lfoGain.gain.value = spec.cut * 0.35;
    lfo.connect(lfoGain); lfoGain.connect(filter.frequency); lfo.start();
    this.nodes.push(lfo, lfoGain, filter);
    spec.freqs.forEach((f, i) => {
      const o = ctx.createOscillator();
      o.type = spec.type;
      o.frequency.value = f;
      o.detune.value = (i % 2 === 0 ? 1 : -1) * spec.det;
      const g = ctx.createGain();
      g.gain.value = (preset === 'deep' ? 0.30 : 0.16) / (1 + i * 0.35);
      o.connect(g); g.connect(filter);
      // very slow amplitude drift per partial
      const al = ctx.createOscillator(); al.frequency.value = 0.05 + i * 0.017;
      const ag = ctx.createGain(); ag.gain.value = g.gain.value * 0.45;
      al.connect(ag); ag.connect(g.gain); al.start();
      o.start();
      this.nodes.push(o, g, al, ag);
    });
  },
  stopNodes() {
    this.nodes.forEach(n => { try { if (n.stop) n.stop(); n.disconnect(); } catch (e) {} });
    this.nodes = [];
  },
  start() {
    if (!this.ensure()) return false;
    if (this.ctx.state === 'suspended') this.ctx.resume();
    if (!this.playing) { this.build(app.audio.preset); this.playing = true; }
    this.volume(app.audio.vol);
    return true;
  },
  stop() {
    this.playing = false;
    if (this.master) this.master.gain.setTargetAtTime(0, this.ctx.currentTime, 0.25);
    setTimeout(() => { if (!this.playing) this.stopNodes(); }, 900);
  },
  volume(v) {
    if (!this.master) return;
    const target = this.playing ? this.gainFor(v) : 0;
    this.master.gain.setTargetAtTime(target, this.ctx.currentTime, 0.18);
  },
  presetChange(p) { this.build(p); if (this.master) this.volume(app.audio.vol); },
  tick(freq) {
    if (!this.ctx || !this.playing || !app.audio.uisfx || app.audio.vol === 0) return;
    const t = this.ctx.currentTime;
    const o = this.ctx.createOscillator();
    const g = this.ctx.createGain();
    o.type = 'sine';
    o.frequency.value = freq || 1180;
    g.gain.setValueAtTime(0.0001, t);
    g.gain.linearRampToValueAtTime(0.14, t + 0.006);
    g.gain.exponentialRampToValueAtTime(0.0001, t + 0.11);
    o.connect(g); g.connect(this.master);
    o.start(t); o.stop(t + 0.13);
  },
};

function paintAudioUI() {
  const on = app.audio.on;
  $('#audio-toggle').classList.toggle('is-on', on);
  $('#audio-state').textContent = on ? 'on' : 'off';
  $('#audio-range').value = app.audio.vol;
  $('#audio-val').textContent = app.audio.vol;
  $('#audio-viz').classList.toggle('is-live', on && app.audio.vol > 0);
  $$('.audio-modes .mini').forEach(b => b.classList.toggle('is-on', b.dataset.preset === app.audio.preset));
  const sa = $('#set-audio'); if (sa) { sa.classList.toggle('is-on', on); sa.setAttribute('aria-checked', String(on)); }
  const sv = $('#set-volume'); if (sv) sv.value = app.audio.vol;
  const svv = $('#set-volume-val'); if (svv) svv.textContent = app.audio.vol;
  const uf = $('#set-uisfx'); if (uf) { uf.classList.toggle('is-on', app.audio.uisfx); uf.setAttribute('aria-checked', String(app.audio.uisfx)); }
}

function setAudio(on, silent) {
  app.audio.on = on;
  store.set('audioOn', on);
  api.post('/api/setting?key=muted&value=' + (on ? 0 : 1)).catch(() => {});
  if (on) { if (!Sound.start()) { app.audio.on = false; toast('err', 'no audio', 'WebAudio is unavailable in this browser.'); } }
  else Sound.stop();
  paintAudioUI();
  if (on && !silent) Sound.tick(880);
}

let volPush = null;
function setVolume(v) {
  app.audio.vol = clamp(Math.round(v), 0, 100);
  store.set('vol', app.audio.vol);
  clearTimeout(volPush);
  volPush = setTimeout(() => {
    api.post('/api/setting?key=volume&value=' + app.audio.vol).catch(() => {});
  }, 350);
  Sound.volume(app.audio.vol);
  const s = $('#set-volume'); if (s) s.value = app.audio.vol;
  const reset = $('#set-volume-val'); if (reset) reset.textContent = app.audio.vol;
  paintAudioUI();
}

/* ═══════════════════ dust field ═══════════════════ */
function initDust() {
  const c = $('#dust');
  if (!c) return;
  const ctx = c.getContext('2d');
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  let W = 0, H = 0, parts = [];
  const reduce = () => document.body.classList.contains('reduce-motion') ||
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  function size() {
    W = c.width = Math.floor(innerWidth * dpr);
    H = c.height = Math.floor(innerHeight * dpr);
    c.style.width = innerWidth + 'px';
    c.style.height = innerHeight + 'px';
  }
  function seed() {
    const n = innerWidth < 760 ? 26 : 68;
    parts = Array.from({ length: n }, () => ({
      x: Math.random() * W, y: Math.random() * H,
      r: (Math.random() * 1.3 + 0.25) * dpr,
      vx: (Math.random() - 0.5) * 0.05 * dpr,
      vy: -(Math.random() * 0.16 + 0.02) * dpr,
      a: Math.random() * 0.45 + 0.12,
      tw: Math.random() * Math.PI * 2,
      ts: Math.random() * 0.014 + 0.003,
    }));
  }
  function draw(animate) {
    ctx.clearRect(0, 0, W, H);
    parts.forEach(p => {
      if (animate) {
        p.x += p.vx; p.y += p.vy; p.tw += p.ts;
        if (p.y < -10) { p.y = H + 8; p.x = Math.random() * W; }
        if (p.x < -10) p.x = W + 8; if (p.x > W + 10) p.x = -8;
      }
      const a = p.a * (0.55 + 0.45 * Math.sin(p.tw));
      ctx.beginPath();
      ctx.arc(p.x, p.y, p.r, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(255,255,255,' + a.toFixed(3) + ')';
      ctx.fill();
    });
    if (animate) requestAnimationFrame(() => draw(true));
  }
  size(); seed();
  draw(!reduce());
  let t = null;
  addEventListener('resize', () => { clearTimeout(t); t = setTimeout(() => { size(); seed(); }, 180); });
}

/* ═══════════════════ chrome / data ═══════════════════ */
function catLabel(id) {
  const c = app.catalog.categories.find(x => x.id === id);
  return c ? c.label : id;
}
function tweakById(id) { return app.catalog.tweaks.find(t => t.id === id); }

function renderChrome() {
  const s = app.state;
  if (!s) return;
  $('#chip-mode').textContent = s.simulated ? 'simulated' : 'native';
  $('#chip-mode').classList.toggle('is-warn', !!s.simulated);
  const admin = $('#chip-admin');
  admin.textContent = s.admin ? 'admin' : 'no admin';
  admin.classList.toggle('is-hot', !!s.admin);
  admin.classList.toggle('is-warn', !s.admin);
  $('#rail-host').textContent = (s.host || 'local').toLowerCase();
  $('#sys-dot').classList.toggle('is-warn', !s.admin);
  $('#d-cpu').textContent = s.cpu || '—';
  $('#d-gpu').textContent = s.gpu || '—';
  $('#d-os').textContent = s.os || '—';
  const a = app.audio;
  a.vol = s.settings.volume;
  a.muted = s.settings.muted;
  paintAudioUI();
  const motion = s.settings.motion === false;
  document.body.classList.toggle('reduce-motion', motion);
  ['set-backup', 'set-dry', 'set-motion'].forEach((id, i) => {
    const el = $('#' + id); if (!el) return;
    const on = i === 0 ? !!s.settings.auto_backup : i === 1 ? !!s.dry_run : motion;
    el.classList.toggle('is-on', on);
    el.setAttribute('aria-checked', String(on));
  });
  if ($('#s-host')) $('#s-host').textContent = (s.user || '') + '@' + (s.host || '') ;
}

let lastLogKey = '';
function renderLog() {
  const box = $('#d-log');
  if (!box || !app.log.length) return;
  const key = app.log.length + ':' + app.log[app.log.length - 1].at;
  if (key === lastLogKey) return;
  lastLogKey = key;
  const rows = app.log.slice(-60).map(l =>
    '<div class="t-line ' + esc(l.level) + '"><span class="t">' + esc(l.at) + '</span><span class="m">' + esc(l.msg) + '</span></div>'
  ).join('');
  box.innerHTML = rows;
  box.scrollTop = box.scrollHeight;
}

function readiness() {
  const base = app.catalog.profiles.find(p => p.id === 'competitive');
  if (!base || !app.state) return 0;
  const applied = new Set(app.state.applied || []);
  // one-shot actions are never persisted, so they cannot count toward readiness
  const ids = base.tweaks.filter(id => { const t = tweakById(id); return t && !t.once; });
  const hit = ids.filter(id => applied.has(id)).length;
  return Math.round(hit / Math.max(1, ids.length) * 100);
}

function renderDashboard() {
  if (!app.state) return;
  const pct = readiness();
  const ring = $('#ring-val');
  if (ring) {
    const C = 2 * Math.PI * 86;
    ring.style.strokeDasharray = C.toFixed(1);
    ring.style.strokeDashoffset = (C - C * pct / 100).toFixed(1);
  }
  $('#d-ready').textContent = pct;
  $('#d-verdict').textContent =
    pct >= 85 ? 'Fully tuned. The machine is waiting for you.' :
    pct >= 60 ? 'Competitive. A few levers are still down.' :
    pct >= 25 ? 'Partially tuned. Jitter is still costing frames.' :
    'Untuned. Every frame is being negotiated with Windows.';
  const applied = app.state.applied || [];
  $('#d-applied').textContent = applied.length;
  $('#d-total').textContent = app.state.total_tweaks || app.catalog.tweaks.length;
  const p = app.catalog.profiles.find(x => x.id === app.state.settings.last_profile);
  $('#d-profile').textContent = p ? p.name : 'no profile';

  const list = $('#d-applied-list');
  if (!applied.length) {
    list.innerHTML = '<p class="empty">Nothing applied yet. Ignite the stack.</p>';
  } else {
    list.innerHTML = applied.slice(0, 14).map(id => {
      const t = tweakById(id) || { name: id, cat: '' };
      return '<div class="stack-row"><span class="tick"><svg viewBox="0 0 24 24"><path d="M4 13l5 5L20 7"/></svg></span>' +
        '<span class="n">' + esc(t.name) + '</span><span class="c">' + esc(catLabel(t.cat)) + '</span></div>';
    }).join('');
  }
  renderLog();
  paintStats();
}

function fmtUptime(sec) {
  sec = Math.max(0, Math.round(sec || 0));
  const h = Math.floor(sec / 3600), m = Math.floor((sec % 3600) / 60);
  if (h > 0) return { v: h + 'h ' + m + 'm', unit: '' };
  return { v: m, unit: 'm' };
}

let jitterSpark = [];
function paintStats() {
  const latest = app.stats[app.stats.length - 1];
  if (!latest) return;
  $('#s-cpu').textContent = Math.round(latest.cpu);
  $('#s-ram').textContent = Math.round(latest.ram);
  const up = fmtUptime(latest.uptime);
  $('#s-up').textContent = up.v;
  if ($('#s-up-unit')) $('#s-up-unit').textContent = up.unit;
  const cpuVals = app.stats.map(s => s.cpu);
  const ramVals = app.stats.map(s => s.ram);
  drawSpark($('#spark-cpu'), cpuVals);
  drawSpark($('#spark-ram'), ramVals);
  if (app.bench) {
    $('#s-jitter').textContent = (app.bench.jitter_ms).toFixed(2);
    jitterSpark.push(app.bench.jitter_ms);
    if (jitterSpark.length > 24) jitterSpark.shift();
    drawSpark($('#spark-jit'), jitterSpark);
  }
  // monitor view
  if ($('#m-cpu')) {
    $('#m-cpu').textContent = Math.round(latest.cpu);
    $('#m-ram').textContent = Math.round(latest.ram);
    $('#m-cpu-bar').style.width = clamp(latest.cpu, 1, 100) + '%';
    $('#m-ram-bar').style.width = clamp(latest.ram, 1, 100) + '%';
    $('#m-ramtxt').textContent = (latest.ram_used || 0).toFixed(1) + ' GB of ' + (latest.ram_total || 0).toFixed(1) + ' GB in use';
    const u = fmtUptime(latest.uptime);
    $('#m-uptime').innerHTML = esc(String(u.v)) + '<em>' + esc(u.unit) + '</em>';
    $('#m-boot').textContent = 'host up · ' + (app.state ? app.state.host : '');
    $('#m-ops').textContent = app.state ? app.state.ops : 0;
    drawSpark($('#spark-cpu2'), cpuVals);
    drawSpark($('#spark-ram2'), ramVals);
    if (app.bench) {
      $('#m-score').textContent = Math.round(app.bench.score);
      $('#m-bench-out').textContent =
        'avg ' + app.bench.avg_ms.toFixed(2) + 'ms · p95 ' + app.bench.p95_ms.toFixed(2) +
        'ms · max ' + app.bench.max_ms.toFixed(2) + 'ms · ' + app.bench.samples + ' samples';
    }
  }
}

function drawSpark(canvas, values) {
  if (!canvas) return;
  const ctx = canvas.getContext('2d');
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (!w || !h) return;
  if (canvas.width !== Math.floor(w * dpr) || canvas.height !== Math.floor(h * dpr)) {
    canvas.width = Math.floor(w * dpr);
    canvas.height = Math.floor(h * dpr);
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  const vals = values.length ? values.slice(-40) : [0, 0];
  const max = Math.max(1, ...vals);
  const step = w / Math.max(1, vals.length - 1);
  ctx.beginPath();
  vals.forEach((v, i) => {
    const x = i * step;
    const y = h - 3 - (v / max) * (h - 8);
    i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
  });
  ctx.strokeStyle = 'rgba(255,255,255,.85)';
  ctx.lineWidth = 1.1;
  ctx.lineJoin = 'round';
  ctx.stroke();
  ctx.lineTo(w, h); ctx.lineTo(0, h); ctx.closePath();
  const g = ctx.createLinearGradient(0, 0, 0, h);
  g.addColorStop(0, 'rgba(255,255,255,.16)');
  g.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.fillStyle = g;
  ctx.fill();
  const lastX = (vals.length - 1) * step;
  const lastY = h - 3 - (vals[vals.length - 1] / max) * (h - 8);
  ctx.beginPath(); ctx.arc(lastX - 1, lastY, 1.8, 0, Math.PI * 2);
  ctx.fillStyle = '#fff'; ctx.fill();
}

/* ═══════════════════ optimize view ═══════════════════ */
function riskTag(r) {
  const label = r === 0 ? 'safe' : r === 1 ? 'moderate' : 'advanced';
  return '<span class="risk r' + r + '">' + label + '</span>';
}

function tweakRow(t) {
  const plan = (t.plan || []).join('\n') || '—';
  const rev = (t.revplan || []).join('\n') || '—';
  return '<div class="tweak" data-id="' + esc(t.id) + '">' +
    '<div class="tw-head">' +
      '<div class="tw-left">' +
        '<div class="tw-name">' + esc(t.name) + riskTag(t.risk) +
          (t.reboot ? '<span class="tag">reboot</span>' : '') +
          (t.once ? '<span class="tag">one-shot</span>' : '') +
        '</div>' +
        '<div class="tw-desc">' + esc(t.desc) + '</div>' +
      '</div>' +
      '<div class="tw-meta">' +
        '<span class="tag">' + esc(catLabel(t.cat)) + '</span>' +
        '<button class="switch' + (t.applied ? ' is-on' : '') + '" data-toggle="' + esc(t.id) + '" aria-label="Toggle ' + esc(t.name) + '"><i></i></button>' +
      '</div>' +
    '</div>' +
    '<div class="tw-body"><div class="tw-body-inner">' +
      '<p class="why">' + esc(t.why) + '</p>' +
      '<pre class="plan"><b>apply</b>\n' + esc(plan) + '</pre>' +
      '<pre class="plan"><b>revert</b>\n' + esc(rev) + '</pre>' +
    '</div></div>' +
  '</div>';
}

function filteredTweaks() {
  const f = app.filter;
  return app.catalog.tweaks.filter(t =>
    (f.cat === 'all' || t.cat === f.cat) &&
    t.risk <= f.risk &&
    (!f.q || (t.name + ' ' + t.desc + ' ' + t.why + ' ' + t.id).toLowerCase().includes(f.q))
  );
}

function renderFilters() {
  const box = $('#o-filters');
  if (!box) return;
  const cats = [{ id: 'all', label: 'All' }].concat(app.catalog.categories);
  box.innerHTML = cats.map(c =>
    '<button class="filter' + (app.filter.cat === c.id ? ' is-on' : '') + '" data-cat="' + esc(c.id) + '">' + esc(c.label) + '</button>'
  ).join('') +
  '<span style="width:8px"></span>' +
  [[3, 'all tiers'], [0, 'safe'], [1, 'moderate'], [2, 'advanced']].map(([r, l]) =>
    '<button class="filter' + (app.filter.risk === r ? ' is-on' : '') + '" data-risk="' + r + '">' + l + '</button>'
  ).join('');
}

function renderOptimize() {
  renderFilters();
  const list = $('#o-list');
  if (!list) return;
  const items = filteredTweaks();
  list.innerHTML = items.length ? items.map(tweakRow).join('') :
    '<p class="empty">No tweak matches that filter.</p>';
  $('#o-count').textContent = app.selected.size;
}

function renderKernel() {
  const box = $('#k-list');
  if (!box) return;
  const items = app.catalog.tweaks.filter(t => t.cat === 'kernel' || t.risk === 2);
  box.innerHTML = items.map(tweakRow).join('');
}

function renderNetwork() {
  const box = $('#n-list');
  if (!box) return;
  const items = app.catalog.tweaks.filter(t => t.cat === 'net');
  box.innerHTML = items.map(t =>
    '<div class="stack-row">' +
      '<span class="tick">' + (t.applied ? '<svg viewBox="0 0 24 24"><path d="M4 13l5 5L20 7"/></svg>' : '') + '</span>' +
      '<span class="n">' + esc(t.name) + '</span>' +
      '<span class="c">' + esc(t.once ? 'one-shot' : t.applied ? 'applied' : 'idle') + '</span>' +
      '<button class="switch' + (t.applied ? ' is-on' : '') + '" data-toggle="' + esc(t.id) + '" aria-label="Toggle"><i></i></button>' +
    '</div>'
  ).join('');
}

function renderProfiles() {
  const grid = $('#p-grid');
  if (!grid) return;
  grid.innerHTML = app.catalog.profiles.map(p =>
    '<div class="profile">' +
      '<div class="p-top"><div class="p-name">' + esc(p.name) + '</div>' +
        '<span class="p-count">' + p.count + ' tweaks</span></div>' +
      '<p class="p-tag">' + esc(p.tagline) + '</p>' +
      '<p class="p-note">' + esc(p.note) + '</p>' +
      '<div class="p-actions">' +
        '<button class="btn" data-profile="' + esc(p.id) + '">engage</button>' +
        '<button class="btn ghost" data-preview="' + esc(p.id) + '">recipe</button>' +
      '</div>' +
    '</div>'
  ).join('');
}

/* ═══════════════════ aim lab ═══════════════════ */
const FF_PRESETS = [
  { name: '2-finger · stable', vals: [95, 90, 85, 70, 50, 70] },
  { name: '4-finger claw', vals: [100, 100, 95, 80, 60, 80] },
  { name: 'drag / sniper main', vals: [88, 85, 78, 92, 100, 60] },
  { name: 'FF MAX · 90 fps', vals: [100, 96, 90, 76, 55, 85] },
  { name: 'low-dpi 400', vals: [100, 100, 100, 95, 85, 90] },
];

function renderAim() {
  const box = $('#aim-presets');
  if (box) {
    box.innerHTML = FF_PRESETS.map(p =>
      '<div class="preset"><span>' + esc(p.name) + '</span><b>G' + p.vals[0] + ' · R' + p.vals[1] +
      ' · 2x' + p.vals[2] + ' · 4x' + p.vals[3] + ' · A' + p.vals[4] + ' · FL' + p.vals[5] + '</b></div>'
    ).join('');
  }
  $('#aim-dpi').value = app.aim.dpi;
  $('#aim-sens').value = app.aim.sens;
  aimCompute();
}

function aimCompute() {
  const edpi = Math.round(app.aim.dpi * app.aim.sens / 100);
  app.aim.edpi = edpi;
  const el = $('#aim-edpi');
  if (el) el.textContent = edpi.toLocaleString();
  const lock = $('#aim-lock');
  if (lock) {
    lock.classList.toggle('is-on', !!app.aim.lock);
    lock.textContent = app.aim.lock ? 'edpi locked · ' + edpi : 'lock edpi';
  }
  store.set('aim', app.aim);
}

/* ═══════════════════ engine actions ═══════════════════ */
function busy(btn, on) {
  if (!btn) return;
  btn.classList.toggle('is-busy', on);
  btn.disabled = on;
}

async function refresh() {
  try {
    const [cat, st] = await Promise.all([api.get('/api/catalog'), api.get('/api/state')]);
    app.catalog = cat;
    app.state = st;
    renderChrome();
  } catch (e) {
    toast('err', 'engine offline', 'Could not reach the VOID engine API.');
  }
  try { app.log = await api.get('/api/log'); } catch (e) {}
  renderActive();
}

function renderActive() {
  const v = app.view;
  if (v === 'dashboard') renderDashboard();
  if (v === 'optimize') renderOptimize();
  if (v === 'profiles') renderProfiles();
  if (v === 'kernel') renderKernel();
  if (v === 'network') renderNetwork();
  if (v === 'monitor') paintStats();
  if (v === 'aim') renderAim();
  if (v === 'settings') renderSettings();
}

function handleResults(res) {
  const rows = (res && res.results) || [];
  const ok = rows.filter(r => r.ok).length;
  const bad = rows.length - ok;
  if (bad === 0) toast('ok', ok + ' step' + (ok === 1 ? '' : 's') + ' applied', rows.length === 1 ? rows[0].msg : 'engine journal updated');
  else toast('err', bad + ' step' + (bad === 1 ? '' : 's') + ' failed', rows.find(r => !r.ok).msg);
  Sound.tick(bad ? 320 : 1320);
}

async function doApply(id, opts = {}) {
  const btn = opts.btn;
  busy(btn, true);
  try {
    const res = await api.post('/api/apply?id=' + encodeURIComponent(id));
    handleResults(res);
  } catch (e) {
    toast('err', 'apply failed', String(e.message || e));
  }
  busy(btn, false);
  await refresh();
}

async function doProfile(id) {
  const p = app.catalog.profiles.find(x => x.id === id);
  if (!p) return;
  if (!(await confirmProfile(p))) return;
  try {
    const res = await api.post('/api/profile?id=' + encodeURIComponent(id));
    handleResults(res);
  } catch (e) { toast('err', 'profile failed', String(e.message || e)); }
  await refresh();
}

async function confirmProfile(p) {
  const code = p.tweaks.map(id => {
    const t = tweakById(id);
    return (t ? t.plan : ['void::' + id]).join('\n');
  }).join('\n');
  const warn = app.state && !app.state.simulated && !app.state.admin
    ? '\n\nNo administrator rights detected: run VOID as Administrator or most steps will fail.'
    : '';
  return confirmModal({
    kicker: 'profile · ' + p.count + ' steps',
    title: p.name,
    body: p.note + warn,
    code,
    ok: 'engage',
  });
}

async function doRevert(id) {
  const ok = await confirmModal({
    kicker: 'revert',
    title: id ? 'Revert this tweak?' : 'Revert everything?',
    body: id ? 'VOID will restore the documented Windows default for this tweak.'
             : 'VOID will restore every value it has changed since you started using it.',
    code: id ? ((tweakById(id) || {}).revplan || []).join('\n') : '',
    ok: 'revert',
  });
  if (!ok) return;
  try {
    const res = await api.post('/api/revert' + (id ? '?id=' + encodeURIComponent(id) : '-all'));
    handleResults(res);
  } catch (e) { toast('err', 'revert failed', String(e.message || e)); }
  await refresh();
}

function renderSettings() {
  const box = $('#set-info');
  if (!box || !app.state) return;
  const s = app.state;
  const items = [
    ['version', 'v' + s.version],
    ['platform', s.os],
    ['host', s.host],
    ['user', s.user],
    ['cpu', s.cpu],
    ['gpu', s.gpu],
    ['admin', s.admin ? 'yes' : 'no'],
    ['engine mode', s.simulated ? 'simulation' : 'native'],
    ['dry run', s.dry_run ? 'on' : 'off'],
    ['tweaks in catalog', String(s.total_tweaks)],
    ['applied', String(s.applied_count)],
    ['operations', String(s.ops)],
  ];
  box.innerHTML = items.map(([k, v]) =>
    '<div class="kv"><span class="micro">' + esc(k) + '</span><b title="' + esc(v) + '">' + esc(v) + '</b></div>'
  ).join('');
  paintAudioUI();
}

/* ═══════════════════ navigation ═══════════════════ */
function go(view, opts = {}) {
  const el = $('.view[data-view="' + view + '"]');
  if (!el) return;
  app.view = view;
  store.set('view', view);
  $$('.nav-item').forEach(b => b.classList.toggle('is-active', b.dataset.view === view));
  $$('.view').forEach(v => v.classList.remove('is-active'));
  void el.offsetWidth;
  el.classList.add('is-active');
  $('#crumb').textContent = view === 'aim' ? 'aim lab' : view.replace(/([a-z])([A-Z])/g, '$1 $2');
  $('#views').scrollTop = 0;
  if (!opts.silent) Sound.tick(1560);
  renderActive();
  if (view === 'monitor' || view === 'dashboard') pollStats();
  if (view === 'network') startJitterLoop();
}

/* ═══════════════════ network jitter canvas ═══════════════════ */
let jitterRAF = null;
function startJitterLoop() {
  const c = $('#jitter');
  if (!c) return;
  const ctx = c.getContext('2d');
  const dpr = Math.min(2, window.devicePixelRatio || 1);
  const data = Array.from({ length: 120 }, () => Math.random() * 0.55 + 0.2);
  let phase = 0;
  if (jitterRAF) cancelAnimationFrame(jitterRAF);

  const frame = () => {
    if (app.view !== 'network') { jitterRAF = null; return; }
    const w = c.clientWidth || 600, h = 240;
    if (c.width !== Math.floor(w * dpr)) { c.width = Math.floor(w * dpr); c.height = Math.floor(h * dpr); }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    // baseline grid
    ctx.strokeStyle = 'rgba(255,255,255,.055)';
    ctx.lineWidth = 1;
    for (let i = 1; i < 5; i++) {
      ctx.beginPath(); ctx.moveTo(0, (h / 5) * i); ctx.lineTo(w, (h / 5) * i); ctx.stroke();
    }

    phase += 0.05;
    const noise = app.stats.length ? app.stats[app.stats.length - 1].cpu / 100 : 0.3;
    data.shift();
    data.push(clamp(0.35 + Math.sin(phase) * 0.12 + (Math.random() - 0.5) * 0.34 + noise * 0.25, 0.06, 1));

    const step = w / (data.length - 1);
    ctx.beginPath();
    data.forEach((v, i) => {
      const x = i * step;
      const y = h - 26 - v * (h - 70);
      i ? ctx.lineTo(x, y) : ctx.moveTo(x, y);
    });
    ctx.strokeStyle = 'rgba(255,255,255,.92)';
    ctx.lineWidth = 1.2;
    ctx.stroke();

    ctx.lineTo(w, h); ctx.lineTo(0, h); ctx.closePath();
    const g = ctx.createLinearGradient(0, 0, 0, h);
    g.addColorStop(0, 'rgba(255,255,255,.13)');
    g.addColorStop(1, 'rgba(255,255,255,0)');
    ctx.fillStyle = g; ctx.fill();

    // spikes
    for (let i = 0; i < data.length; i += 17) {
      const v = data[i];
      if (v > 0.72) {
        ctx.beginPath();
        ctx.arc(i * step, h - 26 - v * (h - 70), 1.8, 0, Math.PI * 2);
        ctx.fillStyle = '#fff'; ctx.fill();
      }
    }
    ctx.font = '9px ui-monospace, monospace';
    ctx.fillStyle = 'rgba(255,255,255,.28)';
    ctx.fillText('TICK · ' + (app.state && app.state.simulated ? 'SIMULATED' : 'LIVE'), 8, 16);
    jitterRAF = requestAnimationFrame(frame);
  };
  jitterRAF = requestAnimationFrame(frame);
}

/* ═══════════════════ stats polling ═══════════════════ */
let statsTimer = null;
async function pollStats() {
  if (document.hidden) { clearTimeout(statsTimer); statsTimer = setTimeout(pollStats, 6000); return; }
  try {
    const s = await api.get('/api/stats');
    app.stats.push(s);
    if (app.stats.length > 48) app.stats.shift();
    paintStats();
  } catch (e) {}
  clearTimeout(statsTimer);
  statsTimer = setTimeout(pollStats, 2200);
}

/* ═══════════════════ command palette ═══════════════════ */
const PALETTE = [
  { k: 'dashboard', label: 'Go — Dashboard', tag: 'view', run: () => go('dashboard') },
  { k: 'optimize', label: 'Go — Optimize', tag: 'view', run: () => go('optimize') },
  { k: 'profiles', label: 'Go — Profiles', tag: 'view', run: () => go('profiles') },
  { k: 'freefire', label: 'Go — Free Fire', tag: 'view', run: () => go('freefire') },
  { k: 'network', label: 'Go — Network', tag: 'view', run: () => go('network') },
  { k: 'kernel', label: 'Go — Kernel', tag: 'view', run: () => go('kernel') },
  { k: 'monitor', label: 'Go — Monitor', tag: 'view', run: () => go('monitor') },
  { k: 'aim', label: 'Go — Aim Lab', tag: 'view', run: () => go('aim') },
  { k: 'settings', label: 'Go — Settings', tag: 'view', run: () => go('settings') },
  { k: 'act-ignite', label: 'Apply — Competitive Sweat', tag: 'action', run: () => doProfile('competitive') },
  { k: 'act-ff', label: 'Apply — Free Fire stack', tag: 'action', run: () => doProfile('freefire') },
  { k: 'act-revert', label: 'Revert everything VOID changed', tag: 'action', run: () => doRevert(null) },
  { k: 'act-restore', label: 'Create a System Restore point', tag: 'action', run: createRestore },
  { k: 'act-bench', label: 'Run the jitter benchmark', tag: 'action', run: runBench },
  { k: 'act-audio', label: 'Toggle ambient sound', tag: 'audio', run: () => setAudio(!app.audio.on) },
  { k: 'act-dry', label: 'Toggle dry run', tag: 'safety', run: toggleDry },
];
let paletteIdx = 0, paletteItems = [];

function openPalette() {
  $('#palette').classList.add('is-open');
  const input = $('#palette-input');
  input.value = '';
  paletteIdx = 0;
  filterPalette('');
  setTimeout(() => input.focus(), 40);
}
function closePalette() { $('#palette').classList.remove('is-open'); }

function filterPalette(q) {
  q = q.toLowerCase().trim();
  paletteItems = PALETTE.filter(p => !q || p.label.toLowerCase().includes(q) || p.tag.includes(q));
  const box = $('#palette-list');
  box.innerHTML = paletteItems.length ? paletteItems.map((p, i) =>
    '<button class="palette-item' + (i === paletteIdx ? ' is-sel' : '') + '" data-pi="' + i + '">' +
      esc(p.label) + '<span class="pk">' + esc(p.tag) + '</span></button>'
  ).join('') : '<div class="palette-item" style="color:var(--faint)">nothing matches</div>';
}
function paletteMove(d) {
  paletteIdx = clamp(paletteIdx + d, 0, Math.max(0, paletteItems.length - 1));
  $$('.palette-item').forEach((el, i) => el.classList.toggle('is-sel', i === paletteIdx));
}
function paletteRun(i) {
  const item = paletteItems[i];
  closePalette();
  if (item) setTimeout(() => item.run(), 60);
}

/* ═══════════════════ misc actions ═══════════════════ */
async function createRestore() {
  try {
    const res = await api.post('/api/restore');
    handleResults(res);
  } catch (e) { toast('err', 'restore point failed', String(e.message || e)); }
  await refresh();
}

async function runBench(silent) {
  if (!silent) toast('ok', 'benchmarking', '120 samples of 1 ms sleeps…');
  try {
    const b = await api.get('/api/bench');
    app.bench = b;
    if (!silent) toast('ok', 'jitter ' + b.jitter_ms.toFixed(2) + ' ms', 'score ' + Math.round(b.score) + '/100');
    if (app.state) app.state.ops = (app.state.ops || 0) + 0;
    paintStats();
  } catch (e) { toast('err', 'benchmark failed', String(e.message || e)); }
  Sound.tick(990);
}

async function toggleDry() {
  if (!app.state) return;
  const next = !app.state.dry_run;
  try {
    await api.post('/api/setting?key=dry_run&value=' + (next ? 1 : 0));
    toast('ok', 'dry run ' + (next ? 'on' : 'off'), next ? 'Commands will be printed, nothing changed.' : 'Commands will actually run.');
  } catch (e) {}
  await refresh();
}

async function setSetting(key, value, label) {
  try {
    await api.post('/api/setting?key=' + encodeURIComponent(key) + '&value=' + encodeURIComponent(value));
    if (label) toast('ok', label, null);
  } catch (e) { toast('err', 'setting failed', String(e.message || e)); }
  await refresh();
}

/* ═══════════════════ events ═══════════════════ */
function bind() {
  // nav
  $('#nav').addEventListener('click', e => {
    const b = e.target.closest('.nav-item');
    if (b) go(b.dataset.view);
  });
  // goto links
  document.addEventListener('click', e => {
    const g = e.target.closest('[data-goto]');
    if (g) go(g.dataset.goto);
  });

  // palette buttons
  $('#palette-hint').addEventListener('click', openPalette);
  $('#palette-input').addEventListener('input', e => { paletteIdx = 0; filterPalette(e.target.value); });
  $('#palette-input').addEventListener('keydown', e => {
    if (e.key === 'ArrowDown') { e.preventDefault(); paletteMove(1); }
    if (e.key === 'ArrowUp') { e.preventDefault(); paletteMove(-1); }
    if (e.key === 'Enter') { e.preventDefault(); paletteRun(paletteIdx); }
    if (e.key === 'Escape') closePalette();
  });
  $('#palette').addEventListener('click', e => {
    const item = e.target.closest('[data-pi]');
    if (item) paletteRun(Number(item.dataset.pi));
    else if (e.target.id === 'palette') closePalette();
  });

  // modal
  $('#modal-x').addEventListener('click', () => closeModal(false));
  $('#modal-cancel').addEventListener('click', () => closeModal(false));
  $('#modal-ok').addEventListener('click', () => closeModal(true));
  $('#modal').addEventListener('click', e => { if (e.target.id === 'modal') closeModal(false); });

  // audio
  $('#audio-toggle').addEventListener('click', () => setAudio(!app.audio.on));
  $('#audio-range').addEventListener('input', e => setVolume(e.target.value));
  $('#set-volume').addEventListener('input', e => setVolume(e.target.value));
  $$('.audio-modes .mini').forEach(b => b.addEventListener('click', () => {
    app.audio.preset = b.dataset.preset;
    store.set('preset', app.audio.preset);
    if (app.audio.on) Sound.presetChange(app.audio.preset);
    paintAudioUI();
    Sound.tick(1050);
  }));
  $('#set-audio').addEventListener('click', () => setAudio(!app.audio.on));
  $('#set-uisfx').addEventListener('click', () => {
    app.audio.uisfx = !app.audio.uisfx;
    store.set('uisfx', app.audio.uisfx);
    paintAudioUI();
    Sound.tick(760);
  });

  // hero + quick actions
  $('#d-quick').addEventListener('click', () => doProfile('competitive'));
  $('#d-revert').addEventListener('click', () => doRevert(null));
  $('#btn-ignite').addEventListener('click', () => {
    if (app.state && app.state.settings && app.state.settings.last_profile) doProfile(app.state.settings.last_profile);
    else doProfile('competitive');
  });

  // optimize
  $('#o-search').addEventListener('input', e => {
    app.filter.q = e.target.value.toLowerCase().trim();
    renderOptimize();
  });
  $('#o-filters').addEventListener('click', e => {
    const f = e.target.closest('.filter');
    if (!f) return;
    if (f.dataset.cat) app.filter.cat = f.dataset.cat;
    if (f.dataset.risk) app.filter.risk = Number(f.dataset.risk);
    renderOptimize();
    Sound.tick(1720);
  });
  $('#o-select-safe').addEventListener('click', () => {
    app.catalog.tweaks.filter(t => t.risk === 0 && !t.applied).forEach(t => app.selected.add(t.id));
    $('#o-count').textContent = app.selected.size;
    toast('ok', app.selected.size + ' safe tweaks selected', 'Apply selected to run them.');
  });
  $('#o-select-none').addEventListener('click', () => {
    app.selected.clear();
    $('#o-count').textContent = 0;
  });
  $('#o-apply').addEventListener('click', async () => {
    if (!app.selected.size) { toast('err', 'nothing selected', 'Toggle tweaks or use “safe only”.'); return; }
    const ids = Array.from(app.selected);
    const code = ids.map(id => (tweakById(id) || {}).plan || ['void::' + id]).map(x => x.join('\n')).join('\n');
    const ok = await confirmModal({
      kicker: 'bulk apply', title: ids.length + ' tweaks',
      body: 'These commands run in order. Any failure is reported in the journal.',
      code, ok: 'apply'
    });
    if (!ok) return;
    for (const id of ids) await doApply(id);
    app.selected.clear();
    $('#o-count').textContent = 0;
  });
  $('#o-revert').addEventListener('click', () => doRevert(null));

  // toggles inside lists
  document.addEventListener('click', e => {
    const sw = e.target.closest('[data-toggle]');
    if (sw) {
      e.stopPropagation();
      const t = tweakById(sw.dataset.toggle);
      if (!t) return;
      if (t.applied) doRevert(t.id);
      else doApply(t.id, { btn: sw });
      return;
    }
    const head = e.target.closest('.tw-head');
    if (head) {
      const row = head.closest('.tweak');
      if (row) { row.classList.toggle('is-open'); Sound.tick(1840); }
      return;
    }
    const prof = e.target.closest('[data-profile]');
    if (prof) { doProfile(prof.dataset.profile); return; }
    const prev = e.target.closest('[data-preview]');
    if (prev) {
      const p = app.catalog.profiles.find(x => x.id === prev.dataset.preview);
      if (p) confirmModal({ kicker: 'recipe', title: p.name, body: p.note, code: p.tweaks.map(id => (tweakById(id) || { name: id }).name).join('\n'), ok: 'close' });
      return;
    }
  });

  // free fire
  $('#ff-boost').addEventListener('click', () => doApply('ff.boost'));
  $('#ff-priority').addEventListener('click', () => doApply('ff.priority'));
  $('#ff-affinity').addEventListener('click', () => doApply('ff.affinity'));
  $('#ff-network').addEventListener('click', () => doApply('ff.network'));

  // network
  $('#n-flush').addEventListener('click', () => doApply('net.flush'));

  // kernel
  $('#k-restore').addEventListener('click', createRestore);

  // monitor
  $('#m-bench').addEventListener('click', runBench);

  // aim
  $('#aim-dpi').addEventListener('input', e => {
    app.aim.dpi = clamp(Number(e.target.value) || 100, 100, 32000);
    if (app.aim.lock) {
      app.aim.sens = clamp(Math.round(app.aim.edpi * 100 / app.aim.dpi), 1, 100);
      $('#aim-sens').value = app.aim.sens;
    }
    aimCompute();
  });
  $('#aim-sens').addEventListener('input', e => {
    app.aim.sens = clamp(Number(e.target.value) || 1, 1, 100);
    if (app.aim.lock) {
      app.aim.dpi = clamp(Math.round(app.aim.edpi * 100 / app.aim.sens), 100, 32000);
      $('#aim-dpi').value = app.aim.dpi;
    }
    aimCompute();
  });
  $('#aim-lock').addEventListener('click', () => {
    aimCompute();
    app.aim.lock = !app.aim.lock;
    aimCompute();
    Sound.tick(app.aim.lock ? 1300 : 700);
  });

  // settings
  $('#set-backup').addEventListener('click', () => {
    const next = !$('#set-backup').classList.contains('is-on');
    setSetting('auto_backup', next ? 1 : 0, 'auto restore point ' + (next ? 'on' : 'off'));
  });
  $('#set-dry').addEventListener('click', toggleDry);
  $('#set-motion').addEventListener('click', () => {
    const next = !$('#set-motion').classList.contains('is-on');
    setSetting('motion', next ? 0 : 1, 'reduced motion ' + (next ? 'on' : 'off'));
  });
  $('#set-catalog').addEventListener('click', () => window.open('/api/catalog', '_blank'));
  $('#set-revert').addEventListener('click', () => doRevert(null));

  // card spotlight
  document.addEventListener('mousemove', e => {
    const card = e.target.closest('.card');
    if (!card) return;
    const r = card.getBoundingClientRect();
    card.style.setProperty('--mx', (e.clientX - r.left) + 'px');
    card.style.setProperty('--my', (e.clientY - r.top) + 'px');
  });

  // keyboard
  document.addEventListener('keydown', e => {
    const mod = e.metaKey || e.ctrlKey;
    if (mod && e.key.toLowerCase() === 'k') { e.preventDefault(); openPalette(); }
    if (e.key === 'Escape') { closePalette(); if ($('#modal').classList.contains('is-open')) closeModal(false); }
    if (mod && e.key === 'Enter') { e.preventDefault(); doProfile((app.state && app.state.settings.last_profile) || 'competitive'); }
  });

  // first gesture arms audio
  const arm = () => {
    document.removeEventListener('pointerdown', arm);
    if (!app.audio.on && store.get('audioOn', false) && !app.audio.muted) setAudio(true, true);
  };
  document.addEventListener('pointerdown', arm);
}

function clock() {
  const d = new Date();
  const p = n => String(n).padStart(2, '0');
  const el = $('#chip-clock');
  if (el) el.textContent = p(d.getHours()) + ':' + p(d.getMinutes()) + ':' + p(d.getSeconds());
}

/* ═══════════════════ boot ═══════════════════ */
function startBoot() {
  const el = $('#boot');
  if (!el) return;
  const bar = $('#bootbar'), log = $('#bootlog');
  const lines = ['engine online', 'catalog indexed', 'ambient bus armed', 'hairline renderer ready'];
  let i = 0;
  const tick = () => {
    if (!document.body.contains(el)) return;
    bar.style.width = Math.min(100, (i + 1) * 25) + '%';
    log.textContent = lines[Math.min(i, lines.length - 1)];
    i++;
    if (i < lines.length) setTimeout(tick, 230);
    else setTimeout(done, 300);
  };
  const done = () => {
    el.classList.add('is-done');
    setTimeout(() => el.remove(), 950);
  };
  setTimeout(tick, 160);
  el.addEventListener('click', done);
}

function init() {
  const saved = {
    vol: store.get('vol', 34),
    preset: store.get('preset', 'pad'),
    uisfx: store.get('uisfx', true),
    audioOn: store.get('audioOn', false),
  };
  app.audio.vol = clamp(Number(saved.vol) || 34, 0, 100);
  app.audio.preset = saved.preset || 'pad';
  app.audio.uisfx = saved.uisfx !== false;
  app.audio.on = false; // starts on first gesture
  if (app.audio.muted) app.audio.on = false;
  const aim = store.get('aim', app.aim);
  app.aim = Object.assign(app.aim, aim || {});

  const viz = $('#audio-viz');
  if (viz) {
    Array.from(viz.children).forEach((bar, i) => {
      bar.style.setProperty('--h', (6 + Math.round(Math.sin(i * 0.9) * 8 + 10)) + 'px');
      bar.style.setProperty('--k', i);
    });
  }

  initDust();
  bind();
  paintAudioUI();
  startBoot();
  clock();
  setInterval(clock, 1000);

  (async () => {
    await refresh();
    go(store.get('view', 'dashboard'), { silent: true });
    pollStats();
    setTimeout(() => runBench(true), 900);
    setInterval(async () => {
      if (app.view !== 'dashboard' && app.view !== 'settings') return;
      try { app.log = await api.get('/api/log'); renderLog(); } catch (e) {}
    }, 5000);
    setInterval(async () => {
      // Only views without expandable rows are re-rendered on a timer, so a
      // tweak you are reading never collapses under you.
      if (app.view !== 'dashboard' && app.view !== 'monitor' && app.view !== 'settings') return;
      if (document.hidden) return;
      try { const st = await api.get('/api/state'); app.state = st; renderChrome(); renderActive(); } catch (e) {}
    }, 9000);
  })();
}

document.addEventListener('DOMContentLoaded', init);
})();
