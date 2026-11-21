//! VOID // console companion.
//!
//! The product is `VOID.exe` — a native desktop window (see `src/gui.rs`).
//! This binary is the scriptable half of the same engine, useful in CI and for
//! people who would rather type than click. It is not a server: there is no
//! HTTP, no preview, no browser.
//!
//! `void-cli status [--json]`  → machine + engine state
//! `void-cli catalog`          → the tweak catalog as JSON
//! `void-cli apply <p|id>`     → apply a profile or one tweak
//! `void-cli revert`           → revert everything VOID has applied
//! `void-cli bench`            → sleep-jitter benchmark
//! `void-cli app`              → launch VOID.exe

mod auth;
mod engine;
mod json;
mod platform;

use crate::engine::{Machine, State};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).cloned().unwrap_or_else(|| "status".to_string());
    match command.as_str() {
        "app" | "ui" | "start" => {
            // hand over to the native window, if it shipped next to us
            let exe = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("VOID.exe")));
            match exe {
                Some(p) if p.exists() => {
                    if let Err(e) = std::process::Command::new(p).spawn() {
                        eprintln!("void-cli: could not start VOID.exe · {}", e);
                    }
                }
                _ => eprintln!("void-cli: VOID.exe not found next to this binary"),
            }
        }
        "status" => cmd_status(&args),
        "catalog" => cmd_catalog(),
        "apply" => cmd_apply(&args),
        "revert" => cmd_revert(),
        "bench" => println!("{}", engine::bench_json()),
        "help" | "--help" | "-h" => usage(),
        "version" | "--version" | "-V" => println!("void {}", engine::VERSION),
        other => {
            eprintln!("void: unknown command `{}`\n", other);
            usage();
            std::process::exit(1);
        }
    }
}

fn usage() {
    println!(
        "\n  VOID // {}  (console companion)\n\
         \n  The desktop app is VOID.exe. This binary is the scriptable half.\n\
         \n  USAGE\n\
         \x20   void-cli status [--json]   machine + engine state\n\
         \x20   void-cli catalog           the tweak catalog as JSON\n\
         \x20   void-cli apply <id>        apply a profile or one tweak\n\
         \x20   void-cli revert            revert everything VOID applied\n\
         \x20   void-cli bench             sleep-jitter benchmark (JSON)\n\
         \x20   void-cli app               launch VOID.exe\n\
         \n  EXAMPLES\n\
         \x20   void-cli apply competitive\n\
         \x20   void-cli apply kernel.timer\n\n",
        engine::VERSION
    );
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

// ───────────────────────────── commands ─────────────────────────────

fn cmd_status(args: &[String]) {
    let state = State::new();
    let machine = Machine::detect();
    if has_flag(args, "--json") {
        println!("{}", engine::state_json(&state, &machine));
        return;
    }
    let (applied, verified) = verified_counts(&state);
    println!("VOID {} — {}", engine::VERSION, machine.os);
    println!("cpu      {}", machine.cpu);
    println!("gpu      {}", machine.gpu);
    println!("host     {}@{}", machine.user, machine.host);
    println!("admin    {}", machine.admin);
    println!(
        "mode     {}",
        if machine.simulated {
            "SIMULATION (not Windows)"
        } else {
            "NATIVE"
        }
    );
    println!("applied  {} / {}   (verified {})", applied, engine::catalog_len(), verified);
    println!(
        "profile  {}",
        if state.last_profile.is_empty() {
            "—"
        } else {
            state.last_profile.as_str()
        }
    );
    println!("power    {}", engine::power_plan());
    println!(
        "timer    {}",
        match engine::timer_resolution_ms() {
            Some(v) => format!("{:.3} ms", v),
            None => "n/a".to_string(),
        }
    );
    println!("stats    {}", engine::stats_json());
}

fn verified_counts(state: &State) -> (usize, usize) {
    let verified = state
        .applied
        .iter()
        .filter(|id| engine::live_status(id) == Some(true))
        .count();
    (state.applied.len(), verified)
}

fn cmd_catalog() {
    let state = State::new();
    println!("{}", engine::catalog_json(&state));
}

fn cmd_apply(args: &[String]) {
    let mut state = State::new();
    let target = match args.get(2) {
        Some(t) => t.clone(),
        None => {
            eprintln!("void apply: missing <profile|tweak>   (try: void apply competitive)");
            std::process::exit(1);
        }
    };
    let outcomes = if engine::find_profile(&target).is_some() {
        engine::apply_profile(&mut state, &target)
    } else if engine::find_tweak(&target).is_some() {
        engine::apply_ids(&mut state, &[target.clone()])
    } else {
        eprintln!("void apply: unknown profile or tweak `{}`", target);
        std::process::exit(1);
    };
    let mut failed = 0;
    for o in outcomes.iter() {
        if !o.ok {
            failed += 1;
        }
        println!(
            "{}  {:<18}  {}",
            if o.ok { "OK  " } else { "FAIL" },
            o.id,
            o.msg
        );
    }
    println!("\n{} step(s), {} failed", outcomes.len(), failed);
    if failed > 0 {
        std::process::exit(2);
    }
}

fn cmd_revert() {
    let mut state = State::new();
    let outcomes = engine::revert_all(&mut state);
    for o in outcomes.iter() {
        println!(
            "{}  {:<18}  {}",
            if o.ok { "OK  " } else { "FAIL" },
            o.id,
            o.msg
        );
    }
    println!("\n{} revert step(s)", outcomes.len());
}
