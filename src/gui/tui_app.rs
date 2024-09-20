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

use crate::{password::{self, v2::Password}, util};
use crate::{password::v2::PasswordStore, password_store};
use crate::clip;


pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub struct TuiApp<'a> {
    exit: bool,
    pub current_state: CurrentState,
    file: &'a mut File,
    password_store: Option<password::v2::PasswordStore>,

    current_menu_item: TabElement,

    password_input: Input,
    password_input_active: bool,
    password_input_show: bool,

    table_state: TableState,
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
            file,

            current_menu_item: TabElement::default(),

            password_input: Input::default(),
            password_input_active: true,
            password_input_show: false,

            table_state: TableState::default().with_selected(0),
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

        let view_instructions = [
            "F2: Copy username",
            "F3: Copy password",
        ].join(" | ");

        let window = Block::new()
            .title("Rooster password manager")
            .borders(Borders::ALL)
            .title_bottom(match self.current_state {
                CurrentState::InputMasterPassword => "F1: Show/Hide password",
                CurrentState::View => match self.current_menu_item {
                    TabElement::Start => "Instructions",
                    TabElement::View => view_instructions.as_str(),
                    _ => "TODO",
                },
            });

        let titles = TabElement::iter().map(|e| Span::styled(e.to_string(), Style::default()));
        let current = self.current_menu_item as usize;

        let menu = Tabs::new(titles)
            .highlight_style(Style::default().fg(Color::White).bg(Color::LightBlue))
            .select(current);

        if self.current_state == CurrentState::InputMasterPassword {
            let scroll = self
                .password_input
                .visual_scroll(centered_content_rect.width as usize);
            let width = centered_content_rect.width as usize;

            let input = Paragraph::new(
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
                    .style(Style::default().fg(Color::White)),
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

            frame.render_widget(input, centered_content_rect);
        }

        if self.current_menu_item == TabElement::View {
            self.render_passwords_table(frame);
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
                if self.current_menu_item == TabElement::View {
                
                    let index = self.table_state.selected().unwrap();
                    let username = self.password_store.as_ref().unwrap().get_all_passwords()[index].clone().username;
                    match clip::copy_to_clipboard(&SafeString::from_string(username.to_string())) {
                        Ok(_) => {} 
                        Err(_) => {} // TODO: handle error (show popup?)
                    }
                }
            }

            crossterm::event::KeyCode::F(3) => {
                if self.current_menu_item == TabElement::View {
                    let index = self.table_state.selected().unwrap();
                    let password = self.password_store.as_ref().unwrap().get_all_passwords()[index].clone().password;
                    match clip::copy_to_clipboard(&password) {
                        Ok(_) => {} 
                        Err(_) => {} // TODO: handle error (show popup?)
                    }
                }
            }

            crossterm::event::KeyCode::Esc => {
                self.exit();
            }

            crossterm::event::KeyCode::Left => {
                if self.current_state == CurrentState::View {
                    self.current_menu_item = self.current_menu_item.prev();
                }
            }

            crossterm::event::KeyCode::Right => {
                if self.current_state == CurrentState::View {
                    self.current_menu_item = self.current_menu_item.next();
                }
            }

            crossterm::event::KeyCode::Up => {
                if self.current_menu_item == TabElement::View { // add out of bounds check
                    self.table_state.select_previous();
                }
            }

            crossterm::event::KeyCode::Down => {
                if self.current_menu_item == TabElement::View { // add out of bounds check
                    self.table_state.select_next();
                }
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
            Err(_) => {
                return Err(Error::new(
                    io::ErrorKind::Other,
                    "Could not load password store",
                ))
            }
        }
        Ok(())
    }

    fn render_passwords_table(&self, frame: &mut Frame) {
        let area = self.centered_rect(75, 85, frame.area());

        let passwords = self.password_store.as_ref().unwrap().get_all_passwords();
        // 30 test passwords
        // let passwords = vec![
        //     Password::new("test1", "test1", "test1"),
        //     Password::new("test2", "test2", "test2"),
        //     Password::new("test3", "test3", "test3"),
        //     Password::new("test4", "test4", "test4"),
        //     Password::new("test5", "test5", "test5"),
        //     Password::new("test6", "test6", "test6"),
        //     Password::new("test7", "test7", "test7"),
        //     Password::new("test8", "test8", "test8"),
        //     Password::new("test9", "test9", "test9"),
        //     Password::new("test10", "test10", "test10"),
        //     Password::new("test11", "test11", "test11"),
        //     Password::new("test12", "test12", "test12"),
        //     Password::new("test13", "test13", "test13"),
        //     Password::new("test14", "test14", "test14"),
        //     Password::new("test15", "test15", "test15"),
        //     Password::new("test16", "test16", "test16"),
        //     Password::new("test17", "test17", "test17"),
        //     Password::new("test18", "test18", "test18"),
        //     Password::new("test19", "test19", "test19"),
        //     Password::new("test20", "test20", "test20"),
        //     Password::new("test21", "test21", "test21"),
        //     Password::new("test22", "test22", "test22"),
        //     Password::new("test23", "test23", "test23"),
        //     Password::new("test24", "test24", "test24"),
        //     Password::new("test25", "test25", "test25"),
        //     Password::new("test26", "test26", "test26"),
        //     Password::new("test27", "test27", "test27"),
        //     Password::new("test28", "test28", "test28"),
        //     Password::new("test29", "test29", "test29"),
        //     Password::new("test30", "test30", "test30"),
        // ];

        let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
        let selected_style = Style::default()
            .add_modifier(Modifier::REVERSED);

        let header = ["App", "Username", "Password"]
            .iter()
            .map(|&s| Cell::from(Text::from(s)))
            .collect::<Row>()
            .style(header_style)
            .height(1);
        
        let rows = passwords.iter().enumerate().map(|(i, data)| {
            let color = Color::Black;

            let item = [
                data.name.as_str(),
                data.username.as_str(),
                data.password.as_str(),
            ];
            item.iter()
                .map(|content| Cell::from(Text::from(*content)))
                .collect::<Row>()
                .style(Style::new().fg(Color::Gray).bg(color))
                .height(1) // height example for the row
        });
        let t = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(30),
                Constraint::Percentage(50),
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
        

        let mut state = self.table_state.clone();
        frame.render_stateful_widget(t, area, &mut state);
    }
}
