use crossterm::{
    event::{self, Event},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    layout::*,
    prelude::CrosstermBackend,
    style::*,
    text::{Span, Text},
    widgets::*,
    Frame, Terminal,
};

use rtoolbox::{safe_string::SafeString, safe_vec::SafeVec};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use std::{
    fs::File,
    io::{self, stdout, Error, Stdout},
    ptr::{null, null_mut},
};

use tui_input::{backend::crossterm::EventHandler, Input};

use crate::clip;
use crate::gui::widgets::text_input::SimpleTextInput;
use crate::{password::v2::PasswordStore, password_store};
use crate::{
    password::{self},
    util,
};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub struct TuiApp<'a> {
    exit: bool,
    pub(crate) current_state: CurrentState,
    file: &'a mut File,

    submenu: TabElement,

    pub(crate) password_store: Option<password::v2::PasswordStore>,

    pub(crate) inputs: [InputWrapper; 4],
    pub(crate) current_active_input: Option<InputType>,

    pub(crate) show_passwords: bool,

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


#[derive(Clone, Copy, Display, FromRepr, EnumIter, PartialEq)]
pub(crate) enum InputType {
    MasterPasswordInput,
    AddAppInput,
    AddUsernameInput,
    AddPasswordInput,
}

pub(crate) struct InputWrapper {
    pub(crate) input: Input,
    pub(crate) active: bool,
}

impl Default for InputWrapper {
    fn default() -> Self {
        Self {
            input: Input::default(),
            active: false,
        }
    }
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

    // pub fn new(password_store: & password::v2::PasswordStore) -> Self {
    pub fn new(file: &'a mut File) -> Self {
        Self {
            exit: false,
            current_state: CurrentState::InputMasterPassword,
            password_store: None,
            file,

            submenu: TabElement::default(),

            
            show_passwords: false,

            table_state: TableState::default().with_selected(0),

            show_popup: false,
            popup_text: String::new(),

            inputs: [InputWrapper::default(), InputWrapper::default(), InputWrapper::default(), InputWrapper::default()],

            current_active_input: None,
        }
    }

    fn prepare(&mut self) -> Result<(), Error> {
        self.set_input_activate(InputType::MasterPasswordInput);
        Ok(())
    }

    pub fn run(&mut self, terminal: &mut Tui) -> Result<PasswordStore, Error> {
        self.prepare()?;
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
            "(F1) Show/Hide passwords",
            "(F2) Copy username",
            "(F3) Copy password",
        ]
        .join(" | ");

        let add_instructions = [
            "(F1) Show/Hide password",
            "(arrow keys) change current active input",
        ]
        .join(" | ");

