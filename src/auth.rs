// Several items here are the documented surface of the flow rather than things
// the current window happens to call (the refresh path, the sign-in path, the
// self-test hooks), so dead-code noise is silenced rather than the API cut.
#![allow(dead_code)]

//! Discord OAuth2 + PKCE, and the gate it guards.
//!
//! VOID has no server. Every part of this flow therefore happens on the machine
//! that is running it: the browser hands an authorization code back over a
//! loopback redirect, VOID trades that code for a token with Discord directly,
//! and the role check is a live `guilds.members.read` call — never a cached
//! "yes". Nothing is uploaded anywhere else, and no client secret exists in the
//! binary because the app is registered as a *public* client.
//!
//! What this can and cannot do, stated plainly:
//!
//! * It stops casual sharing. Somebody handed a build cannot use it unless the
//!   live role check passes.
//! * It does **not** stop a determined person on their own machine, who can
//!   patch the binary, edit `auth.json`, or call `void-cli` directly. That is
//!   the honest limit of client-side enforcement, and the reason the console
//!   companion stays ungated on purpose — it is the escape hatch, not a leak.
//!
//! Fail-closed rules, all of them load-bearing:
//!   * no config            → gate inert (a build with no policy is a normal build)
//!   * config but no session → gate closed
//!   * any network/parse error → gate closed
//!   * the live role check fails → gate closed, even with a cached "tier"
//!   * state mismatch / timeout / user cancels → gate closed

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

const API_HOST: &str = "discord.com";
const AUTHORIZE: &str = "/oauth2/authorize";
const TOKEN: &str = "/api/oauth2/token";
const ME: &str = "/api/users/@me";
/// How long a sign-in may sit waiting for the browser.
const CALLBACK_TIMEOUT: u64 = 180;

// ───────────────────────────── gate (authoritative) ──────────────────────────

static CLOSED: AtomicBool = AtomicBool::new(false);

/// Whether the native engine may change anything. `apply_ids`, `apply_profile`
/// and `revert` all consult this — the UI never has the last word.
pub fn gate_open() -> bool {
    !CLOSED.load(Ordering::SeqCst)
}

pub fn set_gate(closed: bool) {
    CLOSED.store(closed, Ordering::SeqCst);
}

// ───────────────────────────────── config ────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Role {
    pub id: String,
    pub tier: String,
    pub label: String,
}

#[derive(Clone, Debug, Default)]
pub struct Config {
    pub client_id: String,
    pub redirect_port: u16,
    pub guild_id: String,
    pub required_tier: String,
    pub roles: Vec<Role>,
    pub allow_users: Vec<String>,
    pub invite_url: String,
    pub note: String,
}

impl Config {
    /// A build with an empty `client_id` has no policy and no gate.
    pub fn configured(&self) -> bool {
        !self.client_id.trim().is_empty()
    }

    pub fn port(&self) -> u16 {
        if self.redirect_port == 0 {
            43713
        } else {
            self.redirect_port
        }
    }

    pub fn redirect_uri(&self) -> String {
        format!("http://127.0.0.1:{}/auth/discord/callback", self.port())
    }

    /// The role gate exists only when a server and at least one role are named.
    /// Everything else is either the allowlist or nothing at all.
    pub fn role_gate(&self) -> bool {
        !self.guild_id.trim().is_empty() && !self.roles.is_empty()
    }

    pub fn scopes(&self) -> &'static str {
        if self.role_gate() {
            "identify guilds.members.read"
        } else {
            "identify"
        }
    }
}

/// Where `auth.json` is looked for, first hit wins.
fn config_paths() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(explicit) = std::env::var("VOID_AUTH_JSON") {
        if !explicit.trim().is_empty() {
            out.push(std::path::PathBuf::from(explicit));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join("auth.json"));
        }
    }
    out.push(crate::platform::data_dir().join("auth.json"));
    out
}

pub fn config() -> &'static Config {
    static CFG: OnceLock<Config> = OnceLock::new();
    CFG.get_or_init(|| {
        for p in config_paths() {
            if let Ok(text) = std::fs::read_to_string(&p) {
                return parse_config(&text);
            }
        }
        Config::default()
    })
}

