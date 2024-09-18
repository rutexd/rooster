use rclio::CliInputOutput;
use rtoolbox::safe_string::SafeString;
use std::io::Result as IoResult;

pub fn ask_master_password(io: &mut impl CliInputOutput) -> IoResult<SafeString> {
    io.prompt_password("Type your master password: ")
}