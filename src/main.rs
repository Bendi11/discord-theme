pub mod config;
use std::env;
use console::{style};
use std::io::Read;

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



    Ok(())
}
