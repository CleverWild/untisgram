use std::{fmt, marker::PhantomData};

use teloxide::utils::markdown;

use super::{
    event::{Event, Style},
    stream::{Fragment, Render},
};

const SEPARATOR: &str = "----------------";

pub const MAX_TELEGRAM_TEXT_CHARS: usize = 4_096;

/// Audience marker: output without debug blocks, safe for any chat.
#[derive(Debug, Clone, Copy)]
pub enum Public {}

/// Audience marker: output with debug blocks, for the debug chat only.
#[derive(Debug, Clone, Copy)]
pub enum Diagnostic {}

/// MarkdownV2 text for audience `A`. Only the renderer creates it, so the audience can be trusted.
pub struct Rendered<A> {
    text: String,
    audience: PhantomData<A>,
}

impl<A> Rendered<A> {
    fn new(text: String) -> Self {
        Self {
            text,
            audience: PhantomData,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn into_string(self) -> String {
        self.text
    }

    /// Counts the raw MarkdownV2, which overestimates the length Telegram checks after parsing
    /// entities, so a value within the limit is always accepted.
    pub fn char_count(&self) -> usize {
        self.text.chars().count()
    }
}

impl<A> fmt::Display for Rendered<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl<A> fmt::Debug for Rendered<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.text, f)
    }
}

/// Both renderings of one document.
#[derive(Debug)]
pub struct RenderedPair {
    pub public: Rendered<Public>,
    pub diagnostic: Rendered<Diagnostic>,
}

impl RenderedPair {
    pub fn max_char_count(&self) -> usize {
        self.public.char_count().max(self.diagnostic.char_count())
    }
}

/// Renders `document` without its `DebugBlock` parts.
pub fn render_public(document: impl Fragment) -> Rendered<Public> {
    let mut public = PublicSink::default();
    for event in document.parts().events() {
        public.write(&event);
    }
    Rendered::new(public.markdown.text)
}

/// Renders the public and diagnostic variants in one pass over `document`.
pub fn render_both(document: impl Fragment) -> RenderedPair {
    let mut public = PublicSink::default();
    let mut diagnostic = MarkdownV2::default();
    for event in document.parts().events() {
        public.write(&event);
        diagnostic.write(&event);
    }
    RenderedPair {
        public: Rendered::new(public.markdown.text),
        diagnostic: Rendered::new(diagnostic.text),
    }
}

#[derive(Default)]
struct PublicSink {
    diagnostic_depth: usize,
    markdown: MarkdownV2,
}

impl PublicSink {
    fn write(&mut self, event: &Event) {
        match event {
            Event::EnterDiagnostic => self.diagnostic_depth += 1,
            Event::ExitDiagnostic => self.diagnostic_depth -= 1,
            _ if self.diagnostic_depth == 0 => self.markdown.write(event),
            _ => {}
        }
    }
}

#[derive(Default)]
struct MarkdownV2 {
    text: String,
}

impl MarkdownV2 {
    fn write(&mut self, event: &Event) {
        let out = &mut self.text;
        match event {
            Event::Text(text) => out.push_str(&markdown::escape(text.as_str())),
            Event::Newline => out.push('\n'),
            Event::Separator => {
                out.push_str(&markdown::escape(SEPARATOR));
                out.push('\n');
            }
            Event::Code(text) => out.push_str(&markdown::code_inline(text.as_str())),
            Event::CodeBlock {
                code,
                language: None,
            } => out.push_str(&markdown::code_block(code.as_str())),
            Event::CodeBlock {
                code,
                language: Some(language),
            } => out.push_str(&markdown::code_block_with_lang(code.as_str(), language)),
            Event::Link { label, url } => out.push_str(&markdown::link(
                url.as_str(),
                &markdown::escape(label.as_str()),
            )),
            Event::Open(style) | Event::Close(style) => out.push_str(delimiter(*style)),
            Event::EnterDiagnostic | Event::ExitDiagnostic => {}
        }
    }
}

fn delimiter(style: Style) -> &'static str {
    match style {
        Style::Bold => "*",
        Style::Italic => "_",
        Style::Strikethrough => "~",
        Style::Spoiler => "||",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::Text;

    #[test]
    fn rendered_debug_matches_string_debug() {
        assert_eq!(
            format!("{:?}", render_public(Text("a"))),
            format!("{:?}", "a")
        );
    }

    #[test]
    fn rendered_length_counts_unicode_characters() {
        let rendered = render_public(Text("🔄ä"));
        assert_eq!(rendered.char_count(), 2);
    }
}
