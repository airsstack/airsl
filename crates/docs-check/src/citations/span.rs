//! The code spans of a Markdown document.
//!
//! Its own module because the source-path and module-path guards read the same spans and must
//! agree on what a span is: backtick-delimited prose, outside any fenced block.
//!
//! Responsibilities: `code_spans`, each span with its 1-based line.
//!
//! Non-responsibilities: deciding what a span means. A span is a path, a module path, or neither,
//! and that is the caller's grammar.
#![expect(
    clippy::redundant_pub_crate,
    reason = "the sibling guard modules read the spans; pub(super) would name the parent, not the guards"
)]

use crate::workspace::normalize;

/// A backtick-delimited span, trimmed, and the 1-based line it sits on.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Span {
    /// 1-based line of the span's opening backtick.
    pub(crate) line: usize,
    /// The text between the backticks, trimmed.
    pub(crate) text: String,
}

/// Every code span of `doc`. Fenced blocks are skipped: they hold program output and shell
/// sessions, which may mention a `*.rs` glob or a path the program printed without citing it.
/// A line with an odd number of backticks leaves a span open, and none of its text is read.
pub(crate) fn code_spans(doc: &str) -> Vec<Span> {
    let mut found = Vec::new();
    let mut in_fence = false;
    for (index, line) in normalize(doc).lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let parts: Vec<&str> = line.split('`').collect();
        if parts.len().is_multiple_of(2) {
            continue;
        }
        for text in parts.into_iter().skip(1).step_by(2) {
            found.push(Span {
                line: index + 1,
                text: text.trim().to_owned(),
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(doc: &str) -> Vec<(usize, String)> {
        code_spans(doc)
            .into_iter()
            .map(|span| (span.line, span.text))
            .collect()
    }

    #[test]
    fn spans_are_read_from_prose_and_not_from_fenced_blocks() {
        let doc = "See `a` and `b c` here.\n\n```\n`in_fence`\n```\n\nThen ` d `.\n";
        assert_eq!(
            texts(doc),
            vec![
                (1, "a".to_owned()),
                (1, "b c".to_owned()),
                (7, "d".to_owned())
            ]
        );
    }

    #[test]
    fn a_line_with_an_unclosed_span_yields_nothing() {
        assert!(texts("an unclosed `span\n").is_empty());
    }

    #[test]
    fn crlf_documents_read_the_same_as_lf_ones() {
        let lf = "x `a`\n```\n`b`\n```\n`c`\n";
        assert_eq!(texts(&lf.replace('\n', "\r\n")), texts(lf));
    }
}
