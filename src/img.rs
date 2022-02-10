use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use crate::utils::{attempt_double_move, execute_command_str, check_is_existing_directory};
use crate::settings::AppSettings;
use chrono::{DateTime, Utc};


#[derive(Clone, Default, Debug)]
pub struct ProcessItem {
    pub tmp_path: Option<PathBuf>,
    pub processed_path: Option<PathBuf>,
    processing_failed: bool,
}

impl ProcessItem {

    /// Attempt to process the file at provided source path, with provided cmd, 
    /// and place it in provided output directory.
    ///
    /// If this function is called more than once, it will redo the processing.
    /// Unlike ProcessItem::process(...) which will skip if file has already
    /// been processed.
    fn attempt_process(&mut self, source: PathBuf, output_dir: PathBuf, cmd: String, cmd_index: usize) -> Result<(), String>{
        let tmp_filepath = process_tmp_path(&source, &output_dir, cmd_index)?;

        execute_command_str(&cmd, &source, &tmp_filepath);

        let file_md = fs::metadata(&tmp_filepath)
            .map_err(|e| format!("Couldn't open {}: {e}", tmp_filepath.display()))?;

        (file_md.len() > 0)
            .then(|| ())
            .ok_or_else(|| format!("{} is empty", tmp_filepath.display()))?;
        
        self.tmp_path = Some(tmp_filepath);

        Ok(())
    }


    /// Process the file at provided source path, with provided cmd, 
    /// and place it in provided output directory.
    ///
    /// The function can always be called, if the processing has already been 
    /// done for this instance.
    pub fn process(&mut self, source: PathBuf, output_dir: PathBuf, cmd: String, cmd_index: usize){
        // Return early if already processed, or processing failed.
        if self.is_processed() || self.processing_failed {
            return;
        }

        if let Err(e) = self.attempt_process(source, output_dir, cmd, cmd_index) {
            self.processing_failed = true;
            println!("Processing failed: {e}");
        }
    }

    pub fn is_processed(&self) -> bool {
        self.tmp_path.is_some()
    }

    fn is_validated(&self) -> bool {
        self.processed_path.is_some()
    }
}

/// Container for an image and its processed variants.
///
/// source          is the original path for the file provided by user.
/// deleted         is the original file location after it has been moved
///                 if the user validated one of the processed variant.
/// processed       is a container of all the variants processed, or to be
///                 processed.
///
/// Upon loading the image, the file will first be processed by the provided
/// processor command, and the output will be stored at processed_tmp location.
///
/// If the user validates the processing result, the original will be moved
/// (optionnaly with checksum verification), then the processed file will be
/// moved to the original path, possibly with different extension.
///
/// If the user presses undo command, the moves will be reverted. The new image
/// will be moved back to processed_tmp location, and the deleted image will be
/// moved back to original location.
#[derive(Clone)]
pub struct ImgItem {
    pub source: PathBuf,
    pub deleted: Option<PathBuf>,
    pub processed: Vec<Option<ProcessItem>>,
}

impl ImgItem {

    /// Creates an instance of img, with the provided source path of the image
    /// to process
    ///
    /// The instance will contain an option for the deleted path set to None,
    /// to store the new path of the image when it will be moved.
    /// It will also contain a vector of options of size cmds_len for every
    /// processed variants (one for every command provided by user)
    ///
    /// ProcessItem are options, so that they can be sent to other threads with
    /// Option::take (leaving None in place).
    pub fn new(source: &Path, cmds_len: usize) -> ImgItem {
        let processed = (0..cmds_len)
            .map(|_| Some(ProcessItem::default()))
            .collect();

        ImgItem {
            source: source.to_path_buf(),
            processed,
            deleted: None,
        }
    }

