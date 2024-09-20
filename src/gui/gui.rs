use std::rc::Rc;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, BorderType, Borders, Padding},
    Frame,
};

use crate::gui::tui_app::TuiApp;

use super::tui_app::CurrentState;

// impl<'a> TuiApp<'a> {
impl<'a> TuiApp<'a> {
    pub fn get_basic_rects(&self, frame: &Frame) -> Rc<[Rect]> {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                match self.current_state {
                    CurrentState::InputMasterPassword => {
                        [Constraint::Percentage(0), Constraint::Percentage(100)]
                    }
                    CurrentState::View => [Constraint::Length(1), Constraint::Percentage(100)],
                }
                .as_ref(),
            )
            .split(frame.area().clone());

        layout
    }

    // took from https://ratatui.rs/tutorials/json-editor/ui/
    pub fn centered_rect(&self, percent_x: u16, percent_y: u16, r: Rect) -> Rect {
        // Cut the given rectangle into three vertical pieces
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(r);

        // Then cut the middle vertical piece into three width-wise pieces
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1] // Return the middle chunk
    }
}