/// Tolerant reader for the small, hand-written `auth.json`. Unknown keys are
/// ignored; anything malformed degrades to "no config" rather than guessing.
pub fn parse_config(text: &str) -> Config {
    let mut c = Config::default();
    let root = text;
    let d = object_after(root, "discord").unwrap_or(root);
    c.client_id = str_of(d, "client_id").unwrap_or_default();
    c.redirect_port = num_of(d, "redirect_port").unwrap_or(0.0) as u16;
    c.guild_id = str_of(d, "guild_id").unwrap_or_default();
    c.required_tier = str_of(d, "required_tier").unwrap_or_default();
    c.invite_url = str_of(d, "invite_url").unwrap_or_default();
    c.note = str_of(d, "note").unwrap_or_default();
    if let Some(roles) = object_after(d, "roles") {
        for (id, body) in pairs(roles) {
            let tier = str_of(body, "tier").unwrap_or_else(|| "member".to_string());
            let label = str_of(body, "label").unwrap_or_else(|| tier.clone());
            c.roles.push(Role { id, tier, label });
        }
    }
    if let Some(list) = array_after(d, "allow_users") {
        for id in strings_in(list) {
            c.allow_users.push(id);
        }
    }
    c
}

// ──────────────────────────────── policy ─────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    Allow { tier: String },
    Deny(String),
}

fn rank(tier: &str) -> i32 {
    match tier.trim().to_ascii_lowercase().as_str() {
        "" | "free" | "none" | "guest" => 0,
        "user" | "member" => 1,
        "premium" | "vip" => 2,
        "dev" | "developer" | "tester" => 3,
        "owner" | "staff" | "admin" => 4,
        _ => 1,
    }
}

/// The single place that answers "may this account run the optimizer?".
///
/// `roles` is `None` when the role lookup could not be performed — that is a
/// denial, never a pass. When a server gate exists the allowlist is *not*
/// consulted: the doc's rule, and the right one, is that a member must hold the
/// role no matter who they are.
pub fn decide(cfg: &Config, user_id: &str, roles: Option<&[String]>) -> Decision {
    if cfg.role_gate() {
        let Some(held) = roles else {
            return Decision::Deny("the server did not confirm your roles".into());
        };
        let mut best: Option<&Role> = None;
        for r in cfg.roles.iter() {
            if held.iter().any(|h| h == &r.id) {
                if best.map(|b| rank(&r.tier) > rank(&b.tier)).unwrap_or(true) {
                    best = Some(r);
                }
            }
        }
        let Some(hit) = best else {
            return Decision::Deny("this account does not hold a role in the server".into());
        };
        if !cfg.required_tier.trim().is_empty() && rank(&hit.tier) < rank(&cfg.required_tier) {
            return Decision::Deny(format!(
                "{} access needs the {} role",
                "VOID", cfg.required_tier
            ));
        }
        Decision::Allow {
            tier: hit.label.clone(),
        }
    } else if cfg.allow_users.iter().any(|u| u == user_id) {
        Decision::Allow {
            tier: "allowlisted".into(),
        }
    } else {
        Decision::Deny("this account is not on the allowlist".into())
    }
}

// ─────────────────────────────── PKCE + state ────────────────────────────────

pub struct Attempt {
    pub verifier: String,
    pub state: String,
    pub url: String,
}

pub fn begin(cfg: &Config) -> Result<Attempt, String> {
    if !cfg.configured() {
        return Err("no Discord app is configured".into());
    }
    let verifier = b64url(&random(32).ok_or("the system random source failed")?);
    let state = b64url(&random(16).ok_or("the system random source failed")?);
    let challenge = b64url(&sha256(verifier.as_bytes()));
    let url = format!(
        "https://discord.com{}?client_id={}&response_type=code&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256&prompt=consent",
        AUTHORIZE,
        pct(&cfg.client_id),
        pct(&cfg.redirect_uri()),
        pct(cfg.scopes()),
        pct(&state),
        pct(&challenge)
    );
    Ok(Attempt {
        verifier,
        state,
        url,
    })
}

