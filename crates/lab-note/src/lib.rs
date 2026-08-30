//! Reading a note: its frontmatter, its body, and the links out of it.
//!
//! Frontmatter is kept as an untyped mapping rather than a struct. The schema is
//! data (see `lab-schema`), so the set of fields is not known at compile time,
//! and a struct here would quietly become a second definition of the schema that
//! drifts from the first.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_yaml_ng::{Mapping, Value};

mod links;

pub use links::{Link, extract_links, mask_code};

/// One note on disk.
pub struct Note {
    pub path: PathBuf,
    /// The filename without its extension. Derived, never stored in the file —
    /// a name in the frontmatter would be a second copy to keep in sync.
    pub name: String,
    pub fields: Mapping,
    pub body: String,
    /// Line number of the first body line, so diagnostics point at the file
    /// rather than at an offset into the body.
    pub body_start_line: usize,
    pub links: Vec<Link>,
}

/// A note that could not be read far enough to check anything else about it.
pub struct Unreadable {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub problem: String,
}

impl Note {
    pub fn load(path: &Path) -> Result<Self, Unreadable> {
        let text = std::fs::read_to_string(path).map_err(|e| Unreadable {
            path: path.to_path_buf(),
            line: None,
            problem: e.to_string(),
        })?;
        Self::parse(path, &text)
    }

    pub fn parse(path: &Path, text: &str) -> Result<Self, Unreadable> {
        let unreadable = |line, problem: String| Unreadable {
            path: path.to_path_buf(),
            line,
            problem,
        };

        let (front, body, body_start_line) =
            split(text).map_err(|problem| unreadable(Some(1), problem))?;

        // The whole point of surfacing the parse error verbatim: the `: ` in a
        // plain scalar is the failure this catches, and serde's message names
        // the line it happened on.
        let value: Value = serde_yaml_ng::from_str(front).map_err(|e| {
            // serde's location is relative to the frontmatter; the diagnostic
            // wants a file line. Once it is hoisted into the prefix, serde's own
            // trailing " at line N column M" is a second, disagreeing number in
            // the same message -- so drop it.
            let line = e.location().map(|l| l.line() + 1);
            let problem = e.to_string();
            let problem = match problem.find(" at line ") {
                Some(at) if line.is_some() => problem[..at].to_string(),
                _ => problem,
            };
            unreadable(line, problem)
        })?;

        let Value::Mapping(fields) = value else {
            return Err(unreadable(Some(2), "frontmatter is not a mapping".into()));
        };

        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| unreadable(None, "filename is not valid UTF-8".into()))?
            .to_string();

        let links = extract_links(body, body_start_line);

        Ok(Note {
            path: path.to_path_buf(),
            name,
            fields,
            body: body.to_string(),
            body_start_line,
            links,
        })
    }

    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(Value::String(field.to_string()))
    }

    pub fn str_field(&self, field: &str) -> Option<&str> {
        self.get(field).and_then(Value::as_str)
    }
}

/// Splits `---`-delimited frontmatter from the body.
///
/// Returns the frontmatter, the body, and the line the body starts on.
fn split(text: &str) -> Result<(&str, &str, usize), String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let rest = text
        .strip_prefix("---\n")
        .ok_or_else(|| "no frontmatter: the file must open with `---`".to_string())?;

    let end = rest
        .find("\n---\n")
        .ok_or_else(|| "frontmatter is never closed with `---`".to_string())?;

    let front = &rest[..end];
    let body = &rest[end + "\n---\n".len()..];
    // 1 for the opening `---`, one per frontmatter line, 1 for the closing.
    let body_start_line = 2 + front.lines().count() + 1;
    Ok((front, body, body_start_line))
}

/// Every `.md` file under `root`, excluding dotted directories and the
/// generated index.
pub fn walk(root: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    walk_into(root, &mut found).with_context(|| format!("walking {}", root.display()))?;
    found.sort();
    Ok(found)
}

fn walk_into(dir: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk_into(&path, found)?;
        } else if path.extension().is_some_and(|e| e == "md") {
            // Uppercase filenames are the repo's own documents -- README,
            // SCHEMA, INDEX -- not notes.
            if name.chars().next().is_some_and(char::is_uppercase) {
                continue;
            }
            found.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "---\ndescription: A thing.\ntype: lesson\n---\n\nBody here.\n";

    #[test]
    fn splits_frontmatter_from_body() {
        let note = Note::parse(Path::new("x/monitors-self-match.md"), SAMPLE)
            .ok()
            .unwrap();
        assert_eq!(note.name, "monitors-self-match");
        assert_eq!(note.str_field("type"), Some("lesson"));
        assert_eq!(note.body.trim(), "Body here.");
    }

    #[test]
    fn body_line_numbers_are_file_line_numbers() {
        let note = Note::parse(Path::new("x/a.md"), SAMPLE).ok().unwrap();
        // `---`, two fields, `---` -> the body starts on line 5.
        assert_eq!(note.body_start_line, 5);
    }

    /// The failure this whole path exists to catch: a plain scalar containing
    /// `: ` is not a string, and the note silently fails to load.
    #[test]
    fn a_colon_in_a_plain_scalar_is_reported_with_its_line() {
        let text =
            "---\ndescription: Coding principles: gates, brevity\ntype: lesson\n---\n\nBody.\n";
        let err = Note::parse(Path::new("x/a.md"), text).err().unwrap();
        assert_eq!(err.line, Some(2), "{}", err.problem);
    }

    #[test]
    fn a_file_without_frontmatter_is_unreadable_not_empty() {
        let err = Note::parse(Path::new("x/a.md"), "Just prose.\n")
            .err()
            .unwrap();
        assert!(err.problem.contains("must open with"), "{}", err.problem);
    }

    #[test]
    fn unclosed_frontmatter_is_reported_as_such() {
        let err = Note::parse(Path::new("x/a.md"), "---\ntype: lesson\n")
            .err()
            .unwrap();
        assert!(err.problem.contains("never closed"), "{}", err.problem);
    }
}
