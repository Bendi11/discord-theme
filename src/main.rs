pub mod config;
use config::Config;

use console::style;
use console::Color;
use console::Style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Select;
#[cfg(target_os = "linux")]
use dialoguer::{Attribute, Input};
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use std::env;
use std::fs;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

/// The old CSS theme to insert if no input is given to the exe
#[cfg(not(feature = "autoupdate"))]
const OLD_THEME: &str = include_str!("../old.css");

/// The icon file that we will swap with Discord's new one, this is Windows-specific
#[cfg(target_os = "windows")]
const OLD_ICON: &[u8] = include_bytes!("../assets/old.ico");

/// The old icon file in png format because linux uses normal images for icons
#[cfg(not(target_os = "windows"))]
const OLD_ICON: &[u8] = include_bytes!("../assets/old.png");

/// The name of Discord's icon file name
#[cfg(target_os = "windows")]
const ICON_NAME: &str = "app.ico";

/// The non-windows discord icon file name
#[cfg(not(target_os = "windows"))]
const ICON_NAME: &str = "discord.png";

/// The old URL to download the most recent compressed old.css file from
#[cfg(feature = "autoupdate")]
const OLD_URL: &str = "https://raw.githubusercontent.com/Bendi11/discord-theme/master/assets/old-compressed.css";

/// Get the highest-level discord installation directory, not into a specific version folder, but to the root folder containing all of the
/// versioned folders. This is kept separate from the [get_discord_dir] function because we need the root folder when replacing the Discord icon
fn get_discord_root() -> PathBuf {
    #[cfg(all(target_os = "windows"))]
    let path = PathBuf::from(format!(
        "{}\\Discord",
        env::var("LOCALAPPDATA")
            .expect("LOCALAPPDATA environment variable not present... something is wrong")
    )); //Get the path to discord's modules directory

    #[cfg(target_os = "macos")]
    let path = PathBuf::from("/Library/Application Support/Discord"); //We already know the path to the discord install directory

    //Make a prompt to request Discord's intstallation path if on linux, because it could be installed in many locations
    #[cfg(target_os = "linux")]
    let path = PathBuf::from(
        Input::with_theme(&ColorfulTheme {
            prompt_style: Style::default().attr(Attribute::Italic).fg(Color::Yellow),
            error_style: Style::default().attr(Attribute::Bold).fg(Color::Red),
            ..Default::default()
        }).with_prompt("Please enter the directory that Discord is installed to (where the 'Discord') binary is located)...").validate_with(|val: &String| {
            let entered = PathBuf::from(val); //Create a path from the string
            match entered.exists() {
                true => match entered.is_dir() {
                    true => Ok(()),
                    false => Err("The entered path exists but is not a directory: try removing the file name from the path"),
                },
                false => Err("The entered directory does not exist or the application is unable to access it")
            }
        }).interact().unwrap_or_else(|e| panic!("Unable to read input from a query: {}", e))
    );

    path
}

/// Get the location that Discord was installed to based on the current compilation target and navigate to the highest discord version installed
fn get_discord_dir(mut root: PathBuf) -> PathBuf {
    //Read all directories in discord's module dir and get the latest version
    let dirs = fs::read_dir(&root).unwrap_or_else(|_| {
        panic!(
            "Failed to read Discord's installation directory from {}, does it exist?",
            root.display()
        )
    });

    //Get the path to the highest version folder of discord and add it to our path
    root.push(
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
        style(root.display()).cyan()
    );

    root.push("modules/discord_desktop_core-1/discord_desktop_core"); //Push the path to the discord core module folder
    root
}

/// Replace the `app.ico` on windows or `app.png` on linux / mac with the old blurple clyde icon that is embedded in this executable
#[inline]
fn replace_icon(root: &std::path::Path) -> Result<(), std::io::Error> {
    //Overwrite the icon file
    std::fs::write(root.join(ICON_NAME), OLD_ICON)
}

