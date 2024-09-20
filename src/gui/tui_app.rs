use crossterm::{
    event::{self, Event}, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::CrosstermBackend,
    style::{palette::tailwind, Color, Modifier, Style},
    symbols,
    text::Span,
    widgets::{Block, BorderType, Borders, Padding, Paragraph, Tabs, Widget},
    Frame, Terminal,
};
use rtoolbox::{safe_string::SafeString, safe_vec::SafeVec};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use tui_input::{backend::crossterm::EventHandler, Input};

use crate::{password, util};
use crate::{password::v2::PasswordStore, password_store};
use std::{fs::File, io::{self, stdout, Error, Stdout}};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub struct TuiApp<'a> {
    exit: bool,
    pub current_state: CurrentState,
    file: &'a mut File,
    password_store: Option<password::v2::PasswordStore>,

    current_menu_item: TabElement,

    password_input: Input,
    password_input_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurrentState {
    InputMasterPassword,
    View,
}

#[derive(Default, Clone, Copy, Display, FromRepr, EnumIter)]
enum TabElement {
    #[default]
    #[strum(to_string = "View")]
    View,

    #[strum(to_string = "Add")]
    Add,
}

impl TabElement {
    fn next(self) -> Self {
        match self {
            TabElement::View => TabElement::Add,
            TabElement::Add => TabElement::View,
        }
    }

    fn prev(self) -> Self {
        match self {
            TabElement::View => TabElement::Add,
            TabElement::Add => TabElement::View,
        }
    }
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
    pub fn new(file: &'a mut File) -> Self {
        Self {
            exit: false,
            current_state: CurrentState::InputMasterPassword,
            password_store: None,
            file,

            current_menu_item: TabElement::default(),

            password_input: Input::default(),
            password_input_active: true,


        }
    }

    pub fn prepare(&mut self) -> Result<(), Error> {
        Ok(())
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<PasswordStore, Error> {
        self.prepare()?;
        while !self.exit {
            self.preprocess_update()?;
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
            self.post_update()?;
        }
        self.on_exit();

        match self.password_store.take() {
            Some(password_store) => Ok(password_store),
            None => Err(Error::new(
                io::ErrorKind::Other,
                "No password store. user exited",
            )),
        }
    }

    // layout callback
    fn render_frame(&self, frame: &mut Frame) {
        let rects = self.get_basic_rects(frame);
        let menu_rect = rects[0].clone();
        let content_rect = rects[1].clone();
        let centered_content_rect = self.centered_rect(50, 50, content_rect);

        let window = Block::new()
            .title("Rooster password manager")
            .borders(Borders::ALL);

        let titles = TabElement::iter().map(|e| Span::styled(e.to_string(), Style::default()));
        let current = self.current_menu_item as usize;

        let menu = Tabs::new(titles)
            .highlight_style(Style::default().fg(Color::White).bg(Color::LightBlue))
            .select(current);

        if self.current_state == CurrentState::InputMasterPassword {
            let scroll = self.password_input.visual_scroll(centered_content_rect.width as usize);
            let width = centered_content_rect.width as usize;
            
            let input = Paragraph::new(
                self.password_input.value()
                .chars().map(|_| { "*" }).collect::<String>()
                )
                .style(Style::default().fg(Color::Yellow))
                .block(
                    Block::default().borders(Borders::ALL).title("Enter your master password").style(
                    Style::default().fg(Color::White)
                ))
                .scroll((0, scroll as u16));
                
            if self.password_input_active {
                frame.set_cursor_position((
                    centered_content_rect.x 
                    + 1 
                    + (self.password_input.visual_cursor().min(width -2)) as u16, 
                    
                    centered_content_rect.y + 1));
            }
            
            frame.render_widget(input, centered_content_rect);
        }


        
        frame.render_widget(window, content_rect);
        frame.render_widget(menu, menu_rect);
    }

    fn handle_events(&mut self) -> Result<(), Error> {
        if event::poll(std::time::Duration::from_millis(16))? {
            if let Ok(crossterm::event::Event::Key(event)) = crossterm::event::read() {
                if self.password_input_active {
                    self.password_input.handle_event(&Event::Key(event));
                }
                
                if event.kind == crossterm::event::KeyEventKind::Press {
                    self.handle_key_event(event);
                }
            }
        }

        Ok(())
    }

    fn handle_key_event(&mut self, event: crossterm::event::KeyEvent) {
        match event.code {
            crossterm::event::KeyCode::Enter => {
                if self.password_input_active {
                    self.password_input_active = false;
                }

                let master_password = self.password_input.value().into();
                if let Err(_) = self.load_password_store(&master_password) {
                    return;
                    // TODO: handle error
                }
                self.password_input.reset();
                self.current_state = CurrentState::View;

            }
            crossterm::event::KeyCode::Esc => {
                self.exit();
            }
            crossterm::event::KeyCode::F(1) => {
                self.password_input_active = !self.password_input_active;
            }
            _ => {}
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

    fn on_exit(&self) {
        println!("Exiting...");
    }

    fn load_password_store(&mut self, master_password: &String) -> Result<(), Error> {
        let input = match util::read_file(self.file) {
            Ok(input) => input,
            Err(_) => return Err(Error::new(io::ErrorKind::Other, "Could not read file")),
        };

        match password_store::get_password_store_from_input(
            &input,
            &SafeString::from_string(master_password.clone()),
            false,
        ) {
            Ok(store) => self.password_store = Some(store),
            Err(_) => return Err(Error::new(io::ErrorKind::Other, "Could not load password store")),
        }
        Ok(())
    }
}
