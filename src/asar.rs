//! The `asar` module provides a way to manipulate Electron's .asar archive file format
//! using the [Archive] struct

use std::{
    collections::HashMap,
    fmt,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
    path::Path,
};

use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Map, Value};

/// The `FileEntry` struct is contained in the [Entry] enum's [File](Entry::File) variant and contains information about a
/// file's location
#[derive(Debug)]
pub struct FileEntry {
    /// The name of the file
    name: String,

    /// The raw bytes of this file
    data: Cursor<Vec<u8>>,
}

impl Write for FileEntry {
    /// Write a certain amount of bytes to our internal buffer
    #[inline(always)]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.write(buf)
    }

    /// This does nothing
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.data.flush()
    }
}

impl Read for FileEntry {
    /// Read a certain amount of bytes from out internal buffer
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.data.read(buf)
    }
}

impl Seek for FileEntry {
    /// Seek to a certain position in the current buffer
    #[inline(always)]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.data.seek(pos)
    }
}

impl FileEntry {
    /// Get the size of this file
    #[inline(always)]
    pub fn size(&self) -> usize {
        self.data.get_ref().len()
    }
}

impl AsRef<[u8]> for FileEntry {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        self.data.get_ref().as_ref()
    }
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

impl DirEntry {
    /// Get the name of this directory
    #[must_use]
    #[inline(always)]
    pub const fn name(&self) -> &String {
        &self.name
    }

    /// Get an iterator over all the files in this directory
    pub fn files(&self) -> impl Iterator<Item = &FileEntry> {
        self.items.iter().filter_map(|(_, f)| match f {
            Entry::File(ref f) => Some(f),
            _ => None,
        })
    }

    /// Get an iterator over all the directories in this directory
    pub fn dirs(&self) -> impl Iterator<Item = &DirEntry> {
        self.items.iter().filter_map(|(_, f)| match f {
            Entry::Dir(ref f) => Some(f),
            _ => None,
        })
    }