/// Wait for the browser to land on the loopback redirect. One request, one
/// code, then the port is closed again.
pub fn await_code(port: u16, expect: &str) -> Result<String, String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .map_err(|e| format!("cannot listen on 127.0.0.1:{} · {}", port, e))?;
    let _ = listener.set_nonblocking(true);
    let started = std::time::Instant::now();
    loop {
        if started.elapsed().as_secs() > CALLBACK_TIMEOUT {
            return Err("sign-in timed out — try again".into());
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let line = req.lines().next().unwrap_or("").to_string();
                let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let mut it = target.splitn(2, '?');
                let path = it.next().unwrap_or("");
                let query = it.next().unwrap_or("");
                if path != "/auth/discord/callback" {
                    reply(&mut stream, 404, "not here", "This port belongs to VOID's sign-in.");
                    continue;
                }
                let pairs = query_pairs(query);
                let state = pairs
                    .iter()
                    .find(|(k, _)| k == "state")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                if state != expect || state.is_empty() {
                    reply(
                        &mut stream,
                        400,
                        "state mismatch",
                        "The sign-in reply did not match the request VOID made, so it was refused.",
                    );
                    return Err("state mismatch — sign-in refused".into());
                }
                if let Some((_, err)) = pairs.iter().find(|(k, _)| k == "error") {
                    reply(
                        &mut stream,
                        200,
                        "not signed in",
                        "Discord returned an error. You can close this tab and try again.",
                    );
                    return Err(format!("discord refused · {}", err));
                }
                let Some((_, code)) = pairs.iter().find(|(k, _)| k == "code") else {
                    reply(&mut stream, 400, "no code", "Discord sent no authorization code.");
                    return Err("no code came back".into());
                };
                reply(
                    &mut stream,
                    200,
                    "signed in",
                    "VOID has the code it needs. You can close this tab.",
                );
                return Ok(code.clone());
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(120));
            }
            Err(e) => return Err(format!("the sign-in port failed · {}", e)),
        }
    }
}

