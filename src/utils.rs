//! Module with small helper functions

use std::path::{Path, PathBuf};
use std::fs;
use std::os::linux::fs::MetadataExt;
use std::io::{self, BufRead, BufReader};

/// Helper function to move a file, wether it is on the same device  or not.
/// 
/// src_file_path must be an exising file. 
/// dst_path can either end with a filename, in which case the file will be named
/// as such, or a directory name, in which case file will keep the name of 
/// source.
pub fn move_file(src_file_path: &Path, dst_path: &Path) -> io::Result<()> {
    src_file_path.is_file()
        .then(|| ())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "File does not exist"))?;
    
    let dst_dir_path;
    let dst_filename;

    // Case if the provided path is an existing dir
    if dst_path.is_dir() {
        dst_dir_path = dst_path;
        // Can be unwrapped because src_file_path has already been checked to be a file.
        dst_filename = src_file_path.file_name().unwrap();

    // Other case
    } else {
        dst_dir_path = dst_path
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("No directory in {}", dst_path.display())))?; 
        
        // Can be unwrapped because 
        dst_filename = dst_path.file_name()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("No filename in {}", dst_path.display())))?;
    }

    dst_dir_path.is_dir()
        .then(|| ())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, format!("Directory {} does not exist", dst_dir_path.display())))?;


    let src_md = fs::metadata(src_file_path)?;
    let dst_md = fs::metadata(dst_dir_path)?;

    let mut dst_file_path = dst_dir_path.to_path_buf();

    dst_file_path.push(dst_filename);

    // Source and destination are on the same device, we can simply rename.
    if src_md.st_dev() == dst_md.st_dev() {
        fs::rename(src_file_path, dst_file_path)?;

    // Source and destination are on different device, we must copy then delete.
    } else {
        fs::copy(src_file_path, dst_file_path)?;
        fs::remove_file(src_file_path)?;
    }
    
    Ok(())
}

pub fn read_file_lines(path: &Path) -> io::Result<Vec<String>> {
    let file = fs::File::open(path)?;
    let buf = BufReader::new(file);

    let lines = buf.lines();
    let mut v = Vec::new();
    for l in lines {
        match l {
            Ok(l) => v.push(l),
            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("Unable o parse data: {e}"))),
        }
    }

    Ok(v)
}


/// Expand tilde into home directory.
///
/// Based on stackoverflow snippet : https://stackoverflow.com/a/54306906
/// Modified so that function returns an io::Result. If it is not able to expand
/// then it will return an io::Error with variant ErrorKind::NotFound.
pub fn expand_tilde<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let p = path.as_ref();

    if !p.starts_with("~") {
        return Ok(p.to_path_buf());
    }
    if p == Path::new("~") {
        return dirs::home_dir()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Unable to expand home directory from '~'"));
    }

    dirs::home_dir().map(|mut home| {
        if home == Path::new("/") {
            // Corner case: `dir` root directory;
            // don't prepend extra `/`, just drop the tilde.
            p.strip_prefix("~").unwrap().to_path_buf()
        } else {
            home.push(p.strip_prefix("~/").unwrap());
            home
        }
    }).ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Unable to expand home directory from '~'"))
}

