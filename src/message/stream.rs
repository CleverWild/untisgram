use std::iter;

use super::event::{Event, Style};

mod sealed {
    pub trait Sealed {}
}

/// Event stream of a finished message part. Only this module can implement it, so every
/// stream the renderer sees has balanced formatting scopes.
pub trait Render: sealed::Sealed {
    fn events(self) -> impl Iterator<Item = Event>;
}

/// A composable message part. Tuples (up to 12), `()` and `Option` of parts are parts too.
pub trait Fragment {
    fn parts(self) -> impl Render;
}

pub(super) struct Nothing;

impl sealed::Sealed for Nothing {}

impl Render for Nothing {
    fn events(self) -> impl Iterator<Item = Event> {
        iter::empty()
    }
}

pub(super) struct Atom(pub(super) Event);

impl sealed::Sealed for Atom {}

impl Render for Atom {
    fn events(self) -> impl Iterator<Item = Event> {
        iter::once(self.0)
    }
}

pub(super) struct Seq<A, B>(pub(super) A, pub(super) B);

impl<A, B> sealed::Sealed for Seq<A, B> {}

impl<A: Render, B: Render> Render for Seq<A, B> {
    fn events(self) -> impl Iterator<Item = Event> {
        self.0.events().chain(self.1.events())
    }
}

pub(super) struct Styled<R> {
    pub(super) style: Style,
    pub(super) inner: R,
}

impl<R> sealed::Sealed for Styled<R> {}

impl<R: Render> Render for Styled<R> {
    fn events(self) -> impl Iterator<Item = Event> {
        iter::once(Event::Open(self.style))
            .chain(self.inner.events())
            .chain(iter::once(Event::Close(self.style)))
    }
}

/// Stream that the public renderer skips.
pub(super) struct Scoped<R>(pub(super) R);

impl<R> sealed::Sealed for Scoped<R> {}

impl<R: Render> Render for Scoped<R> {
    fn events(self) -> impl Iterator<Item = Event> {
        iter::once(Event::EnterDiagnostic)
            .chain(self.0.events())
            .chain(iter::once(Event::ExitDiagnostic))
    }
}

/// One of two streams, picked when the part was built.
pub(super) enum Choice<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> sealed::Sealed for Choice<L, R> {}

impl<L: Render, R: Render> Render for Choice<L, R> {
    fn events(self) -> impl Iterator<Item = Event> {
        match self {
            Choice::Left(left) => Choice::Left(left.events()),
            Choice::Right(right) => Choice::Right(right.events()),
        }
    }
}

impl<L, R> Iterator for Choice<L, R>
where
    L: Iterator<Item = Event>,
    R: Iterator<Item = Event>,
{
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        match self {
            Choice::Left(left) => left.next(),
            Choice::Right(right) => right.next(),
        }
    }
}

/// Streams of iterator items; an item is pulled only when the renderer reaches it.
pub(super) struct Flatten<I>(pub(super) I);

impl<I> sealed::Sealed for Flatten<I> {}

impl<I> Render for Flatten<I>
where
    I: Iterator,
    I::Item: Fragment,
{
    fn events(self) -> impl Iterator<Item = Event> {
        self.0.flat_map(|part| part.parts().events())
    }
}

impl<F: Fragment> Fragment for Option<F> {
    fn parts(self) -> impl Render {
        match self {
            Some(part) => Choice::Left(part.parts()),
            None => Choice::Right(Nothing),
        }
    }
}

impl Fragment for () {
    fn parts(self) -> impl Render {
        Nothing
    }
}

macro_rules! tuple_fragment {
    ($($part:ident)+) => {
        impl<$($part: Fragment),+> Fragment for ($($part,)+) {
            #[allow(non_snake_case)]
            fn parts(self) -> impl Render {
                let ($($part,)+) = self;
                tuple_fragment!(@seq $($part)+)
            }
        }
    };
    (@seq $head:ident $($tail:ident)+) => {
        Seq($head.parts(), tuple_fragment!(@seq $($tail)+))
    };
    (@seq $head:ident) => {
        $head.parts()
    };
}

tuple_fragment!(A);
tuple_fragment!(A B);
tuple_fragment!(A B C);
tuple_fragment!(A B C D);
tuple_fragment!(A B C D E);
tuple_fragment!(A B C D E F);
tuple_fragment!(A B C D E F G);
tuple_fragment!(A B C D E F G H);
tuple_fragment!(A B C D E F G H I);
tuple_fragment!(A B C D E F G H I J);
tuple_fragment!(A B C D E F G H I J K);
tuple_fragment!(A B C D E F G H I J K L);
