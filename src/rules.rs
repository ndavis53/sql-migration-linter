//! Individual lint rules. Each rule is a plain function from source text to
//! a list of findings, so it can be tested with a string literal and no
//! setup. Matching is done per line rather than with a real SQL parser -
//! that's a deliberate scope cut for now, see README.

use crate::Finding;

/// Canonical rule names, in the order `lint` runs them. This is the list a
/// config file's `disable` directives are checked against, so a typo in a
/// config file is caught instead of silently doing nothing.
pub const RULE_NAMES: [&str; 5] = [
    "drop-table-without-if-exists",
    "drop-column",
    "select-star",
    "add-column-not-null-without-default",
    "rename-column-referenced-by-view",
];

/// `DROP TABLE` without `IF EXISTS` fails outright if a previous deploy
/// already removed the table, which turns a routine migration into a
/// blocked release.
pub fn drop_table_without_if_exists(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let upper = line.to_uppercase();
        if upper.contains("DROP TABLE") && !upper.contains("IF EXISTS") {
            findings.push(Finding {
                line: idx + 1,
                rule: "drop-table-without-if-exists",
                message: "DROP TABLE without IF EXISTS will fail if the table is already gone"
                    .to_string(),
            });
        }
    }
    findings
}

/// Dropping a column destroys data with no way back once the migration
/// runs, so it's worth a human pausing on it even when intentional.
pub fn drop_column(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let upper = line.to_uppercase();
        if upper.contains("DROP COLUMN") {
            findings.push(Finding {
                line: idx + 1,
                rule: "drop-column",
                message: "dropping a column is destructive and not reversible once data is gone"
                    .to_string(),
            });
        }
    }
    findings
}

/// `SELECT *` inside a view or trigger definition silently changes shape
/// the next time a column is added or removed elsewhere.
pub fn select_star(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let upper = line.to_uppercase();
        if upper.contains("SELECT *") {
            findings.push(Finding {
                line: idx + 1,
                rule: "select-star",
                message: "SELECT * in a migration breaks when columns change later".to_string(),
            });
        }
    }
    findings
}

/// Adding a `NOT NULL` column with no `DEFAULT` fails outright against a
/// table that already has rows, since the database has nothing to put in
/// the new column for them.
pub fn add_column_not_null_without_default(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let upper = line.to_uppercase();
        if upper.contains("ADD COLUMN") && upper.contains("NOT NULL") && !upper.contains("DEFAULT")
        {
            findings.push(Finding {
                line: idx + 1,
                rule: "add-column-not-null-without-default",
                message:
                    "ADD COLUMN ... NOT NULL without a DEFAULT fails on a table that already has rows"
                        .to_string(),
            });
        }
    }
    findings
}

/// Renaming a column that a view still selects by name breaks the view the
/// moment the migration runs, but nothing in the `ALTER TABLE` statement
/// itself hints at that - the failure only shows up when something queries
/// the view. This rule looks across the whole file: every `RENAME COLUMN`
/// is checked against every `CREATE VIEW` body for a reference to the old
/// name, regardless of which statement comes first in the file.
pub fn rename_column_referenced_by_view(source: &str) -> Vec<Finding> {
    let statements = split_statements(source);

    let mut renames = Vec::new();
    let mut views = Vec::new();
    for (text, offset) in &statements {
        let tokens = tokenize(text);
        for (i, tok) in tokens.iter().enumerate() {
            if tok.text.eq_ignore_ascii_case("RENAME")
                && tokens.get(i + 1).is_some_and(|t| t.text.eq_ignore_ascii_case("COLUMN"))
            {
                if let (Some(old), Some(kw), Some(_new)) =
                    (tokens.get(i + 2), tokens.get(i + 3), tokens.get(i + 4))
                {
                    if kw.text.eq_ignore_ascii_case("TO") {
                        renames.push((old.text, offset + old.offset));
                    }
                }
            }
        }

        if let Some(view_idx) = tokens.iter().position(|t| t.text.eq_ignore_ascii_case("VIEW")) {
            let name = tokens.get(view_idx + 1).map(|t| t.text).unwrap_or("?");
            views.push((name, *text));
        }
    }

    let mut findings = Vec::new();
    for (old, abs_offset) in &renames {
        let old_upper = old.to_uppercase();
        for (view_name, body) in &views {
            if contains_word(&body.to_uppercase(), &old_upper) {
                findings.push(Finding {
                    line: line_at(source, *abs_offset),
                    rule: "rename-column-referenced-by-view",
                    message: format!(
                        "renaming column `{old}` may break view `{view_name}`, which appears to reference it"
                    ),
                });
                break;
            }
        }
    }
    findings
}

struct Token<'a> {
    text: &'a str,
    offset: usize,
}

