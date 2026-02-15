use unicode_normalization::UnicodeNormalization;

/// Normalize Unicode text to NFC form and clean up whitespace.
///
/// This ensures consistent representation of accented characters
/// (important for Italian: à, è, é, ì, ò, ù) and removes
/// extraneous whitespace from HTML extraction.
pub fn normalize_text(input: &str) -> String {
    let nfc: String = input.nfc().collect();

    nfc.lines()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Collapse multiple consecutive blank lines into a single blank line.
pub fn collapse_blank_lines(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_blank = false;

    for line in input.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue;
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        prev_blank = is_blank;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_nfc() {
        // e + combining acute accent -> é (precomposed)
        let decomposed = "e\u{0301}";
        let result = normalize_text(decomposed);
        assert_eq!(result, "é");
    }

    #[test]
    fn test_trailing_whitespace() {
        let input = "hello   \nworld  ";
        let result = normalize_text(input);
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn test_collapse_blank_lines() {
        let input = "line 1\n\n\n\nline 2\n\nline 3";
        let result = collapse_blank_lines(input);
        assert_eq!(result, "line 1\n\nline 2\n\nline 3");
    }
}
