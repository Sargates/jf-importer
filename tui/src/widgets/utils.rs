use std::time::{Duration, Instant};
use std::rc::Rc;
use std::cell::RefCell;

use ratatui::text::Span;
use ratatui::style::{Color,Stylize};
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

use tokio::sync::watch;

use once_cell::sync::Lazy;
use std::sync::Arc;
use tokio::sync::Mutex;

pub static BRAILLE: Lazy<Arc<Mutex<BrailleLoadingIcon>>> = Lazy::new(|| {
    Default::default()
});

const braille_states: [char; 8] = [
    '⠇', '⠋', '⠙', '⠸', '⢰', '⣠', '⣄', '⡆'
    // '⢿', '⣻', '⣽', '⣾', '⣷', '⣯', '⣟', '⡿',
];

pub struct BrailleLoadingIcon {
    index: usize,
    last: Instant,
}
impl BrailleLoadingIcon {
    pub fn default() -> Self {
        Self { index: 0, last: Instant::now() }
    }
    pub fn tick_next(&mut self) {
        if (Instant::now()-self.last) < Duration::from_millis(100)
            { return }
        self.index = (self.index+1)%8;
        self.last = Instant::now();
    }
}
impl Default for BrailleLoadingIcon {
    fn default() -> Self {
        Self::default()
    }
}
impl<'a> Into<Span<'a>> for &BrailleLoadingIcon {
    fn into(self) -> Span<'a> {
        braille_states[self.index].to_string().into()
    }
}
impl Into<String> for &BrailleLoadingIcon {
    fn into(self) -> String {
        braille_states[self.index].to_string()
    }
}

#[derive(Default)]
pub struct Blinker {
    on: bool
}
impl<'a> Into<Span<'a>> for &'a Blinker {
    fn into(self) -> Span<'a> {
        match self.on {
            true  => " ".fg(Color::White).bg(Color::White),
            false => "".into(), // render nothing
        }
    }
}
