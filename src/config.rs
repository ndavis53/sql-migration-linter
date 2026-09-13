//! Parsing for the optional config file that turns individual rules off.
//!
//! The format is deliberately dumb: one directive per line, `#` starts a
//! comment (inline or whole-line), blank lines are ignored. The only
//! directive is `disable <rule-name>` - every rule ships enabled, so there's
//! nothing to enable that isn't already on.

use std::collections::HashSet;
use std::fmt;

use crate::rules::RULE_NAMES;

/// Which rules to skip. Anything not listed here is enabled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    disabled: HashSet<&'static str>,
}

impl Config {
    pub fn is_enabled(&self, rule: &str) -> bool {
        !self.disabled.contains(rule)
    }

    /// Parses a config file's contents. Fails on the first line that isn't
    /// blank, a comment, or a valid `disable <rule-name>` directive, so a
    /// typo turns into an error instead of a rule that's quietly never
    /// disabled.
    pub fn parse(text: &str) -> Result<Config, ConfigError> {
        let mut disabled = HashSet::new();
        for (idx, raw_line) in text.lines().enumerate() {
            let line = match raw_line.split_once('#') {
                Some((before, _)) => before.trim(),
                None => raw_line.trim(),
            };
            if line.is_empty() {
                continue;
            }

            let mut words = line.split_whitespace();
            let directive = words.next();
            let rule = words.next();
            let extra = words.next();

            if directive != Some("disable") || extra.is_some() {
                return Err(ConfigError {
                    line: idx + 1,
                    message: format!("expected `disable <rule-name>`, got `{line}`"),
                });
            }
            let rule = rule.ok_or_else(|| ConfigError {
                line: idx + 1,
                message: "expected a rule name after `disable`".to_string(),
            })?;
            let canonical = RULE_NAMES.iter().find(|name| **name == rule).ok_or_else(|| {
                ConfigError { line: idx + 1, message: format!("unknown rule `{rule}`") }
            })?;
            disabled.insert(*canonical);
        }
        Ok(Config { disabled })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_config_enables_everything() {
        let config = Config::parse("").unwrap();
        for rule in RULE_NAMES {
            assert!(config.is_enabled(rule));
        }
    }

    #[test]
    fn disables_a_named_rule() {
        let config = Config::parse("disable drop-column\n").unwrap();
        assert!(!config.is_enabled("drop-column"));
        assert!(config.is_enabled("select-star"));
    }

    #[test]
    fn ignores_blank_lines_and_comments() {
        let text = "\n# turn off the noisy one\ndisable select-star\n\n";
        let config = Config::parse(text).unwrap();
        assert!(!config.is_enabled("select-star"));
    }

    #[test]
    fn allows_inline_comment_after_directive() {
        let config = Config::parse("disable drop-column # too noisy for now\n").unwrap();
        assert!(!config.is_enabled("drop-column"));
    }

    #[test]
    fn rejects_unknown_rule_name() {
        let err = Config::parse("disable drop-tables\n").unwrap_err();
        assert_eq!(err.line, 1);
        assert!(err.message.contains("drop-tables"));
    }

    #[test]
    fn rejects_directive_other_than_disable() {
        let err = Config::parse("enable drop-column\n").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn rejects_disable_with_no_rule_name() {
        let err = Config::parse("disable\n").unwrap_err();
        assert_eq!(err.line, 1);
    }

    #[test]
    fn error_line_number_accounts_for_earlier_lines() {
        let text = "disable drop-column\ndisable not-a-rule\n";
        let err = Config::parse(text).unwrap_err();
        assert_eq!(err.line, 2);
    }
}
