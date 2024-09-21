use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    layout::*, prelude::CrosstermBackend, style::*, text::{Span, Text}, widgets::*, Frame, Terminal,
};

use rtoolbox::{safe_string::SafeString, safe_vec::SafeVec};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use std::{
    fs::File,
    io::{self, stdout, Error, Stdout},
};

use tui_input::{backend::crossterm::EventHandler, Input};

use crate::{password::{self}, util};
use crate::{password::v2::PasswordStore, password_store};
use crate::clip;


pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub struct TuiApp<'a> {
    exit: bool,
    pub(crate) current_state: CurrentState,
    file: &'a mut File,
    file_data: SafeVec,

    pub(crate) password_store: Option<password::v2::PasswordStore>,

    submenu: TabElement,

    password_input: Input,
    password_input_active: bool,
    password_input_show: bool,

    pub(crate) table_state: TableState,

    pub(crate) show_popup: bool,
    pub(crate) popup_text: String,

}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CurrentState {
    InputMasterPassword,
    View,
}

#[derive(Default, Clone, Copy, Display, FromRepr, EnumIter, PartialEq)]
enum TabElement {
    #[default]
    #[strum(to_string = "Start")]
    Start,

    #[strum(to_string = "View")]
    View,

    #[strum(to_string = "Add")]
    Add,
}

impl TabElement {
    fn next(self) -> Self {
        let current = self as usize;
        let next = current.saturating_add(1);
        Self::from_repr(next).unwrap_or(self)
    }

    fn prev(self) -> Self {
        let current = self as usize;
        let prev = current.saturating_sub(1);
        Self::from_repr(prev).unwrap_or(self)
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
            file: file,
            file_data: SafeVec::new(Vec::new()),

            submenu: TabElement::default(),

            password_input: Input::default(),
            password_input_active: true,
            password_input_show: false,

            table_state: TableState::default().with_selected(0),

            show_popup: false,
            popup_text: String::new(),
        }
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<PasswordStore, Error> {
        while !self.exit {
            self.update()?;
            terminal.draw(|frame| self.render_frame(frame))?;
            self.handle_events()?;
        }

        // on exit { ... }

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

        let view_instructions = [
            "(F2) Copy username",
            "(F3) Copy password",
        ].join(" | ");

        let window = Block::new()
            .title("Rooster password manager")
            .borders(Borders::ALL)
            .title_bottom(match self.current_state {
                CurrentState::InputMasterPassword => "(F1) Show/Hide password",
                CurrentState::View => match self.submenu {
                    TabElement::Start => "Instructions",
                    TabElement::View => view_instructions.as_str(),
                    _ => "TODO",
                },
            });

        let titles = TabElement::iter().map(|e| Span::styled(e.to_string(), Style::default()));
        let current = self.submenu as usize;

        let menu = Tabs::new(titles)
            .highlight_style(Style::default().fg(Color::White).bg(Color::LightBlue))
            .select(current);

        // render window and menu so we always having skeleton
        frame.render_widget(menu, menu_rect);
        frame.render_widget(window, content_rect);

        if self.show_popup {
            let popup = Paragraph::new(self.popup_text.clone())
                .style(Style::default().fg(Color::Red))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Error")
                        .style(Style::default().fg(Color::LightRed))
                        .title_bottom("(Esc) Close"),
                );

            let popup_rect = self.centered_rect(95, 50, content_rect);
            frame.render_widget(popup, popup_rect);
            return;
        }

        if self.current_state == CurrentState::InputMasterPassword {
            let centered_content_rect = self.centered_rect(25, 50, content_rect);

            let scroll = self
                .password_input
                .visual_scroll(centered_content_rect.width as usize);
            let width = centered_content_rect.width as usize;

            let password_input = Paragraph::new(
                self.password_input
                    .value()
                    .chars()
                    .map(|c| if self.password_input_show { c } else { '*' })
                    .collect::<String>(),
            )
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Enter your master password")
                    .style(Style::default().fg(Color::White))
                    ,
            )
            .scroll((0, scroll as u16));

            if self.password_input_active {
                frame.set_cursor_position((
                    centered_content_rect.x
                        + 1
                        + (self.password_input.visual_cursor().min(width - 2)) as u16,
                    centered_content_rect.y + 1,
                ));
            }

