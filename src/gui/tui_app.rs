use crossterm::{
    event, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    prelude::CrosstermBackend, style::{Color, Style}, symbols, widgets::{Block, BorderType, Borders, Padding, Paragraph}, Frame, Terminal
};

use std::io::{self, stdout, Error, Stdout};
use tui_textarea::{Input, TextArea};
use crate::password_store;
use crate::password;

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurrentState {
    InputMasterPassword,
    View
}

pub struct TuiApp<'a> {
    exit: bool,
    current_state: CurrentState,
    password_store: Option<password::v2::PasswordStore>,

    text_area: TextArea<'a>,
}

impl<'a> TuiApp<'a> {
    pub fn initialize() -> io::Result<Tui> {
        execute!(stdout(), EnterAlternateScreen)?;
        enable_raw_mode()?;
        Terminal::new(CrosstermBackend::new(stdout()))
    }

    pub fn reset() -> io::Result<()> {
        execute!(stdout(), LeaveAlternateScreen)?;
        disable_raw_mode()?;
        Ok(())
    }

    // pub fn new(password_store: &'a password::v2::PasswordStore) -> Self {
        pub fn new() -> Self {
        Self {
            exit: false,
            current_state: CurrentState::InputMasterPassword,
            password_store: None,
            text_area: TextArea::default(),
        }
    }

    pub fn prepare(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<(), Error> {
        self.prepare()?;
        while !self.exit {
            self.preprocess_update()?;
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
            self.post_update()?;
        }

        Ok(())
    }

    // layout callback
    fn render_frame(&self, frame: &mut Frame) {
        let window = Block::new()
            .title("Rooster passwor manager")
            .borders(Borders::ALL);

        

        frame.render_widget(window, frame.area());

    }

    fn handle_events(&mut self) -> Result<(), Error> {
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Ok(crossterm::event::Event::Key(event)) = crossterm::event::read() {
                if event.kind == crossterm::event::KeyEventKind::Press {
                    self.handle_key_event(event);
                }
            }
        }
     
        Ok(())
    }

    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent) {
        match event.code {
            crossterm::event::KeyCode::Char('q') => {
                self.exit = true;
            }
            _ => {
            }
        }
    }

    fn preprocess_update(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn post_update(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn exit(&mut self) {
        self.exit = true;
    }
}