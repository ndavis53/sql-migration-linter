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

## Config file

Pass `--config <file>` to turn individual rules off:

```
./target/release/sqlmig-lint --config sqlmig-lint.conf migrations/*.sql
```

The file has one directive per line - `disable <rule-name>` - with `#`
starting a comment and blank lines ignored:

```
# this table is deliberately dropped without IF EXISTS during teardown
disable drop-table-without-if-exists
disable select-star
```

An unknown rule name or a malformed line is a hard error rather than a
silent no-op, so a typo in the config doesn't leave a rule looking disabled
when it's still running. There's no way to disable every rule and enable a
few back on - every rule starts enabled, so `enable` has nothing to do yet.

## Rules implemented so far

- `drop-table-without-if-exists`
- `drop-column`
- `select-star`
- `add-column-not-null-without-default`
- `rename-column-referenced-by-view`

More rules (transactions that mix DDL and DML) are planned; see the design
note below before adding one.

## Design

Every public function in `src/lib.rs` and `src/rules.rs` is pure: it takes
`&str` and returns a `Vec<Finding>`, with no file I/O and no global state.
`src/main.rs` is the only place that touches the filesystem or the process
exit code. That split is what keeps rules trivial to unit test - each one
is a string literal in, a list of findings out - and it's worth preserving
when adding new rules.

Most rules still match on uppercased line content rather than parsing SQL
properly, but before they run, `src/tokenizer.rs` blanks out the contents of
string literals and comments (`--`, `/* */`, quoted strings and identifiers,
including doubled-quote escapes) so a keyword mentioned there doesn't get
flagged as if it were a real statement. It's still not a real parser:
multi-statement lines and dialect-specific quoting rules beyond the ANSI
basics aren't handled.

`rename-column-referenced-by-view` is the exception: it needs to compare a
`RENAME COLUMN` against every `CREATE VIEW` in the file, so it splits the
source on `;` into statements and pulls identifier tokens out of each one
instead of matching a single line.

## License

MIT, see `LICENSE`.