    /// Validates the selected variant by moving it to the source directory
    ///
    /// To maximze safety, the original file is first moved to the trash
    /// folder, then the processed file is moved to the source_dir with its
    /// final filename.
    pub fn validate(&mut self, cmd_index: usize, settings: &AppSettings) -> Result<(), String> {
        let p = self.processed[cmd_index]
            .as_mut()
            .ok_or_else(|| "No instance at provided index".to_string())
            .and_then(|p| match p.is_processed() {
                true => Ok(p),
                false => Err("Instance at provided index is not processed.".to_string()),
            })?;

        let processed_path = p
            .tmp_path
            .as_ref()
            .ok_or_else(|| "No processed path at provided index".to_string())?;

        let deleted_path = deleted_file_path(&self.source, &settings.trash_directory)?;

        attempt_double_move(&self.source, &deleted_path, processed_path, &self.source)?;
        self.deleted = Some(deleted_path);
        p.processed_path = Some(self.source.clone());

        Ok(())
    }

    /// Reverse the validation, put back validated image in tmp, and put back
    /// deleted picture in source.
    pub fn undo(&mut self) -> Result<(), String> {
        let p = self
            .get_validated()
            .ok_or_else(|| "No validated process available".to_string())?;

        let processed_path = p
            .tmp_path
            .clone()
            .ok_or_else(|| "No processed file available".to_string())?;

        let deleted_path = self
            .deleted
            .clone()
            .ok_or_else(|| "No deleted file available".to_string())?;

        attempt_double_move(
            &self.source.clone(),
            &processed_path,
            &deleted_path,
            &self.source.clone(),
        )?;

        let mut validated = self.get_validated_mut();
        let p = validated
            .as_mut()
            .ok_or_else(|| "No validated process available".to_string())?;
        p.processed_path.take();
        self.deleted.take();

        Ok(())
    }

    /// If we have defined a deleted path, that means that the image has been
    /// validated.
    pub fn is_validated(&self) -> bool {
        self.deleted.is_some()
    }

    /// Retrieves an option on a reference on the processed instance that was
    /// validated.
    pub fn get_validated(&self) -> Option<&ProcessItem> {
       self.processed.iter().flatten().find(|&p| p.is_validated())
    }

    /// Retrieves an option on a mutable reference on the processed instance that
    /// was validated.
    fn get_validated_mut(&mut self) -> Option<&mut ProcessItem> {
        self.processed
            .iter_mut()
            .flatten()
            .find(|p| p.is_validated())
    }
}


/// Given the source path, the processing_directory path, and the command
/// index, generates the temporary output file path.
///
/// The temporary output file path is generated as follows:
/// - The storage directory will be the provided processing_directory.
/// - The filename will be the source filename, with _processed_i appended before
///   the extension, where `i` is the index of the command.
fn process_tmp_path(
    source: &Path,
    processing_directory: &Path,
    i: usize,
) -> Result<PathBuf, String> {
    check_is_existing_directory(processing_directory)?;

    let suffix = format!("_processed_{}", i);
    let extension = source.extension();

    let mut output_path = processing_directory.to_path_buf();
    let mut filename = source
        .file_stem()
        .ok_or_else(|| format!("No file name in {}", source.display()))?
        .to_os_string();

    filename.push(suffix);
    if let Some(extension) = extension {
        filename.push(".");
        filename.push(extension);
    }

    output_path.push(filename);

    Ok(output_path)
}


/// Given the source path, the and the trash directory path, generates the
/// deleted file path.
///
/// The deleted file path is generated as follows:
/// - The storage directory will be the provided processing_directory.
/// - The filename will be the source filename, with _processed_i appended before
///   the extension, where `i` is the index of the command.
fn deleted_file_path(source: &Path, trash_directory: &Path) -> Result<PathBuf, String> {
    check_is_existing_directory(trash_directory)?;

    let mut output_path = trash_directory.to_path_buf();

    let extension = source.extension();

    // let mut filename = source
    //     .file_stem()
    //     .ok_or_else(|| "Missing file name".to_string())?
    //     .to_os_string();
    //             
    // let dt = format!("_{}", Utc::now().format("%y-%m-%d_%Hh%Mm%Ss"));
    //
    // filename.push(dt);
    // 
    // if let Some(extension) = extension {
    //     filename.push(".");
    //     filename.push(extension);
    // }

    // FIXME: It doesn't seem ideal to use to_string_lossy, what could be a way
    // to avoid that?
    let filename: OsString = source.to_string_lossy()
        .replace("%","%%")
        .replace("/","%")
        .into();

    output_path.push(filename);
    Ok(output_path)
}


