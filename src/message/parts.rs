use super::{
    event::{Event, Str, Style},
    stream::{Atom, Choice, Flatten, Fragment, Render, Scoped, Styled},
};

/// Plain text, escaped when rendered.
pub struct Text<S>(pub S);

impl<S: Into<Str>> Fragment for Text<S> {
    fn parts(self) -> impl Render {
        Atom(Event::Text(self.0.into()))
    }
}

/// Inline code. Only backticks and backslashes are escaped.
pub struct Code<S>(pub S);

impl<S: Into<Str>> Fragment for Code<S> {
    fn parts(self) -> impl Render {
        Atom(Event::Code(self.0.into()))
    }
}

/// Fenced code block with an optional language tag.
pub struct CodeBlock<S> {
    pub code: S,
    pub language: Option<&'static str>,
}

impl<S: Into<Str>> Fragment for CodeBlock<S> {
    fn parts(self) -> impl Render {
        Atom(Event::CodeBlock {
            code: self.code.into(),
            language: self.language,
        })
    }
}

/// Inline link. The label is escaped as text, the URL only for `)` and backticks.
pub struct Link<L, U> {
    pub label: L,
    pub url: U,
}

impl<L: Into<Str>, U: Into<Str>> Fragment for Link<L, U> {
    fn parts(self) -> impl Render {
        Atom(Event::Link {
            label: self.label.into(),
            url: self.url.into(),
        })
    }
}

/// Line break.
pub struct Newline;

impl Fragment for Newline {
    fn parts(self) -> impl Render {
        Atom(Event::Newline)
    }
}

/// Sixteen escaped dashes followed by a newline.
pub struct Separator;

impl Fragment for Separator {
    fn parts(self) -> impl Render {
        Atom(Event::Separator)
    }
}