        let window = Block::new()
            .title("Rooster password manager")
            .borders(Borders::ALL)
            .title_bottom(match self.current_state {
                CurrentState::InputMasterPassword => "(F1) Show/Hide password",
                CurrentState::View => match self.submenu {
                    TabElement::Start => "Instructions",
                    TabElement::View => &view_instructions,
                    TabElement::Add => &add_instructions,
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

            let area = self.centered_rect(95, 50, content_rect);
            frame.render_widget(popup, area);
            return;
        }

        if self.current_state == CurrentState::InputMasterPassword {
            self.render_master_password_input(frame, content_rect);
            return;
        }

        self.render_tabs(frame);
    }

    fn render_tabs(& self, frame: &mut Frame) {
        match self.submenu {
            TabElement::Start => self.render_start_screen(frame),
            TabElement::View => self.render_view_tab(frame),
            TabElement::Add => self.render_add_tab(frame),
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
        match self.current_active_input {
            Some(index) => {
                self.inputs[index as usize].input.handle_event(&Event::Key(event));
                
                if event.code == crossterm::event::KeyCode::Esc || event.code == crossterm::event::KeyCode::Enter {
                    self.deactivate_input();
                }


                // the keys are allowed to pass to the next handler, FIXME this is a bit hacky
                if  event.code != crossterm::event::KeyCode::Enter && 
                    event.code != crossterm::event::KeyCode::Up && 
                    event.code != crossterm::event::KeyCode::Down &&
                    event.code != crossterm::event::KeyCode::F(1)    
                {
                    return;
                }

                
            }
            None => {}
        }

        match event.code {
            crossterm::event::KeyCode::Enter => {
                if self.current_state == CurrentState::InputMasterPassword {
                    let master_password = self.inputs[InputType::MasterPasswordInput as usize].input.value().into();

                    // TODO fix invalid -> valid read (prob something being consumed?)
                    if let Err(_) = self.load_password_store(&master_password) {
                        return; // TODO: handle error
                    }

                    self.current_state = CurrentState::View;
                    return;
                }

                // TODO adapt commands handlers to share code
                if self.submenu == TabElement::Add {
                    let app = self.inputs[InputType::AddAppInput as usize].input.value().to_string();
                    let username = self.inputs[InputType::AddUsernameInput as usize].input.value().to_string();
                    let password = self.inputs[InputType::AddPasswordInput as usize].input.value().to_string();

                    let password_store = self.password_store.as_mut().unwrap();


                    if password_store.has_password(&app) {
                        self.popup("App already exists");
                        return;
                    }

                    let password = password::v2::Password::new(app, username, password);

                    match password_store.add_password(password) {
                        Ok(_) => {
                            self.clear_input(InputType::AddAppInput);
                            self.clear_input(InputType::AddUsernameInput);
                            self.clear_input(InputType::AddPasswordInput);

                            self.submenu = TabElement::View;
                        }
                        Err(err) => {
                            self.popup(&format!("Error: {:?}", err));
                        }
                    }
                }
            }



            crossterm::event::KeyCode::F(1) => {
                self.show_passwords = !self.show_passwords;
            }

            crossterm::event::KeyCode::F(2) => {
                if self.submenu == TabElement::View {
                    let index = self.table_state.selected().unwrap();
                    let username = self.password_store.as_ref().unwrap().get_all_passwords()[index]
                        .clone()
                        .username;
                    match clip::copy_to_clipboard(&SafeString::from_string(username.to_string())) {
                        Ok(_) => {}
                        Err(_) => {} // TODO: handle error (show popup?)
                    }
                }
            }

            crossterm::event::KeyCode::F(3) => {
                if self.submenu == TabElement::View {
                    let index = self.table_state.selected().unwrap();
                    let password = self.password_store.as_ref().unwrap().get_all_passwords()[index]
                        .clone()
                        .password;
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
                        let total = self
                            .password_store
                            .as_ref()
                            .unwrap()
                            .get_all_passwords()
                            .len();
                        let current = self.table_state.selected().unwrap();
                        if current == 0 {
                            self.table_state.select(Some(total - 1));
                        } else {
                            self.table_state.select_previous();
                        }
                    }

                    TabElement::Add => {
                        // deactivate current input and activate previous. if current is first, activate last.
                        // if current is none, activate first

                        match self.current_active_input {
                            Some(index) => {
                                let prev = match index {
                                    InputType::AddAppInput => InputType::AddPasswordInput,
                                    InputType::AddUsernameInput => InputType::AddAppInput,
                                    InputType::AddPasswordInput => InputType::AddUsernameInput,
                                    _ => unreachable!(),
                                };

                                self.deactivate_input();
                                self.set_input_activate(prev);
                            }
                            None => {
                                self.set_input_activate(InputType::AddPasswordInput);
                            }
                        }

                       
                    }
                    _ => {}
                }
            }

            crossterm::event::KeyCode::Down => {
                match self.submenu {
                    TabElement::View => {
                        let total = self
                            .password_store
                            .as_ref()
                            .unwrap()
                            .get_all_passwords()
                            .len();
                        let current = self.table_state.selected().unwrap();
                        if current == total - 1 {
                            self.table_state.select(Some(0));
                        } else {
                            self.table_state.select_next();
                        }
                    }
                    TabElement::Add => {
                        // deactivate current input and activate next. if current is last, activate first.
                        // if current is none, activate first

                        match self.current_active_input {
                            Some(index) => {
                                let next = match index {
                                    InputType::AddAppInput => InputType::AddUsernameInput,
                                    InputType::AddUsernameInput => InputType::AddPasswordInput,
                                    InputType::AddPasswordInput => InputType::AddAppInput,
                                    _ => unreachable!(),
                                };

                                self.deactivate_input();
                                self.set_input_activate(next);
                            }
                            None => {
                                self.set_input_activate(InputType::AddAppInput);
                            }
                        }
                        

                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    pub(crate) fn set_input_activate(&mut self, input: InputType) {
        self.inputs[input as usize].active = true;
        self.current_active_input = Some(input);
    }

    fn _deactivate_input(&mut self, reset: bool){
        match self.current_active_input {
            Some(index) => {
                if reset {
                    self.inputs[index as usize].input.reset();
                }
                self.inputs[index as usize].active = false;
                self.current_active_input = None;
            }
            None => {}
        }
    }

    pub(crate) fn clear_input(&mut self, input: InputType) {
        self.inputs[input as usize].input.reset();
    }

    pub(crate) fn deactivate_input(&mut self) {
        self._deactivate_input(false);
    }

    pub(crate) fn deactivate_input_and_reset(&mut self) {
        self._deactivate_input(true);
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
