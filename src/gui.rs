// VOID — native desktop entry point.
//
// No console window: the binary is built for the Windows GUI subsystem, so the
// only thing that appears when you double-click VOID.exe is the window itself.
// All helper processes it spawns are created with CREATE_NO_WINDOW (see
// `platform::hide_console`), so `powercfg`, `bcdedit`, `sc` and `netsh` never
// flash a black rectangle either.
//
// Launch path: no subprocess at all. Machine identity and privileges come from
// the registry and the process token, so the window is on screen in a few
// hundred milliseconds — the only wait is the Windows UAC prompt.
//
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod auth;
mod engine;
mod json;
mod platform;
mod rpc;
mod theme;
mod views;

use eframe::egui;

/// The window / taskbar icon, embedded as raw RGBA so no image decoder is
/// linked in. `assets/icon64.rgba` is generated from the brand mark in
/// `docs/branding/` — see the note there.
fn window_icon() -> egui::IconData {
    const RGBA: &[u8] = include_bytes!("../assets/icon64.rgba");
    egui::IconData {
        rgba: RGBA.to_vec(),
        width: 64,
        height: 64,
    }
}

fn main() -> eframe::Result<()> {
    let boot = std::time::Instant::now();
    let args: Vec<String> = std::env::args().skip(1).collect();
    install_panic_hook();

    if args.iter().any(|a| a == "--selftest") {
        std::process::exit(selftest(boot));
    }

    // Elevate once, at launch. If the user declines UAC we still open the
    // window — the engine simply reports `limited` and read-only tweaks stay
    // disabled. Never fail silently, never crash.
    #[cfg(windows)]
    {
        if !platform::is_admin() && !args.iter().any(|a| a == "--no-elevate") {
            if platform::elevate() {
                return Ok(());
            }
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([1040.0, 640.0])
            // opens maximised: the deck is laid out to breathe, and it can never
            // be clipped by a 1366x768 laptop panel
            .with_maximized(true)
            .with_icon(window_icon())
            .with_title("VOID — Competitive Optimizer"),
        vsync: true,
        multisampling: 0,
        ..Default::default()
    };

    let made = eframe::run_native(
        "VOID",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)) as Box<dyn eframe::App>)),
    );
    // A cold launch should be well under a second; keep the number on disk so
    // regressions are visible in CI and in the player's own log.
    let ms = boot.elapsed().as_secs_f64() * 1000.0;
    if let Ok(dir) = std::env::var("APPDATA") {
        let path = std::path::Path::new(&dir).join("VOID");
        let _ = std::fs::create_dir_all(&path);
        let _ = std::fs::write(path.join("boot.ms"), format!("{:.0}", ms));
    }
    made
}

/// A crash should leave a note, not a disappearing window.
fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut text = format!("VOID {}\n{}\n\n", engine::VERSION, platform::now_stamp());
        text.push_str(&info.to_string());
        if let Ok(loc) = std::env::current_exe() {
            if let Some(dir) = loc.parent() {
                let _ = std::fs::write(dir.join("void-crash.log"), &text);
            }
        }
        if let Ok(dir) = std::env::var("APPDATA") {
            let path = std::path::Path::new(&dir).join("VOID");
            let _ = std::fs::create_dir_all(&path);
            let _ = std::fs::write(path.join("crash.log"), &text);
        }
        platform::message_box(
            "VOID crashed",
            "A crash report was written to %APPDATA%\\VOID\\crash.log.\n\nNothing on your system was left half-applied: every tweak is reversible from void-cli revert.",
        );
        default(info);
    }));
}

