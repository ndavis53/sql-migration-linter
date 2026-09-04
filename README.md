# sqlmig-lint

A command-line linter for SQL migration files. It reads `.sql` files and
reports risky or fragile statements with a file, a line number, and a rule
name, the same shape as compiler warnings.

## Why

Migration files get reviewed like any other diff, but the failure modes are
different: a statement that's syntactically fine can still be destructive
(`DROP COLUMN`), or only fail once it hits a database that's already been
migrated (`DROP TABLE` without `IF EXISTS`), or quietly break later when the
schema changes underneath it (`SELECT *` baked into a view). Those are easy
to miss in review and easy to catch with a plain text scan before the
migration ever runs.

## Usage

```
cargo build --release
./target/release/sqlmig-lint migrations/0007_drop_legacy_columns.sql
```

Given a file like:

```sql
DROP TABLE sessions;
ALTER TABLE users DROP COLUMN legacy_id;
CREATE VIEW active_users AS SELECT * FROM users WHERE active;
```

the linter prints:

```
migrations/0007_drop_legacy_columns.sql:1: [drop-table-without-if-exists] DROP TABLE without IF EXISTS will fail if the table is already gone
migrations/0007_drop_legacy_columns.sql:2: [drop-column] dropping a column is destructive and not reversible once data is gone
migrations/0007_drop_legacy_columns.sql:3: [select-star] SELECT * in a migration breaks when columns change later
```

It exits non-zero if it found anything, so it can be dropped into a
pre-commit hook or CI step.

## Rules implemented so far

- `drop-table-without-if-exists`
- `drop-column`
- `select-star`

More rules (renaming a column that's still referenced elsewhere, missing
`NOT NULL` defaults on new columns, transactions that mix DDL and DML) are
planned; see the design note below before adding one.

## Design

Every public function in `src/lib.rs` and `src/rules.rs` is pure: it takes
`&str` and returns a `Vec<Finding>`, with no file I/O and no global state.
`src/main.rs` is the only place that touches the filesystem or the process
exit code. That split is what keeps rules trivial to unit test - each one
is a string literal in, a list of findings out - and it's worth preserving
when adding new rules.

The current rules match on uppercased line content rather than parsing SQL
properly. That's a real limitation: a `DROP TABLE` mentioned inside a
string literal or a comment will still be flagged. Fixing that means adding
a minimal tokenizer, which is on the roadmap rather than in this first cut.

## License

MIT, see `LICENSE`.
