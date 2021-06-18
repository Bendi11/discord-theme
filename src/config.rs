use std::fs;

use console::style;

/// The path to the configuration file that we will load options from
const CONFIG_PATH: &str = "config.toml";

/// The `Config` struct holds all configuration options given as a .toml file to the
/// program, or default values.
pub struct Config {
    /// The custom javascript to run along with the css injection; only for people who know what they're doing
    pub customjs: String,
    /// Wether or not to make a backup of the original electron .asar file
    pub make_backup: bool,

    /// Wether to attempt to replace Discord's desktop icon or not
    pub replace_icon: bool,
}

impl Config {
    /// Create a default config file with default values and return a default instance of self
    fn default_file() -> Self {
        let toml = toml::toml! {
            custom-js = ""
            make-backup = true
            replace-icon = true
        };
        //Write the TOML configuration to the default file location
        std::fs::write(CONFIG_PATH, toml::to_vec(&toml).unwrap()).unwrap();
        Self {
            customjs: "".into(),
            make_backup: true,
            replace_icon: true,
        }
    }

    /// Load a configuration file from the `CONFIG_PATH` file or load defaults and create the file
    pub fn load() -> Self {
        match fs::read_to_string(CONFIG_PATH) {
            Ok(buf) => {
                let config =
                    match buf.parse::<toml::Value>() {
                        //Make a toml from the file's contents
                        Ok(toml) => toml, //Return the TOML value
                        Err(e) => {
                            eprintln!(
                            "{} {}",
                            style("Failed to parse config.toml, switching to default file. Error: ")
                                .red(),
                            e
                        );
                            return Self::default_file();
                        } //Return a default file if there was an error
                    };

                Self {
                    //Get the custom javascript and escape the '`' character so that javascript insertion is not messed up
                    customjs: config
                        .get("custom-js")
                        .unwrap_or(&toml::Value::String("".to_owned()))
                        .as_str()
                        .unwrap_or("")
                        .replace("`", "\\`") //Escape any characters that would mess up Discord's files
                        .replace("\\", "\\\\"),
                    make_backup: config
                        .get("make-backup")
                        .unwrap_or(&toml::Value::Boolean(true))
                        .as_bool()
                        .unwrap_or(true), //Get wether or not to make a backup of the electron file
                    replace_icon: config
                        .get("replace-icon")
                        .unwrap_or(&toml::Value::Boolean(true))
                        .as_bool()
                        .unwrap_or(true),
                }
            }
            Err(_) => {
                Self::default_file() //Create the default file and return the defualt instance of Self
            }
        }
    }
}
