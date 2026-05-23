//! 引擎可执行路径规范化与等价比较。

pub(crate) fn sanitize_engine_path(raw: &str) -> Option<String> {
    let mut s = raw.trim().to_string();
    if s.len() >= 2 {
        let b = s.as_bytes();
        if (b[0] == b'"' && b[s.len() - 1] == b'"') || (b[0] == b'\'' && b[s.len() - 1] == b'\'') {
            s = s[1..s.len() - 1].trim().to_string();
        }
    }
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

pub(crate) fn normalize_engine_path_for_compare(raw: &str) -> Option<String> {
    let s = sanitize_engine_path(raw)?;
    #[cfg(windows)]
    {
        let mut t = s.replace('/', "\\");
        t = t.to_ascii_lowercase();
        while t.ends_with('\\') {
            t.pop();
        }
        Some(t)
    }
    #[cfg(not(windows))]
    {
        let mut t = s;
        while t.ends_with(std::path::MAIN_SEPARATOR) {
            t.pop();
        }
        Some(t)
    }
}

pub(crate) fn same_engine_path(a: &str, b: &str) -> bool {
    match (
        normalize_engine_path_for_compare(a),
        normalize_engine_path_for_compare(b),
    ) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

pub(crate) fn has_non_ascii(s: &str) -> bool {
    !s.is_ascii()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_quotes_and_whitespace() {
        assert_eq!(
            sanitize_engine_path("  \"C:\\eng\\pikafish.exe\"  ").as_deref(),
            Some("C:\\eng\\pikafish.exe")
        );
        assert!(sanitize_engine_path("   ").is_none());
    }

    #[test]
    fn same_engine_path_windows_case_insensitive() {
        assert!(same_engine_path(
            r"C:\Games\engine.EXE",
            r"c:\games\engine.exe"
        ));
        assert!(!same_engine_path(r"C:\a.exe", r"C:\b.exe"));
    }
}
