use pulldown_cmark::{CowStr, Event, Tag, TagEnd};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Reusable footnote filter.
pub(crate) struct FootnoteFilter<'a> {
    footnotes: Vec<Vec<Event<'a>>>,
    in_footnote: Vec<Vec<Event<'a>>>,
    footnote_numbers: HashMap<CowStr<'a>, (usize, usize)>,
    is_enabled: bool,
}
impl<'a> FootnoteFilter<'a> {
    pub fn new(is_enabled: bool) -> Self {
        Self {
            footnotes: Vec::new(),
            in_footnote: Vec::new(),
            footnote_numbers: HashMap::new(),
            is_enabled,
        }
    }
    pub fn apply(&mut self, event: Event<'a>) -> Option<Event<'a>> {
        if !self.is_enabled {
            return Some(event);
        }
        match event {
            Event::Start(Tag::FootnoteDefinition(_)) => {
                self.in_footnote.push(vec![event]);
                None
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                self.move_from_in_to_footnotes(event);
                None
            }
            Event::FootnoteReference(name) => {
                let n = self.footnote_numbers.len() + 1;
                let (n, nr) = self
                    .footnote_numbers
                    .entry(name.clone())
                    .or_insert((n, 0usize));
                *nr += 1;
                let html = Event::Html(format!(r##"<sup class="footnote-reference" id="fr-{name}-{nr}"><a href="#fn-{name}">[{n}]</a></sup>"##).into());
                if self.in_footnote.is_empty() {
                    Some(html)
                } else {
                    self.replace_last_in_footnote(html);
                    None
                }
            }
            _ if !self.in_footnote.is_empty() => {
                self.replace_last_in_footnote(event);
                None
            }
            _ => Some(event),
        }
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.footnotes.is_empty()
    }
    fn move_from_in_to_footnotes(&mut self, event: Event<'a>) {
        let popped_vector = self.in_footnote.pop();
        match popped_vector {
            None => {}
            Some(mut vector) => {
                vector.push(event);
                self.footnotes.push(vector);
            }
        }
    }
    fn replace_last_in_footnote(&mut self, event: Event<'a>) {
        if let Some(value) = self.in_footnote.last_mut() {
            value.push(event)
        }
    }
    pub(crate) fn retain(&mut self) -> bool {
        let original_len = self.footnotes.len(); // number before retain
        self.footnotes.retain(|f| match f.first() {
            Some(Event::Start(Tag::FootnoteDefinition(name))) => {
                self.footnote_numbers.get(name).unwrap_or(&(0, 0)).1 != 0
            }
            _ => false,
        });
        self.footnotes.len() != original_len // true, if number has changed
    }
    pub(crate) fn sort_by_cached_key(&mut self) {
        self.footnotes.sort_by_cached_key(|f| match f.first() {
            Some(Event::Start(Tag::FootnoteDefinition(name))) => {
                self.footnote_numbers.get(name).unwrap_or(&(0, 0)).0
            }
            _ => unreachable!(),
        });
    }
    pub(crate) fn get_events(&self) -> impl Iterator<Item = Event<'_>> {
        self.footnotes
            .clone()
            .into_iter()
            .flat_map(move |external_event| {
                // To write backrefs, the name needs kept until the end of the footnote definition.
                let mut name = CowStr::from("");
                // Backrefs are included in the final paragraph of the footnote, if it's normal text.
                // For example, this DOM can be produced:
                //
                // Markdown:
                //
                //     five [^feet].
                //
                //     [^feet]:
                //         A foot is defined, in this case, as 0.3048 m.
                //
                //         Historically, the foot has not been defined this way, corresponding to many
                //         subtly different units depending on the location.
                //
                // HTML:
                //
                //     <p>five <sup class="footnote-reference" id="fr-feet-1"><a href="#fn-feet">[1]</a></sup>.</p>
                //
                //     <ol class="footnotes-list">
                //     <li id="fn-feet">
                //     <p>A foot is defined, in this case, as 0.3048 m.</p>
                //     <p>Historically, the foot has not been defined this way, corresponding to many
                //     subtly different units depending on the location. <a href="#fr-feet-1">↩</a></p>
                //     </li>
                //     </ol>
                //
                // This is mostly a visual hack, so that footnotes use less vertical space.
                //
                // If there is no final paragraph, such as a tabular, list, or image footnote, it gets
                // pushed after the last tag instead.
                let mut has_written_backrefs = false;
                let fl_len = external_event.len();
                let mut _written_footnote_numbers: Vec<usize> = Vec::new();
                // let footnote_numbers_ref = &self.footnote_numbers;
                external_event
                    .into_iter()
                    .enumerate()
                    .map(move |(i, internal_event)| match internal_event {
                        Event::Start(Tag::Paragraph) => {
                            let fn_number = self.footnote_numbers.get(&name).unwrap().0;
                            if _written_footnote_numbers.contains(&fn_number) {
                                Event::Html("<p>".into())
                            } else {
                                // At this point we have started rendering a Tag::FootnoteDefinition, so already wrote an
                                // opening <div> tag, and starting to write the paragraphs of the definition.
                                //
                                // If we haven't written this footnote reference number yet, then write it at the beginning of
                                // the paragraph in a <span>.
                                //
                                // This will include the footnote number in the <div>, but NOT as a block element, and
                                // hence it correcly shows up in footnote pop-ups.
                                //
                                // Use a <div> instead of an <aside> tag, because iBooks doesn't display <aside>.
                                //
                                // Tested on: ReadEra and Moon+ Reader on Android, Kindle Paperwhite, iBooks, KOReader on ReMarkable 2.
                                _written_footnote_numbers.push(fn_number);
                                Event::Html(format!(r##"<p><span class="footnote-definition-label">[{fn_number}]</span> "##).into())
                            }
                        }
                        Event::Start(Tag::FootnoteDefinition(current_name)) => {
                            name = current_name;
                            has_written_backrefs = false;
                            Event::Html(format!(r##"<div class="footnote-definition" id="fn-{name}" epub:type="footnote">"##).into())
                        }
                        Event::End(TagEnd::FootnoteDefinition) | Event::End(TagEnd::Paragraph)
                            if !has_written_backrefs && i >= fl_len - 2 =>
                        {
                            let usage_count = self.footnote_numbers.get(&name).unwrap().1;
                            let mut end = String::with_capacity(
                                name.len() + (r##" <a href="#fr--1">↩</a></div>"##.len() * usage_count),
                            );
                            for usage in 1..=usage_count {
                                if usage == 1 {
                                    write!(&mut end, r##" <a href="#fr-{name}-{usage}">↩</a>"##)
                                        .unwrap();
                                } else {
                                    write!(
                                        &mut end,
                                        r##" <a href="#fr-{name}-{usage}">↩{usage}</a>"##
                                    )
                                    .unwrap();
                                }
                            }
                            has_written_backrefs = true;
                            if internal_event == Event::End(TagEnd::FootnoteDefinition) {
                                end.push_str("</div>\n");
                            } else {
                                end.push_str("</p>\n");
                            }
                            Event::Html(end.into())
                        }
                        Event::End(TagEnd::FootnoteDefinition) => Event::Html("</div>\n".into()),
                        Event::FootnoteReference(_) => unreachable!("converted to HTML earlier"),
                        f => f,
                    })
                    .collect::<Vec<_>>()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    fn parse_markdown_with_footnotes(input: &str) -> Vec<Event<'_>> {
        let mut footnote_filter = FootnoteFilter::new(true);
        Parser::new_ext(input, Options::ENABLE_FOOTNOTES)
            .filter_map(|event| footnote_filter.apply(event))
            .collect()
    }

    #[test]
    fn test_simple_footnote() {
        let input = "This is a[^1] footnote.\n\n[^1]: Simple footnote content";
        let events = parse_markdown_with_footnotes(input);

        // Check that the main text contains an HTML link to the footnote
        let footnote_ref = events.iter().find(
            |e| matches!(e, Event::Html(html) if html.contains("class=\"footnote-reference\"")),
        );
        assert!(footnote_ref.is_some());

        if let Some(Event::Html(html)) = footnote_ref {
            assert!(html.contains("href=\"#fn-1\""));
            assert!(html.contains("[1]"));
        }
    }

    #[test]
    fn test_multiple_references_to_same_footnote() {
        let input = "First ref[^a] and second ref[^a].\n\n[^a]: Footnote content";
        let mut filter = FootnoteFilter::new(true);
        let mut events = Vec::new();

        // We collect all events
        for event in Parser::new_ext(input, Options::ENABLE_FOOTNOTES) {
            if let Some(e) = filter.apply(event) {
                events.push(e);
            }
        }

        // We check that both references exist and have different ids
        let refs: Vec<_> = events
            .iter()
            .filter(|e| matches!(e, Event::Html(html) if html.contains("footnote-reference")))
            .collect();

        assert_eq!(refs.len(), 2);

        if let (Event::Html(first), Event::Html(second)) = (&refs[0], &refs[1]) {
            assert!(first.contains("id=\"fr-a-1\""));
            assert!(second.contains("id=\"fr-a-2\""));
        }
    }

    #[test]
    fn test_footnote_definition() {
        let mut filter = FootnoteFilter::new(true);

        // Simulating the definition of a footnote
        filter.apply(Event::Start(Tag::FootnoteDefinition("test".into())));
        filter.apply(Event::Text("Footnote content".into()));
        filter.apply(Event::End(TagEnd::FootnoteDefinition));

        assert!(!filter.is_empty());

        // Check that the definition is saved
        let footnotes = filter.footnotes.clone();
        assert_eq!(footnotes.len(), 1);

        let definition = &footnotes[0];
        assert!(matches!(
            definition.first(),
            Some(Event::Start(Tag::FootnoteDefinition(_)))
        ));
    }

    #[test]
    fn test_unused_footnote_removal() {
        let mut filter = FootnoteFilter::new(true);

        // Adding a footnote definition without referencing it
        filter.apply(Event::Start(Tag::FootnoteDefinition("unused".into())));
        filter.apply(Event::Text("Unused content".into()));
        filter.apply(Event::End(TagEnd::FootnoteDefinition));

        // Check that retain removes unused footnotes
        filter.retain();
        assert!(filter.footnotes.is_empty());
    }

    #[test]
    fn test_footnote_sorting() {
        let input = r#"First[^1] then[^2].

[^2]: Second footnote
[^1]: First footnote"#;
        let mut filter = FootnoteFilter::new(true);

        // Processing input data
        for event in Parser::new_ext(input, Options::ENABLE_FOOTNOTES) {
            filter.apply(event);
        }

        // Let's make sure we have footnote
        assert!(!filter.is_empty());

        // Sorting footnotes
        filter.sort_by_cached_key();

        // Get sorted events
        let sorted_footnotes = filter.get_events().collect::<Vec<_>>();

        // Output events for debugging
        debug!("Sorted events:");
        for (i, event) in sorted_footnotes.iter().enumerate() {
            debug!("{}: {:?}", i, event);
        }

        // We check the order by content
        let first_content_pos = sorted_footnotes
            .iter()
            .position(|e| matches!(e, Event::Text(text) if text.contains("First footnote")))
            .expect("First footnote not found");
        let second_content_pos = sorted_footnotes
            .iter()
            .position(|e| matches!(e, Event::Text(text) if text.contains("Second footnote")))
            .expect("Second footnote not found");

        assert!(
            first_content_pos < second_content_pos,
            "First footnote (pos={}) should appear before second footnote (pos={})",
            first_content_pos,
            second_content_pos
        );
    }
}
