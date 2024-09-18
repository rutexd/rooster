use std::{fs::File, io::Read};

use rclio::{CliInputOutput, OutputType};
use rtoolbox::{safe_string::SafeString, safe_vec::SafeVec};

use crate::{password, user_input};

pub fn get_password_store(
    file: &mut File,
    io: &mut impl CliInputOutput,
) -> Result<password::v2::PasswordStore, i32> {
    // Read the Rooster file contents.
    let mut input: SafeVec = SafeVec::new(Vec::new());
    file.read_to_end(input.inner_mut()).map_err(|_| 1)?;

    return get_password_store_from_input_interactive(&input, 3, false, false, io).map_err(|_| 1);
}

pub fn get_password_store_from_input_interactive(
    input: &SafeVec,
    retries: i32,
    force_upgrade: bool,
    retry: bool,
    io: &mut impl CliInputOutput,
) -> Result<password::v2::PasswordStore, password::PasswordError> {
    if retries == 0 {
        io.error(
            "Decryption of your Rooster file keeps failing. \
             Your Rooster file is probably corrupted.",
            OutputType::Error,
        );
        return Err(password::PasswordError::CorruptionLikelyError);
    }

    if retry {
        io.error(
            "Woops, that's not the right password. Let's try again.",
            OutputType::Error,
        );
    }

    let master_password = match user_input::ask_master_password(io) {
        Ok(p) => p,
        Err(err) => {
            io.error(
                format!(
                    "Woops, I could not read your master password (reason: {}).",
                    err
                ),
                OutputType::Error,
            );
            return Err(password::PasswordError::Io(err));
        }
    };

    match get_password_store_from_input(&input, &master_password, force_upgrade) {
        Ok(store) => {
            return Ok(store);
        }
        Err(password::PasswordError::CorruptionError) => {
            io.error("Your Rooster file is corrupted.", OutputType::Error);
            return Err(password::PasswordError::CorruptionError);
        }
        Err(password::PasswordError::OutdatedRoosterBinaryError) => {
            io.error(
                "I could not open the Rooster file because your version of Rooster is outdated.",
                OutputType::Error,
            );
            io.error(
                "Try upgrading Rooster to the latest version.",
                OutputType::Error,
            );
            return Err(password::PasswordError::OutdatedRoosterBinaryError);
        }
        Err(password::PasswordError::Io(err)) => {
            io.error(
                format!("I couldn't open your Rooster file (reason: {:?})", err),
                OutputType::Error,
            );
            return Err(password::PasswordError::Io(err));
        }
        Err(password::PasswordError::NeedUpgradeErrorFromV1) => {
            io.error("Your Rooster file has version 1. You need to upgrade to version 2.\n\nWARNING: If in doubt, it could mean you've been hacked. Only \
                 proceed if you recently upgraded your Rooster installation.\nUpgrade to version 2? [y/n]", OutputType::Error
            );
            loop {
                match io.read_line() {
                    Ok(line) => {
                        if line.starts_with('y') {
                            // This time we'll try to upgrade
                            return get_password_store_from_input_interactive(
                                &input, retries, true, false, io,
                            );
                        } else if line.starts_with('n') {
                            // The user doesn't want to upgrade, that's fine
                            return Err(password::PasswordError::NoUpgradeError);
                        } else {
                            io.error(
                                "I did not get that. Upgrade from v1 to v2? [y/n]",
                                OutputType::Error,
                            );
                        }
                    }
                    Err(io_err) => {
                        io.error(format!(
                                "Woops, an error occured while reading your response (reason: {:?}).",
                                io_err
                            ), OutputType::Error,
                        );
                        return Err(password::PasswordError::Io(io_err));
                    }
                }
            }
        }
        _ => {
            return get_password_store_from_input_interactive(&input, retries - 1, false, true, io);
        }
    }
}

pub fn get_password_store_from_input(
    input: &SafeVec,
    master_password: &SafeString,
    upgrade: bool,
) -> Result<password::v2::PasswordStore, password::PasswordError> {
    // Try to open the file as is.
    match password::v2::PasswordStore::from_input(master_password.clone(), input.clone()) {
        Ok(store) => {
            return Ok(store);
        }
        Err(password::PasswordError::CorruptionError) => {
            return Err(password::PasswordError::CorruptionError);
        }
        Err(password::PasswordError::OutdatedRoosterBinaryError) => {
            return Err(password::PasswordError::OutdatedRoosterBinaryError);
        }
        Err(password::PasswordError::NeedUpgradeErrorFromV1) => {
            if !upgrade {
                return Err(password::PasswordError::NeedUpgradeErrorFromV1);
            }

            // If we can't open the file, we may need to upgrade its format first.
            match password::upgrade(master_password.clone(), input.clone()) {
                Ok(store) => {
                    return Ok(store);
                }
                Err(err) => {
                    return Err(err);
                }
            }
        }
        Err(err) => {
            return Err(err);
        }
    }
}