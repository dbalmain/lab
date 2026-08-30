//! Finding `[[wiki-links]]` in a body.
//!
//! The delicate part is not the pattern but what it must ignore. `[[ … ]]` is
//! also bash's test syntax, so a note about shell scripting contains double
//! brackets that are code, not links — and reporting those as broken links
//! trains the reader to ignore the linter.

use std::sync::LazyLock;

use regex::Regex;

static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").expect("static pattern"));

/// One `[[…]]` occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    /// The source prefix of a `[[source:name]]` link, if it has one.
    pub source: Option<String>,
    pub name: String,
    pub line: usize,
}

impl Link {
    /// How it was written, for messages and for rewriting.
    pub fn as_written(&self) -> String {
        match &self.source {
            Some(source) => format!("[[{source}:{}]]", self.name),
            None => format!("[[{}]]", self.name),
        }
    }
}

pub fn extract_links(body: &str, first_line: usize) -> Vec<Link> {
    let masked = mask_code(body);
    LINK.captures_iter(&masked)
        .map(|caps| {
            let whole = caps.get(0).expect("group 0");
            let target = caps.get(1).expect("group 1").as_str().trim();
            let line = first_line + masked[..whole.start()].matches('\n').count();
            match target.split_once(':') {
                Some((source, name)) => Link {
                    source: Some(source.trim().to_string()),
                    name: name.trim().to_string(),
                    line,
                },
                None => Link {
                    source: None,
                    name: target.to_string(),
                    line,
                },
            }
        })
        .collect()
}

/// Replaces the contents of fenced blocks and inline code spans with spaces.
///
/// Masking rather than deleting is what keeps byte offsets — and therefore line
/// numbers — pointing at the right place in the original text. Newlines survive
/// for the same reason.
pub fn mask_code(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let mut in_fence: Option<usize> = None;

    while i < bytes.len() {
        let at_line_start = i == 0 || bytes[i - 1] == b'\n';

        // A fence is three or more backticks at the start of a line. The closing
        // fence must be at least as long as the opening one, so a ```` block can
        // contain ``` lines.
        if at_line_start && bytes[i] == b'`' {
            let ticks = count_ticks(bytes, i);
            if ticks >= 3 {
                match in_fence {
                    None => in_fence = Some(ticks),
                    Some(open) if ticks >= open => in_fence = None,
                    Some(_) => {}
                }
                blank(&mut out, &text[i..i + ticks]);
                i += ticks;
                continue;
            }
        }

        if in_fence.is_some() {
            take_char(&mut out, text, &mut i, Masked::Yes);
            continue;
        }

        // An inline span opens on one or more backticks and closes on a run of
        // exactly the same length. An unterminated run is literal text, not an
        // open span that swallows the rest of the file.
        if bytes[i] == b'`' {
            let ticks = count_ticks(bytes, i);
            if let Some(close) = find_close(bytes, i + ticks, ticks) {
                blank(&mut out, &text[i..close + ticks]);
                i = close + ticks;
                continue;
            }
        }

        take_char(&mut out, text, &mut i, Masked::No);
    }

    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Masked {
    Yes,
    No,
}

fn count_ticks(bytes: &[u8], from: usize) -> usize {
    bytes[from..].iter().take_while(|&&b| b == b'`').count()
}

/// The offset of a closing run of exactly `ticks` backticks, if there is one.
fn find_close(bytes: &[u8], from: usize, ticks: usize) -> Option<usize> {
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = count_ticks(bytes, i);
            if run == ticks {
                return Some(i);
            }
            i += run;
        } else {
            i += 1;
        }
    }
    None
}

/// Advances one character, either copying it or replacing it with a space.
///
/// A newline is always copied, whether masked or not: line numbers are computed
/// by counting newlines in the masked text, so losing one moves every
/// diagnostic after it.
fn take_char(out: &mut String, text: &str, i: &mut usize, masked: Masked) {
    let ch = text[*i..].chars().next().expect("in bounds");
    out.push(match masked {
        Masked::Yes if ch != '\n' => ' ',
        _ => ch,
    });
    *i += ch.len_utf8();
}

/// Replaces a span with spaces, keeping newlines so line numbers survive.
fn blank(out: &mut String, span: &str) {
    for ch in span.chars() {
        out.push(if ch == '\n' { '\n' } else { ' ' });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(body: &str) -> Vec<String> {
        extract_links(body, 1)
            .into_iter()
            .map(|l| l.as_written())
            .collect()
    }

    #[test]
    fn finds_a_plain_link() {
        assert_eq!(
            names("See [[monitors-self-match]] for why."),
            ["[[monitors-self-match]]"]
        );
    }

    #[test]
    fn finds_a_cross_source_link_and_splits_it() {
        let links = extract_links("See [[kb:monitors-self-match]].", 1);
        assert_eq!(links[0].source.as_deref(), Some("kb"));
        assert_eq!(links[0].name, "monitors-self-match");
    }

    /// The discriminating case. A linter that only strips fenced blocks passes
    /// every test written from a note about Rust and fails on the one note in
    /// the corpus that is about shell.
    #[test]
    fn double_brackets_in_an_inline_code_span_are_not_links() {
        assert!(names("Bash's `[[ -f foo ]]` test.").is_empty());
    }

    #[test]
    fn double_brackets_in_a_fenced_block_are_not_links() {
        let body =
            "Before.\n\n```sh\nif [[ -f foo ]]; then echo hi; fi\n```\n\nAfter [[real-link]].";
        assert_eq!(names(body), ["[[real-link]]"]);
    }

    #[test]
    fn line_numbers_survive_masking() {
        let body = "```sh\n[[ -f x ]]\n```\n\nSee [[a-note]].";
        let links = extract_links(body, 10);
        assert_eq!(
            links[0].line, 14,
            "body starts at 10, link is on the fifth line"
        );
    }

    #[test]
    fn a_longer_fence_may_contain_a_shorter_one() {
        let body = "````md\n```\n[[not-a-link]]\n```\n````\n\n[[a-link]]";
        assert_eq!(names(body), ["[[a-link]]"]);
    }

    /// A stray backtick in prose must not swallow the rest of the note.
    #[test]
    fn an_unterminated_backtick_does_not_hide_later_links() {
        assert_eq!(names("A stray ` tick, then [[a-link]]."), ["[[a-link]]"]);
    }

    #[test]
    fn a_double_tick_span_closes_on_a_double_tick() {
        assert!(names("Literal ``[[x]]`` span.").is_empty());
    }
}
