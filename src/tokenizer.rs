//! Minimal SQL tokenizer used to blank out string literals and comments
//! before rules run, so a keyword mentioned inside a string or a comment
//! isn't mistaken for actual SQL. It doesn't parse SQL - it just tracks
//! enough state (in a string, in a line comment, in a block comment) to
//! know what to mask, character by character. Line and column positions
//! are preserved: only characters are replaced, newlines never are, so
//! callers can still report accurate line numbers against the result.

#[derive(PartialEq)]
enum State {
    Code,
    SingleQuoted,
    DoubleQuoted,
    LineComment,
    BlockComment,
}

/// Returns `source` with the contents of string literals and comments
/// replaced by spaces. `--` line comments, `/* */` block comments,
/// single-quoted strings, and double-quoted identifiers are all masked,
/// including the doubled-quote escape (`''` / `""`) SQL uses inside them.
pub fn mask_strings_and_comments(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut state = State::Code;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match state {
            State::Code => {
                if c == '\'' {
                    state = State::SingleQuoted;
                    out.push(c);
                } else if c == '"' {
                    state = State::DoubleQuoted;
                    out.push(c);
                } else if c == '-' && chars.get(i + 1) == Some(&'-') {
                    state = State::LineComment;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                } else if c == '/' && chars.get(i + 1) == Some(&'*') {
                    state = State::BlockComment;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                } else {
                    out.push(c);
                }
            }
            State::SingleQuoted => {
                if c == '\'' && chars.get(i + 1) == Some(&'\'') {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                } else if c == '\'' {
                    state = State::Code;
                    out.push(c);
                } else if c == '\n' {
                    out.push(c);
                } else {
                    out.push(' ');
                }
            }
            State::DoubleQuoted => {
                if c == '"' && chars.get(i + 1) == Some(&'"') {
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                } else if c == '"' {
                    state = State::Code;
                    out.push(c);
                } else if c == '\n' {
                    out.push(c);
                } else {
                    out.push(' ');
                }
            }
            State::LineComment => {
                if c == '\n' {
                    state = State::Code;
                    out.push(c);
                } else {
                    out.push(' ');
                }
            }
            State::BlockComment => {
                if c == '*' && chars.get(i + 1) == Some(&'/') {
                    state = State::Code;
                    out.push(' ');
                    out.push(' ');
                    i += 2;
                    continue;
                } else if c == '\n' {
                    out.push(c);
                } else {
                    out.push(' ');
                }
            }
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(source: &str) -> Vec<String> {
        mask_strings_and_comments(source)
            .lines()
            .map(|l| l.to_string())
            .collect()
    }

    #[test]
    fn preserves_code_untouched() {
        let sql = "DROP TABLE users;";
        assert_eq!(mask_strings_and_comments(sql), sql);
    }

    #[test]
    fn masks_single_quoted_string_contents() {
        let sql = "SELECT 'DROP TABLE users' FROM logs;";
        let masked = mask_strings_and_comments(sql);
        assert!(!masked.to_uppercase().contains("DROP TABLE"));
        // quotes themselves stay so the string still parses as one token
        assert!(masked.contains('\''));
    }

    #[test]
    fn masks_escaped_quote_inside_string() {
        let sql = "SELECT 'it''s DROP TABLE fine' FROM logs;";
        let masked = mask_strings_and_comments(sql);
        assert!(!masked.to_uppercase().contains("DROP TABLE"));
        // the statement itself resumes correctly after the string closes
        assert!(masked.trim_end().ends_with("FROM logs;"));
    }

    #[test]
    fn masks_double_quoted_identifier_contents() {
        let sql = r#"SELECT * FROM "DROP TABLE";"#;
        let masked = mask_strings_and_comments(sql);
        assert_eq!(masked.matches("DROP TABLE").count(), 0);
    }

    #[test]
    fn masks_line_comment_to_end_of_line() {
        let sql = "-- DROP TABLE users\nSELECT 1;";
        let result = lines(sql);
        assert!(!result[0].to_uppercase().contains("DROP TABLE"));
        assert_eq!(result[1], "SELECT 1;");
    }

    #[test]
    fn masks_multiline_block_comment() {
        let sql = "/* DROP TABLE users\nstill a comment */\nSELECT 1;";
        let result = lines(sql);
        assert!(!result[0].to_uppercase().contains("DROP TABLE"));
        assert!(!result[1].to_uppercase().contains("COMMENT"));
        assert_eq!(result[2], "SELECT 1;");
    }

    #[test]
    fn preserves_line_count_and_newlines() {
        let sql = "SELECT 'a\nb' FROM t;\n-- trailing\nDROP TABLE x;";
        let masked = mask_strings_and_comments(sql);
        assert_eq!(masked.lines().count(), sql.lines().count());
    }
}
