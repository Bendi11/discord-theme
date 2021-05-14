use std::fs;

/// The path to the configuration file that we will load options from
const CONFIG_PATH: &'static str = "/config.toml";

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
        
        Self {
            customjs: "".into(),
            make_backup: true,
        }
    } 

    /// Load a configuration file from the `CONFIG_PATH` file or load defaults and create the file
    pub fn load() -> Self {
        match fs::read(CONFIG_PATH) {
            Ok(buf) => {
                let config = match toml::from_slice(buf.as_slice()) { //Make a toml from the file
                    Ok(toml) => toml, //Return the TOML value
                    Err(_)   => return Self::default_file() //Return a default file if there was an error
                };
                

                Self {

                }
            },
            Err(_) => {
                let config = toml::toml!(

                );
                fs::File::create(CONFIG_PATH).unwrap();
            }
        }
    }
}