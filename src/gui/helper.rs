use ratatui::Frame;
use tui_input::Input;

use super::tui_app::TuiApp;

impl<'a> TuiApp<'a> {
    pub(crate) fn set_cursor(&self, zero_x: u16, zero_y: u16, width: usize, input: &Input, frame: &mut Frame) {
        frame.set_cursor_position((
            zero_x
            + 1
            + (input.visual_cursor().min(width - 2)) as u16,
            zero_y + 1,
        ));
    }
}