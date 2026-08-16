use std::collections::HashMap;
use pulldown_cmark::Event;
use tracing::debug;

/// Filter for PostProcessing included document by removing lines
/// starting with # symbol that exist between HTML tags like:
/// <pre><code class="language-rust"># line 1
/// # line 2
///</code></pre>
pub(crate) struct IncludeDocFilter<'a> {
    block_code: Vec<Vec<Event<'a>>>,
    is_enabled: bool,
}

impl<'a> IncludeDocFilter<'a> {
    pub fn new(is_enabled: bool) -> Self {
        Self {
            block_code: Vec::new(),
            is_enabled,
        }
    }
    pub(crate) fn apply(&mut self, event: Event<'a>) -> Event<'a> {
        if !self.is_enabled {
            return event;
        }
        debug!("IncludeDocFilter: Processing Event = {:?}", &event);
        // match event {}
        todo!()
    }
}