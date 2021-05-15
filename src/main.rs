pub mod config;
use config::Config;

use std::env;
use console::{style};
use std::io::Read;
use std::path::PathBuf;
use std::fs;

/// Prompt the user to quit the application by entering any character, used to make sure that the program doesn't immediately exit
/// on error
fn prompt_quit(errcode: i32) -> ! {
    println!("Enter any character to exit...");
    let mut buf = [0;1]; 
    std::io::stdin().read_exact(&mut buf).unwrap(); //Read one byte from the input before exiting
    std::process::exit(errcode);
}

fn main() -> Result<(), Box<dyn std::error::Error> > {
    //Get the input file path from the arguments
    let css_path = match env::args().nth(1) {
        Some(p) => p,
        //No input path given, print an error and exit
        None    => {
            //Print the error message in red
            println!("{}", style("No input given! Drag and drop a .css theme file onto the executable or pass a path as an argument on the command line.").red());
            prompt_quit(-1);
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

    println!("Got path to Discords highest version folder: {}", style(path.display()).cyan());
    path.push("modules/discord_desktop_core-1/discord_desktop_core"); //Push the path to the discord core module folder

    //If make_backup is on then make a backup asar file
    if cfg.make_backup {
        let mut backup_path = path.clone();
        backup_path.push("core.asar.backup"); //Add the backup file name to the discord dir
        //Copy the file and write an error message on error
        if let Err(e) = fs::copy(format!("{}/core.asar", path.display()), &backup_path) {
            eprintln!("Failed to make a backup of file {}! Reason {:?}", backup_path.display(), style(e).red()); 
            prompt_quit(-1);
        }
    }


    Ok(())
}