/// Prompt the user to quit the application by entering any character, used to make sure that the program doesn't immediately exit
/// on error
fn prompt_quit(errcode: i32) -> ! {
    //Render a dialog based on the error code (non-zero means error)
    println!(
        "{}",
        match errcode != 0 {
            true => style("Enter any character to exit...").red().bold(),
            false => style("Enter any character to exit...").bold().bright(),
        }
    );
    if console::user_attended() {
        let _ = console::Term::stdout().read_key();
    }
    std::process::exit(errcode);
}

/// Create a backup of Discord's data core.asar file and return any errors that occurred. Because making a backup is deemed important,
/// this function will `panic` instead of returning a `Result`. This is the default behavior, but if the user wants they can edit the config file and turn
/// backups off.
fn make_backup(root: PathBuf, dir: PathBuf) {
    let mut backup_path = dir.clone();
    backup_path.push("core.asar.backup"); //Add the backup file name to the discord dir

    //If the path already exists, then don't overwrite the backup. The reason that we do this instead of overwriting is because we want to keep the original Discord data
    //intact, with no changes from our program.
    if backup_path.exists() {
        println!("Discord backup file {} already exists, not creating a new backup that overrides the old one", backup_path.display());
    }
    // Otherwise create a backup file
    else {
        let mut original = fs::File::open(format!("{}/core.asar", dir.display())).unwrap_or_else(|e| panic!("Failed to open Discord's original core.asar file when creating a backup! Error: {}", e)); //Open the Discord archive file
        let backup = fs::File::create(&backup_path).unwrap_or_else(|e| {
            panic!(
                "Failed to create a backup file for Discord's data! Error: {}",
                e
            )
        }); //Create the backup file

        //Create a progress bar that shows the backup file copying progress
        let copyprog = ProgressBar::new(match original.metadata() {
            Ok(meta) => meta.len(),
            Err(_) => 100,
        }); //Create a progress bar to show backup copy progress
        copyprog.set_style(
            ProgressStyle::default_bar()
                .template("{bar} {bytes}/{total_bytes} - {binary_bytes_per_sec}"),
        );
        copyprog.println("Creating a backup of Discord's files...");

        std::io::copy(&mut original, &mut copyprog.wrap_write(backup)).unwrap_or_else(|e| {
            panic!(
                "Failed to copy Discord's core.asar file to a backup file! Error: {}",
                e
            )
        }); //Wrap the writer in a progress bar and copy the file

        //Copy the file and write an error message on error
        if let Err(e) = fs::copy(format!("{}/core.asar", dir.display()), &backup_path) {
            eprintln!(
                "Failed to make a backup of file {}! Reason {:?}",
                backup_path.display(),
                style(e).red()
            );
            prompt_quit(-1);
        }
    }

    //Create a backup icon file now

    let icon = root.join(ICON_NAME); //Get the discord icon name

    let icon_backup = root.join("icon-backup"); //We store the backup without extension because it doesn't really matter and it allows me to write non platform-specific code
                                                //Only create a backup if there is not a backup there already, this is so that we don't overwrite the old icon backup
    if !icon_backup.exists() {
        //Copy the file to a backup
        match std::fs::copy(icon, icon_backup) {
            Ok(_) => (),
            Err(e) => println!(
                "{}",
                style(format!("Failed to make a backup of Discord's icon: {}", e))
                    .fg(Color::Color256(172))
            ), //Print a warning but don't panic if we couldn't make an icon backup
        }
    }
}

