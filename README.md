<div align="center">

# V O I D

**A luxury-grade competitive gaming optimizer.**
One Rust binary. Zero dependencies. Every lever documented, every change reversible.

`v0.1.0` · Windows tweaks · Free Fire / FF Max tuned · monochrome UI · ambient engine

[![build](https://github.com/husainprime20/VOID-OPTIMIZER/actions/workflows/build.yml/badge.svg)](https://github.com/husainprime20/VOID-OPTIMIZER/actions/workflows/build.yml)
[![latest release](https://img.shields.io/github/v/release/husainprime20/VOID-OPTIMIZER?label=download&color=ffffff)](https://github.com/husainprime20/VOID-OPTIMIZER/releases/latest)

</div>

---

## Download

**[⬇ VOID.exe — v0.1.0](https://github.com/husainprime20/VOID-OPTIMIZER/releases/latest/download/VOID.exe)** · 588 KB · Windows 10/11 x64
&nbsp;·&nbsp; [portable zip + `RUN-AS-ADMIN.bat` + checksums](https://github.com/husainprime20/VOID-OPTIMIZER/releases/latest)

```
1. download VOID.exe
2. right-click → Run as administrator     (kernel-tier tweaks need elevation)
3. a window opens on http://127.0.0.1:8722 → press IGNITE
```

* Single file. No installer, no runtime, no dependencies, no telemetry.
* Static CRT — no Visual C++ redistributable needed.
* Not code-signed: SmartScreen asks once → **More info** → **Run anyway**.
* `SHA256SUMS.txt` ships with every release.
* Every release is built by GitHub Actions on `windows-latest` and the binary is **executed**
  (`--version`, full catalog dump) before publishing — a release cannot contain a broken exe.

---

## What VOID is

VOID is a **desktop performance toolkit for people who care about the last 3 milliseconds**.
It modifies *Windows* — power, kernel timers, scheduler bias, GPU frame path, TCP latency,
input stack — through the official `powercfg` / `bcdedit` / `reg` / `netsh` / `sc` / `fsutil`
interfaces, and it tells you exactly which command it will run *before* it runs it.

It ships as a **single 588 KB executable with no dependencies at all** — not a single crate
in `Cargo.toml`. The interface is HTML/CSS/JS compiled into the binary with `include_str!`
and served on localhost; the Rust core is the engine, the router and the API.

```
 cargo build --release      # no network needed, no crates, no npm, no Electron
 ./void                     # opens the UI in an app window
```

---

## Build from source (Windows)

```bat
:: 1. install Rust (one command, no admin) — https://rustup.rs
:: 2. build
build.bat

:: 3. run it elevated (kernel-tier tweaks need it)
run-admin.bat
```

Releasing is one commit: put `[release]` in the message (or push a `v*` tag) and the workflow
builds, smoke-tests and publishes `VOID.exe` + zip + `SHA256SUMS.txt` to a GitHub release
targeted at `main`.

`build.bat` produces `void.exe`. Drop it anywhere — it is fully portable. Settings and the
journal live in `%APPDATA%\VOID\`.

**Linux / macOS** (UI review and CLI only — the tweak engine runs in clearly-labelled
simulation mode because every entry is a Windows subsystem):

```sh
./run.sh                       # builds + serves the UI
./target/release/void status   # machine + engine state
```

---

## The interface

Nine sections, all keyboard-drivable (`Ctrl/⌘ + K` opens the command palette):

| Section | What's in it |
|---|---|
| **Dashboard** | Readiness ring vs. the competitive baseline, live CPU/RAM/jitter sparklines, active stack, engine journal |
| **Optimize** | The full catalog — 42 tweaks — filterable by category and risk tier. Expand any row to read **why** it matters and the exact `apply` / `revert` commands |
| **Profiles** | 7 curated bundles, one click each |
| **Free Fire** | Emulator priority, physical-core pinning, low-latency net stack, recommended emulator settings, anti-cheat statement |
| **Network** | Net stack list + a live kernel-tick visualisation |
| **Kernel** | The advanced tier (BCD, HAGS, VBS) with restore-point tooling |
| **Monitor** | Live gauges, sparklines, jitter benchmark (real measurement, 120 samples) |
| **Aim Lab** | eDPI calculator with a lock, Free Fire sensitivity baselines, 5-minute warm-up protocol |
| **Settings** | Ambient audio, volume, UI sounds, reduced motion, dry-run, auto restore points |

**Design language:** pure black `#050506`, hairline borders, a drifting dust field, film grain,
and a 250-weight typographic hierarchy. Everything is monochrome by design — no accent colour,
so the only thing that "pops" is state. Motion respects `prefers-reduced-motion` and the
Settings toggle.

**Ambient engine:** background sound is *synthesised live* with the Web Audio API — three
presets (`pad`, `rain`, `deep`) built from detuned sine partials, a slow LFO on the filter and
shaped noise. That means **zero audio bytes shipped**, real volume control, and a soft UI tick
on every action. It arms on your first click (browser autoplay rules) and remembers your volume.

---

## The catalog

42 reversible tweaks. Risk tiers: **safe** (no downside for gaming), **moderate** (behaviour
change you should understand), **advanced** (BCD/security tier, reboot required).

<details>
<summary><b>Power &amp; thermals</b> (7)</summary>

- Ultimate Performance plan (cloned + activated)
- Unpark all cores (`CPMINCORES 100`)
- Aggressive clock boost (`PROCTHROTTLEMIN 100`, `PERFBOOSTMODE 2`)
- Disable deep idle C-states (advanced, runs hotter)
- PCIe link always on (ASPM off)
- USB selective suspend off
- Disk sleep off
</details>

<details>
<summary><b>Kernel &amp; scheduler</b> (6)</summary>

- High-resolution kernel timer (`useplatformtick`, `disabledynamictick`) — advanced, reboot
- Enhanced TSC sync policy (`tscsyncpolicy enhanced`) — advanced, reboot
- Hardware GPU scheduling (HAGS) — reboot
- Foreground scheduler bias (`Win32PrioritySeparation 38`)
- Disable power throttling
- Disable memory integrity / VBS — advanced, **security trade-off**, reboot
</details>

<details>
<summary><b>CPU &amp; background</b> (3)</summary>

- Disable SysMain (Superfetch)
- Disable Windows Search indexing
- Kill telemetry services (`DiagTrack`, `dmwappushservice`, `AllowTelemetry 0`)
</details>

<details>
<summary><b>GPU &amp; frame pacing</b> (3)</summary>

- Disable Game DVR / background capture
- Bypass fullscreen optimisations (true exclusive fullscreen)
- Disable NVIDIA telemetry container
</details>

<details>
<summary><b>Network &amp; latency</b> (9)</summary>

- Nagle's algorithm off on **every** interface — enumerates `Tcpip\Parameters\Interfaces`
- Low-latency congestion provider (CTCP, build-aware fallback)
- TCP auto-tuning normal, RSS on, ECN off
- Network throttling off (`NetworkThrottlingIndex 0xFFFFFFFF`)
- System responsiveness → 10
- MMCSS Games task profile (GPU priority 8, scheduling category High)
- Flush DNS cache (one-shot)
</details>

<details>
<summary><b>Input</b> (3)</summary>

- Mouse acceleration off (`MouseSpeed/Threshold1/Threshold2`)
- Zero keyboard delay, max repeat
- Shrink HID input queues (advanced-ish, reboot)
</details>

<details>
<summary><b>Windows surface</b> (3) &amp; <b>Storage</b> (4)</summary>

- Freeze background (UWP) apps
- Game Mode on, tips off, app capture off
- Best-performance visuals
- Ensure TRIM, disable last-access writes, disable prefetch on SSD, purge temp files
</details>

<details>
<summary><b>Free Fire</b> (4)</summary>

- Emulator → **HIGH** priority (BlueStacks, LDPlayer, MuMu, MEmu, Nox)
- Pin emulator to **physical cores** (skips SMT siblings)
- Emulator low-latency net stack (Nagle off + throttling off + MMCSS)
- Pre-match boost: priority + affinity + working-set unlock in one pass
</details>

### Profiles

| Profile | Intent |
|---|---|
| **Competitive Sweat** | The full safe stack — 21 steps, no reboot |
| **Free Fire** | Emulator-tuned: power, latency, priority, affinity |
| **Pure Latency** | Kernel timer + TSC + net + input, reboot expected |
| **Frame Pacing** | HAGS, exclusive fullscreen, DVR off, PCIe always-on |
| **Kernel Purist** | Advanced only. Read the warnings. |
| **Stream Ready** | High FPS with encoder headroom |
| **Benchmark Clean** | Removes the variables that make runs lie |

---

## Safety model

VOID is built for people who would rather understand a tweak than trust a "one-click booster".

1. **Every command is shown before it runs.** Expand any tweak, or look at the confirmation
   dialog — the exact `reg`/`powercfg`/`bcdedit` line is printed.
2. **Dry run** (`void --dry-run`, or Settings → Dry run) prints everything and changes nothing.
3. **Auto restore point** before any `advanced` tweak (System Restore checkpoint).
4. **`revert` is a first-class citizen.** Every entry stores its documented default.
   `void revert` or Dashboard → *Revert all* puts Windows back.
5. **A journal** of every operation: `%APPDATA%\VOID\void.log`.
6. **No game files, no memory, no injection, no DLLs, no drivers.** VOID runs *no* kernel
   driver and *no* code inside any game process. It only writes Windows configuration.

### On Free Fire and anti-cheat

VOID cannot get you banned by design: it never reads or writes Free Fire files, never attaches
to the emulator's process memory, and never modifies APKs. It changes **Windows** scheduling,
power and networking. Anything that claims to "boost" by injecting into the game is a cheat
and a ban risk — VOID is the opposite of that.

---

## CLI

```text
void                        open the UI (app window when possible)
void serve [--host H] [--port P] [--no-window]
void status [--json]        machine + engine state
void catalog                full catalog as JSON
void apply <id> [--dry-run] apply a profile or one tweak
void revert [--dry-run]     revert everything VOID applied
void bench                  sleep-jitter benchmark (JSON)
```

Examples:

```bat
void apply competitive
void apply ff.affinity
void apply kernel.timer --dry-run
void status --json
```

---

## HTTP API

The UI is just a client. Everything works from `curl`:

| Method | Route | Purpose |
|---|---|---|
| GET | `/api/catalog` | tweaks, categories, profiles (+ `applied` flags) |
| GET | `/api/state` | machine identity, applied set, settings, counters |
| GET | `/api/log` | engine journal |
| GET | `/api/stats` | CPU, RAM, uptime sample |
| GET | `/api/bench` | run the jitter benchmark |
| POST | `/api/apply?id=` | apply one tweak |
| POST | `/api/profile?id=` | engage a profile |
| POST | `/api/revert?id=` / `/api/revert-all` | revert one / everything |
| POST | `/api/restore` | System Restore checkpoint |
| POST | `/api/setting?key=&value=` | volume, muted, motion, dry_run, auto_backup |

The server binds **127.0.0.1** by default. Do not expose it to a LAN: it can write system
settings. Use `--host` only if you understand that.

---

## Architecture

```
src/
  main.rs            CLI, routing, static asset embedding
  http.rs            micro HTTP/1.1 server on std::net (thread per conn)
  json.rs            hand-rolled JSON encoder
  platform.rs        process exec, admin check, machine identity, dates
  engine/
    catalog.rs       42 tweaks + 7 profiles as static data (documented, reversible)
    mod.rs           state, apply/revert engine, "specials", telemetry, benchmark
assets/ui/
  index.html         the interface
  app.css            design system
  app.js             client (no framework, no build step)
tools/
  preview.py         UI preview server — parses the real catalog, simulates every action
```

**Why no crates?** A gaming optimizer runs as Administrator on someone else's machine. Fewer
moving parts means fewer supply-chain surfaces, no CVE-chasing, no build tooling, and an
auditable ~3k lines. The whole product builds offline in a few seconds.

**Why is the UI HTML in a Rust binary?** It keeps the binary tiny (no GUI toolkit, no WebView2
dependency), and gives the kind of typography and animation that native toolkits make painful.
The Rust core owns every privileged action — the UI can only ask for a named, catalogued tweak.

---

## Preview the UI without Rust

```sh
python3 tools/preview.py --port 8787
```

It parses `src/engine/catalog.rs` directly (so the list can never drift), answers the same JSON
API, and **simulates every action** — nothing on your system is touched. Handy for design work
and for reviewing on Linux/macOS.

---

## Roadmap (v0.2)

- Per-device **MSI mode** (interrupt affinity) with automatic PCI enumeration
- Per-executable **GPU preference** (`DirectX\UserGpuPreferences`) for detected emulators
- Embedded Windows **manifest** so the exe self-elevates
- Tray icon + global hotkey for the pre-match boost
- ETW-based frame-time capture instead of synthetic tick graphs
- Import/export of profiles as JSON, shareable between machines

---

## License

MIT. Use it, fork it, ship it. No telemetry, no accounts, no network calls except localhost.

<div align="center"><sub>built for people who care about the last 3 milliseconds</sub></div>
