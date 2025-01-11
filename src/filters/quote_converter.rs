use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

/// From `mdbook/src/utils/mod.rs`, where this is a private struct.
pub struct QuoteConverterFilter {
    enabled: bool,
    convert_text: bool,
}

impl QuoteConverterFilter {
    pub(crate) fn new(enabled: bool) -> Self {
        QuoteConverterFilter {
            enabled,
            convert_text: true,
        }
    }

    pub(crate) fn apply<'a>(&mut self, event: Event<'a>) -> Event<'a> {
        if !self.enabled {
            return event;
        }

        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                self.convert_text = false;
                event
            }
            Event::End(TagEnd::CodeBlock) => {
                self.convert_text = true;
                event
            }
            Event::Text(ref text) if self.convert_text => {
                Event::Text(CowStr::from(Self::convert_quotes_to_curly(text)))
            }
            _ => event,
        }
    }

    fn convert_quotes_to_curly(original_text: &str) -> String {
        // We'll consider the start to be "whitespace".
        let mut preceded_by_whitespace = true;

        original_text
            .chars()
            .map(|original_char| {
                let converted_char = match original_char {
                    '\'' => {
                        if preceded_by_whitespace {
                            '‘'
                        } else {
                            '’'
                        }
                    }
                    '"' => {
                        if preceded_by_whitespace {
                            '“'
                        } else {
                            '”'
                        }
                    }
                    _ => original_char,
                };

                preceded_by_whitespace = original_char.is_whitespace();

                converted_char
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{CodeBlockKind, CowStr};

    #[test]
    fn test_basic_quote_conversion() {
        let mut filter = QuoteConverterFilter::new(true);

        // Test single quotes
        let input = Event::Text(CowStr::from("Here's a 'quote'"));
        if let Event::Text(result) = filter.apply(input) {
            println!("{}", result.as_ref());
            assert_eq!(result.as_ref(), "Here’s a ‘quote’");
        } else {
            panic!("Expected Text event");
        }

        // Test double quotes
        let input = Event::Text(CowStr::from(r#"He said "hello""#));
        if let Event::Text(result) = filter.apply(input) {
            assert_eq!(result.as_ref(), r#"He said “hello”"#);
        } else {
            panic!("Expected Text event");
        }
    }

    #[test]
    fn test_disabled_filter() {
        let mut filter = QuoteConverterFilter::new(false);
        let input = Event::Text(CowStr::from(r#"'test' and "test""#));
        if let Event::Text(result) = filter.apply(input) {
            assert_eq!(result.as_ref(), r#"'test' and "test""#);
        } else {
            panic!("Expected Text event");
        }
    }

    #[test]
    fn test_code_block_handling() {
        let mut filter = QuoteConverterFilter::new(true);

        // Start code block
        filter.apply(Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(
            CowStr::from("rust"),
        ))));

        // Text within code block shouldn't be converted
        let code_text = Event::Text(CowStr::from(r#"let s = "string";"#));
        if let Event::Text(result) = filter.apply(code_text) {
            assert_eq!(result.as_ref(), r#"let s = "string";"#);
        } else {
            panic!("Expected Text event");
        }

        // End code block
        filter.apply(Event::End(TagEnd::CodeBlock));

        // Text after code block should be converted again
        let after_code = Event::Text(CowStr::from(r#""test""#));
        if let Event::Text(result) = filter.apply(after_code) {
            assert_eq!(result.as_ref(), r#"“test”"#);
        } else {
            panic!("Expected Text event");
        }
    }

    #[test]
    fn test_convert_quotes_to_curly() {
        // Test various quote patterns
        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly("'start' mid 'end'"),
            "‘start’ mid ‘end’"
        );

        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly(r#""Hello" he's "saying""#),
            r#"“Hello” he’s “saying”"#
        );
    }

    #[test]
    fn test_whitespace_handling() {
        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly("word'word'word"),
            "word’word’word"
        );

        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly("word 'word' word"),
            "word ‘word’ word"
        );

        // Test with various whitespace characters
        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly("\t'tab'\n'newline'\r'return'"),
            "\t‘tab’\n‘newline’\r‘return’"
        );
    }

    #[test]
    fn test_mixed_quotes() {
        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly(r#"'single' and "double" quotes"#),
            r#"‘single’ and “double” quotes"#
        );
    }

    #[test]
    fn test_empty_and_whitespace() {
        assert_eq!(QuoteConverterFilter::convert_quotes_to_curly(""), "");
        assert_eq!(QuoteConverterFilter::convert_quotes_to_curly(" "), " ");
        assert_eq!(QuoteConverterFilter::convert_quotes_to_curly("''"), "‘’");
        assert_eq!(
            QuoteConverterFilter::convert_quotes_to_curly(r#""""#),
            r#"“”"#
        );
    }
}
