use crossterm::{
    event::{self, Event, ModifierKeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{prelude::CrosstermBackend, style::*, text::Span, widgets::*, Frame, Terminal};

use rtoolbox::safe_string::SafeString;
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};

use std::{
    borrow::Borrow, fs::File, io::{self, stdout, Error, Stdout}
};

use tui_input::{backend::crossterm::EventHandler, Input};

use crate::clip;
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

/// enum that represents all possible inputs in the app. casting to usize should give correct index of a inputs array
#[derive(Clone, Copy, Display, PartialEq)]
pub(crate) enum InputType {
    MasterPasswordInput,
    AddAppInput,
    AddUsernameInput,
    AddPasswordInput,
}

pub(crate) struct InputWrapper {
    pub(crate) input: Input,
    pub(crate) active: bool,
    pub(crate) disable_after_enter: bool,
}

impl Default for InputWrapper {
    fn default() -> Self {
        Self {
            input: Input::default(),
            active: false,
            disable_after_enter: true,
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

impl std::ops::Index<InputType> for [InputWrapper] {
    type Output = InputWrapper;

    fn index(&self, index: InputType) -> &Self::Output {
        &self[index as usize]
    }
}

impl std::ops::IndexMut<InputType> for [InputWrapper] {
    fn index_mut(&mut self, index: InputType) -> &mut Self::Output {
        &mut self[index as usize]
    }
}

// same for tab element




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

            inputs: [
                InputWrapper::default(), // master password
                InputWrapper::default(), // add app
                InputWrapper::default(), // add username
                InputWrapper::default(), // add password
            ],

            current_active_input: None,
        }
    }

    fn prepare(&mut self) -> Result<(), Error> {
        self.set_input_activate(InputType::MasterPasswordInput);
        self.inputs[InputType::MasterPasswordInput].disable_after_enter = false;
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
            "(CTRL + SHIFT + Del) Delete password",
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
            return frame.render_widget(popup, area);
        }

        if self.current_state == CurrentState::InputMasterPassword {
            return self.render_master_password_input(frame, content_rect);
        }

        self.render_tabs(frame);
    }

    fn render_tabs(&self, frame: &mut Frame) {
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
        
        if event.code == crossterm::event::KeyCode::Esc && self.show_popup {
            self.show_popup = false;
            return;
        }

        match self.current_active_input {
            Some(index) => {
                // so we can always exit
                if event.modifiers == crossterm::event::KeyModifiers::CONTROL {
                    match event.code {
                        crossterm::event::KeyCode::Char('c') => {
                            return self.exit();
                        }
                        _ => {}
                    }
                }

                self.inputs[index]
                    .input
                    .handle_event(&Event::Key(event));

                if (event.code == crossterm::event::KeyCode::Esc
                    || event.code == crossterm::event::KeyCode::Enter 
                ) && self.inputs[index].disable_after_enter 

                {
                    self.deactivate_input();
                }

                // the keys are allowed to pass to the next handler, FIXME this is a bit hacky
                if event.code != crossterm::event::KeyCode::Enter
                    && event.code != crossterm::event::KeyCode::Up
                    && event.code != crossterm::event::KeyCode::Down
                    && event.code != crossterm::event::KeyCode::F(1)
                {
                    return;
                }
            }
            None => {}
        }

        match event.code {
            crossterm::event::KeyCode::Enter => {
                if self.current_state == CurrentState::InputMasterPassword {
                    let master_password = self.inputs[InputType::MasterPasswordInput]
                        .input
                        .value()
                        .into();

                    // TODO fix invalid -> valid read (prob something being consumed?)
                    if let Err(err) = self.load_password_store(&master_password) {
                        return self.popup(&format!("Failed to load password store: {:?}", err));
                    }

                    self.current_state = CurrentState::View;
                    self.show_passwords = false;
                    self.deactivate_input_and_reset();

                    return;
                }

                // TODO adapt commands handlers to share code
                if self.submenu == TabElement::Add {
                    let app = self.inputs[InputType::AddAppInput]
                        .input
                        .value()
                        .to_string();
                    let username = self.inputs[InputType::AddUsernameInput]
                        .input
                        .value()
                        .to_string();
                    let password = self.inputs[InputType::AddPasswordInput]
                        .input
                        .value()
                        .to_string();

                    let password_store = self.password_store.as_mut().unwrap();

                    if password_store.has_password(&app) {
                        return self.popup("App with that name already exists.");
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
                        if total == 0 {
                            return;
                        }

                        match self.table_state.selected() {
                            Some(index) => {
                                let prev = if index == 0 {
                                    total - 1
                                } else {
                                    index - 1
                                };

                                self.table_state.select(Some(prev));
                            }
                            None => {
                                self.table_state.select(Some(total - 1));
                            }
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
                        if total == 0 {
                            return;
                        }

                        match self.table_state.selected() {
                            Some(index) => {
                                let next = if index == total - 1 {
                                    0
                                } else {
                                    index + 1
                                };

                                self.table_state.select(Some(next));
                            }
                            None => {
                                self.table_state.select(Some(0));
                            }
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

            crossterm::event::KeyCode::Delete => {
                if self.submenu == TabElement::View {
                    // ensure user is pressed ctrl shift del. 
                    let modifiers_pressed = event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL)
                        && event.modifiers.contains(crossterm::event::KeyModifiers::SHIFT);

                    if !modifiers_pressed {
                        return self.popup("Press CTRL + SHIFT + Del to delete password");
                    }

                    let current_selected_index = match self.table_state.selected() {
                        Some(index) => index,
                        None => return,
                    };

                    let total = self.password_store.as_ref().unwrap().get_all_passwords().len();


                    // TODO: split command for password deletion
                    let index = self.table_state.selected().unwrap();
                    let password_name = self.password_store.as_ref().unwrap().get_all_passwords()[index].name.clone();

                    match self.password_store.as_mut().unwrap().delete_password(&password_name) {
                        Ok(_) => {}
                        Err(e) => {
                            self.popup(&format!("Failed to delete password: {:?}", e));
                        }
                    }

                    // update selected index
                    self.table_state.select(if total - 1 == 0 { None } else { Some(if current_selected_index == 0 { 0 } else { current_selected_index - 1 }) });

                    
                }
            }
            _ => {}
        }
    }

    pub(crate) fn set_input_activate(&mut self, input: InputType) {
        self.inputs[input].active = true;
        self.current_active_input = Some(input);
    }

    fn _deactivate_input(&mut self, reset: bool) {
        match self.current_active_input {
            Some(index) => {
                if reset {
                    self.inputs[index].input.reset();
                }
                self.inputs[index].active = false;
                self.current_active_input = None;
            }
            None => {}
        }
    }

    pub(crate) fn clear_input(&mut self, input: InputType) {
        self.inputs[input].input.reset();
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
                    format!("{:?}", e),
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