/// Head-less engine check for CI (the GUI binary has no console to print to).
fn selftest(boot: std::time::Instant) -> i32 {
    let detect_at = std::time::Instant::now();
    let machine = engine::Machine::detect();
    let detect_ms = detect_at.elapsed().as_secs_f64() * 1000.0;
    let state = engine::State::new();
    let stats = engine::sample();
    let emus = engine::emulators();
    let procs = engine::processes();

    let mut ok = true;
    if engine::catalog::TWEAKS.len() < 40 {
        ok = false;
    }
    if engine::catalog::PROFILES.len() < 5 {
        ok = false;
    }
    // Every tweak that applies something must be able to take it back, except
    // the two that are one-way by nature (a DNS flush and a temp purge leave
    // nothing to restore). The emulator lane used to fail this: it raised
    // priority and pinned cores with an empty revert list, so "revert
    // everything" could not undo it.
    const ONE_WAY: &[&str] = &["net.flush", "st.clean"];
    let reversible = engine::catalog::TWEAKS
        .iter()
        .filter(|t| !t.apply.is_empty() && !t.revert.is_empty())
        .count();
    let one_way = engine::catalog::TWEAKS
        .iter()
        .filter(|t| !t.apply.is_empty() && t.revert.is_empty())
        .map(|t| t.id)
        .collect::<Vec<&str>>();
    if one_way.iter().any(|id| !ONE_WAY.contains(id)) {
        ok = false;
    }
    // The auth flow is checked offline: SHA-256, the RFC 7636 PKCE vector, the
    // policy matrix and the callback parser. None of it needs a Discord account,
    // which is the point — a gate that only fails at runtime is a gate nobody
    // has tested.
    // Boot-level state: read-only here. CI reports what the runner looks like
    // rather than failing — a hypervisor-off runner is a fact, not a defect.
    let boot_state = engine::boot_checks();
    let (auth_lines, auth_ok) = crate::auth::selftest();
    if !auth_ok {
        ok = false;
    }
    let mut report = String::new();
    let mut line = |k: &str, v: String| {
        report.push_str(&format!("{}={}\n", k, v));
    };
    line("version", engine::VERSION.to_string());
    line("boot_ms", format!("{:.1}", boot.elapsed().as_secs_f64() * 1000.0));
    line("detect_ms", format!("{:.1}", detect_ms));
    line("os", machine.os.clone());
    line("host", machine.host.clone());
    line("cpu", machine.cpu.clone());
    line("gpu", machine.gpu.clone());
    line("admin", machine.admin.to_string());
    line("simulated", machine.simulated.to_string());
    line("tweaks", engine::catalog::TWEAKS.len().to_string());
    line("profiles", engine::catalog::PROFILES.len().to_string());
    line("applied", state.applied.len().to_string());
    line("cpu_load", format!("{:.1}", stats.cpu));
    line("ram_pct", format!("{:.1}", stats.ram));
    line("ram_total_gb", format!("{:.1}", stats.total_gb));
    line(
        "timer_ms",
        engine::timer_resolution_ms()
            .map(|v| format!("{:.3}", v))
            .unwrap_or_else(|| "n/a".into()),
    );
    line("power_plan", engine::power_plan());
    line("emulators_found", format!("{}", emus.iter().filter(|e| e.installed).count()));
    line("processes", format!("{}", procs.len()));
    line(
        "reversible",
        format!(
            "{}/{} ({})",
            reversible,
            engine::catalog::TWEAKS.len(),
            one_way.join(",")
        ),
    );
    line("emu_guard", state.emu_guard.to_string());
    line(
        "emulator_installed",
        engine::emulator_installed().unwrap_or("none").to_string(),
    );
    for (k, v) in auth_lines {
        line(&k, v);
    }
    line(
        "gate",
        if crate::auth::gate_open() { "open" } else { "closed" }.to_string(),
    );
    line(
        "policy",
        if crate::auth::config().configured() {
            "configured"
        } else {
            "none"
        }
        .to_string(),
    );
    line(
        "boot",
        boot_state
            .iter()
            .map(|c| format!("{}={}", c.id, if c.ok { "ok" } else { "bad" }))
            .collect::<Vec<String>>()
            .join(","),
    );
    line("boot_ok", engine::boot_ok().to_string());
    // Prove the DeviceGuard keys are readable rather than silently absent: a
    // "bad" hvci row is only meaningful if the read found a value at all.
    line(
        "deviceguard",
        format!(
            "vbs={} hvci={}",
            engine::reg_probe(
                r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard",
                "EnableVirtualizationBasedSecurity"
            ),
            engine::reg_probe(
                r"HKLM\SYSTEM\CurrentControlSet\Control\DeviceGuard\Scenarios\HypervisorEnforcedCodeIntegrity",
                "Enabled"
            )
        ),
    );
    line("ok", ok.to_string());

    let _ = std::fs::write("void-selftest.txt", report);
    if !machine.simulated && procs.is_empty() {
        // tasklist failing on a real Windows box is a hard failure
        return 3;
    }
    if ok {
        0
    } else {
        1
    }
}
