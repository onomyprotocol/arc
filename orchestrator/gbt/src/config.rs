//! Handles configuration structs + saving and loading for Gravity bridge tools

use std::{
    fs::{self, create_dir},
    path::{Path, PathBuf},
};

use gravity_utils::{
    error::GravityError,
    types::{GravityBridgeToolsConfig, TomlGravityBridgeToolsConfig},
};

use crate::args::InitOpts;

/// The name of the config file, this file is copied
/// from default-config.toml when generated so that we
/// can include comments
pub const CONFIG_NAME: &str = "config.toml";
/// The folder name for the config
pub const CONFIG_FOLDER: &str = ".gbt";

/// Creates the config directory and default config file if it does
/// not already exist
pub fn init_config(_init_ops: InitOpts, home_dir: PathBuf) -> Result<(), GravityError> {
    if home_dir.exists() {
        warn!(
            "The Gravity bridge tools config folder {} already exists!",
            home_dir.to_str().unwrap()
        );
        warn!("You can delete this folder and run init again, you will lose any keys or other config data!");
        Err(GravityError::ValidationError(
            "Directory already exists".into(),
        ))
    } else {
        create_dir(home_dir.clone()).expect("Failed to create config directory!");

        fs::write(home_dir.join(CONFIG_NAME), get_default_config())
            .expect("Unable to write config file");

        Ok(())
    }
}

/// Loads the default config from the default-config.toml file
/// done at compile time and is included in the binary
/// This is done so that we can have hand edited and annotated
/// config
fn get_default_config() -> String {
    include_str!("default-config.toml").to_string()
}

pub fn get_home_dir(home_arg: Option<PathBuf>) -> Result<PathBuf, GravityError> {
    match (dirs::home_dir(), home_arg) {
        (_, Some(user_home)) => Ok(PathBuf::from(&user_home)),
        (Some(default_home_dir), None) => Ok(default_home_dir.join(CONFIG_FOLDER)),
        (None, None) => {
            Err(GravityError::UnrecoverableError(
                "Failed to automatically determine your home directory, please provide a path to the --home argument!".into(),
            ))
        }
    }
}

/// Load the config file, this operates at runtime
pub fn load_config(home_dir: &Path) -> Result<GravityBridgeToolsConfig, GravityError> {
    let config_file = home_dir.join(CONFIG_FOLDER).with_file_name(CONFIG_NAME);
    if !config_file.exists() {
        return Ok(GravityBridgeToolsConfig::default());
    }

    let config =
        fs::read_to_string(config_file).expect("Could not find config file! Run `gbt init`");
    let val: Result<TomlGravityBridgeToolsConfig, _> = toml::from_str(&config);
    match val {
        Ok(v) => Ok(v.into()),
        Err(e) => Err(GravityError::UnrecoverableError(format!(
            "Invalid config! {e:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that the config is both valid toml for the struct and that it's values are
    /// equal to the default values of the config.
    #[test]
    fn test_default_config() {
        // make sure the default config default-config.toml is the same as the default config struct
        let res: TomlGravityBridgeToolsConfig = toml::from_str(&get_default_config()).unwrap();
        let res: GravityBridgeToolsConfig = res.into();
        assert_eq!(res, GravityBridgeToolsConfig::default());
    }
}
