pub mod config;
use config::Config;

use console::style;
use std::env;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

/// The old CSS theme to insert if no input is given to the exe
#[cfg(not(feature = "autoupdate"))]
const OLD_THEME: &str = include_str!("../old.css");

/// The old URL to download the most recent old.css file from
#[cfg(feature = "autoupdate")]
const OLD_URL: &str = "https://raw.githubusercontent.com/Bendi11/discord-theme/master/old.css";

/// Get the location that Discord was installed to based on the current compilation target and navigate to the highest discord version folder's
/// core module folder
fn get_discord_dir() -> PathBuf {
    #[cfg(all(target_os = "windows"))]
    let mut path = PathBuf::from(format!(
        "{}\\Discord",
        env::var("LOCALAPPDATA")
            .expect("LOCALAPPDATA environment variable not present... something is wrong")
    )); //Get the path to discord's modules directory

    #[cfg(target_os = "macos")]
    let mut path = PathBuf::from("/Library/Application Support/Discord"); //We already know the path to the discord install directory

    #[cfg(target_os = "linux")]
    let mut path = PathBuf::from("/opt/Discord"); //Get the location of Discord's install

    //Read all directories in discord's module dir and get the latest version
    let dirs = fs::read_dir(&path).expect("Failed to read discord data directory!");

    //Get the path to the highest version folder of discord and add it to our path
    path.push(
        dirs.filter(|entry| entry.as_ref().unwrap().metadata().unwrap().is_dir()) //Filter for only directories in the iterator
            //Take the maximum semver from the directory
            .max_by(|entry: &std::io::Result<fs::DirEntry>, next| {
                //Trim the prefix to the folder to get at the nice semver string
                match (
                    entry
                        .as_ref()
                        .unwrap()
                        .file_name()
                        .to_str()
                        .unwrap()
                        .strip_prefix("app-"),
                    next.as_ref()
                        .unwrap()
                        .file_name()
                        .to_str()
                        .unwrap()
                        .strip_prefix("app-"),
                ) {
                    (Some(version), Some(next_version)) => {
                        //Compare the two semantic versioned folders and determine which is a bigger semver number
                        semver::Version::parse(version)
                            .unwrap()
                            .cmp(&semver::Version::parse(next_version).unwrap())
                    }
                    (Some(_), None) => std::cmp::Ordering::Greater, //If the next dir doesn't start with the prefix, we are automatically higher semver
                    (None, Some(_)) => std::cmp::Ordering::Less,    //Same but in reverse
                    (None, None) => std::cmp::Ordering::Equal,
                }
            })
            .ok_or_else(|| "No directories found in Discord data directory".to_owned())
            .unwrap()
            .unwrap()
            .path(),
    );

    println!(
        "Got path to Discords highest version folder: {}",
        style(path.display()).cyan()
    );
    path.push("modules/discord_desktop_core-1/discord_desktop_core"); //Push the path to the discord core module folder
    path
}

/// Prompt the user to quit the application by entering any character, used to make sure that the program doesn't immediately exit
/// on error
fn prompt_quit(errcode: i32) -> ! {
    println!("Enter any character to exit...");
    let mut buf = [0; 1];
    std::io::stdin().read_exact(&mut buf).unwrap(); //Read one byte from the input before exiting
    std::process::exit(errcode);
}

