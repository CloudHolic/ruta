//! Turning numbers into the text Lua shows.

/// Enough significant digits that every double reads back unchanged.
const EXACT: usize = 17;
/// Few enough that ordinary values do not pick up noise - `1.1` stays `1.1`.
const READABLE: usize = 15;

pub fn float(value: f64) -> String {
    if value.is_nan() {
        return nan(value).to_owned();
    }

    if value.is_infinite() {
        return match value.is_sign_negative() {
            true => "-inf".to_owned(),
            false => "inf".to_owned(),
        };
    }

    let mut text = general(value, READABLE);

    if text.parse::<f64>() != Ok(value) {
        text = general(value, EXACT);
    }

    if text
        .bytes()
        .all(|byte| byte == b'-' || byte.is_ascii_digit())
    {
        text.push_str(".0");
    }

    text
}

/// Fixed notation while the exponent stays small, scientific otherwise, with the trailing zeros
/// a fixed precision leaves behind removed.
fn general(value: f64, digits: usize) -> String {
    let scientific = format!("{:.*e}", digits - 1, value);
    let (mantissa, exponent) = scientific.split_once('e').expect("a written exponent");
    let exponent: i32 = exponent.parse().expect("an exponent just written");

    if exponent < -4 || exponent >= digits as i32 {
        let sign = match exponent < 0 {
            true => '-',
            false => '+',
        };

        return format!("{}e{sign}{:02}", trim(mantissa), exponent.unsigned_abs());
    }

    trim(&format!(
        "{:.*}",
        (digits as i32 - 1 - exponent) as usize,
        value
    ))
}

fn trim(text: &str) -> String {
    match text.contains('.') {
        true => text.trim_end_matches('0').trim_end_matches('.').to_owned(),
        false => text.to_owned(),
    }
}

/// The C runtime writes this one.
#[cfg(windows)]
fn nan(value: f64) -> &'static str {
    match value.is_sign_negative() {
        true => "-nan(ind)",
        false => "nan",
    }
}

#[cfg(not(windows))]
fn nan(value: f64) -> &'static str {
    match value.is_sign_negative() {
        true => "-nan",
        false => "nan",
    }
}
