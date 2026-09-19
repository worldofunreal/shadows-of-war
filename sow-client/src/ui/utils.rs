pub fn format_number(mut num: f64) -> String {
    let rounded = num.round();
    if (num - rounded).abs() < 1e-3 {
        num = rounded;
    }
    num = num.max(0.0);
    if num >= 10_000_000.0 {
        format!("{:.1}M", (num / 100_000.0).floor() / 10.0)
    } else if num >= 1_000_000.0 {
        format!("{:.2}M", (num / 10_000.0).floor() / 100.0)
    } else if num >= 100_000.0 {
        format!("{}K", (num / 1000.0).floor())
    } else if num >= 10_000.0 {
        format!("{:.1}K", (num / 100.0).floor() / 10.0)
    } else if num >= 1_000.0 {
        format!("{:.2}K", (num / 10.0).floor() / 100.0)
    } else {
        format!("{:.0}", num.round())
    }
}
