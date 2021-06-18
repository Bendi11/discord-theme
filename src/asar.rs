//! The `asar` module provides a way to manipulate Electron's .asar archive file format
//! using the [Archive] struct

use std::{
    collections::HashMap,
    fmt,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

use serde_json::{Map, Value};

/// The `FileEntry` struct is contained in the [Entry] enum's [File](Entry::File) variant and contains information about a
/// file's location
#[derive(Debug)]
pub struct FileEntry {
    /// The name of the file
    name: String,

    /// The raw bytes of this file
    data: Vec<u8>,
}

/// The `DirEntry` struct is contained in the [Dir](Entry::Dir) variant of the [Entry] enum and contains information like contained
/// files and directories and name of the dir
#[derive(Debug)]
pub struct DirEntry {
    /// The name of this directory
    name: String,
    /// The files or directories that this directory contains
    items: HashMap<String, Entry>,
}

/// The `Entry` struct represents one file or directory in an asar archive's header portion
#[derive(Debug)]
pub enum Entry {
    /// The `Dir` variant contains a list of more entries that the directory contains
    Dir(DirEntry),

    /// The `File` variant represents a file with information on how to read the file from an archive like offset and file size
    File(FileEntry),
}

impl Entry {
    /// Read an entry from JSON, either a directory or a file
    pub fn from_json(
        name: &String,
        obj: &Map<String, Value>,
        file: &mut (impl Read + Seek),
        header_size: u32,
    ) -> Result<Self, Error> {
        //See if this is a file by checking for the 'size' item
        match obj.get("size") {
            //This is a file
            Some(Value::Number(size)) => {
                let mut data = vec![0u8; size.as_u64().unwrap() as usize]; //Get a vector of bytes to read the file
                let offset = obj
                    .get("offset")
                    .ok_or(Error::InvalidJsonFormat(format!(
                        "The 'offset' field in file {} is not present",
                        name
                    )))?
                    .as_str()
                    .ok_or(Error::InvalidJsonFormat(format!(
                        "The 'offset' field is present in file entry {}, but is not a string",
                        name
                    )))?; //Read the string offset
                let offset: u64 = offset.parse::<u64>().map_err(|e| Error::InvalidJsonFormat(format!("The 'offset' field is present and is a string in file {}, but could not be parsed as an integer value: {}", name, e)))? + header_size as u64; //Get the offset as a number, I hate JS
                file.seek(SeekFrom::Start(offset))?; //Seek to the offset of the file's data
                file.read_exact(&mut data)?; //Read the file's bytes from the reader

                Ok(Self::File(FileEntry {
                    name: name.clone(),
                    data,
                }))
            }
            //This is a directory, read all child nodes
            _ => Ok(Self::Dir(DirEntry {
                name: name.clone(),
                items: obj
                    .get("files")
                    .ok_or(Error::InvalidJsonFormat(format!("The 'files' object for directory {} does not exist", name)))?
                    .as_object()
                    .ok_or(Error::InvalidJsonFormat(format!("The 'files' field exists for directory {}, but is not an object", name)))?
                    .iter()
                    .map(|(name, val)| {
                        let object = val.as_object().ok_or(Error::InvalidJsonFormat(format!(
                            "The directory {} is present in header JSON but is not an object",
                            name
                        )))?;
                        match Self::from_json(name, object, file, header_size) {
                            Ok(child) => Ok((name.clone(), child)),
                            Err(e) => Err(e),
                        }
                    })
                    .collect::<Result<HashMap<String, Self>, Error>>()?,
            })),
        }
    }

    /// Get a file or directory from this entry, returns `None` if `self` is a [File](enum@Entry::File) or if `self` is a [Dir](enum@Entry::Dir) but
    /// has no entry of that name
    pub fn get_entry(&self, name: &str) -> Option<&Self> {
        match self {
            Self::Dir(DirEntry { name: _, items }) => items.get(name),
            _ => None,
        }
    }

    /// Get `self` as a [FileEntry] if `self` is a [File](Entry::File)
    pub fn as_file(&self) -> Option<&FileEntry> {
        match self {
            Self::File(me) => Some(me),
            _ => None,
        }
    }

    /// Get `self` as a [DirEntry] if `self` is a [File](Entry::Dir)
    pub fn as_dir(&self) -> Option<&DirEntry> {
        match self {
            Self::Dir(me) => Some(me),
            _ => None,
        }
    }

    /// Display this directory or file using the given amount of offset tabs for directories
    fn display(&self, offset: u32, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if offset != 0 {
            [0..offset].iter().try_for_each(|_| writeln!(f, "\t"))?;
            write!(f, "|\n - ")?;
        }
        match self {
            Self::File(file) => write!(f, "{} - size: {}", file.name, file.data.len()),
            Self::Dir(d) => {
                writeln!(f, "{}", d.name)?;
                d.items.iter().try_for_each(|(_, entry)| entry.display(offset + 1, f))
            }
        }
    }
}

/// The `Archive` struct contains all information stored in an asar archive file and methods to both unpack
/// an archive into the struct and pack a struct into an archive. The archive is backed by any type that implements
/// Read and Seek. If the backing storage also implements `Write`, then more methods to write files are also availible.
/// This is commonly a file, and represents the binary data of the asar archive
#[derive(Debug)]
pub struct Archive {
    /// The `data` field contains information like the directory layout and sizes of files
    data: HashMap<String, Entry>,
}

impl Archive {
    /// Open an asar file from the given path and return an `Archive` that contains it as backing storage. Returns errors if any occurred when
    /// parsing the archive or opening the file
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut asar = std::fs::OpenOptions::new().read(true).open(path)?; //Open the file from the given path
        Ok(Self {
            data: Self::read_headers(&mut asar)?,
        })
    }

    /// Read two u32s from the beginning 16 bytes, returning the (json size, header size)
    fn read_sizes(read: &mut (impl Read + Seek)) -> Result<(u32, u32), io::Error> {
        read.seek(SeekFrom::Start(0))?;
        let mut buf = [0; 16]; //Make a buffer large enough to hold a two u32s
        read.read_exact(&mut buf)?; //Read bytes to fill the buffer

        //Read the header size first
        let mut header_size = [0u8 ; 4];
        (&buf[4..8]).read_exact(&mut header_size)?;
        let header_size = u32::from_le_bytes(header_size); //Get a u32 from the bytes

        //Read the json size next
        let mut json_size = [0u8 ; 4];
        (&buf[12..]).read_exact(&mut json_size)?;
        let json_size = u32::from_le_bytes(json_size); //Get a u32 from the bytes

        //let buf = [buf[4], buf[5], buf[6], buf[7]];
        Ok((json_size, header_size + 8)) //Get a u32 from the data
    }

    /// Read headers from a file and return a hashmap of directories and file data
    fn read_headers<R: Read + Sized + Seek>(mut file: R) -> Result<HashMap<String, Entry>, Error> {
        let (json_size, header_size) = Self::read_sizes(&mut file)?; //Read the header and json size from the file

        file.seek(SeekFrom::Start(16))?; //Skip the rest of the header (why is it 16 bytes?)
        let mut bytes = vec![0u8; json_size as usize]; //Make a vector for reading the json bytes
        file.read_exact(&mut bytes)?; //Read the json into the vector of bytes

        let header: Value = serde_json::from_slice(bytes.as_ref())?; //Parse the header as JSON
        let header = header
            .get("files")
            .ok_or(Error::InvalidJsonFormat(
                "The 'files' object in the JSON header is not present".to_owned(),
            ))?
            .as_object()
            .ok_or(Error::InvalidJsonFormat(
                "The 'files' field is present in the JSON header, but is not an object".to_owned(),
            ))?;
        let mut data = HashMap::new(); //Make a new hashmap for the JSON data
        for (name, val) in header {
            data.insert(
                name.clone(),
                Entry::from_json(
                    name,
                    val.as_object().ok_or(Error::InvalidJsonFormat(format!(
                        "Value {} in the header is not a JSON object",
                        name
                    )))?,
                    &mut file,
                    header_size,
                )?,
            );
        }
        Ok(data)
    }

    /// Get a [file](FileEntry) using an abosulute path from the root
    pub fn get_file<P: AsRef<std::path::Path>>(&self, path: P) -> Option<&FileEntry> {
        let path = path.as_ref();
        match path.parent() {
            Some(dir) if dir.as_os_str().len() > 0 => {
                let mut entry = self
                    .data
                    .get(dir.components().next()?.as_os_str().to_str().unwrap())?; //Get the directory at the first path
                                                                                   //Get all the rest of the directories
                for part in dir.components().skip(1) {
                    entry = entry.get_entry(part.as_os_str().to_str().unwrap())?;
                    //Get the directory
                }
                entry
                    .get_entry(path.file_name().unwrap().to_str().unwrap())?
                    .as_file()
            }
            None | Some(_)=> self.data.get(path.to_str().unwrap())?.as_file(),
            _ => None,
        }
    }
}

