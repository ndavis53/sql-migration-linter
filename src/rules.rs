//! Individual lint rules. Each rule is a plain function from source text to
//! a list of findings, so it can be tested with a string literal and no
//! setup. Matching is done per line rather than with a real SQL parser -
//! that's a deliberate scope cut for now, see README.

use crate::Finding;

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
}
