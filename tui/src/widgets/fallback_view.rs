use ratatui::widgets::*;
use ratatui::prelude::*;

use crate::config::*;

pub struct ConfigView {
    config: JfiConfig,
}

impl ConfigView {
    pub fn render(frame: &mut Frame) {
        let p = Paragraph::new("Hello");

        p.render(frame.area(), frame.buffer_mut());
    }
}
