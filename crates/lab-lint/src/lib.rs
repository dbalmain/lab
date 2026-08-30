//! Checking a corpus of notes against a schema.
//!
//! Every check here is driven by the schema data file rather than hardcoded, so
//! adding a field or a rule is a data edit. The set of *predicates* is closed
//! and lives in [`lab_schema::Check`] — P1 through P4 have no expression
//! language, so lint never waits on one.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use jiff::civil::Date;
use lab_note::Note;
use lab_schema::{Check, Rule, Schema, Severity};
use serde_yaml_ng::Value;

mod fields;

/// One thing wrong with one note.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub line: Option<usize>,
    /// The rule that produced this, so `--explain` has something to look up.
    pub rule: String,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub fn render(&self, root: &Path) -> String {
        let path = self.path.strip_prefix(root).unwrap_or(&self.path).display();
        let level = match self.severity {
            Severity::Error => "error",
            Severity::Warn => "warn",
        };
        match self.line {
            Some(line) => format!("{path}:{line}: {level}: {} [{}]", self.message, self.rule),
            None => format!("{path}: {level}: {} [{}]", self.message, self.rule),
        }
    }
}

pub struct Report {
    pub diagnostics: Vec<Diagnostic>,
    pub notes_checked: usize,
    /// Rules in the schema that nothing ran, because they need something this
    /// invocation did not have — the kb-priv link index, chiefly.
    pub not_run: Vec<String>,
}

impl Report {
    pub fn errors(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.diagnostics.len() - self.errors()
    }
}

/// Checks every note under `root`.
///
/// `today` is passed rather than read so that a staleness check is testable and
/// so a run is reproducible; see the `asserted-staleness` rule.
pub fn check(schema: &Schema, root: &Path, today: Date) -> Result<Report> {
    let paths = lab_note::walk(root)?;
    let mut diagnostics = Vec::new();
    let mut notes = Vec::new();

    for path in &paths {
        match Note::load(path) {
            Ok(note) => notes.push(note),
            Err(bad) => diagnostics.push(Diagnostic {
                path: bad.path,
                line: bad.line,
                rule: rule_id_for(schema, Check::YamlParses)
                    .unwrap_or("unreadable")
                    .to_string(),
                severity: Severity::Error,
                message: bad.problem,
            }),
        }
    }

    let by_name: BTreeMap<&str, &Note> = notes.iter().map(|n| (n.name.as_str(), n)).collect();

    for note in &notes {
        fields::check(schema, note, &mut diagnostics);
        for rule in &schema.rule {
            if !applies(rule, note) {
                continue;
            }
            match rule.check {
                Check::NoDerivableFields => derivable(rule, note, &mut diagnostics),
                Check::AssertedAge => asserted_age(rule, note, today, &mut diagnostics),
                Check::LinkResolvesLocal => local_links(rule, note, &by_name, &mut diagnostics),
                _ => {}
            }
        }
    }

    for rule in &schema.rule {
        if rule.check == Check::InboundLink {
            inbound_links(rule, &notes, &mut diagnostics);
        }
    }

    diagnostics.sort_by(|a, b| (&a.path, a.line).cmp(&(&b.path, b.line)));

    Ok(Report {
        diagnostics,
        notes_checked: notes.len(),
        not_run: schema
            .rule
            .iter()
            .filter(|r| needs_index(r.check))
            .map(|r| r.id.clone())
            .collect(),
    })
}

/// Rules that need the kb-priv link index or the registry, neither of which this
/// slice reads. Reported rather than skipped silently: a check that never runs
/// looks exactly like a check that always passes.
fn needs_index(check: Check) -> bool {
    matches!(
        check,
        Check::LinkResolvesCross | Check::PublicationDirection | Check::PendingMoves
    )
}

fn applies(rule: &Rule, note: &Note) -> bool {
    if rule.applies_to.is_empty() {
        return true;
    }
    note.str_field("type")
        .is_some_and(|t| rule.applies_to.iter().any(|a| a == t))
}

fn rule_id_for(schema: &Schema, check: Check) -> Option<&str> {
    schema
        .rule
        .iter()
        .find(|r| r.check == check)
        .map(|r| r.id.as_str())
}

/// Fields the tool derives and therefore refuses to see stored.
const DERIVABLE: [&str; 2] = ["name", "visibility"];

fn derivable(rule: &Rule, note: &Note, out: &mut Vec<Diagnostic>) {
    for field in DERIVABLE {
        if note.get(field).is_some() {
            out.push(Diagnostic {
                path: note.path.clone(),
                line: None,
                rule: rule.id.clone(),
                severity: rule.severity,
                message: format!("`{field}` is derived and must not be stored — remove it",),
            });
        }
    }
}

fn asserted_age(rule: &Rule, note: &Note, today: Date, out: &mut Vec<Diagnostic>) {
    let Some(max_age) = rule.max_age_days else {
        return;
    };
    let Some(asserted) = note
        .str_field("asserted")
        .and_then(|s| s.parse::<Date>().ok())
    else {
        return; // A missing or malformed date is the field check's business.
    };
    let age = today.since(asserted).map(|s| s.get_days()).unwrap_or(0);
    if age > i64::from(max_age) as i32 {
        out.push(Diagnostic {
            path: note.path.clone(),
            line: None,
            rule: rule.id.clone(),
            severity: rule.severity,
            message: format!("asserted {age} days ago; recheck it and update `asserted`"),
        });
    }
}

fn local_links(
    rule: &Rule,
    note: &Note,
    by_name: &BTreeMap<&str, &Note>,
    out: &mut Vec<Diagnostic>,
) {
    for link in &note.links {
        if link.source.is_some() {
            continue; // Cross-source links are a different rule, and warn only.
        }
        if !by_name.contains_key(link.name.as_str()) {
            out.push(Diagnostic {
                path: note.path.clone(),
                line: Some(link.line),
                rule: rule.id.clone(),
                severity: rule.severity,
                message: format!("{} does not resolve in this source", link.as_written()),
            });
        }
    }
}

fn inbound_links(rule: &Rule, notes: &[Note], out: &mut Vec<Diagnostic>) {
    let linked: BTreeSet<&str> = notes
        .iter()
        .flat_map(|n| n.links.iter())
        .filter(|l| l.source.is_none())
        .map(|l| l.name.as_str())
        .collect();

    for note in notes {
        if !applies(rule, note) || linked.contains(note.name.as_str()) {
            continue;
        }
        out.push(Diagnostic {
            path: note.path.clone(),
            line: None,
            rule: rule.id.clone(),
            severity: rule.severity,
            message: "nothing links to this note; it will not be found by a reader \
                      following related notes"
                .to_string(),
        });
    }
}

/// Shared by the field checks: a value's shape, for messages.
fn describe(value: &Value) -> &'static str {
    match value {
        Value::Null => "empty",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Sequence(_) => "a list",
        Value::Mapping(_) => "a mapping",
        Value::Tagged(_) => "a tagged value",
    }
}
