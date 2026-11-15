//! VOID // console companion.
//!
//! The product is `VOID.exe` (a native desktop window — see `src/gui.rs`).
//! This binary is the head-less half: scriptable, CI-friendly, and able to
//! serve the legacy web preview over localhost for anyone who wants it.
//!
//! `void-cli status` → machine + engine state
//! `void apply <p>`  → apply a profile or a single tweak
//! `void revert`     → revert everything VOID has applied
//! `void status`     → machine + engine state (use --json for machines)
//! `void bench`      → sleep-jitter benchmark

mod engine;
mod http;
mod json;
mod platform;

use crate::engine::{Machine, State};
use crate::http::{Request, Response};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

const INDEX_HTML: &str = include_str!("../assets/ui/index.html");
const APP_CSS: &str = include_str!("../assets/ui/app.css");
const APP_JS: &str = include_str!("../assets/ui/app.js");

const FAVICON: &str = "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 32 32'>\
<rect width='32' height='32' fill='#000000'/>\
<path d='M7 8 L16 25 L25 8' fill='none' stroke='#ffffff' stroke-width='2.2' stroke-linejoin='miter'/>\
</svg>";

struct App {
    state: Mutex<State>,
    machine: Machine,
}

fn st(app: &App) -> MutexGuard<'_, State> {
    match app.state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Non-blocking lock for read-only endpoints: while a long apply is running the
/// UI keeps painting instead of freezing on a held mutex. `None` means "busy".
fn try_st(app: &App) -> Option<MutexGuard<'_, State>> {
    match app.state.try_lock() {
        Ok(guard) => Some(guard),
        Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner()),
        Err(_) => None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).cloned().unwrap_or_else(|| "status".to_string());
    match command.as_str() {
        "ui" | "start" | "serve" => {
            let open = command != "serve" && !has_flag(&args, "--no-window");
            cmd_serve(&args, open);
        }
        "app" => {
            // hand over to the native window, if it shipped next to us
            let exe = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("VOID.exe")));
            match exe {
                Some(p) if p.exists() => {
                    let _ = std::process::Command::new(p).spawn();
                }
                _ => eprintln!("void-cli: VOID.exe not found next to this binary"),
            }
        }
        "status" => cmd_status(&args),
        "catalog" => cmd_catalog(),
        "apply" => cmd_apply(&args),
        "revert" => cmd_revert(&args),
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
         \x20   void-cli status [--json]         machine + engine state\n\
         \x20   void-cli catalog                 the tweak catalog as JSON\n\
         \x20   void-cli apply <id> [--dry-run]  apply a profile or one tweak\n\
         \x20   void-cli revert [--dry-run]      revert everything VOID applied\n\
         \x20   void-cli bench                   sleep-jitter benchmark (JSON)\n\
         \x20   void-cli app                     launch VOID.exe\n\
         \x20   void-cli serve [--host H] [--port P]   legacy web preview\n\
         \n  EXAMPLES\n\
         \x20   void-cli apply competitive\n\
         \x20   void-cli apply ff.affinity --dry-run\n\n",
        engine::VERSION
    );
}

fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

fn flag_value(args: &[String], name: &str) -> Option<String> {
    let prefix = format!("{}=", name);
    for (i, a) in args.iter().enumerate() {
        if a == name {
            if let Some(v) = args.get(i + 1) {
                return Some(v.clone());
            }
        }
        if let Some(rest) = a.strip_prefix(&prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

// ───────────────────────────── commands ─────────────────────────────

fn cmd_serve(args: &[String], open: bool) {
    let host = flag_value(args, "--host").unwrap_or_else(|| "127.0.0.1".to_string());
    let port = flag_value(args, "--port").unwrap_or_else(|| "8722".to_string());
    let url = format!("http://{}:{}/", host, port);

    let app = Arc::new(App {
        state: Mutex::new(State::new()),
        machine: Machine::detect(),
    });

    banner(&app, &url);

    if open {
        let target = url.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(400));
            platform::open_window(&target);
        });
    }

    let handler: http::Handler = {
        let app = app.clone();
        Arc::new(move |req: Request| route(req, app.clone()))
    };

    if let Err(e) = http::serve(&format!("{}:{}", host, port), handler) {
        eprintln!("void: could not bind {}:{} → {}", host, port, e);
        std::process::exit(1);
    }
}

fn banner(app: &App, url: &str) {
    let state = st(app);
    println!("\n  V O I D   //   competitive optimizer   v{}", engine::VERSION);
    println!("  ─────────────────────────────────────────────────────────");
    println!("  os    {}", app.machine.os);
    println!("  cpu   {}", app.machine.cpu);
    println!("  gpu   {}", app.machine.gpu);
    println!("  user  {}@{}", app.machine.user, app.machine.host);
    println!(
        "  mode  {}   admin {}   tweaks {}",
        if app.machine.simulated { "SIMULATION" } else { "NATIVE" },
        if app.machine.admin { "yes" } else { "no" },
        engine::catalog_len()
    );
    if !app.machine.admin && !app.machine.simulated {
        println!("        → relaunch as Administrator, or most kernel tweaks will fail");
    }
    if state.dry_run {
        println!("        → dry-run is ON: nothing will be written");
    }
    println!("  ─────────────────────────────────────────────────────────");
    println!("  ui    {}", url);
    println!("        ctrl+c to stop\n");
}

