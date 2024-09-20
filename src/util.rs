use std::{fs::File, io::Read};

use rclio::CliInputOutput;
use rtoolbox::safe_vec::SafeVec;

use crate::password;

pub fn read_file(file: &mut File) -> Result<SafeVec, i32> {
    let mut input: SafeVec = SafeVec::new(Vec::new());
    file.read_to_end(input.inner_mut()).map_err(|_| 1)?;
    return Ok(input);
}

// empty stub 
pub fn empty_callback_exec(
    _matches: &clap::ArgMatches,
    store: &mut password::v2::PasswordStore,
    io: &mut impl CliInputOutput,
) -> Result<(), i32> {
    Ok(())
}