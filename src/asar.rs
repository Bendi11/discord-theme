//! The `asar` module provides a way to manipulate Electron's .asar archive file format 
//! using the [Archive] struct

use std::{collections::HashMap, fs::File, io::{self, Read, Seek, SeekFrom, Write}, path::Path};

/// The `Entry` struct represents one file or directory in an asar archive's header
pub enum Entry {
    /// The `Dir` variant contains a list of more entries that the directory contains
    Dir {
        /// The name of this directory
        name: String,
        /// The files or directories that this directory contains
        items: HashMap<String, Entry>,
    },

    /// The `File` variant represents a file with information on how to read the file from an archive like offset and file size
    File {
        /// The name of the file
        name: String,
        /// The offset into the archive file that we must seek to before reading the file
        off: u64,

        /// The size of the file in bytes
        size: usize,
    },
}

impl Entry {
    /// Check if this Entry is a directory
    pub const fn is_dir(&self) -> bool {
        matches!(self, Self::Dir{name: _, items: _})
    }

    /// Check if this Entry is a file
    pub const fn is_file(&self) -> bool {
        matches!(self , Self::File{name: _, off: _, size: _})
    }

    /// Return either a file if the Value has both an `offset` and `size` field, or a Dir if the Value is a map of file names to more entries. 
    /// This is recursive for directories, so calling `from_json` on a dir also populates the dirs entries with data
    pub fn from_json(name: String, json: &serde_json::Value) -> Result<Self, &'static str> {
        //Get the json value 
        let json = match json.as_object() {
            Some(obj) => obj,
            None => return Err("The json value passed is not an object"),
        };


        //Check if this is a file or not judging by the size and offset fields
        Ok( match ( json.get("size"), json.get("offset") ) {
            //Make sure that the offset is a string and the size is a javascript number
            (Some(size), Some(off)) if size.is_number() && off.is_string() => Self::File {
                name,
                off: off.as_str().unwrap().parse().unwrap(),
                size: size.as_u64().unwrap() as usize,
            },
            //Otherwise it must be a directory
            _ => Self::Dir {
                name,
                //Recursively parse headers for all items in this directory
                items: {
                    let mut err = Ok(()); //The error that occurred when parsing our descendant nodes
                    let items = json.into_iter().scan(&mut err , |err, (name, val)| {
                        //Attempt to parse our children header nodes and check for errors
                        match Self::from_json(name.clone(), val) {
                            Ok(contained) => Some((name.clone(), contained)),
                            Err(e) => {
                                **err = Err(e); //Set the error level
                                None //Stop parsing
                            }
                        }
                    } ).collect(); 
                    err?;
                    items
                }
            }
        } )
    }
}




/// The `Archive` struct contains all information stored in an asar archive file and methods to both unpack 
/// an archive into the struct and pack a struct into an archive. The archive is backed by any type that implements 
/// Read and Seek. If the backing storage also implements `Write`, then more methods to write files are also availible.
/// This is commonly a file, and represents the binary data of the asar archive
pub struct Archive<T: Read + Seek + Sized> {
    /// The `header` field contains information like the directory layout and sizes of files
    header: HashMap<String, Entry>,

    /// The backing storage that contains all data for the asar archive
    back: T,
}

impl Archive<File> {
    /// Open an asar file from the given path and return an `Archive` that contains it as backing storage. Returns errors if any occurred when
    /// parsing the archive or opening the file
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let asar = std::fs::OpenOptions::new().write(true).read(true).open(path)?; //Open the file from the given path
        let mut me = Self {
            back: asar,
            header: HashMap::new()
        };
        me.read_headers()?; //Read the header values and store them
        Ok(me)
    }
}

impl<T: Read + Seek + Sized> Archive<T> {
    /// Read one u32 from our backing storage, consuming 4 bytes from it
    fn read_u32(&mut self) -> Result<u32, io::Error> {
        let mut buf = [0 ; 4]; //Make a buffer large enough to hold a u32
        self.back.read_exact(&mut buf)?; //Read bytes to fill the buffer
        Ok(u32::from_le_bytes(buf)) //Get a u32 from the data
    }

    /// Read all headers from our asar storage and store them in our `header` field
    fn read_headers(&mut self) -> Result<(), Error> {   
        self.back.seek(SeekFrom::Start(0))?; //Seek to the beginning of our storage
        let header_len = self.read_u32()?; //Read the header size from the beginning of our storage
        let mut header_json = Vec::with_capacity(header_len as usize); //Create a new vector with the given capacity
        unsafe { header_json.set_len(header_len as usize); } //Set the len to the capacity, this means the memory is currently uninitialized, so unsafe
        self.back.read_exact(header_json.as_mut_slice())?; //Read the header bytes from our storage

        let header_json: serde_json::Value = serde_json::from_slice(header_json.as_slice())?; //Parse the JSON header that shows what files are in the archive
        if !header_json.is_object() {
            return Err(Error::InvalidJson)
        }

        let mut err = Ok(());
        //Parse all headers from the archive file
        self.header = header_json.as_object().unwrap().into_iter().scan(&mut err, |err, (name, val)| {
            match Entry::from_json(name.clone(), val) {
                Ok(ent) => Some( (name.clone(), ent) ),
                Err(_) => {
                    **err = Err( Error::InvalidJson );
                    None
                }
            }
        }).collect();
        err?; //Check if we had any errors parsing headers
        Ok(())
    }
}

/// The `Error` enum represents all errors that can happen when parsing an asar archive
pub enum Error {
    /// The header JSON failed to be parsed
    InvalidJson,

    /// Read or write error
    IOErr(io::Error),

    /// Invalid UTF8 text in storage
    InvalidUTF8,

}

impl From<serde_json::Error> for Error {
    fn from(_: serde_json::Error) -> Self {
        Self::InvalidJson
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IOErr(err)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(_: std::str::Utf8Error) -> Self {
        Self::InvalidUTF8
    }   
}