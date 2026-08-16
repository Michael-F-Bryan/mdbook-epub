use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use tracing::debug;

/// Filter for post-processing included documents by removing hidden rustdoc
/// lines (starting with `#` followed by whitespace) that exist inside fenced
/// code blocks with a language tag, like:
///
/// ```text
/// <pre><code class="language-rust"># line 1
/// # line 2
///</code></pre>
/// ```
///
/// Only fenced code blocks that will be rendered as
/// `<pre><code class="language-...">` are processed; indented code blocks and
/// fenced blocks without a language tag are left untouched.
pub(crate) struct IncludeDocFilter {
    /// Whether the currently opened code block is a fenced block with a
    /// non-empty language tag.
    in_language_code_block: bool,
    is_enabled: bool,
}

impl IncludeDocFilter {
    pub fn new(is_enabled: bool) -> Self {
        Self {
            in_language_code_block: false,
            is_enabled,
        }
    }

    pub(crate) fn apply<'a>(&mut self, event: Event<'a>) -> Event<'a> {
        if !self.is_enabled {
            return event;
        }
        debug!("IncludeDocFilter: Processing Event = {:?}", &event);
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                self.in_language_code_block = match &kind {
                    CodeBlockKind::Fenced(info) => !info.is_empty(),
                    CodeBlockKind::Indented => false,
                };
                Event::Start(Tag::CodeBlock(kind))
            }
            Event::Text(text) if self.in_language_code_block => {
                let filtered = remove_hidden_lines(text.as_ref());
                if filtered == text.as_ref() {
                    Event::Text(text)
                } else {
                    Event::Text(CowStr::from(filtered))
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                self.in_language_code_block = false;
                event
            }
            _ => event,
        }
    }
}

/// Returns `true` when the line is a hidden rustdoc line, i.e. it starts
/// with `#` followed by whitespace or is a bare `#`. Attribute lines like
/// `#[cfg(test)]` are real code and should not be hidden.
fn is_hidden_line(line: &str) -> bool {
    let mut chars = line.chars();
    match chars.next() {
        Some('#') => chars.next().is_none_or(char::is_whitespace),
        _ => false,
    }
}

/// Removes hidden lines from a code block content, keeping the rest intact.
/// Joining with `\n` keeps the trailing empty element produced by `split()`
/// when the block ends with a newline, so the original trailing newline is
/// preserved.
fn remove_hidden_lines(text: &str) -> String {
    text.split('\n')
        .filter(|line| !is_hidden_line(line))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Options, Parser};

    fn new_parser(text: &str) -> Parser<'_> {
        let mut opts = Options::empty();
        opts.insert(Options::ENABLE_TABLES);
        opts.insert(Options::ENABLE_FOOTNOTES);
        opts.insert(Options::ENABLE_STRIKETHROUGH);
        opts.insert(Options::ENABLE_TASKLISTS);
        Parser::new_ext(text, opts)
    }

    fn text_events(md: &str, is_enabled: bool) -> Vec<String> {
        let mut filter = IncludeDocFilter::new(is_enabled);
        new_parser(md)
            .map(|event| filter.apply(event))
            .filter_map(|event| match event {
                Event::Text(text) => Some(text.to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn test_is_hidden_line() {
        assert!(is_hidden_line("#"));
        assert!(is_hidden_line("# "));
        assert!(is_hidden_line("#\t"));
        assert!(is_hidden_line("# fn main() {}"));
        assert!(is_hidden_line("#     part: &'a str,"));
        assert!(!is_hidden_line("fn main() {}"));
        assert!(!is_hidden_line("#[cfg(test)]"));
        assert!(!is_hidden_line("#![allow(unused)]"));
        assert!(!is_hidden_line("  # not at column zero"));
        assert!(!is_hidden_line(""));
    }

    #[test]
    fn test_remove_hidden_lines() {
        assert_eq!(remove_hidden_lines(""), "");
        assert_eq!(remove_hidden_lines("a\nb\n"), "a\nb\n");
        assert_eq!(remove_hidden_lines("# a\nb\n"), "b\n");
        assert_eq!(remove_hidden_lines("a\n# b\n"), "a\n");
        assert_eq!(remove_hidden_lines("# a\n# b\n"), "");
        assert_eq!(remove_hidden_lines("# a\n# b"), "");
        assert_eq!(remove_hidden_lines("impl Foo {}\n# \n# fn main() {}\n"), "impl Foo {}\n");
        assert_eq!(
            remove_hidden_lines("# impl A {\n# }\n\nimpl B {}\n"),
            "\nimpl B {}\n"
        );
        assert_eq!(
            remove_hidden_lines("#[cfg(test)]\nmod tests {}\n"),
            "#[cfg(test)]\nmod tests {}\n"
        );
    }

    #[test]
    fn test_apply_removes_hidden_lines_in_language_block() {
        let md = concat!(
            "```rust\n",
            "# struct Foo {\n",
            "#     x: i32,\n",
            "# }\n",
            "\n",
            "impl Foo {\n",
            "    fn f() {}\n",
            "}\n",
            "# \n",
            "# fn main() {}\n",
            "```",
        );
        assert_eq!(
            text_events(md, true),
            vec!["\nimpl Foo {\n    fn f() {}\n}\n".to_string()]
        );
    }

    #[test]
    fn test_apply_keeps_attribute_lines() {
        let md = concat!(
            "```rust\n",
            "pub fn add(left: u64, right: u64) -> u64 {\n",
            "    left + right\n",
            "}\n",
            "\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    #[test]\n",
            "    fn it_works() {}\n",
            "}\n",
            "```",
        );
        assert_eq!(
            text_events(md, true),
            vec![
                "pub fn add(left: u64, right: u64) -> u64 {\n    left + right\n}\n\n\
                 #[cfg(test)]\nmod tests {\n    #[test]\n    fn it_works() {}\n}\n"
                    .to_string()
            ]
        );
    }

    #[test]
    fn test_apply_ignores_blocks_without_language() {
        let md = "```\n# not a hidden line\n```\n\n    # indented too\n";
        assert_eq!(
            text_events(md, true),
            vec!["# not a hidden line\n".to_string(), "# indented too\n".to_string()]
        );
    }

    #[test]
    fn test_apply_does_not_touch_regular_text() {
        let md = "# Heading\n\nSome text with # not at line start\n";
        assert_eq!(
            text_events(md, true),
            vec!["Heading".to_string(), "Some text with # not at line start".to_string()]
        );
    }

    #[test]
    fn test_apply_when_disabled() {
        let md = "```rust\n# fn main() {}\n```";
        assert_eq!(text_events(md, false), vec!["# fn main() {}\n".to_string()]);
    }
}
