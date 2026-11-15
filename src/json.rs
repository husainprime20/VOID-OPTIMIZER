//! Tiny JSON encoder.
//!
//! VOID ships with **zero external crates**, so the whole API layer is encoded
//! by hand. These helpers produce compact, standards-compliant JSON.

/// Escape and quote a string value.
pub fn esc(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn int(v: i64) -> String {
    format!("{}", v)
}

pub fn float(v: f64) -> String {
    if v.is_finite() {
        format!("{:.2}", v)
    } else {
        "0".to_string()
    }
}

pub fn boolean(v: bool) -> String {
    if v {
        "true".to_string()
    } else {
        "false".to_string()
    }
}

pub fn obj(pairs: Vec<String>) -> String {
    format!("{{{}}}", pairs.join(","))
}

pub fn arr(items: Vec<String>) -> String {
    format!("[{}]", items.join(","))
}

pub fn kv(k: &str, v: String) -> String {
    format!("{}:{}", esc(k), v)
}

pub fn ks(k: &str, v: &str) -> String {
    format!("{}:{}", esc(k), esc(v))
}

pub fn kb(k: &str, v: bool) -> String {
    format!("{}:{}", esc(k), boolean(v))
}

pub fn ki(k: &str, v: i64) -> String {
    format!("{}:{}", esc(k), int(v))
}

pub fn kf(k: &str, v: f64) -> String {
    format!("{}:{}", esc(k), float(v))
}