fn reply(stream: &mut std::net::TcpStream, status: u16, title: &str, body: &str) {
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>VOID · {}</title><style>\
         html{{background:#08080a;color:#f1f1f1;font:15px/1.55 -apple-system,Segoe UI,sans-serif}}\
         body{{display:grid;place-items:center;height:100vh;margin:0}}\
         div{{max-width:420px;padding:36px 40px;border:1px solid rgba(255,255,255,.14);border-radius:24px;background:rgba(19,19,19,.88)}}\
         h1{{font:600 13px/1 -apple-system,Segoe UI,sans-serif;letter-spacing:.22em;text-transform:uppercase;color:#8a8a8a;margin:0 0 14px}}\
         p{{margin:0;color:#c9c9c9}}</style><div><h1>void</h1><p><b>{}</b><br>{}</p></div>",
        title, title, body
    );
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        if status == 200 { "OK" } else { "Bad Request" },
        html.as_bytes().len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(html.as_bytes());
    let _ = stream.flush();
}

// ──────────────────────────────── network ────────────────────────────────────

pub struct Tokens {
    pub access: String,
    pub refresh: String,
}

fn token_request(cfg: &Config, form: String) -> Result<Tokens, String> {
    let (status, body) = crate::platform::https(
        "POST",
        API_HOST,
        TOKEN,
        &[
            ("Content-Type", "application/x-www-form-urlencoded"),
            ("User-Agent", "VOID/1.0"),
        ],
        Some(&form),
    )?;
    if status != 200 {
        let why = str_of(&body, "error_description")
            .or_else(|| str_of(&body, "error"))
            .unwrap_or_else(|| format!("http {}", status));
        return Err(format!("discord refused the token · {}", why));
    }
    let _ = cfg;
    Ok(Tokens {
        access: str_of(&body, "access_token").ok_or("no access token in the reply")?,
        refresh: str_of(&body, "refresh_token").unwrap_or_default(),
    })
}

pub fn exchange(cfg: &Config, attempt: &Attempt, code: &str) -> Result<Tokens, String> {
    let form = format!(
        "client_id={}&grant_type=authorization_code&code={}&redirect_uri={}&code_verifier={}",
        pct(&cfg.client_id),
        pct(code),
        pct(&cfg.redirect_uri()),
        pct(&attempt.verifier)
    );
    token_request(cfg, form)
}

pub fn refresh(cfg: &Config, token: &str) -> Result<Tokens, String> {
    if token.trim().is_empty() {
        return Err("no saved refresh token".into());
    }
    let form = format!(
        "client_id={}&grant_type=refresh_token&refresh_token={}",
        pct(&cfg.client_id),
        pct(token)
    );
    token_request(cfg, form)
}

fn api_get(access: &str, path: &str) -> Result<(u32, String), String> {
    crate::platform::https(
        "GET",
        API_HOST,
        path,
        &[
            ("Authorization", &format!("Bearer {}", access)),
            ("User-Agent", "VOID/1.0"),
        ],
        None,
    )
}

pub struct Identity {
    pub id: String,
    pub name: String,
}

pub fn identify(access: &str) -> Result<Identity, String> {
    let (status, body) = api_get(access, ME)?;
    if status != 200 {
        return Err(format!("profile request failed · http {}", status));
    }
    Ok(Identity {
        id: str_of(&body, "id").ok_or("the profile had no id")?,
        name: str_of(&body, "global_name")
            .filter(|s| !s.trim().is_empty())
            .or_else(|| str_of(&body, "username"))
            .unwrap_or_else(|| "member".into()),
    })
}

/// The live role check. A 403/404 means "not in the server" or "no roles", and
/// both are closed doors — Discord answers with an error rather than an empty
/// list precisely so a client cannot mistake absence for success.
pub fn member_roles(cfg: &Config, access: &str) -> Result<Vec<String>, String> {
    if !cfg.role_gate() {
        return Ok(Vec::new());
    }
    let path = format!("/api/users/@me/guilds/{}/member", cfg.guild_id);
    let (status, body) = api_get(access, &path)?;
    match status {
        200 => Ok(strings_in(&body_after_colon(&body, "roles").unwrap_or_default())),
        403 | 404 => Err("you are not in the server".into()),
        s => Err(format!("role check failed · http {}", s)),
    }
}

// ─────────────────────────────── session store ───────────────────────────────

#[derive(Clone, Debug, Default)]
pub struct Session {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub at: i64,
}

fn session_file() -> std::path::PathBuf {
    crate::platform::data_dir().join("auth.session")
}

fn token_file() -> std::path::PathBuf {
    crate::platform::data_dir().join("auth.token")
}

/// Save the *identity* (no token) and the refresh token separately, each sealed
/// with the OS keystore. On a machine without one nothing is written: VOID would
/// rather ask again than leave a bearer token on disk in the clear.
pub fn save(session: &Session, refresh_token: &str) {
    let json = crate::json::obj(vec![
        crate::json::ks("id", &session.id),
        crate::json::ks("name", &session.name),
        crate::json::ks("tier", &session.tier),
        crate::json::ki("at", session.at),
    ]);
    if let Some(sealed) = crate::platform::seal(json.as_bytes()) {
        let _ = std::fs::write(session_file(), sealed);
    }
    if !refresh_token.trim().is_empty() {
        if let Some(sealed) = crate::platform::seal(refresh_token.as_bytes()) {
            let _ = std::fs::write(token_file(), sealed);
        }
    }
}

pub fn load() -> Option<Session> {
    let raw = std::fs::read(session_file()).ok()?;
    let json = String::from_utf8(crate::platform::unseal(&raw)?).ok()?;
    Some(Session {
        id: str_of(&json, "id").unwrap_or_default(),
        name: str_of(&json, "name").unwrap_or_default(),
        tier: str_of(&json, "tier").unwrap_or_default(),
        at: num_of(&json, "at").unwrap_or(0.0) as i64,
    })
}

pub fn saved_token() -> Option<String> {
    let raw = std::fs::read(token_file()).ok()?;
    String::from_utf8(crate::platform::unseal(&raw)?).ok()
}

/// Sign-out means sign-out: identity file, token file, and the gate.
pub fn clear() {
    let _ = std::fs::remove_file(session_file());
    let _ = std::fs::remove_file(token_file());
    set_gate(true);
}

/// The whole path in one call, for the worker thread: refresh → identify →
/// live role check → policy. Any failure at any step is a denial.
pub fn revalidate(cfg: &Config) -> Result<Session, String> {
    let token = saved_token().ok_or("no saved sign-in")?;
    let tokens = refresh(cfg, &token)?;
    let who = identify(&tokens.access)?;
    let roles = member_roles(cfg, &tokens.access)?;
    let decision = decide(cfg, &who.id, if cfg.role_gate() { Some(&roles) } else { None });
    match decision {
        Decision::Allow { tier } => {
            let s = Session {
                id: who.id,
                name: who.name,
                tier,
                at: now(),
            };
            save(&s, &tokens.refresh);
            set_gate(false);
            Ok(s)
        }
        Decision::Deny(why) => {
            clear();
            Err(why)
        }
    }
}

/// First sign-in, also for the worker thread.
pub fn sign_in(cfg: &Config) -> Result<Session, String> {
    let attempt = begin(cfg)?;
    let _ = crate::platform::open_url(&attempt.url);
    let code = await_code(cfg.port(), &attempt.state)?;
    let tokens = exchange(cfg, &attempt, &code)?;
    let who = identify(&tokens.access)?;
    let roles = member_roles(cfg, &tokens.access)?;
    let held = if cfg.role_gate() { Some(roles) } else { None };
    match decide(cfg, &who.id, held.as_deref()) {
        Decision::Allow { tier } => {
            let s = Session {
                id: who.id,
                name: who.name,
                tier,
                at: now(),
            };
            save(&s, &tokens.refresh);
            set_gate(false);
            Ok(s)
        }
        Decision::Deny(why) => {
            clear();
            Err(why)
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ─────────────────────────────── crypto ──────────────────────────────────────

/// SHA-256 (FIPS 180-4). VOID carries no crates, and PKCE needs exactly this.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let bits = (data.len() as u64).wrapping_mul(8);
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());

    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[i * 4], chunk[i * 4 + 1], chunk[i * 4 + 2], chunk[i * 4 + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d) = (h[0], h[1], h[2], h[3]);
        let (mut e, mut f, mut g, mut hh) = (h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&h[i].to_be_bytes());
    }
    out
}

pub fn b64url(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len() * 4 / 3 + 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(T[n as usize & 63] as char);
        }
    }
    out
}

/// System randomness, or `None` — and a `None` here stops the sign-in rather
/// than falling back to anything predictable.
pub fn random(n: usize) -> Option<Vec<u8>> {
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        extern "system" {
            fn BCryptGenRandom(
                alg: *mut std::ffi::c_void,
                buf: *mut u8,
                len: u32,
                flags: u32,
            ) -> i32;
        }
        let mut buf = vec![0u8; n];
        // BCRYPT_USE_SYSTEM_PREFERRED_RNG
        let rc = unsafe {
            BCryptGenRandom(
                std::ptr::null_mut(),
                buf.as_mut_ptr(),
                n as u32,
                0x0000_0002,
            )
        };
        if rc == 0 {
            Some(buf)
        } else {
            None
        }
    }
    #[cfg(not(windows))]
    {
        let mut buf = vec![0u8; n];
        let mut f = std::fs::File::open("/dev/urandom").ok()?;
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    }
}

/// Percent-encode for a query string or a form body.
pub fn pct(v: &str) -> String {
    let mut out = String::with_capacity(v.len() * 3);
    for b in v.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ────────────────────────── tiny JSON (read side) ────────────────────────────

fn query_pairs(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (unpct(k), unpct(v))
        })
        .collect()
}

fn unpct(v: &str) -> String {
    let bytes = v.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(b) = u8::from_str_radix(hex, 16) {
                    out.push(b);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

/// `"key": { ... }` → the object's body, braces matched and excluded.
fn object_after<'a>(src: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\"", key);
    let mut from = 0usize;
    while let Some(i) = src[from..].find(&pat) {
        let after = from + i + pat.len();
        let rest = &src[after..];
        if let Some(open) = rest.find('{') {
            // only accept if nothing but whitespace and ':' comes first
            if rest[..open].trim_matches(|c: char| c == ':' || c.is_whitespace()).is_empty() {
                let body = &rest[open + 1..];
                let mut depth = 1i32;
                let mut in_str = false;
                let mut esc = false;
                for (k, ch) in body.char_indices() {
                    if in_str {
                        if esc {
                            esc = false;
                        } else if ch == '\\' {
                            esc = true;
                        } else if ch == '"' {
                            in_str = false;
                        }
                        continue;
                    }
                    match ch {
                        '"' => in_str = true,
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                return Some(&body[..k]);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        from = after;
    }
    None
}

/// `"key": [ ... ]` → the array's body, brackets excluded.
fn array_after<'a>(src: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{}\"", key);
    let i = src.find(&pat)?;
    let rest = &src[i + pat.len()..];
    let open = rest.find('[')?;
    let body = &rest[open + 1..];
    let close = body.find(']')?;
    Some(&body[..close])
}

fn body_after_colon<'a>(src: &'a str, key: &str) -> Option<&'a str> {
    array_after(src, key).or_else(|| object_after(src, key))
}

/// Split an object body into `("id", body)` for each member.
fn pairs(body: &str) -> Vec<(String, &str)> {
    let mut out = Vec::new();
    let b = body.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        while i < b.len() && b[i] != b'"' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let start = i + 1;
        let mut j = start;
        let mut esc = false;
        while j < b.len() {
            if esc {
                esc = false;
            } else if b[j] == b'\\' {
                esc = true;
            } else if b[j] == b'"' {
                break;
            }
            j += 1;
        }
        if j >= b.len() {
            break;
        }
        let key = body[start..j].to_string();
        let mut k = j + 1;
        while k < b.len() && b[k] != b'{' && b[k] != b'"' && b[k] != b'[' && b[k] != b']' && b[k] != b'}' {
            k += 1;
        }
        if k >= b.len() {
            break;
        }
        if b[k] == b'{' {
            let mut depth = 1i32;
            let mut in_str = false;
            let mut esc2 = false;
            let mut m = k + 1;
            while m < b.len() {
                let ch = b[m];
                if in_str {
                    if esc2 {
                        esc2 = false;
                    } else if ch == b'\\' {
                        esc2 = true;
                    } else if ch == b'"' {
                        in_str = false;
                    }
                } else if ch == b'"' {
                    in_str = true;
                } else if ch == b'{' {
                    depth += 1;
                } else if ch == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        out.push((key, &body[k..=m]));
                        break;
                    }
                }
                m += 1;
            }
            i = m + 1;
        } else {
            // a plain value: skip it
            while k < b.len() && b[k] != b',' && b[k] != b'}' {
                k += 1;
            }
            i = k + 1;
        }
    }
    out
}