/// Run the discord theme setter main application
fn run() -> Result<(), Box<dyn std::error::Error>> {
    //Set a panic handler for panic printing without weird debug info
    std::panic::set_hook(Box::new(|pinfo: &std::panic::PanicInfo| {
        if let Some(s) = pinfo.payload().downcast_ref::<String>() {
            eprintln!(
                "A fatal error occurred when executing program: {}",
                style(s).red()
            );
        } else if let Some(s) = pinfo.payload().downcast_ref::<&str>() {
            eprintln!(
                "A fatal error occurred when executing program: {}",
                style(s).red()
            );
        } else {
            eprintln!(
                "{}",
                style("An unknown error occurred when executing").red()
            );
        }
        prompt_quit(-1);
    }));

    //Get the input file path from the arguments
    let theme = match env::args().nth(1) {
        //Read the user CSS theme to a string and escape any '`' characters to not mess up CSS insertion
        Some(p) => std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("Failed to read custom theme CSS file: {:?}", e))
            .replace("`", "\\`"),
        //No input path given, print an error and exit
        None => {
            //Print the error message in red
            eprintln!("{}", style("No input given! Drag and drop a .css theme file onto the executable or pass a path as an argument on the command line.").yellow());
            println!("No input was given to the program, would you like to\n1.) Patch Discord to have the old theme\n2.) Reset Discord's theme to factory default from a backup\n3.) Quit the program"); //Prompt the user to reset Discord if no input was given

            let mut input = String::new(); //Make a string to hold the user input
            std::io::stdin().read_line(&mut input).unwrap(); //Read one line from stdin
            match input.trim() {
                //Restore a backup of Discord's asar
                "2" => {
                    let dir = get_discord_dir(); //Get the path to Discord
                                                 //Get the path to both the backup and archive files
                    let (backup, real) = (dir.join("core.asar.backup"), dir.join("core.asar"));
                    //If the file doesn't exist then print an error and prompt the user to quit
                    if !backup.exists() {
                        eprintln!("Discord backup file {} doesn't exist, if you want to revert Discord to factory defaults uninstall and then reinstall it", backup.display());
                        prompt_quit(-1);
                    }

                    let _ = fs::remove_file(&real); //Remove the original asar file if it exists
                                                    //Copy the backup file to the real file name, don't rename because that would effectively delete the original backup
                    if let Err(e) = fs::copy(&backup, real) {
                        eprintln!("Failed to restore backup file {} with error {}, reinstall Discord to restore factory default settings", backup.display(), e);
                        prompt_quit(-1);
                    }

                    //Print that the operation was good and the backup was restored
                    println!("{}", style("Restored backup file successfully").green());
                    prompt_quit(0);
                }
                "1" => {
                    //Download the most recent old.css file from github if the feature is enabled
                    #[cfg(feature = "autoupdate")]
                    println!(
                        "{}",
                        style(format!("Downloading latest old theme from {}", OLD_URL)).blue()
                    );

                    //Download the newest version of the theme
                    #[cfg(feature = "autoupdate")]
                    let text = ureq::get(OLD_URL)
                        .call()
                        .unwrap_or_else(|e| panic!("Failed to download newest old theme from {} with error: {}", OLD_URL, e))
                        .into_string()
                        .unwrap_or_else(|e| panic!("Failed to get text response from {} when downloading newest theme: {}", OLD_URL, e));
                    #[cfg(feature = "autoupdate")]
                    println!(
                        "{}",
                        style("Downloaded newest version of theme successfully").green()
                    );

                    //Otherwise just return the old.css that this was compiled with
                    #[cfg(not(feature = "autoupdate"))]
                    let text = OLD_THEME.to_owned();
                    //Return the text that was returned based on conditional compilation
                    text
                } //Return the default old theme CSS string
                _ => std::process::exit(0), //Exit the program if the user doesn't want to roll back changes or set the old theme
            }
        }
    };

    let cfg = Config::load(); //Load the configuration toml file or create a default one

    //Make a css injection javascript
    let css = format!(
        "
    mainWindow.webContents.on('dom-ready', () => {{
        mainWindow.webContents.executeJavaScript(`
            let CSS_INJECTION_USER_CSS =  \\`{css}\\`;
            const style = document.createElement('style');
            style.innerHTML = CSS_INJECTION_USER_CSS;
            document.head.appendChild(style);
            
            //JS_SCRIPT_BEGIN
            {js}
            //JS_SCRIPT_END
        `);
    }});mainWindow.webContents.
    ",
        css = theme,
        js = cfg.customjs
    );

    let mut path = get_discord_dir();

    //If make_backup is on then make a backup asar file
    if cfg.make_backup {
        let mut backup_path = path.clone();
        backup_path.push("core.asar.backup"); //Add the backup file name to the discord dir
                                              //If the path already exists, then don't overwrite the backup
        if backup_path.exists() {
            println!("Discord backup file {} already exists, not creating a new backup that overrides the old one", backup_path.display());
        }
        // Otherwise create a backup file
        else {
            //Copy the file and write an error message on error
            if let Err(e) = fs::copy(format!("{}/core.asar", path.display()), &backup_path) {
                eprintln!(
                    "Failed to make a backup of file {}! Reason {:?}",
                    backup_path.display(),
                    style(e).red()
                );
                prompt_quit(-1);
            }
        }
    }

    path.push("core.asar"); //Push the core file name to the path

    //Unpack the asar archive
    rasar::extract(path.to_str().unwrap(), "./coreasar")?;

    //Make a path to the unpacked js file
    let main_file = PathBuf::from("./coreasar/app/mainScreen.js");

    //Open the asar electron archive in a buffered reader
    let mut js = BufReader::new(
        fs::OpenOptions::new()
            .read(true)
            .open(&main_file)
            .unwrap_or_else(|e| {
                panic!(
                    "Failed to open discord asar file from {} Error: {:?}",
                    main_file.display(),
                    e
                )
            }),
    );

    let mut jsstr = Vec::new();
    js.read_to_end(&mut jsstr)?; //Read the file into a string for string replacement
    let mut jsstr = unsafe { String::from_utf8_unchecked(jsstr) }; //Turn the bytes into an ASCII string

    //If the injection string is already in the asar archive then don't replace anything but the user CSS
    match jsstr.find("CSS_INJECTION_USER_CSS") {
        //The CSS string is already present, replace the CSS
        Some(mut idx) => {
            println!("{}", style("CSS injection string already present, replacing contents with new CSS theme...").yellow()); //Print that we already did this once

            //Get to the index of the first string quote
            let begin = loop {
                //If we reached the ES6 raw string literal return the idx
                if jsstr.get(idx..idx + 1).unwrap() == "`" {
                    idx += 1;
                    break idx;
                }
                idx += 1;
            };
            let end = loop {
                //If we reached the ES6 raw string literal return the idx
                if jsstr.get(idx..idx + 1).unwrap() == "`" {
                    idx += 1;
                    break idx;
                }
                idx += 1;
            };

            jsstr.replace_range((begin)..(end - 2), &theme); //Replace the user CSS with the new user CSS

            let mut idx = jsstr.find("//JS_SCRIPT_BEGIN").expect(
                "Failed to get JS injection string, please reset Discord and re-apply theme",
            );
            idx += "//JS_SCRIPT_BEGIN\n".len(); //Increment the index to go past the end of the JS_SCRIPT_BEGIN string
                                                //Get to the index of the first string quote
            let begin = idx;
            let end = jsstr
                .find("//JS_SCRIPT_END")
                .expect("Failed to find JS injection terminator, please reset and re-apply theme");

            jsstr.replace_range((begin)..(end), &cfg.customjs); //Replace the JS script path with the new custom JS
        }
        //If there is no injection string then replace the strings with an injection string
        None => {
            //Replace the string with the CSS injection string inserted
            jsstr = jsstr.replacen("mainWindow.webContents.", &css, 1);
            println!("{}", style("Added user CSS theme to Discord!").green()); //Print the success message
        }
    }

    let mut asar = BufWriter::new(fs::File::create(main_file)?); //Open a new buffer writer to write the contents of the file again
    asar.write_all(jsstr.as_bytes())?; //Write all bytes to the file

    drop(asar);
    drop(js);
    println!(
        "{}",
        style("Successfully inserted user CSS into Discord!").green()
    );
    rasar::pack("./coreasar", path.to_str().unwrap())?; //Re pack the archive to discord
    prompt_quit(0);
}

fn main() {
    run().unwrap()
}
