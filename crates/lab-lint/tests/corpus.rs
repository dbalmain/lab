//! The linter against a corpus of deliberately broken notes.
//!
//! Every note in `tests/corpus` is wrong in exactly one way, or right, and each
//! test names the note it is about. That shape matters: a test asserting "there
//! are seven errors" passes for the wrong reasons the moment a rule is added,
//! and stops saying which rule caught what.

use std::path::{Path, PathBuf};

use jiff::civil::date;
use lab_lint::{Diagnostic, Report};
use lab_schema::{Schema, Severity};

fn corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn schema() -> Schema {
    Schema::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/note.toml"))
        .expect("the shipped note schema must load")
}

/// A fixed date, so `asserted-staleness` does not start failing on its own.
fn report() -> Report {
    lab_lint::check(&schema(), &corpus(), date(2026, 8, 30)).expect("corpus is readable")
}

fn about<'a>(report: &'a Report, note: &str) -> Vec<&'a Diagnostic> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.path.ends_with(note))
        .collect()
}

fn rules_for(report: &Report, note: &str) -> Vec<String> {
    about(report, note).iter().map(|d| d.rule.clone()).collect()
}

#[test]
fn a_well_formed_note_produces_nothing() {
    let report = report();
    assert!(
        about(&report, "good-note.md").is_empty(),
        "{:?}",
        about(&report, "good-note.md")
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_value_outside_the_enum_is_reported() {
    let report = report();
    let messages: Vec<_> = about(&report, "bad-enum.md")
        .iter()
        .map(|d| d.message.clone())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("`parable`")),
        "{messages:?}"
    );
}

#[test]
fn a_scope_outside_the_pattern_is_reported() {
    let report = report();
    let messages: Vec<_> = about(&report, "bad-scope.md")
        .iter()
        .map(|d| d.message.clone())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("does not match")),
        "{messages:?}"
    );
}

/// `project:lab` must pass where `whatever` fails — the pattern has to accept
/// its prefixed forms, not merely reject unknown words.
#[test]
fn a_prefixed_scope_is_accepted() {
    let report = report();
    assert!(about(&report, "linked-note.md").is_empty());
}

#[test]
fn a_stored_derivable_field_is_reported() {
    let report = report();
    assert!(
        rules_for(&report, "stores-derivable.md").contains(&"nothing-derivable".to_string()),
        "{:?}",
        rules_for(&report, "stores-derivable.md")
    );
}

#[test]
fn a_missing_required_field_is_reported() {
    let report = report();
    let messages: Vec<_> = about(&report, "missing-field.md")
        .iter()
        .map(|d| d.message.clone())
        .collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("`triggers` is required")),
        "{messages:?}"
    );
}

#[test]
fn a_stale_assertion_warns_rather_than_fails() {
    let report = report();
    let stale = about(&report, "stale.md");
    assert_eq!(stale.len(), 1, "{stale:?}");
    assert_eq!(stale[0].rule, "asserted-staleness");
    assert_eq!(stale[0].severity, Severity::Warn);
}

#[test]
fn a_dangling_local_link_is_an_error_with_a_line_number() {
    let report = report();
    let broken: Vec<_> = about(&report, "broken-link.md")
        .into_iter()
        .filter(|d| d.rule == "local-link-resolves")
        .collect();
    assert_eq!(broken.len(), 1, "{broken:?}");
    assert!(broken[0].line.is_some(), "no line number");
    assert!(broken[0].message.contains("[[no-such-note]]"));
}

/// The cross-source link in the same note must NOT be reported here: kb cannot
/// depend on another repo being checked out.
#[test]
fn a_cross_source_link_is_not_treated_as_local() {
    let report = report();
    let messages: Vec<_> = about(&report, "broken-link.md")
        .iter()
        .map(|d| d.message.clone())
        .collect();
    assert!(
        !messages.iter().any(|m| m.contains("otherrepo")),
        "{messages:?}"
    );
}

#[test]
fn an_unquoted_colon_is_caught_at_its_file_line() {
    let report = report();
    let bad = about(&report, "colon-in-scalar.md");
    assert_eq!(bad.len(), 1, "{bad:?}");
    assert_eq!(bad[0].rule, "plain-scalar-colon");
    assert_eq!(bad[0].line, Some(2), "line must be file-relative");
    assert!(
        !bad[0].message.contains("at line"),
        "message carries a second, disagreeing line number: {}",
        bad[0].message
    );
}

#[test]
fn an_orphaned_lesson_is_reported() {
    let report = report();
    assert!(
        rules_for(&report, "orphan.md").contains(&"lesson-inbound-link".to_string()),
        "{:?}",
        rules_for(&report, "orphan.md")
    );
}

/// The discriminating case for the inbound-link rule: a guide is also unlinked,
/// and must not be reported. A rule that simply flagged every unlinked note
/// would pass the orphan test above and be wrong.
#[test]
fn an_unlinked_guide_is_exempt() {
    let report = report();
    assert!(
        !rules_for(&report, "an-entry-point.md").contains(&"lesson-inbound-link".to_string()),
        "{:?}",
        rules_for(&report, "an-entry-point.md")
    );
}

/// The guide is the corpus's shell note: double brackets in code must produce
/// no link diagnostics at all.
#[test]
fn double_brackets_in_code_produce_no_diagnostics() {
    let report = report();
    assert!(
        about(&report, "an-entry-point.md").is_empty(),
        "{:?}",
        about(&report, "an-entry-point.md")
            .iter()
            .map(|d| &d.message)
            .collect::<Vec<_>>()
    );
}

/// A note that fails to parse is still counted as a note that could not be
/// checked, not as one that passed.
#[test]
fn an_unparseable_note_is_not_counted_as_checked() {
    let report = report();
    let files = lab_note::walk(&corpus()).unwrap().len();
    assert_eq!(
        report.notes_checked,
        files - 1,
        "one note in the corpus does not parse"
    );
}

/// Rules the tool cannot run yet must be named on every run. A check that never
/// runs is indistinguishable from a check that always passes.
#[test]
fn rules_needing_the_index_are_declared_not_run() {
    let report = report();
    for rule in [
        "cross-source-link",
        "public-links-public",
        "lazy-move-rewrite",
    ] {
        assert!(
            report.not_run.contains(&rule.to_string()),
            "{:?}",
            report.not_run
        );
    }
}
