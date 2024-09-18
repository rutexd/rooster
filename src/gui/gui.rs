use crate::password;

use super::tui_app;


pub fn run_gui(
) -> Result<(), i32> {
    let mut terminal = tui_app::TuiApp::initialize().map_err(|_| 1)?;
    tui_app::TuiApp::new().run(&mut terminal).map_err(|_| 1)?;
    tui_app::TuiApp::reset().map_err(|_| 1)?;
    Ok(())
}