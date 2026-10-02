//! Shared text helpers.

/// Strip ANSI escape sequences from a captured line.
///
/// `nh` and `nixos-rebuild` emit ANSI color codes on some lines. egui does not
/// interpret them, so we remove them and color the output ourselves instead.
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c == 'm' {
                in_esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        out.push(c);
    }
    out
}
