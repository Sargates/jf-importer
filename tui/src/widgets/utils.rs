use std::time::{Duration, Instant};

use ratatui::text::Span;
use ratatui::style::{Color,Stylize};
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;

pub const BRAILLE: [char; 8] = [
    '⢿',
    '⣻',
    '⣽',
    '⣾',
    '⣷',
    '⣯',
    '⣟',
    '⡿',
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
impl Into<String> for &BrailleLoadingIcon {
    fn into(self) -> String {
        BRAILLE[self.index].to_string()
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
