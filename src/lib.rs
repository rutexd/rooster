// #![allow(useless_format, too_many_arguments)]

use crate::password::v2::PasswordStore;
use clap::{Arg, ArgAction, Command};
use rclio::CliInputOutput;
use rclio::OutputType;
use rtoolbox::safe_string::SafeString;
use rtoolbox::safe_vec::SafeVec;
use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Result as IoResult;
use std::ops::Deref;
use std::path::{Path, PathBuf};

mod aes;
mod clip;
mod commands;
mod ffi;
mod generate;
mod list;
mod password;
mod user_input;
mod password_store;
mod gui;


#[cfg(unix)]
mod quale;
#[cfg(unix)]
mod shell_escape;

#[cfg(windows)]
fn example_environment_variable_configuration() -> &'static str {
    return "set ROOSTER_FILE=C:\\Users\\my-user\\path\\to\\rooster.file"
}

#[cfg(unix)]
fn example_environment_variable_configuration() -> &'static str {
    return "export ROOSTER_FILE=$HOME/path/to/rooster.file"
}

fn only_digits(s: &str) -> bool {
    s.chars()
        .map(|c| char::is_ascii_digit(&c))
        .collect::<Vec<bool>>()
        .contains(&false)
}

fn validate_arg_usize(v: &str) -> Result<usize, String> {
    if only_digits(v) {
        return Err(String::from("The value must be made of digits"));
    }
    Ok(v.parse::<usize>().unwrap())
}

fn validate_arg_u8(v: &str) -> Result<u8, String> {
    if only_digits(v) {
        return Err(String::from("The value must be made of digits"));
    }
    Ok(v.parse::<u8>().unwrap())
}

fn validate_arg_u32(v: &str) -> Result<u32, String> {
    if only_digits(v) {
        return Err(String::from("The value must be made of digits"));
    }
    Ok(v.parse::<u32>().unwrap())
}

fn open_password_file(filename: &str) -> IoResult<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    options.write(true);
    options.create(false);
    options.open(&Path::new(filename))
}

fn create_password_file(filename: &str) -> IoResult<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    options.write(true);
    options.create(true);
    options.open(&Path::new(filename))
}

fn sync_password_store(
    store: &mut PasswordStore,
    file: &mut File,
    io: &mut impl CliInputOutput,
) -> Result<(), i32> {
    if let Err(err) = store.sync(file) {
        io.error(
            format!("I could not save the password file (reason: {:?}).", err),
            OutputType::Error,
        );
        return Err(1);
    }

    return Ok(());
}





