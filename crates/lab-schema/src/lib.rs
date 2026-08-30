//! The note and ticket schema: a described set of fields and rules.
//!
//! The schema is data rather than code so that a change to it is a data edit
//! and a migration, not a recompile and a bespoke fixup pass. Everything that
//! consumes a schema -- the linter, `migrate`, and later the console's form
//! renderer -- reads the same description, so a field added here is a field all
//! three understand without further work.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

mod render;

pub use render::render_markdown;

/// A whole schema: the fields a record carries and the rules it must satisfy.
#[derive(Debug, Deserialize)]
pub struct Schema {
    pub schema: Meta,
    #[serde(default)]
    pub field: Vec<Field>,
    #[serde(default)]
    pub rule: Vec<Rule>,
    #[serde(default)]
    pub section: Vec<Section>,
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    pub title: String,
    pub version: u32,
    #[serde(default)]
    pub preamble: String,
}

/// One frontmatter field.
#[derive(Debug, Deserialize)]
pub struct Field {
    pub name: String,
    pub required: bool,
    pub kind: Kind,
    #[serde(default)]
    pub doc: String,
    /// Permitted values, for `Kind::Enum`.
    #[serde(default)]
    pub values: Vec<String>,
    /// Human-readable shape, for `Kind::Pattern`.
    #[serde(default)]
    pub pattern: Option<String>,
    /// Minimum length, for `Kind::List`.
    #[serde(default)]
    pub min: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Text,
    Enum,
    Pattern,
    Date,
    List,
}

/// One rule a record must satisfy.
///
/// `check` names a built-in predicate. The set is closed -- P1 through P4 have
/// no expression language at all, so lint never waits on one. A rule whose
/// violation cannot be mechanically detected is still worth stating, and says so
/// with [`Check::None`] rather than by being left out.
#[derive(Debug, Deserialize)]
pub struct Rule {
    pub id: String,
    pub section: String,
    pub severity: Severity,
    pub check: Check,
    pub fix: Fix,
    pub title: String,
    #[serde(default)]
    pub doc: String,
    /// Note types this applies to. Empty means every type.
    #[serde(default)]
    pub applies_to: Vec<String>,
    #[serde(default)]
    pub max_age_days: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Lint exits non-zero.
    Error,
    /// Lint reports and exits zero.
    Warn,
}

/// The closed set of built-in predicates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// Stated for the writer, not detectable by the tool.
    None,
    YamlParses,
    NoDerivableFields,
    InboundLink,
    AssertedAge,
    StripCodeSpans,
    LinkResolvesLocal,
    LinkResolvesCross,
    PublicationDirection,
    PendingMoves,
}

/// Whether `lint --fix` can repair a violation or only report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fix {
    Fix,
    Report,
}

/// Prose that argues for the schema rather than describing it.
#[derive(Debug, Deserialize)]
pub struct Section {
    pub id: String,
    pub title: String,
    /// The generated section this follows.
    pub after: String,
    #[serde(default)]
    pub doc: String,
}

impl Schema {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading schema {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("parsing schema {}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self> {
        let schema: Schema = toml::from_str(text)?;
        schema.validate()?;
        Ok(schema)
    }

    /// A schema that describes rules nothing can run, or names a section that
    /// does not exist, is a schema whose renderer silently drops content. Catch
    /// it at load rather than at read.
    fn validate(&self) -> Result<()> {
        for field in &self.field {
            match field.kind {
                Kind::Enum if field.values.is_empty() => {
                    anyhow::bail!("field `{}` is an enum with no values", field.name)
                }
                Kind::Pattern if field.pattern.is_none() => {
                    anyhow::bail!("field `{}` is a pattern with no shape", field.name)
                }
                _ => {}
            }
        }
        for rule in &self.rule {
            if !SECTIONS.contains(&rule.section.as_str()) {
                anyhow::bail!(
                    "rule `{}` is in section `{}`, which is not one of {SECTIONS:?}",
                    rule.id,
                    rule.section
                );
            }
            if rule.check == Check::None && rule.fix == Fix::Fix {
                anyhow::bail!(
                    "rule `{}` claims to be autofixable but has no check to detect it",
                    rule.id
                );
            }
        }
        for section in &self.section {
            if !SECTIONS.contains(&section.after.as_str()) {
                anyhow::bail!(
                    "section `{}` follows `{}`, which is not one of {SECTIONS:?}",
                    section.id,
                    section.after
                );
            }
        }
        Ok(())
    }

    pub fn rules_in(&self, section: &str) -> impl Iterator<Item = &Rule> {
        self.rule.iter().filter(move |r| r.section == section)
    }

    pub fn sections_after(&self, section: &str) -> impl Iterator<Item = &Section> {
        self.section.iter().filter(move |s| s.after == section)
    }
}

/// The generated sections, in render order.
pub const SECTIONS: [&str; 3] = ["frontmatter", "rules", "links"];

#[cfg(test)]
mod tests {
    use super::*;

    /// The real schema is the fixture: a change that breaks the loader should
    /// fail here rather than at the next `kb lint`.
    fn note_schema() -> Schema {
        Schema::load(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../schemas/note.toml"
        )))
        .expect("the shipped note schema must load")
    }

    #[test]
    fn shipped_schema_loads() {
        let schema = note_schema();
        assert_eq!(schema.schema.name, "note");
        assert!(!schema.field.is_empty());
        assert!(!schema.rule.is_empty());
    }

    #[test]
    fn every_field_is_required_so_far() {
        // Not a law -- an observation worth being told about when it stops
        // being true, since an optional field needs a default and migrate needs
        // to know it.
        let schema = note_schema();
        assert!(schema.field.iter().all(|f| f.required));
    }

    #[test]
    fn unenforceable_rules_are_marked_not_omitted() {
        let schema = note_schema();
        let unchecked: Vec<_> = schema
            .rule
            .iter()
            .filter(|r| r.check == Check::None)
            .map(|r| r.id.as_str())
            .collect();
        assert!(
            unchecked.contains(&"one-concept"),
            "one-concept-per-note cannot be detected and must say so, got {unchecked:?}"
        );
    }

    #[test]
    fn enum_without_values_is_rejected() {
        let err = Schema::parse(
            r#"
[schema]
name = "t"
title = "T"
version = 1

[[field]]
name = "type"
required = true
kind = "enum"
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("enum with no values"), "{err}");
    }

    #[test]
    fn a_rule_cannot_claim_a_fix_it_cannot_detect() {
        let err = Schema::parse(
            r#"
[schema]
name = "t"
title = "T"
version = 1

[[rule]]
id = "wishful"
section = "rules"
severity = "warn"
check = "none"
fix = "fix"
title = "Write good notes."
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("no check to detect it"), "{err}");
    }

    #[test]
    fn a_rule_in_an_unknown_section_is_rejected() {
        let err = Schema::parse(
            r#"
[schema]
name = "t"
title = "T"
version = 1

[[rule]]
id = "lost"
section = "appendix"
severity = "warn"
check = "none"
fix = "report"
title = "Somewhere else."
"#,
        )
        .unwrap_err();
        assert!(err.to_string().contains("not one of"), "{err}");
    }
}