/// Splits `s` into runs of identifier characters, dropping everything else.
/// Good enough to pull keywords and names out of a statement without a real
/// SQL tokenizer.
fn tokenize(s: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (i, c) in s.char_indices() {
        if c.is_alphanumeric() || c == '_' {
            if start.is_none() {
                start = Some(i);
            }
        } else if let Some(st) = start.take() {
            tokens.push(Token { text: &s[st..i], offset: st });
        }
    }
    if let Some(st) = start {
        tokens.push(Token { text: &s[st..], offset: st });
    }
    tokens
}

/// Splits on `;` and returns each statement's text alongside its byte
/// offset into `source`, so callers can still map back to a line number.
fn split_statements(source: &str) -> Vec<(&str, usize)> {
    let mut statements = Vec::new();
    let mut start = 0;
    for (i, c) in source.char_indices() {
        if c == ';' {
            statements.push((&source[start..i], start));
            start = i + c.len_utf8();
        }
    }
    if start < source.len() {
        statements.push((&source[start..], start));
    }
    statements
}

fn line_at(source: &str, offset: usize) -> usize {
    1 + source[..offset].matches('\n').count()
}

/// Case-insensitive search for `needle` as a whole word in `haystack`.
/// Both arguments are expected to already be uppercased by the caller.
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    let mut start = 0;
    while let Some(pos) = haystack[start..].find(needle) {
        let idx = start + pos;
        let before_ok = idx == 0 || !is_ident(bytes[idx - 1]);
        let after = idx + needle.len();
        let after_ok = after >= bytes.len() || !is_ident(bytes[after]);
        if before_ok && after_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_drop_table_without_if_exists() {
        let sql = "DROP TABLE users;";
        let findings = drop_table_without_if_exists(sql);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
    }

    #[test]
    fn allows_drop_table_with_if_exists() {
        let sql = "DROP TABLE IF EXISTS users;";
        assert!(drop_table_without_if_exists(sql).is_empty());
    }

    #[test]
    fn is_case_insensitive() {
        let sql = "drop table users;";
        assert_eq!(drop_table_without_if_exists(sql).len(), 1);
    }

    #[test]
    fn flags_drop_column() {
        let sql = "ALTER TABLE users DROP COLUMN email;";
        let findings = drop_column(sql);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "drop-column");
    }

    #[test]
    fn flags_select_star() {
        let sql = "CREATE VIEW v AS SELECT * FROM users;";
        let findings = select_star(sql);
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn flags_add_column_not_null_without_default() {
        let sql = "ALTER TABLE users ADD COLUMN age INTEGER NOT NULL;";
        let findings = add_column_not_null_without_default(sql);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "add-column-not-null-without-default");
    }

    #[test]
    fn allows_add_column_not_null_with_default() {
        let sql = "ALTER TABLE users ADD COLUMN age INTEGER NOT NULL DEFAULT 0;";
        assert!(add_column_not_null_without_default(sql).is_empty());
    }

    #[test]
    fn allows_add_column_that_is_nullable() {
        let sql = "ALTER TABLE users ADD COLUMN nickname TEXT;";
        assert!(add_column_not_null_without_default(sql).is_empty());
    }

    #[test]
    fn line_numbers_track_multiline_input() {
        let sql = "-- comment\nDROP TABLE accounts;\nSELECT * FROM accounts;";
        let mut findings = drop_table_without_if_exists(sql);
        findings.extend(select_star(sql));
        findings.sort_by_key(|f| f.line);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[1].line, 3);
    }

    #[test]
    fn flags_rename_column_still_used_by_view() {
        let sql = "ALTER TABLE users RENAME COLUMN email TO email_address;\n\
                    CREATE VIEW active_users AS SELECT email FROM users WHERE active;";
        let findings = rename_column_referenced_by_view(sql);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[0].rule, "rename-column-referenced-by-view");
        assert!(findings[0].message.contains("active_users"));
    }

    #[test]
    fn detects_rename_regardless_of_statement_order() {
        let sql = "CREATE VIEW active_users AS SELECT email FROM users WHERE active;\n\
                    ALTER TABLE users RENAME COLUMN email TO email_address;";
        let findings = rename_column_referenced_by_view(sql);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
    }

    #[test]
    fn allows_rename_when_no_view_references_it() {
        let sql = "ALTER TABLE users RENAME COLUMN legacy_id TO legacy_identifier;\n\
                    CREATE VIEW active_users AS SELECT email FROM users WHERE active;";
        assert!(rename_column_referenced_by_view(sql).is_empty());
    }

    #[test]
    fn does_not_match_column_name_as_a_substring() {
        let sql = "ALTER TABLE users RENAME COLUMN id TO user_id;\n\
                    CREATE VIEW valid_users AS SELECT valid_id FROM users;";
        assert!(rename_column_referenced_by_view(sql).is_empty());
    }

    #[test]
    fn rule_names_has_no_duplicates() {
        for (i, name) in RULE_NAMES.iter().enumerate() {
            assert!(
                !RULE_NAMES[..i].contains(name),
                "{name} appears more than once in RULE_NAMES"
            );
        }
    }
}