fn cmd_status(args: &[String]) {
    let state = State::new();
    let machine = Machine::detect();
    if has_flag(args, "--json") {
        println!("{}", engine::state_json(&state, &machine));
        return;
    }
    println!("VOID {} — {}", engine::VERSION, machine.os);
    println!("cpu      {}", machine.cpu);
    println!("gpu      {}", machine.gpu);
    println!("host     {}@{}", machine.user, machine.host);
    println!("admin    {}", machine.admin);
    println!("mode     {}", if machine.simulated { "SIMULATION (not Windows)" } else { "NATIVE" });
    println!("dry-run  {}", state.dry_run);
    println!("applied  {} / {}", state.applied.len(), engine::catalog_len());
    println!("profile  {}", if state.last_profile.is_empty() { "—" } else { state.last_profile.as_str() });
    println!("stats    {}", engine::stats_json());
}

fn cmd_catalog() {
    let state = State::new();
    println!("{}", engine::catalog_json(&state));
}

fn cmd_apply(args: &[String]) {
    let mut state = State::new();
    if has_flag(args, "--dry-run") {
        state.dry_run = true;
    }
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
        println!("{}  {:<18}  {}", if o.ok { "OK  " } else { "FAIL" }, o.id, o.msg);
    }
    println!("\n{} step(s), {} failed{}", outcomes.len(), failed, if state.dry_run { "  ·  dry-run" } else { "" });
}

fn cmd_revert(args: &[String]) {
    let mut state = State::new();
    if has_flag(args, "--dry-run") {
        state.dry_run = true;
    }
    let outcomes = engine::revert_all(&mut state);
    for o in outcomes.iter() {
        println!("{}  {:<18}  {}", if o.ok { "OK  " } else { "FAIL" }, o.id, o.msg);
    }
    println!("\n{} revert step(s)", outcomes.len());
}

// ───────────────────────────── http routing ─────────────────────────────

fn route(req: Request, app: Arc<App>) -> Response {
    if req.method == "OPTIONS" {
        return Response::text(204, "");
    }

    match req.path.as_str() {
        "/" | "/index.html" => Response::html(INDEX_HTML),
        "/assets/app.css" => Response::css(APP_CSS),
        "/assets/app.js" => Response::js(APP_JS),
        "/favicon.ico" | "/favicon.svg" => {
            Response::new(200, "image/svg+xml", FAVICON.as_bytes().to_vec())
        }
        "/api/catalog" => match try_st(&app) {
            Some(guard) => Response::json(engine::catalog_json(&guard)),
            None => Response::text(503, "busy"),
        },
        "/api/state" => match try_st(&app) {
            Some(guard) => Response::json(engine::state_json(&guard, &app.machine)),
            None => Response::text(503, "busy"),
        },
        "/api/log" => match try_st(&app) {
            Some(guard) => Response::json(engine::log_json(&guard)),
            None => Response::text(503, "busy"),
        },
        "/api/stats" => Response::json(engine::stats_json()),
        "/api/bench" => Response::json(engine::bench_json()),
        "/api/apply" => {
            let id = req.q("id");
            let mut guard = st(&app);
            if id.is_empty() {
                return Response::text(400, "missing id");
            }
            let out = engine::apply_ids(&mut guard, &[id]);
            Response::json(json::obj(vec![json::kv(
                "results",
                engine::outcomes_json(&out),
            )]))
        }
        "/api/profile" => {
            let id = req.q("id");
            let mut guard = st(&app);
            if id.is_empty() {
                return Response::text(400, "missing id");
            }
            let out = engine::apply_profile(&mut guard, &id);
            Response::json(json::obj(vec![
                json::kv("results", engine::outcomes_json(&out)),
                json::ki("applied_count", guard.applied.len() as i64),
            ]))
        }
        "/api/revert" => {
            let id = req.q("id");
            let mut guard = st(&app);
            let out = if id.is_empty() {
                engine::revert_all(&mut guard)
            } else {
                engine::revert_ids(&mut guard, &[id])
            };
            Response::json(json::obj(vec![
                json::kv("results", engine::outcomes_json(&out)),
                json::ki("applied_count", guard.applied.len() as i64),
            ]))
        }
        "/api/revert-all" => {
            let mut guard = st(&app);
            let out = engine::revert_all(&mut guard);
            Response::json(json::obj(vec![
                json::kv("results", engine::outcomes_json(&out)),
                json::ki("applied_count", 0),
            ]))
        }
        "/api/restore" => {
            let mut guard = st(&app);
            let out = engine::restore_point(&mut guard);
            Response::json(json::obj(vec![json::kv(
                "results",
                engine::outcomes_json(&[out]),
            )]))
        }
        "/api/setting" => {
            let key = req.q("key");
            let value = req.q("value");
            let mut guard = st(&app);
            if key.is_empty() {
                return Response::text(400, "missing key");
            }
            guard.set_setting(&key, &value);
            Response::json(json::obj(vec![json::kb("ok", true)]))
        }
        _ => Response::not_found(),
    }
}
