/// Add thousands separators to a number.
pub fn commas(value: usize) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }

    output
}

/// Format `part` as a percentage of `total` with one decimal place.
pub fn percent(part: usize, total: usize) -> String {
    if total == 0 {
        return "0.0%".to_string();
    }
    format!("{:.1}%", part as f64 / total as f64 * 100.0)
}

/// Format the average of `part` per `count` with one decimal place.
pub fn average(part: usize, count: usize) -> String {
    if count == 0 {
        return "0.0".to_string();
    }
    format!("{:.1}", part as f64 / count as f64)
}
