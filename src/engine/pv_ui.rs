//! 引擎 PV 在 TUI 中的长度上限。

/// 单条 PV 中最多保留的 UCI 半着数量。
pub const ENGINE_PV_UI_MAX_STEPS: usize = 16;

/// 截断 PV 列表，只保留合法四字符 UCI 着法。
pub fn truncate_engine_pv_for_ui(pv: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for s in pv {
        if out.len() >= ENGINE_PV_UI_MAX_STEPS {
            break;
        }
        if s.len() >= 4 {
            out.push(s.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_empty() {
        assert!(truncate_engine_pv_for_ui(&[]).is_empty());
    }

    #[test]
    fn truncate_keeps_valid_uci_up_to_max() {
        let uci = |i| format!("a{}b{}", i % 9, (i + 1) % 10);
        let pv: Vec<String> = (0..50).map(uci).collect();
        let out = truncate_engine_pv_for_ui(&pv);
        assert_eq!(out.len(), ENGINE_PV_UI_MAX_STEPS);
        assert_eq!(out[0], "a0b1");
        assert_eq!(
            out[ENGINE_PV_UI_MAX_STEPS - 1],
            uci(ENGINE_PV_UI_MAX_STEPS - 1)
        );
    }

    #[test]
    fn truncate_skips_short_strings() {
        let pv = vec![
            "ab".to_string(),
            "h2e2".to_string(),
            "x".to_string(),
            "h9g7".to_string(),
        ];
        assert_eq!(
            truncate_engine_pv_for_ui(&pv),
            vec!["h2e2".to_string(), "h9g7".to_string()]
        );
    }

    #[test]
    fn truncate_large_pv_under_time_budget() {
        let pv: Vec<String> = (0..5000)
            .map(|i| format!("a{}b{}", i % 9, i % 10))
            .collect();
        let t0 = std::time::Instant::now();
        let out = truncate_engine_pv_for_ui(&pv);
        let ms = t0.elapsed().as_secs_f64() * 1000.0;
        assert!(out.len() <= ENGINE_PV_UI_MAX_STEPS);
        assert!(ms < 5.0, "truncate took {}ms (expected < 5ms)", ms);
    }
}
