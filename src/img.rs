use std::fs;
use std::path::Path;
use std::path::PathBuf;
use crate::utils::{attempt_double_move, execute_command_str};
use crate::settings::AppSettings;

/// State machine for the image processing. It is wrapped in an enum because we
/// store all the files as a state machine in a vector collection.
#[derive(PartialEq, Clone, Debug)]
pub enum ProcessingState {
    NotProcessed,
    Processed,
    Failed,
    Validated,
}

impl Default for ProcessingState {
    fn default() -> Self {
        Self::NotProcessed
    }
}

#[derive(Clone, Default, Debug)]
pub struct ProcessItem {
    pub tmp_path: Option<PathBuf>,
    pub processed_path: Option<PathBuf>,
    pub state: ProcessingState,
}

impl ProcessItem {
    pub fn process(&mut self, source: PathBuf, output_dir: PathBuf, cmd: String, cmd_index: usize) {
        // Return early if wrong state
        if self.state != ProcessingState::NotProcessed {
            return;
        }

        let tmp_filepath = process_tmp_path(&source, &output_dir, cmd_index);

        if let Err(e) = tmp_filepath {
            println!("Failed to process filepath {}", e);
            self.state = ProcessingState::Failed;
            return;
        }

        let tmp_filepath = tmp_filepath.unwrap();

        execute_command_str(&cmd, &source, &tmp_filepath);

        let file_md = fs::metadata(&tmp_filepath);

        match file_md {
            Ok(file_md) => {
                if file_md.len() > 0 {
                    self.tmp_path = Some(tmp_filepath.to_path_buf());
                    self.state = ProcessingState::Processed;
                } else {
                    println!(
                        "Could not read output file {}, file is empty",
                        tmp_filepath.display()
                    );
                    self.state = ProcessingState::Failed;
                }
            }
            Err(e) => {
                println!(
                    "Could not open file {}, maybe file doesn't exist. Error: {}",
                    tmp_filepath.display(),
                    e
                );
                self.state = ProcessingState::Failed;
            }
        }
    }

    fn is_processed(&self) -> bool {
        self.tmp_path.is_some()
    }

    fn is_validated(&self) -> bool {
        self.processed_path.is_some()
    }
}

/// Container for an image and its processed variants.
///
/// original        is the original path for the file provided by user.
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
    pub fn new(source: &Path, len: usize) -> ImgItem {
        let processed = (0..len).map(|_| Some(ProcessItem::default())).collect();

        ImgItem {
            source: source.to_path_buf(),
            processed,
            deleted: None,
        }
    }

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
        let v = self.processed.iter().flatten().find(|&p| p.is_validated());

        println!("validated instance: {v:?}");
        v
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
    if !processing_directory.exists() {
        return Err("Provided directory does not exist".to_string());
    }
    if !processing_directory.is_dir() {
        return Err("Provided directory is not a directory".to_string());
    }

    let suffix = format!("_processed_{}", i);
    let extension = source.extension();

    let mut output_path = processing_directory.to_path_buf();
    let mut filename = source
        .file_stem()
        .ok_or_else(|| "Missing file name".to_string())?
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
    if !trash_directory.exists() {
        return Err(format!(
            "Directory {} does not exist",
            trash_directory.display()
        ));
    }
    if !trash_directory.is_dir() {
        return Err(format!("{} is not a directory", trash_directory.display()));
    }

    let mut output_path = trash_directory.to_path_buf();
    let filename = source
        .file_name()
        .ok_or_else(|| "Missing file name".to_string())?
        .to_os_string();

    output_path.push(filename);

    Ok(output_path)
}