            frame.render_widget(password_input, centered_content_rect);
        } else {
            match self.submenu {
                TabElement::Start => {
                    self.render_start_screen(frame);
                },
                TabElement::View => {
                    self.render_view_tab(frame);
                }
                TabElement::Add => {},
            }

        }


        
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
        if self.show_popup {
            if event.code == crossterm::event::KeyCode::Esc {
                self.show_popup = false;
            }

            return;
        }

        if self.password_input_active {
            self.password_input.handle_event(&Event::Key(event));
        }

        match event.code {
            crossterm::event::KeyCode::Enter => {
                if self.current_state == CurrentState::InputMasterPassword {
                    // TODO: better way to handle this
                    let master_password = self.password_input.value().into();

                    // TODO: fix invalid -> valid read (prob something being consumed?)
                    if let Err(_) = self.load_password_store(&master_password) {
                        return; // TODO: handle error
                    }

                    self.password_input_active = false;
                    self.current_state = CurrentState::View;
                    self.password_input.reset();

                }
            }

            crossterm::event::KeyCode::F(1) => {
                self.password_input_show = !self.password_input_show;
            }

            crossterm::event::KeyCode::F(2) => {
                if self.submenu == TabElement::View {
                
                    let index = self.table_state.selected().unwrap();
                    let username = self.password_store.as_ref().unwrap().get_all_passwords()[index].clone().username;
                    match clip::copy_to_clipboard(&SafeString::from_string(username.to_string())) {
                        Ok(_) => {} 
                        Err(_) => {} // TODO: handle error (show popup?)
                    }
                }
            }

            crossterm::event::KeyCode::F(3) => {
                if self.submenu == TabElement::View {
                    let index = self.table_state.selected().unwrap();
                    let password = self.password_store.as_ref().unwrap().get_all_passwords()[index].clone().password;
                    match clip::copy_to_clipboard(&password) {
                        Ok(_) => {} 
                        Err(_) => {} // TODO: handle error (show popup?)
                    }
                }
            }

            crossterm::event::KeyCode::F(8) => {
                self.popup("test");
            }

            crossterm::event::KeyCode::Esc => {
                if self.show_popup {
                    self.show_popup = false;
                    return;
                }

                // IDEA: esc to go back to previous state first?
                // if self.current_state == CurrentState::InputMasterPassword {
                //     self.exit();
                // } else {
                //     self.current_state = CurrentState::InputMasterPassword;
                //     self.password_input_active = true;
                //     return;
                // }

                self.exit();
            }

            crossterm::event::KeyCode::Left => {
                self.submenu = self.submenu.prev();
            }

            crossterm::event::KeyCode::Right => {
                self.submenu = self.submenu.next();
            }

            crossterm::event::KeyCode::Up => {
                match self.submenu {
                    TabElement::View => {
                        let total = self.password_store.as_ref().unwrap().get_all_passwords().len();
                        let current = self.table_state.selected().unwrap();
                        if current == 0 {
                            self.table_state.select(Some(total - 1));
                        } else {
                            self.table_state.select_previous();
                        }
                    }
                    _ => {}
                }
            }

            crossterm::event::KeyCode::Down => {
                match self.submenu {
                    TabElement::View => {
                        let total = self.password_store.as_ref().unwrap().get_all_passwords().len();
                        let current = self.table_state.selected().unwrap();
                        if current == total - 1 {
                            self.table_state.select(Some(0));
                        } else {
                            self.table_state.select_next();
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // called before render
    fn update(&mut self) -> Result<(), Error> {
        Ok(())
    }


    fn exit(&mut self) {
        self.exit = true;
    }


    fn load_password_store(&mut self, master_password: &String) -> Result<(), Error> {
        let input = match util::read_file(&mut self.file) {
            Ok(input) => input,
            Err(_) => return Err(Error::new(io::ErrorKind::Other, "Could not read file")),
        };

        match password_store::get_password_store_from_input(
            &input,
            &SafeString::from_string(master_password.clone()),
            false,
        ) {
            Ok(store) => self.password_store = Some(store),
            Err(e) => {
                return Err(Error::new(
                    io::ErrorKind::Other,
                    "Could not load password store",
                ))
            }
        }
        Ok(())
    }

    fn popup(&mut self, text: &str) {
        self.show_popup = true;
        self.popup_text = text.to_string();
    }
}