macro_rules! styled_fragment {
    ($(#[$doc:meta])* $name:ident, $style:ident) => {
        $(#[$doc])*
        pub struct $name<F>(pub F);

        impl<F: Fragment> Fragment for $name<F> {
            fn parts(self) -> impl Render {
                Styled {
                    style: Style::$style,
                    inner: self.0.parts(),
                }
            }
        }
    };
}

styled_fragment!(
    /// Bold content.
    Bold, Bold
);
styled_fragment!(
    /// Italic content.
    Italic, Italic
);
styled_fragment!(
    /// Strikethrough content.
    Strike, Strikethrough
);
styled_fragment!(
    /// Content hidden behind a spoiler.
    Spoiler, Spoiler
);

/// Content followed by a newline.
pub struct Line<F>(pub F);

impl<F: Fragment> Fragment for Line<F> {
    fn parts(self) -> impl Render {
        (self.0, Newline).parts()
    }
}

/// Content shown only in diagnostic rendering.
pub struct DebugBlock<F>(pub F);

impl<F: Fragment> Fragment for DebugBlock<F> {
    fn parts(self) -> impl Render {
        Scoped(self.0.parts())
    }
}

/// One of two parts, for branches that build different part types.
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L: Fragment, R: Fragment> Fragment for Either<L, R> {
    fn parts(self) -> impl Render {
        match self {
            Either::Left(left) => Choice::Left(left.parts()),
            Either::Right(right) => Choice::Right(right.parts()),
        }
    }
}

/// Every item of an iterator, visited lazily while rendering.
pub struct Each<I>(pub I);

impl<I> Fragment for Each<I>
where
    I: IntoIterator,
    I::Item: Fragment,
{
    fn parts(self) -> impl Render {
        Flatten(self.0.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::{
        message::{render_both, render_public},
        test_support::{Golden, assert_golden},
    };

    fn choose(value: Option<u8>) -> impl Fragment {
        match value {
            Some(0) => Either::Left(Text("zero")),
            Some(n) => Either::Right(Either::Left((Text("n="), Text(n.to_string())))),
            None => Either::Right(Either::Right(Code("none"))),
        }
    }

    #[test]
    fn text_is_escaped() {
        assert_eq!(
            render_public(Text("a_b*c[d](e)~f`g>h#i+j-k=l|m{n}o.p!q\\r")).as_str(),
            "a\\_b\\*c\\[d\\]\\(e\\)\\~f\\`g\\>h\\#i\\+j\\-k\\=l\\|m\\{n\\}o\\.p\\!q\\\\r"
        );
    }

    #[test]
    fn newline_and_separator() {
        assert_eq!(
            render_public((Newline, Separator)).as_str(),
            "\n\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\\-\n"
        );
    }

    #[test]
    fn code_blocks_with_and_without_language() {
        let document = (
            CodeBlock {
                code: "x`",
                language: None,
            },
            CodeBlock {
                code: "x",
                language: Some("c++"),
            },
        );
        assert_eq!(
            render_public(document).as_str(),
            "```\nx\\`\n``````c\\+\\+\nx\n```"
        );
    }

    #[test]
    fn link_escapes_label_and_url() {
        let link = Link {
            label: "Docs (v2)",
            url: "https://e.com/a_(b)",
        };
        assert_eq!(
            render_public(link).as_str(),
            "[Docs \\(v2\\)](https://e.com/a_(b\\))"
        );
    }

    #[test]
    fn styles_wrap_nested_parts() {
        assert_eq!(
            render_public(Bold((Text("a"), Italic(Strike(Spoiler(Text("b"))))))).as_str(),
            "*a_~||b||~_*"
        );
    }

    #[test]
    fn line_appends_newline() {
        assert_eq!(render_public(Line(Code("x"))).as_str(), "`x`\n");
    }

    #[test]
    fn option_and_either_select_one_branch() {
        assert_eq!(
            render_public((Some(Text("a")), None::<Text<&str>>)).as_str(),
            "a"
        );
        assert_eq!(render_public(choose(Some(0))).as_str(), "zero");
        assert_eq!(render_public(choose(Some(7))).as_str(), "n\\=7");
        assert_eq!(render_public(choose(None)).as_str(), "`none`");
    }

    #[test]
    fn empty_tuple_renders_nothing() {
        assert_eq!(render_public(()).as_str(), "");
    }

    #[test]
    fn debug_blocks_are_dropped_from_public_output() {
        let document = || {
            (
                Text("visible"),
                DebugBlock((Text("secret"), DebugBlock(Text("nested")))),
                Spoiler((Text("a"), DebugBlock(Text("b")))),
            )
        };

        assert_eq!(render_public(document()).as_str(), "visible||a||");

        let rendered = render_both(document());
        assert_eq!(rendered.public.as_str(), "visible||a||");
        assert_eq!(rendered.diagnostic.as_str(), "visiblesecretnested||ab||");
    }

    #[test]
    fn each_is_visited_only_while_rendering_and_once() {
        let visits = Cell::new(0);
        let items = (0..3)
            .inspect(|_| visits.set(visits.get() + 1))
            .map(|item| Line(Text(item.to_string())));
        let document = Each(items);
        assert_eq!(visits.get(), 0);

        let rendered = render_both(document);
        assert_eq!(visits.get(), 3);
        assert_eq!(rendered.public.as_str(), "0\n1\n2\n");
        assert_eq!(rendered.diagnostic.as_str(), "0\n1\n2\n");
    }

    #[test]
    fn golden_escaping() {
        let rendered = render_both((
            Line(Text("Plain: a_b*c[d](e)~f`g>h#i+j-k=l|m{n}o.p!q\\r")),
            Line(Bold(Text("Bold: 1.5 * 2"))),
            Line(Italic(Text("Italic: snake_case"))),
            Line(Code("inline `tick` and \\ backslash")),
            Line(CodeBlock {
                code: "fn main() { println!(\"`x`\\n\"); }",
                language: None,
            }),
            Line(CodeBlock {
                code: "let x = 1;",
                language: Some("c++"),
            }),
            Line(Strike((Text("gone (really)"), Code("old-value")))),
            Line(Spoiler(Link {
                label: "Docs (v2) [beta]",
                url: "https://example.com/a_(b)?q=`x`",
            })),
            Line(Spoiler(Strike(Text("nested!")))),
            DebugBlock((Text("debug-only: 1.0"), Spoiler(Text("hidden")))),
        ));

        assert_golden(Golden::EscapingPublic, rendered.public.as_str());
        assert_golden(Golden::EscapingDiagnostic, rendered.diagnostic.as_str());
    }
}
