use std::fmt::Write;

pub fn format_number(num: f64) -> String {
    let mut out = String::with_capacity(12);
    format_number_into(&mut out, num);
    out
}

pub fn format_number_into(out: &mut String, mut num: f64) {
    out.clear();
    let rounded = num.round();
    if (num - rounded).abs() < 1e-3 {
        num = rounded;
    }
    num = num.max(0.0);
    if num >= 10_000_000.0 {
        let _ = write!(out, "{:.1}M", (num / 100_000.0).floor() / 10.0);
    } else if num >= 1_000_000.0 {
        let _ = write!(out, "{:.2}M", (num / 10_000.0).floor() / 100.0);
    } else if num >= 100_000.0 {
        let _ = write!(out, "{}K", (num / 1000.0).floor());
    } else if num >= 10_000.0 {
        let _ = write!(out, "{:.1}K", (num / 100.0).floor() / 10.0);
    } else if num >= 1_000.0 {
        let _ = write!(out, "{:.2}K", (num / 10.0).floor() / 100.0);
    } else {
        let _ = write!(out, "{:.0}", num.round());
    }
}
