extern crate alloc;
use alloc::borrow::Cow;
use ratatui::text::Span;
use ratatui::style::{Style, Styled};

pub trait StyledExt: Styled {
    /// Sets the style of the object if the predicate is `true`
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    fn set_style_if<S: Into<Style>>(self, predicate: bool, style: S) -> Self::Item;
}

impl<T: Styled + Into<T::Item>> StyledExt for T {
    fn set_style_if<S: Into<Style>>(self, predicate: bool, style: S) -> Self::Item {
        if predicate {
            self.set_style(style)
        } else {
            <T as Into<T::Item>>::into(self)
        }
    }
}