/// Run the discord theme setter main application
fn run() -> Result<(), Box<dyn std::error::Error>> {
    //Set a panic handler for printing error messages cleanly
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

    //Get the input file path from the arguments or let the user select an option
    let theme = match env::args().nth(1) {
        //Read the user CSS theme to a string and escape any '`' characters to not mess up CSS insertion
        Some(p) => std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("Failed to read custom theme CSS file: {:?}", e)),
        //No input path given, ask for either a theme download, backup restoration, or exit
        None => {
            #[cfg(feature = "autoupdate")]
            let patch_text = "Download the latest old theme from Github and apply it do Discord";

            #[cfg(not(feature = "autoupdate"))]
            let patch_text = "Apply the default old theme that the program was compiled with";
            
            
            let selection = Select::with_theme(&ColorfulTheme {
                prompt_style: Style::default().fg(Color::Blue).bold(),
                active_item_style: Style::default().fg(Color::Green),
                active_item_prefix: style(">>".to_owned()).blink(),
                hint_style: Style::default().fg(Color::Color256(252)),

                ..Default::default()
            }).with_prompt("No input given! Drag and drop a .css theme file onto the executable or pass a path as an argument on the command line if you would like to apply a custom css theme, or select an option")
            
            .item(patch_text)
            .item("Reset Discord's theme to factory defaults from a backup file")
            .item("Exit the program")
            .default(0)
            .interact()
            .expect("Failed to take a selection from the menu!");

            match selection {
                //Restore a backup of Discord's asar
                1 => {
                    let root = get_discord_root(); //Get the root folder of Discord by searching or querying
                    let dir = get_discord_dir(root.clone()); //Get the path to Discord
                                                 //Get the path to both the backup and archive files
                    let (backup, real) = (dir.join("core.asar.backup"), dir.join("core.asar"));
                    //If the file doesn't exist then print an error and prompt the user to quit
                    if !backup.exists() {
                        eprintln!("Discord backup file {} doesn't exist, if you want to revert Discord to factory defaults uninstall and then reinstall it", backup.display());
                        prompt_quit(-1);
                    }

                    //Get a progress bar showing how far we are in copying the backup over
                    let rest_prog = ProgressBar::new(match real.metadata() {
                        Ok(m) => m.len(),
                        Err(_) => 100,
                    }).with_style(ProgressStyle::default_bar().template("{bar} {bytes}/{total_bytes} - {binary_bytes_per_sec}: {msg}")).with_message("Restoring backup file...");

                    let _ = fs::remove_file(&real); //Remove the original asar file if it exists

                    //Open the backup file so that we can wrap it in a progress bar
                    let mut backup_file = std::fs::File::open(&backup).unwrap_or_else(|e| panic!("Failed to open Discord backup file at {}: {}", backup.display(), e));

                    let real_file = std::fs::File::create(&real).unwrap_or_else(|e| panic!("Failed to open the file that backup is restoring: {}", e)); //Open the real file that we will be copying the backed-up data to

                    //Copy the backup file to the real file, we copy here instead of moving the file to keep a backup just in case the copy operation fails somehow
                    if let Err(e) = std::io::copy(&mut backup_file, &mut rest_prog.wrap_write(real_file)) {
                        eprintln!("{}", style(format!("Failed to restore backup file {} with error {}, reinstall Discord to restore factory default settings", backup.display(), e)).fg(Color::Red));
                        prompt_quit(-1);
                    }

                    rest_prog.finish_with_message(style("Restored backup file!").green().to_string()); //Finish the progress bar

                    let (iconb, iconr) = (root.join("icon-backup"), root.join(ICON_NAME)); //Get a path to Discord's icon file and backup file
                    if let Err(e) = fs::copy(iconb, iconr) {
                        eprintln!("{}", style(format!("Failed to restore Discord's icon from a backup file at {}: {}", root.join("icon-backup").display(), e)).fg(Color::Color256(172)) ); //Print a warning if the backup was not restored
                    }

                    //Print that the operation was good and the backup was restored
                    println!("{}", style("Restored backup file successfully").green());
                    prompt_quit(0);
                },
                #[cfg(feature = "autoupdate")]
                //Download the most recent version of the theme from github
                0 => {
                    let dlprog = ProgressBar::new_spinner(); //Create a spinner to show download progress
                    dlprog.enable_steady_tick(10);
                    dlprog.set_message(format!("Downloading most recent theme file from {}...", OLD_URL));

                    //Download the newest version of the theme from github
                    let text = ureq::get(OLD_URL)
                        .call()
                        .unwrap_or_else(|e| panic!("Failed to download newest old theme from {} with error: {}", OLD_URL, e))
                        .into_string()
                        .unwrap_or_else(|e| panic!("Failed to get text response from {} when downloading newest theme: {}", OLD_URL, e));

                    dlprog.finish_with_message(style("Downloaded most updated theme file!").green().to_string());

                    //Return the text that was returned based on conditional compilation
                    text
                } ,
                #[cfg(not(feature = "autoupdate"))]
                0 => OLD_THEME.to_owned(),
                //Return the default old theme CSS string
                _ => std::process::exit(0), //Exit the program if the user doesn't want to roll back changes or set the old theme
            }
        }
    }
    .replace("\\", "\\\\") //Escape characters in CSS will mess up Javascript, so escape the escape sequences
    .replace("`", "\\`"); //In ES6 template literals, the only character needing escaping is the backtick. I don't know if CSS will ever have this character but just in case

    let cfg = Config::load(); //Load the configuration toml file or create a default one

    //Make a css injection javascript
    let css = format!(
        "
    mainWindow.webContents.on('dom-ready', () => {{
        mainWindow.webContents.executeJavaScript(`
            let CSS_INJECTION_USER_CSS = String.raw \\`{css}\\`;  
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

    let root = get_discord_root(); //Get the Discord root folder by automatic searching or querying on Linux

    let mut path = get_discord_dir(root.clone()); //Get the path to the highest version Discord installation

    //Replace the icon file if needed
    if cfg.replace_icon {
        if let Err(e) = replace_icon(&root) {
            eprintln!(
                "{}",
                style(format!("Failed to replace Discord's icon file: {}", e))
                    .fg(Color::Color256(172))
            ); //Print a warning but don't fail if the icon couldn't be swapped
        }
    }
    //If make_backup is on then make a backup asar file
    if cfg.make_backup {
        make_backup(root, path.clone());
    }

    path.push("core.asar"); //Push the core file name to the path

    //Create a spinner to show that we are reading Discord's files
    let js_prog = ProgressBar::new_spinner();
    js_prog.set_message("Unpacking Discord's archive files...");
    js_prog.enable_steady_tick(10);

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

    //Finish the first progress bar
    js_prog.finish_with_message(
        style("Unpacked Discord's archive")
            .fg(Color::Green)
            .to_string(),
    );

    //Create a spinner to show that we are doing the search and replace for the custom CSS theme
    let ins_prog = ProgressBar::new_spinner();
    ins_prog.set_message("Inserting CSS theme into Discord's archive...");
    ins_prog.enable_steady_tick(10);

    //If the injection string is already in the asar archive then don't replace anything but the user CSS
    match jsstr.find("CSS_INJECTION_USER_CSS") {
        //The CSS string is already present, replace the CSS
        Some(mut idx) => {
            println!("{}", style("CSS injection string already present, replacing contents with new CSS theme...").yellow()); //Print that we already did this once

            //Get to the index of the first string quote
            let begin = loop {
                //If we reached the ES6 raw string literal return the idx
                if jsstr
                    .get(idx..idx + 1)
                    .ok_or_else(|| panic!("Failed to get the first opening backtick"))
                    .unwrap()
                    == "`"
                {
                    idx += 1;
                    break idx;
                }
                idx += 1;
            };
            let end = loop {
                //If we reached the ES6 raw string literal return the idx
                if jsstr
                    .get(idx..idx + 1)
                    .ok_or_else(|| panic!("Failed to get the closing backtick"))
                    .unwrap()
                    == "`"
                {
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

    ins_prog.finish_with_message("Inserted user CSS into discord's archive");

    //Create a spinner to show that we are re-packing discord's asar file
    let pack_prog = ProgressBar::new(jsstr.len() as u64).with_style(
        ProgressStyle::default_bar()
            .template("{bar} {bytes}/{total_bytes} - {binary_bytes_per_sec}: {msg}"),
    );
    pack_prog.set_message("Re-packing modified Discord archive files...");

    let mainscreenjs = BufWriter::new(fs::File::create(main_file)?); //Open a new buffer writer to write the contents of the file again
    pack_prog
        .wrap_write(mainscreenjs)
        .write_all(jsstr.as_bytes())?; //Write all bytes to the file and track the progress using a progress bar

    pack_prog.finish_with_message(
        style("Re-packed modified Discord archive, restart Discord for the changes to take effect")
            .fg(Color::Green)
            .to_string(),
    );

    drop(pack_prog);
    drop(js);
    rasar::pack("./coreasar", path.to_str().unwrap())?; //Re pack the archive to discord

    prompt_quit(0);
}

fn main() {
    run().unwrap()
}
