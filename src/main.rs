pub mod config;
use config::Config;

use std::env;
use console::{style};
use std::io::Read;
use std::path::PathBuf;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error> > {
    //Get the input file path from the arguments
    let css_path = match env::args().nth(1) {
        Some(p) => p,
        //No input path given, print an error and exit
        None    => {
            //Print the error message in red
            println!("{}", style("No input given! Drag and drop a .css theme file onto the executable or pass a path as an argument on the command line.\nEnter any character to exit...\n").red());
            let mut buf = [0;1]; 
            std::io::stdin().read_exact(&mut buf)?; //Read one byte from the input before exiting
            std::process::exit(0);
        }
    };
    let cfg = Config::load(); //Load the configuration toml file or create a default one

    #[cfg(all(target_os="windows"))]
    let mut path: PathBuf = PathBuf::from(format!("{}\\Discord", env::var("LOCALAPPDATA")?)); //Get the path to discord's modules directory

    //Read all directories in discord's module dir and get the latest version
    let dirs = fs::read_dir(&path)?;
    
    //Get the path to the highest version folder of discord and add it to our path
    path.push(
        dirs
        .filter(|entry| entry.as_ref().unwrap().metadata().unwrap().is_dir()) //Filter for only directories in the iterator
        //Take the maximum semver from the directory
        .max_by(|entry: &std::io::Result<fs::DirEntry>, next| {
            //Trim the prefix to the folder to get at the nice semver string
            match ( entry.as_ref().unwrap().file_name().to_str().unwrap().strip_prefix("app-"), next.as_ref().unwrap().file_name().to_str().unwrap().strip_prefix("app-")  ) {
                (Some(version), Some(next_version)) => {
                    //Compare the two semantic versioned folders and determine which is a bigger semver number
                    semver::Version::parse(version).unwrap().cmp(&semver::Version::parse(next_version).unwrap())
                },
                (Some(_), None) => std::cmp::Ordering::Greater, //If the next dir doesn't start with the prefix, we are automatically higher semver
                (None, Some(_)) => std::cmp::Ordering::Less,    //Same but in reverse
                (None, None) => std::cmp::Ordering::Equal,
            }
    }).ok_or(format!("No directories found in Discord data directory"))??.path());

    println!("Got path to Discord highest version: {}", style(path.display()).cyan());
    Ok(())
}
