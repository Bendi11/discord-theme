//! The `asar` module provides a way to manipulate Electron's .asar archive file format 
//! using the [Archive] struct

use std::{fs::File, io::{self, Read, Seek, SeekFrom, Write}, path::Path};

/// The `Entry` struct represents one file or directory in an asar archive's header
pub enum Entry {
    /// The `Dir` variant contains a list of more entries that the directory contains
    Dir {
        /// The name of this directory
        name: String,
        /// The files or directories that this directory contains
        items: Vec<Entry>,
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
}




/// The `Archive` struct contains all information stored in an asar archive file and methods to both unpack 
/// an archive into the struct and pack a struct into an archive. The archive is backed by any type that implements 
/// Read, Write, and Seek. This is commonly a file, and represents the binary data of the asar archive
pub struct Archive<T: Read + Write + Seek + Sized> {
    /// The `header` field contains information like the directory layout and sizes of files
    header: Vec<Entry>,

    /// The backing storage that contains all data for the asar archive
    back: T,
}

impl Archive<File> {
    /// Open an asar file from the given path and return an `Archive` that contains it as backing storage. Returns errors if any occurred when
    /// parsing the archive or opening the file
    pub fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let mut asar = std::fs::OpenOptions::new().write(true).read(true).open(path)?; //Open the file from the given path
    }
}

impl<T: Read + Write + Seek + Sized> Archive<T> {
    /// Read one u32 from our backing storage 
    fn read_u32(&mut self) -> Result<u32, io::Error> {
        let mut buf = [0 ; 4]; //Make a buffer large enough to hold a u32
        self.back.read_exact(&mut buf)?; //Read bytes to fill the buffer
        Ok(u32::from_le_bytes(buf)) //Get a u32 from the data
    }

    /// Read all headers from our asar storage and store them in our `header` field
    fn read_headers(&mut self) -> Result<(), Error> {   
        self.back.seek(SeekFrom::Start(0))?; //Seek to the beginning of our storage
        let header_len = self.read_u32()?; //Read the header size from the beginning of our storage
        let header_json = Vec::with_capacity(header_len as usize); //Create a new vector with the given capacity
        unsafe { header_json.set_len(header_len as usize); } //Set the len to the capacity, this means the memory is currently uninitialized, so unsafe
        self.back.read_exact(header_json.as_mut_slice())?; //Read the header bytes from our storage

        let header_json = serde_json::from_slice(header_json.as_slice())?; //Parse the JSON header that shows what files are in the archive

    }
}

/// The `Error` enum represents all errors that can happen when parsing an asar archive
pub enum Error {
    /// The header JSON failed to be parsed
    InvalidJson(serde_json::Error),

    /// Read or write error
    IOErr(io::Error),

    /// Invalid UTF8 text in storage
    InvalidUTF8,
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::InvalidJson(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Self::IOErr(err)
    }
}

impl From<std::str::Utf8Error> for Error {
    fn from(err: std::str::Utf8Error) -> Self {
        Self::InvalidUTF8
    }   
}