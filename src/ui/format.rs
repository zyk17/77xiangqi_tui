//! D 区与状态栏用的紧凑数字格式。

pub fn format_count_k(value: u64) -> String {
    if value >= 1_000 {
        let k = value as f64 / 1_000.0;
        if k >= 100.0 {
            format!("{:.0}k", k)
        } else {
            format!("{:.1}k", k)
        }
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_k_only() {
        assert_eq!(format_count_k(7675653), "7676k");
        assert_eq!(format_count_k(5994685), "5995k");
        assert_eq!(format_count_k(500), "500");
        assert_eq!(format_count_k(1500), "1.5k");
    }
}
