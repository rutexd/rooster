use std::fs::File;

use crate::password::{self, v2::PasswordStore};

use super::tui_app;


pub fn run_gui(
    mut file: &mut File,
) -> Result<PasswordStore, i32> {
    let mut terminal = tui_app::TuiApp::initialize().map_err(|_| 1)?;
    let store = tui_app::TuiApp::new(file).run(&mut terminal).map_err(|_| 1)?;
    tui_app::TuiApp::reset().map_err(|_| 1)?;
    Ok(store)
}