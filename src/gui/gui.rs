use std::rc::Rc;

use ratatui::{
    layout::{Constraint, Direction, Layout, Margin, Offset, Rect},
    style::{Color, Modifier, Style},
    text::Text,
    widgets::{Block, Borders, Cell, Padding, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table},
    Frame,
};
use rtoolbox::safe_string::SafeString;

use crate::gui::tui_app::TuiApp;

use super::{tui_app::{CurrentState, InputType}, widgets::text_input::SimpleTextInput};

// impl<'a> TuiApp<'a> {
impl<'a> TuiApp<'a> {

    pub(crate) fn render_master_password_input(&self, frame: &mut Frame, content_rect: Rect) {
        let centered_content_rect = self.centered_rect(25, 50, content_rect);

        let width = centered_content_rect.width as usize;
        
        let input = &self.inputs[InputType::MasterPasswordInput];
    
        let password_input = SimpleTextInput::new(
            "Enter your master password",
            &input.input,
            !self.show_passwords,
        );

        if input.active {
            self.set_cursor(centered_content_rect.x, centered_content_rect.y, width, &input.input, frame);
        }

        frame.render_widget(password_input, centered_content_rect);
    }

    /// Returns basic skeleton for the app. 0 is for the top part of the screen, 1 is for the bottom part of the screen where the main content is.
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

    /// took from https://ratatui.rs/tutorials/json-editor/ui/
    /// Returns a rectangle that is centered in the given rectangle.
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

    pub(crate) fn render_start_screen(&self, frame: &mut Frame) {
        let area = frame.area();
        let text = vec![
            Text::styled(
                "Rooster password manager",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Text::raw("\n"),
            Text::raw("Use arrows (← →) to navigate between tabs.\n"),
            Text::raw("You can see hints on the bottom of the tab you are on.\n"),
            Text::raw("Press ESC to exit the app, do not kill the process by force, otherwise your changes will not be saved.\n"),
        ];

        frame.render_widget(
            Table::new(
                text.iter()
                    .map(|t| Row::new(vec![Cell::from(t.clone())]).height(1)),
                vec![Constraint::Percentage(100)],
            )
            .block(Block::default()),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
        );
    }

    pub(crate) fn render_view_tab(&self, frame: &mut Frame) {
        let area = self.centered_rect(90, 90, frame.area());

        let passwords = self.password_store.as_ref().unwrap().get_all_passwords();
        const TABLE_ITEM_HEIGHT: usize = 1;

        // 30 test passwords
        // let mut passwords = vec![]; for i in 0..30 {
        // passwords.push(Password::new(format!("test{}", i), format!("test{}", i), format!("test{}", i)));
        // }

        let header_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let selected_style = Style::default().add_modifier(Modifier::REVERSED);

        let header = ["App", "Username", "Password"]
            .iter()
            .map(|&s| Cell::from(Text::from(s)))
            .collect::<Row>()
            .style(header_style)
            .height(1);

        let rows = passwords.iter().enumerate().map(|(i, data)| {
            let password = match self.show_passwords {
                true => data.password.clone(),
                false => SafeString::from_string(String::from("*").repeat(data.password.len())),
            };

            [
                data.name.clone(),
                data.username.clone(),
                password.to_string(),
            ]
            .iter()
            .map(|content| Cell::from(Text::from(content.clone())))
            .collect::<Row>()
            .style(Style::new().fg(Color::Gray).bg(Color::Black))
            .height(TABLE_ITEM_HEIGHT as u16) // height of the row
        });

        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(35),
                Constraint::Percentage(45),
            ],
        )
        .header(header)
        .highlight_style(selected_style)
        .block(Block::default().padding(Padding {
            top: 1,
            right: 0,
            bottom: 0,
            left: 0,
        }));

        let mut table_state = self.table_state.clone();
        let mut scroll_state = ScrollbarState::new((passwords.len() - 1) * TABLE_ITEM_HEIGHT);

        scroll_state = scroll_state.position(table_state.selected().unwrap() * TABLE_ITEM_HEIGHT);

        frame.render_stateful_widget(table, area, &mut table_state);
        frame.render_stateful_widget(
            Scrollbar::default()
                .style(Style::default().fg(Color::Red))
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            })
            .offset(Offset { x: 2, y: 1 }),
            &mut scroll_state,
        );
    }

    fn validate_input(&self, input: &str) -> bool {
        input.len() > 0
    }
    
    pub(crate) fn render_add_tab(&self, frame: &mut Frame) {
        // activate the input

        let area = self.centered_rect(90, 85, frame.area());
        
        let app_input = SimpleTextInput::new(
            "App",
            &self.inputs[InputType::AddAppInput].input,
            false,
        );

        let username_input = SimpleTextInput::new(
            "Username",
            &self.inputs[InputType::AddUsernameInput].input,
            false,
        );

        let password_input = SimpleTextInput::new(
            "Password",
            &self.inputs[InputType::AddPasswordInput].input,
            !self.show_passwords,
        );

        let inputs = 3;
        let layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                (0..inputs+2).map(|i| Constraint::Length( if i == 0 {1} else {3} )).collect::<Vec<_>>(),
            )
            .split(area);

        frame.render_widget(app_input, layouts[1]);
        frame.render_widget(username_input, layouts[2]);
        frame.render_widget(password_input, layouts[3]);

        let p = Paragraph::new("Press Enter to save the password")
            .style(Style::default().fg(Color::Yellow))
            .block(Block::default().borders(Borders::ALL).title("Hint"));
        frame.render_widget(p, layouts[4]);
      
        
        if self.inputs[InputType::AddAppInput].active {
            self.set_cursor(layouts[1].x, layouts[1].y, layouts[1].width as usize, &self.inputs[InputType::AddAppInput].input, frame);
        }
        if self.inputs[InputType::AddUsernameInput].active {
            self.set_cursor(layouts[2].x, layouts[2].y, layouts[2].width as usize, &self.inputs[InputType::AddUsernameInput].input, frame);
        }

        if self.inputs[InputType::AddPasswordInput].active {
            self.set_cursor(layouts[3].x, layouts[3].y, layouts[3].width as usize, &self.inputs[InputType::AddPasswordInput].input, frame);
        }
        
    }
}