fn strings_in(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let b = body.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'"' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let mut esc = false;
        let mut val = String::new();
        while j < b.len() {
            let c = b[j];
            if esc {
                val.push(c as char);
                esc = false;
            } else if c == b'\\' {
                esc = true;
            } else if c == b'"' {
                break;
            } else {
                val.push(c as char);
            }
            j += 1;
        }
        out.push(val);
        i = j + 1;
    }
    out
}

fn str_of(src: &str, key: &str) -> Option<String> {
    let pat = format!("\"{}\"", key);
    let i = src.find(&pat)?;
    let rest = &src[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let b = after.as_bytes();
    let mut j = 1usize;
    let mut esc = false;
    let mut out = String::new();
    while j < b.len() {
        let c = b[j];
        if esc {
            out.push(match c {
                b'n' => '\n',
                b't' => '\t',
                b'r' => '\r',
                b'u' => ' ',
                other => other as char,
            });
            esc = false;
        } else if c == b'\\' {
            esc = true;
        } else if c == b'"' {
            break;
        } else {
            out.push(c as char);
        }
        j += 1;
    }
    Some(out)
}

fn num_of(src: &str, key: &str) -> Option<f64> {
    let pat = format!("\"{}\"", key);
    let i = src.find(&pat)?;
    let rest = &src[i + pat.len()..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let end = after
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

// ──────────────────────────────── self-test ──────────────────────────────────

/// Deterministic checks that need no network and no Discord account. Returns the
/// lines to drop into `void-selftest.txt`, and whether all of them held.
pub fn selftest() -> (Vec<(String, String)>, bool) {
    let mut out = Vec::new();
    let mut ok = true;
    let mut line = |k: &str, v: String| out.push((k.to_string(), v));

    // FIPS 180-4 vector for "abc".
    let digest = sha256(b"abc");
    let hex = digest
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    let sha_ok = hex == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    ok &= sha_ok;
    line("sha256_abc", if sha_ok { "match".into() } else { hex });

    // RFC 7636 appendix B vector.
    let verifier = "dBjftJeZ4RLt9nbnPn6U-2l6oIvS6a6RVE-pVLVpK1A";
    let challenge = b64url(&sha256(verifier.as_bytes()));
    let pkce_ok = challenge == "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
    ok &= pkce_ok;
    line("pkce_s256", if pkce_ok { "match".into() } else { challenge });

    // A policy with a role gate must ignore the allowlist entirely.
    let cfg = Config {
        client_id: "123".into(),
        guild_id: "9".into(),
        required_tier: "user".into(),
        roles: vec![Role {
            id: "7".into(),
            tier: "user".into(),
            label: "Member".into(),
        }],
        allow_users: vec!["42".into()],
        ..Config::default()
    };
    let no_roles = decide(&cfg, "42", None);
    let wrong = decide(&cfg, "42", Some(&["1".to_string()]));
    let held = decide(&cfg, "42", Some(&["7".to_string()]));
    let policy_ok = matches!(no_roles, Decision::Deny(_))
        && matches!(wrong, Decision::Deny(_))
        && matches!(held, Decision::Allow { .. });
    ok &= policy_ok;
    line("policy_role_gate", if policy_ok { "closed/closed/open".into() } else { format!("{:?}/{:?}/{:?}", no_roles, wrong, held) });

    let empty = Config::default();
    let closed = matches!(decide(&empty, "1", None), Decision::Deny(_));
    ok &= closed && !empty.configured();
    line("policy_unconfigured", if closed { "denied".into() } else { "leaked".into() });

    // Round trip through the same writer the browser callback uses.
    let (k, v) = ("state", "abc123");
    let q = format!("{}={}&code=x-y_z", k, pct(v));
    let parsed = query_pairs(&q);
    let round = parsed.iter().any(|(a, b)| a == "state" && b == v)
        && parsed.iter().any(|(a, b)| a == "code" && b == "x-y_z");
    ok &= round;
    line("callback_parse", if round { "match".into() } else { q });

    (out, ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256(b"").iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256(b"abc").iter().map(|b| format!("{:02x}", b)).collect::<String>(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn pkce_matches_rfc7636() {
        let v = "dBjftJeZ4RLt9nbnPn6U-2l6oIvS6a6RVE-pVLVpK1A";
        assert_eq!(b64url(&sha256(v.as_bytes())), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
    }

    #[test]
    fn config_parses_the_documented_shape() {
        let cfg = parse_config(
            r#"{"discord":{"client_id":"900","redirect_port":43713,"guild_id":"7",
               "required_tier":"premium","roles":{"111":{"tier":"premium","label":"Premium"},
               "222":{"tier":"user","label":"User"}},"allow_users":["5"]}}"#,
        );
        assert_eq!(cfg.client_id, "900");
        assert_eq!(cfg.roles.len(), 2);
        assert!(cfg.role_gate());
        // premium (2) satisfies required premium, user (1) does not
        assert!(matches!(
            decide(&cfg, "5", Some(&["111".to_string()])),
            Decision::Allow { .. }
        ));
        assert!(matches!(
            decide(&cfg, "5", Some(&["222".to_string()])),
            Decision::Deny(_)
        ));
    }

    #[test]
    fn no_roles_means_closed() {
        let cfg = Config {
            client_id: "1".into(),
            guild_id: "2".into(),
            roles: vec![Role { id: "3".into(), tier: "user".into(), label: "U".into() }],
            allow_users: vec!["9".into()],
            ..Config::default()
        };
        assert!(matches!(decide(&cfg, "9", None), Decision::Deny(_)));
    }
}
