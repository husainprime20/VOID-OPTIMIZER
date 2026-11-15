// VOID — native desktop entry point.
//
// No console window: the binary is built for the Windows GUI subsystem, so the
// only thing that appears when you double-click VOID.exe is the window itself.
// All helper processes it spawns are created with CREATE_NO_WINDOW (see
// `platform::hide_console`), so `reg`, `sc`, `powercfg` and PowerShell never
// flash a black rectangle either.
//
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod engine;
mod json;
mod platform;
mod theme;
mod views;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    install_panic_hook();

    if args.iter().any(|a| a == "--selftest") {
        std::process::exit(selftest());
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
            .with_inner_size([1440.0, 940.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_title("VOID — Competitive Optimizer"),
        vsync: true,
        multisampling: 0,
        ..Default::default()
    };

    eframe::run_native(
        "VOID",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)) as Box<dyn eframe::App>)),
    )
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
fn selftest() -> i32 {
    let state = engine::State::new();
    let machine = engine::Machine::detect();
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
    let mut report = String::new();
    let mut line = |k: &str, v: String| {
        report.push_str(&format!("{}={}\n", k, v));
    };
    line("version", engine::VERSION.to_string());
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
