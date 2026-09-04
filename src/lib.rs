//! Core linting logic. Every public function here is pure: given the same
//! source text it always returns the same findings, with no file or network
//! access. That's what makes the rules cheap to unit test and safe to run
//! against migration files before they touch a real database.

pub mod rules;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// 1-indexed, matching how editors and `grep -n` report lines.
    pub line: usize,
    pub rule: &'static str,
    pub message: String,
}

/// Runs every rule against `source` and returns findings ordered by line.
pub fn lint(source: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(rules::drop_table_without_if_exists(source));
    findings.extend(rules::drop_column(source));
    findings.extend(rules::select_star(source));
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
}
