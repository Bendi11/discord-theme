use std::fs;

use console::style;

/// The path to the configuration file that we will load options from
const CONFIG_PATH: &'static str = "config.toml";

/// The `Config` struct holds all configuration options given as a .toml file to the 
/// program, or default values.
pub struct Config {
    /// The custom javascript to run along with the css injection; only for people who know what they're doing
    pub customjs: String,
    /// Wether or not to make a backup of the original electron .asar file
    pub make_backup: bool,
}

impl Config {
    /// Create a default config file with default values and return a default instance of self
    fn default_file() -> Self {
        let toml = toml::toml!{
            custom-js = ""
            make-backup = true
        };
        //Write the TOML configuration to the default file location
        std::fs::write(CONFIG_PATH, toml::to_vec(&toml).unwrap()).unwrap();
        Self {
            customjs: "".into(),
            make_backup: true,
        }
    } 

    /// Load a configuration file from the `CONFIG_PATH` file or load defaults and create the file
    pub fn load() -> Self {
        match fs::read_to_string(CONFIG_PATH) {
            Ok(buf) => {
                let config = match buf.parse::<toml::Value>() { //Make a toml from the file's contents
                    Ok(toml) => toml, //Return the TOML value
                    Err(e)   => {
                        eprintln!("{} {}", style("Failed to parse config.toml, switching to default file...").red(), e);
                        return Self::default_file()
                     } //Return a default file if there was an error
                };
                

                Self {
                    customjs: config["custom-js"].as_str().unwrap_or("").to_owned(), //Set an empty custom Javascript string
                    make_backup: config["make-backup"].as_bool().unwrap_or(false), //Get wether or not to make a backup of the electron file
                }
            },
            Err(_) => {
                Self::default_file() //Create the default file and return the defualt instance of Self
            }
        }
    }
}