pub fn main_with_args(
    args: &[&str],
    io: &mut impl CliInputOutput,
    rooster_file_path: &PathBuf,
) -> i32 {
    let matches = Command::new("rooster")
        .help_expected(true)
        .disable_help_subcommand(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .about("Welcome to Rooster, a simple password manager")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::new("init")
                .about("Create a new password file")
                .arg(
                    Arg::new("force-for-tests")
                        .action(ArgAction::SetTrue)
                        .long("force-for-tests")
                        .hide(true)
                        .help("Forces initializing the file, used in integration tests only"),
                ),
        )
        .subcommand( 
            Command::new("gui")
                .about("Start the CLI GUI")
        )
        .subcommand(
            Command::new("add")
                .about("Add a new password manually")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("username")
                        .required(true)
                        .help("Your username for this account"),
                )
                .arg(
                    Arg::new("show")
                        .action(ArgAction::SetTrue)
                        .short('s')
                        .long("show")
                        .help("Show the password instead of copying it to the clipboard"),
                ),
        )
        .subcommand(
            Command::new("change")
                .about("Change a password manually")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("show")
                        .action(ArgAction::SetTrue)
                        .short('s')
                        .long("show")
                        .help("Show the password instead of copying it to the clipboard"),
                ),
        )
        .subcommand(
            Command::new("delete").about("Delete a password").arg(
                Arg::new("app")
                    .required(true)
                    .help("The name of the app (fuzzy-matched)"),
            ),
        )
        .subcommand(
            Command::new("generate")
                .about("Generate a password")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("username")
                        .required(true)
                        .help("Your username for this account"),
                )
                .arg(
                    Arg::new("show")
                        .action(ArgAction::SetTrue)
                        .short('s')
                        .long("show")
                        .help("Show the password instead of copying it to the clipboard"),
                )
                .arg(
                    Arg::new("alnum")
                        .action(ArgAction::SetTrue)
                        .short('a')
                        .long("alnum")
                        .help("Only use alpha numeric (a-z, A-Z, 0-9) in generated passwords"),
                )
                .arg(
                    Arg::new("length")
                        .short('l')
                        .long("length")
                        .default_value("32")
                        .help("Set a custom length for the generated password")
                        .value_parser(validate_arg_usize),
                ),
        )
        .subcommand(
            Command::new("regenerate")
                .about("Regenerate a previously existing password")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("show")
                        .action(ArgAction::SetTrue)
                        .short('s')
                        .long("show")
                        .help("Show the password instead of copying it to the clipboard"),
                )
                .arg(
                    Arg::new("alnum")
                        .action(ArgAction::SetTrue)
                        .short('a')
                        .long("alnum")
                        .help("Only use alpha numeric (a-z, A-Z, 0-9) in generated passwords"),
                )
                .arg(
                    Arg::new("length")
                        .short('l')
                        .long("length")
                        .default_value("32")
                        .help("Set a custom length for the generated password")
                        .value_parser(validate_arg_usize),
                ),
        )
        .subcommand(
            Command::new("get")
                .about("Retrieve a password")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("show")
                        .action(ArgAction::SetTrue)
                        .short('s')
                        .long("show")
                        .help("Show the password instead of copying it to the clipboard"),
                ),
        )
        .subcommand(
            Command::new("rename")
                .about("Rename the app for a password")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The current name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("new_name")
                        .required(true)
                        .help("The new name of the app"),
                ),
        )
        .subcommand(
            Command::new("transfer")
                .about("Change the username for a password")
                .arg(
                    Arg::new("app")
                        .required(true)
                        .help("The current name of the app (fuzzy-matched)"),
                )
                .arg(
                    Arg::new("new_username")
                        .required(true)
                        .help("Your new username for this account"),
                ),
        )
        .subcommand(Command::new("list").about("List all apps and usernames"))
        .subcommand(
            Command::new("import")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .about("Import all your existing passwords from elsewhere")
                .subcommand(
                    Command::new("json")
                        .about("Import a file generated with `rooster export json`")
                        .arg(
                            Arg::new("path")
                                .required(true)
                                .help("The path to the file you want to import"),
                        ),
                )
                .subcommand(
                    Command::new("csv")
                        .about("Import a file generated with `rooster export csv`")
                        .arg(
                            Arg::new("path")
                                .required(true)
                                .help("The path to the file you want to import"),
                        ),
                )
                .subcommand(
                    Command::new("1password")
                        .about("Import a \"Common Fields\" CSV export from 1Password")
                        .arg(
                            Arg::new("path")
                                .required(true)
                                .help("The path to the file you want to import"),
                        ),
                ),
        )
        .subcommand(
            Command::new("export")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .about("Export raw password data")
                .subcommand(Command::new("json").about("Export raw password data in JSON format"))
                .subcommand(Command::new("csv").about("Export raw password data in CSV format"))
                .subcommand(
                    Command::new("1password")
                        .about("Export raw password data in 1Password compatible CSV format"),
                ),
        )
        .subcommand(Command::new("set-master-password").about("Set your master password"))
        .subcommand(
            Command::new("set-scrypt-params")
                .about("Set the key derivation parameters")
                .arg(
                    Arg::new("log2n")
                        .required(true)
                        .help("The log2n parameter")
                        .value_parser(validate_arg_u8),
                )
                .arg(
                    Arg::new("r")
                        .required(true)
                        .help("The r parameter")
                        .value_parser(validate_arg_u32),
                )
                .arg(
                    Arg::new("p")
                        .required(true)
                        .help("The p parameter")
                        .value_parser(validate_arg_u32),
                )
                .arg(
                    Arg::new("force")
                        .action(ArgAction::SetTrue)
                        .short('f')
                        .long("force")
                        .help("Disable parameter checks"),
                ),
        )
        .get_matches_from(args);

    let subcommand = matches.subcommand_name().unwrap();

    let command_matches = matches.subcommand_matches(subcommand).unwrap();

    if subcommand == "init" {
        match commands::init::callback_exec(command_matches, io, rooster_file_path) {
            Err(i) => return i,
            _ => return 0,
        }
    }

    let password_file_path_as_string = rooster_file_path.to_string_lossy().into_owned();

    if !rooster_file_path.exists() {
        io.title("First time user", OutputType::Standard);
        io.nl(OutputType::Standard);
        io.info("Try `rooster init`.", OutputType::Standard);
        io.nl(OutputType::Standard);
        io.title("Long time user", OutputType::Standard);
        io.nl(OutputType::Standard);
        io.info(
            "Set the ROOSTER_FILE environment variable. For instance:",
            OutputType::Standard,
        );
        io.info(
            format!("    {}", example_environment_variable_configuration()),
            OutputType::Standard,
        );
        return 1;
    }

    let mut file = match open_password_file(password_file_path_as_string.deref()) {
        Ok(file) => file,
        Err(err) => {
            match err.kind() {
                std::io::ErrorKind::NotFound => {
                    io.error(
                        "Woops, I can't find your password file. Run `rooster init` to create one.",
                        OutputType::Error,
                    );
                }
                _ => {
                    io.error(
                        format!(
                            "Woops, I couldn't read your password file ({} for \"{}\").",
                            err, password_file_path_as_string
                        ),
                        OutputType::Error,
                    );
                }
            }
            return 1;
        }
    };

    
    if subcommand == "gui" {
        if !cfg!(feature = "gui") {
            io.error("GUI feature is not enabled. Please install Rooster with the `gui` feature enabled.", OutputType::Error);
            return 1;
        } else {
            #[cfg(feature = "gui")]
            match gui::gui::run_gui() {
                Ok(_) => return 0, // TODO: remove this when the GUI is implemented
                Err(_) => return 1,
            }
        }
            
    }

    let mut store = match password_store::get_password_store(&mut file, io) {
        Err(code) => return code,
        Ok(store) => store,
    };
    

    let callback = match subcommand {
        "get" => commands::get::callback_exec,
        "add" => commands::add::callback_exec,
        "delete" => commands::delete::callback_exec,
        "generate" => commands::generate::callback_exec,
        "regenerate" => commands::regenerate::callback_exec,
        "list" => commands::list::callback_exec,
        "import" => commands::import::callback_exec,
        "export" => commands::export::callback_exec,
        "set-master-password" => commands::set_master_password::callback_exec,
        "set-scrypt-params" => commands::set_scrypt_params::callback_exec,
        "rename" => commands::rename::callback_exec,
        "transfer" => commands::transfer::callback_exec,
        "change" => commands::change::callback_exec,
        _ => unreachable!("Validation should have been done by `clap` before"),
    };

    if let Err(code) = callback(command_matches, &mut store, io) {
        return code;
    }

    if let Err(code) = sync_password_store(&mut store, &mut file, io) {
        return code;
    }

    return 0;
}
