//! Core linting logic. Every public function here is pure: given the same
//! source text it always returns the same findings, with no file or network
//! access. That's what makes the rules cheap to unit test and safe to run
//! against migration files before they touch a real database.

pub mod config;
pub mod rules;
pub mod tokenizer;

use config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 1-indexed, matching how editors and `grep -n` report lines.
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

/// Runs every rule against `source` and returns findings ordered by line.
///
/// Rules see `source` with string and comment contents blanked out first,
/// so a keyword that only appears inside a string literal or a comment
/// doesn't get flagged as if it were a real statement.
pub fn lint(source: &str) -> Vec<Finding> {
    lint_with_config(source, &Config::default())
}

/// Same as [`lint`], but skips any rule `config` disables.
pub fn lint_with_config(source: &str, config: &Config) -> Vec<Finding> {
    let masked = tokenizer::mask_strings_and_comments(source);
    let mut findings = Vec::new();
    if config.is_enabled("drop-table-without-if-exists") {
        findings.extend(rules::drop_table_without_if_exists(&masked));
    }
    if config.is_enabled("drop-column") {
        findings.extend(rules::drop_column(&masked));
    }
    if config.is_enabled("select-star") {
        findings.extend(rules::select_star(&masked));
    }
    if config.is_enabled("add-column-not-null-without-default") {
        findings.extend(rules::add_column_not_null_without_default(&masked));
    }
    if config.is_enabled("rename-column-referenced-by-view") {
        findings.extend(rules::rename_column_referenced_by_view(&masked));
    }
    findings.sort_by_key(|f| f.line);
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_findings_from_multiple_rules_in_line_order() {
        let sql = "DROP TABLE accounts;\nSELECT * FROM accounts;";
        let findings = lint(sql);
        assert_eq!(findings.len(), 2);
        assert_eq!(findings[0].line, 1);
        assert_eq!(findings[0].rule, "drop-table-without-if-exists");
        assert_eq!(findings[1].line, 2);
        assert_eq!(findings[1].rule, "select-star");
    }

    #[test]
    fn clean_migration_has_no_findings() {
        let sql = "CREATE TABLE accounts (id INTEGER PRIMARY KEY);";
        assert!(lint(sql).is_empty());
    }

    #[test]
    fn disabled_rule_is_skipped_but_others_still_run() {
        let sql = "DROP TABLE accounts;\nSELECT * FROM accounts;";
        let config = Config::parse("disable drop-table-without-if-exists\n").unwrap();
        let findings = lint_with_config(sql, &config);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].rule, "select-star");
    }

    #[test]
    fn ignores_keywords_inside_strings_and_comments() {
        let sql = "-- DROP TABLE accounts is just an example in this comment\n\
                    INSERT INTO log(msg) VALUES ('remember to DROP TABLE later');";
        assert!(lint(sql).is_empty());
    }
}