    /// Get an iterator over all the files and directories in this directory
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.items.iter().map(|(_, e)| e)
    }
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
        name: &str,
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
                    .ok_or_else(|| {
                        Error::InvalidJsonFormat(format!(
                            "The 'offset' field in file {} is not present",
                            name
                        ))
                    })?
                    .as_str()
                    .ok_or_else(|| {
                        Error::InvalidJsonFormat(format!(
                            "The 'offset' field is present in file entry {}, but is not a string",
                            name
                        ))
                    })?; //Read the string offset
                let offset: u64 = offset.parse::<u64>().map_err(|e| Error::InvalidJsonFormat(format!("The 'offset' field is present and is a string in file {}, but could not be parsed as an integer value: {}", name, e)))? + header_size as u64; //Get the offset as a number, I hate JS
                file.seek(SeekFrom::Start(offset))?; //Seek to the offset of the file's data
                file.read_exact(&mut data)?; //Read the file's bytes from the reader

                Ok(Self::File(FileEntry {
                    name: name.to_owned(),
                    data: Cursor::new(data),
                }))
            }
            //This is a directory, read all child nodes
            _ => Ok(Self::Dir(DirEntry {
                name: name.to_owned(),
                items: obj
                    .get("files")
                    .ok_or_else(|| {
                        Error::InvalidJsonFormat(format!(
                            "The 'files' object for directory {} does not exist",
                            name
                        ))
                    })?
                    .as_object()
                    .ok_or_else(|| {
                        Error::InvalidJsonFormat(format!(
                            "The 'files' field exists for directory {}, but is not an object",
                            name
                        ))
                    })?
                    .iter()
                    .map(|(name, val)| {
                        let object = val.as_object().ok_or_else(|| {
                            Error::InvalidJsonFormat(format!(
                                "The directory {} is present in header JSON but is not an object",
                                name
                            ))
                        })?;
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

    /// Get a file or directory from this entry, returns `None` if `self` is a [File](enum@Entry::File) or if `self` is a [Dir](enum@Entry::Dir) but
    /// has no entry of that name
    pub fn get_entry_mut(&mut self, name: &str) -> Option<&mut Self> {
        match self {
            Self::Dir(DirEntry { name: _, items }) => items.get_mut(name),
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

    /// Get `self` as a [DirEntry] if `self` is a [Dir](Entry::Dir)
    pub fn as_dir(&self) -> Option<&DirEntry> {
        match self {
            Self::Dir(me) => Some(me),
            _ => None,
        }
    }

    /// Get `self` as a [FileEntry] if `self` is a [File](Entry::File)
    pub fn as_file_mut(&mut self) -> Option<&mut FileEntry> {
        match self {
            Self::File(me) => Some(me),
            _ => None,
        }
    }

    /// Get `self` as a [DirEntry] if `self` is a [Dir](Entry::Dir)
    pub fn as_dir_mut(&mut self) -> Option<&mut DirEntry> {
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
            Self::File(file) => write!(f, "{} - size: {}", file.name, file.size()),
            Self::Dir(d) => {
                writeln!(f, "{}", d.name)?;
                d.items
                    .iter()
                    .try_for_each(|(_, entry)| entry.display(offset + 1, f))
            }
        }
    }

    /// Write this `Entry`'s metadata to a header JSON structure, and if this `Entry` is a [File](Entry::File), writing the file's data
    /// to the writer
    fn write<W: Write + Seek>(&self, ar: &mut W, progress: ProgressBar, offset: &mut u32) -> Result<(String, Value), Error> {
        match self {
            Self::Dir(dir) => {
                //Start building a JSON value for this
                let dir_item = json!({
                    "files": dir.items
                    .iter()
                    .map(|(_, entry)| match entry.write(ar, progress.clone(), offset) {
                        Ok(val) => Ok(val),
                        Err(e) => Err(e)
                    })
                    .collect::<Result<HashMap<String, Value>, _>>()?,
                });
                
                Ok((dir.name.clone(), dir_item))
            },
            Self::File(file) => {
                let file_item = json!({
                    "offset": offset.to_string(),
                    "size": file.size()
                }); //Make a JSON item for the 
                *offset += file.size() as u32; //Increment the offset by the amount of bytes written to the vec
                progress.set_message(format!("Archiving file {}", style(&file.name).yellow())); //Set the message 
                ar.write_all(file.as_ref())?; //Write the file data to the buffer
                progress.inc(1);
                Ok((file.name.clone(), file_item))
            }
        }
    }

    /// Get the number of files are contained in the directory if `self` is a directory, or 1 if 
    /// `self` is a file
    pub fn count(&self) -> u32 {
        match self {
            Self::Dir(DirEntry{ name: _, items}) => items.iter().map(|(_, item)| item.count()).sum(),
            Self::File(_) => 1,
        }
    }
}

/// The `Archive` struct contains all information stored in an asar archive file and methods to both unpack
/// an archive into the struct and pack a struct into an archive file.
#[derive(Debug)]
pub struct Archive {
    /// The `data` field contains information like the directory layout and sizes of files
    data: HashMap<String, Entry>,
}

impl Archive {
    /// Open an asar file from the given path and return an `Archive` that contains it as backing storage. Returns errors if any occurred when
    /// parsing the archive or opening the file
    pub fn read<R: Read + Seek>(asar: &mut R) -> Result<Self, Error> {
        //let mut asar = std::fs::OpenOptions::new().read(true).open(path)?; //Open the file from the given path
        Ok(Self {
            data: Self::read_headers(asar)?,
        })
    }

    /// Read two u32s from the beginning 16 bytes, returning the (json size, header size)
    fn read_sizes(read: &mut (impl Read + Seek)) -> Result<(u32, u32), io::Error> {
        read.seek(SeekFrom::Start(0))?;
        let mut buf = [0; 16]; //Make a buffer large enough to hold a two u32s
        read.read_exact(&mut buf)?; //Read bytes to fill the buffer

        //Read the header size first
        let mut header_size = [0u8; 4];
        (&buf[4..8]).read_exact(&mut header_size)?;
        let header_size = u32::from_le_bytes(header_size); //Get a u32 from the bytes

        //Read the json size next
        let mut json_size = [0u8; 4];
        (&buf[12..]).read_exact(&mut json_size)?;
        let json_size = u32::from_le_bytes(json_size); //Get a u32 from the bytes

        //let buf = [buf[4], buf[5], buf[6], buf[7]];
        Ok((json_size, header_size + 8)) //Get a u32 from the data
    }

    /// Read headers from a file and return a hashmap of directories and file data
    fn read_headers<R: Read + Seek>(file: &mut R) -> Result<HashMap<String, Entry>, Error> {
        let (json_size, header_size) = Self::read_sizes(file)?; //Read the header and json size from the file

        file.seek(SeekFrom::Start(16))?; //Skip the rest of the header (why is it 16 bytes?)
        let mut bytes = vec![0u8; json_size as usize]; //Make a vector for reading the json bytes
        file.read_exact(&mut bytes)?; //Read the json into the vector of bytes

        let header: Value = serde_json::from_slice(bytes.as_ref())?; //Parse the header as JSON
        let header = header
            .get("files")
            .ok_or_else(|| {
                Error::InvalidJsonFormat(
                    "The 'files' object in the JSON header is not present".to_owned(),
                )
            })?
            .as_object()
            .ok_or_else(|| {
                Error::InvalidJsonFormat(
                    "The 'files' field is present in the JSON header, but is not an object"
                        .to_owned(),
                )
            })?;
        let mut data = HashMap::new(); //Make a new hashmap for the JSON data
        for (name, val) in header {
            data.insert(
                name.clone(),
                Entry::from_json(
                    name,
                    val.as_object().ok_or_else(|| {
                        Error::InvalidJsonFormat(format!(
                            "Value {} in the header is not a JSON object",
                            name
                        ))
                    })?,
                    file,
                    header_size,
                )?,
            );
        }
        Ok(data)
    }

    /// Get an entry from the given path, used in [get_file] and [get_dir] functions
    fn get_entry(&self, path: impl AsRef<Path>) -> Option<&Entry> {
        let path = path.as_ref();
        match path.parent() {
            Some(dir) if dir.as_os_str().is_empty() => {
                let mut entry = self
                    .data
                    .get(dir.components().next()?.as_os_str().to_str().unwrap())?; //Get the directory at the first path
                                                                                   //Get all the rest of the directories
                for part in dir.components().skip(1) {
                    entry = entry.get_entry(part.as_os_str().to_str().unwrap())?;
                    //Get the directory
                }
                entry.get_entry(path.file_name().unwrap().to_str().unwrap())
            }
            None | Some(_) => self.data.get(path.to_str().unwrap()),
        }
    }

    /// Get a mutable reference to the given entry
    fn get_entry_mut(&mut self, path: impl AsRef<Path>) -> Option<&mut Entry> {
        let path = path.as_ref();
        match path.parent() {
            Some(dir) if dir.as_os_str().is_empty() => {
                let mut entry = self
                    .data
                    .get_mut(dir.components().next()?.as_os_str().to_str().unwrap())?; //Get the directory at the first path
                                                                                       //Get all the rest of the directories
                for part in dir.components().skip(1) {
                    entry = entry.get_entry_mut(part.as_os_str().to_str().unwrap())?;
                    //Get the directory
                }
                entry.get_entry_mut(path.file_name().unwrap().to_str().unwrap())
            }
            None | Some(_) => self.data.get_mut(path.to_str().unwrap()),
        }
    }

    /// Get a [file](FileEntry) using an absolute path
    /// ### Example
    /// ```
    /// # use crate::asar::Archive;
    /// # use std::fs::File;
    /// # fn main() -> Result<(), Box<dyn std::error::Error> {
    /// let ar = Archive::open(File::open("core.asar")?)?; //Open an archive from a file
    /// let ar.get_file("usr/bin/ls").unwrap(); //Open the file
    ///
    /// # }
    ///```
    /// ------
    ///
    /// Do NOT include a root symbol like `/usr/bin` or `C:\Program Files` in the given path
    ///
    #[inline]
    #[must_use]
    pub fn get_file<P: AsRef<Path>>(&self, path: P) -> Option<&FileEntry> {
        self.get_entry(path).map(|e| e.as_file()).flatten()
    }

    /// Get a [directory](DirEntry) using the abosulute path of a directory.
    /// Returns `None` if
    /// - The directory does not exist
    /// - The entry at the given path is not a directory
    #[inline]
    #[must_use]
    pub fn get_dir<P: AsRef<Path>>(&self, path: P) -> Option<&DirEntry> {
        self.get_entry(path).map(|e| e.as_dir()).flatten()
    }

    // Get a mutable reference to a file using a given path
    pub fn get_file_mut<P: AsRef<Path>>(&mut self, path: P) -> Option<&mut FileEntry> {
        self.get_entry_mut(path).map(|e| e.as_file_mut()).flatten()
    }

    // Get a mutable reference to a directory using a given path
    pub fn get_dir_mut<P: AsRef<Path>>(&mut self, path: P) -> Option<&mut DirEntry> {
        self.get_entry_mut(path).map(|e| e.as_dir_mut()).flatten()
    }

    /// Pack this archive's contents into any type implementing `Write` and `Seek`
    /// This will display progress of packing files, then progress of writing the file
    pub fn pack<W: Write + Seek>(&self, ar: &mut W, progressbar: bool) -> Result<(), Error> {
        let mut json = json!({"files": {}}); //Create a new JSON for the header data
        let mut buffer: Cursor<Vec<u8>> = Cursor::new(Vec::new()); //Create a vector to hold the temporarily saved file data

        let num_files: u32 = self.data.iter().map(|(_, e)| e.count()).sum(); //Get the total number of files in the archive

        let progress = match progressbar {
            true => ProgressBar::new(num_files as u64).with_style(ProgressStyle::default_bar().template("{bar} {pos}/{len} - {per_sec} : {msg}")),
            false => ProgressBar::hidden(),
        };
        progress.set_length(num_files as u64); //Set the length of the progress bar

        let mut offset = 0;
        for (_, entry) in self.data.iter() {
            let (name, saved) = entry.write(&mut buffer, progress.clone(), &mut offset)?;
            json["files"][name] = saved; //Write the header JSON
        }

        let mut header = serde_json::to_vec(&json)?; //Save the JSON header as a vector of bytes
        let json_size = header.len(); //Get the size of the JSON 
        let header_size = header.len() + (4 - (header.len() % 4)) % 4; //Get the size of the JSON header and round it up to 4
        header.resize(header_size + 16, 0); //Resize the header to fit the size bytes

        header.rotate_right(16); //Rotate the vec so that the JSON comes after the size bytes
        header[0..4].copy_from_slice(&u32::to_le_bytes(4)); //Copy the size bytes
        header[4..8].copy_from_slice(&u32::to_le_bytes((header_size + 8) as u32)); 
        header[8..12].copy_from_slice(&u32::to_le_bytes((header_size + 4) as u32)); 
        header[12..16].copy_from_slice(&u32::to_le_bytes(json_size as u32)); 

        ar.write_all(header.as_ref())?; //Write the header bytes to the file
        ar.write_all(buffer.into_inner().as_ref())?; //Write the buffer bytes to the file
        Ok(())
    }
}

impl fmt::Display for Archive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (_, entry) in self.data.iter() {
            entry.display(0, f)?;
            writeln!(f)?;
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
    #[allow(unused_imports)]
    use indicatif::ProgressBar;

    //This is a bug, I need to import items for the program to compile
    #[allow(unused_imports)]
    use super::Archive;

    #[test]
    pub fn loading() {
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .open("out.asar")
            .unwrap();
        let asar = Archive::read(&mut file).unwrap();
        println!("{}", asar);
        //println!("File config.rs: {:#?}", asar.get_file("Banner.png"));
        //panic!();
        //std::fs::write("out.png", &asar.get_file("Banner.png").unwrap()).unwrap();

        let mut writer = std::fs::File::create("write.asar").unwrap(); 
        asar.pack(&mut writer, false).unwrap();
    }
}
