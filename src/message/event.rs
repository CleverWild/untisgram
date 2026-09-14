use std::borrow::Cow;

/// Render instruction produced by parts and consumed by the MarkdownV2 sinks. `pub` because
/// `Render` returns it; the private `event` module keeps it out of reach.
pub enum Event {
    Text(Str),
    Newline,
    Separator,
    Code(Str),
    CodeBlock {
        code: Str,
        language: Option<&'static str>,
    },
    Link {
        label: Str,
        url: Str,
    },
    Open(Style),
    Close(Style),
    EnterDiagnostic,
    ExitDiagnostic,
}

/// Paired delimiter; every `Open` is followed by a `Close` of the same style.
#[derive(Clone, Copy)]
pub enum Style {
    Bold,
    Italic,
    Strikethrough,
    Spoiler,
}

/// Unescaped text. Only the renderer escapes it.
pub struct Str(Cow<'static, str>);

impl Str {
    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&'static str> for Str {
    fn from(value: &'static str) -> Self {
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for Str {
    fn from(value: String) -> Self {
        Self(Cow::Owned(value))
    }
}

impl From<&String> for Str {
    fn from(value: &String) -> Self {
        Self(Cow::Owned(value.clone()))
    }
}
