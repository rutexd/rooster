use std::fs::File;

use crate::password::{self, v2::PasswordStore};

use super::tui_app;


pub fn run_gui(
    file: &mut File,
) -> Result<PasswordStore, i32> {
    let mut terminal = tui_app::TuiApp::initialize().unwrap();
    let store = tui_app::TuiApp::new(file).run(&mut terminal).map_err(|_| 1);
    
    match tui_app::TuiApp::reset() {
        Ok(_) => (),
        Err(_) => return Err(1),
    }

    match store {
        Ok(store) => Ok(store),
        Err(_) => Err(1),
    }
}