//! Module with helper functions

use std::path::{Path, PathBuf};
use std::fs;
use std::os::linux::fs::MetadataExt;
use std::io::{self, BufRead, BufReader};
use std::process::Command;

/// Simple helper function to verify that path is an existing file or return
/// an error.
pub fn check_is_existing_file(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File {} does not exist", path.display()));
    }

    if !path.is_file() {
        return Err(format!("{} is not a file", path.display()));
    }

    Ok(())
}


/// Simple helper function to verify that path is an existing directory or
/// return an error.
pub fn check_is_existing_directory(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("Directory {} does not exist", path.display()));
    }

    if !path.is_dir() {
        return Err(format!("{} is not a directory", path.display()));
    }

    Ok(())
}


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


/// Attempts to move src_1 to dst_1, then src_2 to dst_2.
///
/// If the move fails, the function fail, attempts to revert back to the state
/// before the call. i.e. if it fails on the first move, nothing is done, if
/// it fails on the second move, the function tries to move back dst_1 to src_1
/// before exiting.
pub fn attempt_double_move(
    src_1: &Path,
    dst_1: &Path,
    src_2: &Path,
    dst_2: &Path,
) -> Result<(), String> {
    move_file(src_1, dst_1).map_err(|e| format!("Unable to move file : {}", e))?;

    // Move trash back to original
    if let Err(e) = move_file(src_2, dst_2) {
        println!(
            "Unable to move {}, attempting to revert. Err: {}",
            src_2.display(),
            e
        );
        move_file(dst_1, src_1).map_err(|e| {
            format!(
                "Unable to revert move file {} back to {}. Err: {}",
                dst_1.display(),
                src_1.display(),
                e
            )
        })?;
    }

Ok(())
}


/// Converts a Command instance to a String, as the command would be typed.
pub fn command_to_string(command: &Command) -> String {
    let mut cmd_string = String::new();
    cmd_string += command.get_program().to_str().unwrap();
    for a in command.get_args() {
        cmd_string += " ";
        cmd_string += a.to_str().unwrap();
    }

    cmd_string
}


/// Executes a &str as a command. Replacing %i with input_file and %o with
/// output_file.
pub fn execute_command_str(command: &str, input_file: &Path, output_file: &Path) {
    let split = command.split(' ').collect::<Vec<&str>>();
    if !split.is_empty() {
        let mut cmd = Command::new(split[0]);
        for item in split[1..].iter() {
            if *item == "%i" {
                cmd.arg(input_file);
            } else if *item == "%o" {
                cmd.arg(output_file);
            } else {
                cmd.arg(item);
            }
        }
        cmd.status().expect("Failed to execute command");
    }
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


fn rounded_div(dividend: u64, divisor: u64) -> u64 {
    (dividend + divisor - 1) / divisor
}


/// Takes a size in bytes, and returns a string with appropriate format and unit
///
/// Format is similar to `ls -h` command. Except the value is rounded instead of 
/// ceiled.
pub fn human_readable_size(byte_size: u64) -> String {
    const ONE_G: u64 = 1024 * 1024 * 1024;
    const ONE_M: u64 = 1024 * 1024;
    const ONE_K: u64 = 1024;

    // Display in giga byte
    let (size, unit) = if byte_size > ONE_G {
        (rounded_div(byte_size*10, ONE_G), "G")
    } else if byte_size > ONE_M {
        (rounded_div(byte_size*10, ONE_M), "M")
    } else if byte_size > ONE_K {
        (rounded_div(byte_size*10, ONE_K), "K")
    } else {
        (byte_size, "")
    };

    let int = size / 10;
    let dec = size - int * 10;

    if int >= 10 {
        format!("{int}{unit}")
    } else {
        format!("{int}.{dec}{unit}")
    }
}