impl fmt::Display for Archive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (_, entry) in self.data.iter() {
            entry.display(0, f)?;
            write!(f, "\n")?;
        }
        Ok(())
    }
}

/// The `Error` enum represents all errors that can happen when parsing an asar archive
#[derive(Debug)]
pub enum Error {
    /// The header JSON failed to be parsed
    InvalidJson(serde_json::Error),

    /// The JSON is correct, but something is wrong with the format
    InvalidJsonFormat(String),

    /// Read or write error
    IOErr(io::Error),

    /// Invalid UTF8 text in storage
    InvalidUTF8,

    /// The file at the requested asar archive path doesn't exist
    NoFile,
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::InvalidJson(e)
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

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IOErr(err) => write!(f, "IO Error: {}", err),
            Self::InvalidJson(err) => write!(f, "Invalid header JSON: {}", err),
            Self::InvalidJsonFormat(err) => write!(f, "Invalid header JSON format: {}", err),
            Self::InvalidUTF8 => write!(f, "Invalid UTF-8"),
            Self::NoFile => write!(f, "The specified file or directory does not exist"),
        }
    }
}



mod tests {
    use super::*;

    #[test]
    pub fn loading() {
        let asar = Archive::open("out.asar").unwrap();
        println!("{}", asar);
        //println!("File config.rs: {:#?}", asar.get_file("Banner.png"));
        //panic!();
        std::fs::write("out.png", &asar.get_file("Banner.png").unwrap().data).unwrap();
    }
}
