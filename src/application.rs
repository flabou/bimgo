use sdl2::rect::Point;
use sdl2::rect::Rect;
use sdl2::ttf::Font;
use sdl2::ttf::FontResult;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::FullscreenType;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

use sdl2::image::LoadTexture;
use sdl2::pixels::Color;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::rect_utils::ViewRect;

use crate::processing_order::*;
use crate::settings::*;
use crate::utils::*;

/// State machine for the image processing. It is wrapped in an enum because we
/// store all the files as a state machine in a vector collection.
#[derive(PartialEq, Clone, Debug)]
enum ProcessingState {
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
struct ProcessItem {
    tmp_path: Option<PathBuf>,
    processed_path: Option<PathBuf>,
    state: ProcessingState,
}

impl ProcessItem {
    fn process(&mut self, source: PathBuf, output_dir: PathBuf, cmd: String, cmd_index: usize) {
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
    source: PathBuf,
    deleted: Option<PathBuf>,
    processed: Vec<Option<ProcessItem>>,
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

    fn validate(&mut self, cmd_index: usize, settings: &AppSettings) -> Result<(), String> {
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

    // Reverse the validation, put back validated image in tmp, and put back
    // deleted picture in source.
    fn undo(&mut self) -> Result<(), String> {
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
    fn is_validated(&self) -> bool {
        self.deleted.is_some()
    }

    /// Retrieves an option on a reference on the processed instance that was
    /// validated.
    fn get_validated(&self) -> Option<&ProcessItem> {
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

/// Attempts to move src_1 to dst_1, then src_2 to dst_2.
///
/// If the move fails, the function fail, attempts to revert back to the state
/// before the call. i.e. if it fails on the first move, nothing is done, if
/// it fails on the second move, the function tries to move back dst_1 to src_1
/// before exiting.
fn attempt_double_move(
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

/// Converts a Command instance to a String, as the command would be typed.
fn command_to_string(command: &Command) -> String {
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
fn execute_command_str(command: &str, input_file: &Path, output_file: &Path) {
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

/// This struct is used to mannage the program. Key presses will trigger methods
/// attached to it. There should only be one instance of this.
pub struct App<'a> {
    settings: AppSettings,
    canvas: &'a mut Canvas<Window>,
    cmds: Vec<String>,
    imgs: Vec<ImgItem>,
    rxs: Vec<mpsc::Receiver<((usize, usize), ProcessItem)>>,
    index: usize,
    cmd_index: usize,
    source_view: ViewRect,
    processed_view: ViewRect,
    texture_creator: &'a TextureCreator<WindowContext>,
    source_texture: Texture<'a>,
    processed_texture: Texture<'a>,
    ttf_context: &'a Sdl2TtfContext,
}

impl<'a> App<'a> {
    pub fn new(
        canvas: &'a mut Canvas<Window>,
        texture_creator: &'a TextureCreator<WindowContext>,
        ttf_context: &'a Sdl2TtfContext,
        img_paths: Vec<PathBuf>,
    ) -> Result<Self, String> {
        let settings = AppSettings::new().map_err(|e| format!("Error: {e}"))?;

        /*  The external conversion command must be provided with special characters
           denoting where to put the input and output file names in the command.
           The special characters are the following:
            - %i   Location of the input file argument, the value provided by user
                   will be placed here.
            - %o   Location of the output file argument. If the file extension must
                   be changed, it needs to be specified as `%o.ext` where `ext` is
                   the new extension.
        */
        let cmds = read_file_lines(&settings.cmds_file).map_err(|e| e.to_string())?;

        let source_texture = texture_creator
            .create_texture_static(None, 1, 1)
            .map_err(|e| e.to_string())?;

        let processed_texture = texture_creator
            .create_texture_static(None, 1, 1)
            .map_err(|e| e.to_string())?;

        let imgs = img_paths
            .iter()
            .map(|item| ImgItem::new(item, cmds.len()))
            .collect::<Vec<ImgItem>>();

        let mut app = App {
            settings,
            canvas,
            cmds,
            imgs,
            rxs: Vec::new(),
            index: 0,
            cmd_index: 0,
            source_view: ViewRect::default(),
            processed_view: ViewRect::default(),
            texture_creator,
            source_texture,
            processed_texture,
            ttf_context,
        };

        app.update_views()?;
        app.first_image()?;

        Ok(app)
    }

    /// Returns a tuple (w, h) of the width and height of the window.
    ///
    /// This function is only needed for readability.
    fn window_size(&self) -> (u32, u32) {
        self.canvas.window().size()
    }

    /// Returns a rectangle denoting the window, but the position is (0, 0)
    fn window_rect(&self) -> Rect {
        let (w, h) = self.window_size();
        Rect::new(0, 0, w, h)
    }

    fn get_source_path(&self) -> PathBuf {
        if self.imgs[self.index].is_validated() {
            // load source is validated
            if let Some(d) = &self.imgs[self.index].deleted {
                return d.clone();
            } 
        } else {
            // load source is not validated
            return self.imgs[self.index].source.clone();
        }

        PathBuf::from("")
    }

    fn get_current_processed_path(&self) -> Result<PathBuf, String> {
        if let Some(p) = self.imgs[self.index].get_validated() {
            // load processed is validated
            if let Some(ref o) = p.processed_path {
                return Ok(o.clone());
            }
        } else if let Some(processed_img) = &self.imgs[self.index].processed[self.cmd_index] {
            // load processed is not validated but processed
            if let Some(ref processed_path) = processed_img.tmp_path {
                return Ok(processed_path.clone());
            }
        }

        Err(String::from("Processed image at index currently not available"))
    }

    /// Draws a border around validated pictures, so the user has a visual cue
    /// of which file has been saved on disk.
    fn draw_selected(&mut self) -> Result<(), String> {
        let clip = self
            .processed_view
            .clip_rect
            .intersection(self.processed_view.virt_rect);
        if let Some(clip) = clip {
            let rect = self.processed_view.clip_rect;
            let thickness = std::cmp::min(rect.height(), rect.width()) / 20;

            let mut outer = clip;
            let mut side_1 = clip;
            let mut side_2;

            match self.settings.source_position {
                SourcePosition::Left | SourcePosition::Right => {
                    outer.set_width(thickness);
                    side_1.set_height(thickness);
                    side_1.set_y(clip.top());
                    side_2 = side_1;
                    side_2.set_bottom(clip.bottom());
                }
                SourcePosition::Top | SourcePosition::Bottom => {
                    outer.set_height(thickness);
                    side_1.set_width(thickness);
                    side_1.set_x(clip.left());
                    side_2 = side_1;
                    side_2.set_right(clip.right());
                }
            };

            match self.settings.source_position {
                SourcePosition::Left => outer.set_right(clip.right()),
                SourcePosition::Right => outer.set_x(clip.left()),
                SourcePosition::Top => outer.set_bottom(clip.bottom()),
                SourcePosition::Bottom => outer.set_y(clip.top()),
            }

            self.canvas.set_draw_color(Color::RGBA(0, 128, 128, 255));
            self.canvas.fill_rects(&[outer, side_1, side_2])?;
        }

        Ok(())
    }

    /// Adds source file path and size to the image
    ///
    /// If the split is vertical, path is written below the image, if the split
    /// is horizontal, path is diplayed above top image and below bottom image.
    #[allow(dead_code)]
    fn draw_source_data(&mut self) -> Result<(), String> {
        let source_path = self.get_source_path();
        let source_md = if let Ok(source_md) = fs::metadata(&source_path){
            source_md
        } else {
            return Ok(());
        };

        let info_str = format!("{}\nsize: {}", 
                               source_path.display(), 
                               human_readable_size(source_md.len()));

        // Load font
        let font_path = expand_tilde("~/bimgo/fonts/FiraMono-Medium.ttf")
                .map_err(|e| format!("{e}"))?;
        let font = self.ttf_context.load_font(font_path, 30)?;

        // Draw at correct position
        let (w, h) = self.window_size();

        let (position, anchor) = match self.settings.source_position {
            SourcePosition::Top     => (Point::new(0, 0), Anchor::TopLeft),
            SourcePosition::Bottom  => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Left    => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Right   => (Point::new(w as i32 / 2, h as i32), Anchor::BottomLeft),
        };

        let txt = TextBox::new(&info_str, &font, self.texture_creator)
            .wrapped(self.source_view.clip_rect.width());

        txt.draw(self.canvas, position, anchor)?;

        Ok(())
    }

    fn draw_processed_data(&mut self) -> Result<(), String>{
        let processed_path = if let Ok(path) = self.get_current_processed_path(){
            path
        } else {
            return Ok(());
        };

        let processed_md = if let Ok(processed_md) = fs::metadata(&processed_path){
            processed_md
        } else {
            return Ok(());
        };

        let info_str = format!("{}\nsize: {}", 
                               processed_path.display(), 
                               human_readable_size(processed_md.len()));

        // Load font
        let font_path = expand_tilde("~/bimgo/fonts/FiraMono-Medium.ttf")
                .map_err(|e| format!("{e}"))?;
        let font = self.ttf_context.load_font(font_path, 30)?;

        // Draw at correct position
        let (w, h) = self.window_size();

        let (position, anchor) = match self.settings.source_position {
            SourcePosition::Bottom  => (Point::new(0, 0), Anchor::TopLeft),
            SourcePosition::Top     => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Right   => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Left    => (Point::new(w as i32 / 2, h as i32), Anchor::BottomLeft),
        };

        let txt = TextBox::new(&info_str, &font, self.texture_creator)
            .wrapped(self.processed_view.clip_rect.width());

        txt.draw(self.canvas, position, anchor)?;

        Ok(())
    }

    fn draw(&mut self) -> Result<(), String> {
        self.canvas.set_draw_color(Color::RGB(36, 40, 59));
        self.canvas.clear();

        match self.settings.display_mode {
            DisplayMode::Continuous => self.processed_view.sync_continuous_with(&self.source_view),
            DisplayMode::Duplicate => self.processed_view.sync_duplicate_with(&self.source_view),
        };

        self.canvas.copy(
            &self.source_texture,
            Some(self.source_view.src_rect),
            Some(self.source_view.dst_rect),
        )?;
        self.canvas.copy(
            &self.processed_texture,
            Some(self.processed_view.src_rect),
            Some(self.processed_view.dst_rect),
        )?;
        if self.imgs[self.index].is_validated() {
            self.draw_selected()?;
        }

        self.draw_source_data()?;
        self.draw_processed_data()?;
        self.canvas.present(); // Update the screen with canvas.

        Ok(())
    }

    /// Calls the appropriate fit function based on settings then draws the image
    pub fn fit_draw(&mut self) -> Result<(), String> {
        let fit_rect = match self.settings.display_mode {
            DisplayMode::Continuous => self.window_rect(),
            DisplayMode::Duplicate => self.source_view.clip_rect,
        };

        match self.settings.fit_mode {
            FitMode::FitBest => self.source_view.fit_best_to_rect(fit_rect),
            FitMode::FitWidth => self.source_view.fit_width_to_rect(fit_rect),
            FitMode::FitHeight => self.source_view.fit_height_to_rect(fit_rect),
            FitMode::Fill => self.source_view.fit_fill_to_rect(fit_rect),
            _ => (),
        };
        self.draw()?;

        Ok(())
    }

    /// Zooms towards the center of the image.
    ///
    /// Scale factor above 1.0 zooms in, while scale factor below 1.0 zooms out
    fn zoom(&mut self, scale: f32) -> Result<(), String> {
        let zoom_point = match self.settings.display_mode {
            DisplayMode::Duplicate => self.source_view.clip_rect.center(),
            DisplayMode::Continuous => {
                (self.source_view.clip_rect.center() + self.processed_view.clip_rect.center()) / 2
            }
        };

        let (w, h) = self.window_size();
        let window_rect = Rect::new(0, 0, w, h);
        self.source_view
            .zoom_towards_point_on_rect(zoom_point, window_rect, scale);
        self.draw()?;

        Ok(())
    }

    pub fn zoom_in(&mut self) -> Result<(), String> {
        self.zoom(1.1)?;

        Ok(())
    }

    pub fn zoom_out(&mut self) -> Result<(), String> {
        self.zoom(0.9)?;

        Ok(())
    }

    /// Updates the source_view and processed_view.
    ///
    /// There are several instances where it might be necessary to update them,
    /// such as when the window size has changed, or when settings that impact
    /// the Views' geometry have changed.
    pub fn update_views(&mut self) -> Result<(), String> {
        let (w, h) = self.window_size();
        let padding = self.settings.padding;

        println!("Updating view with window parameters: w={w}, h={h}");

        let (source_rect, processed_rect) = match self.settings.source_position {
            SourcePosition::Left => (
                Rect::new(0, 0, w / 2 - padding, h),
                Rect::new(w as i32 / 2 + padding as i32, 0, w / 2 - padding, h),
            ),

            SourcePosition::Top => (
                Rect::new(0, 0, w, h / 2 - padding),
                Rect::new(0, h as i32 / 2 + padding as i32, w, h / 2 - padding),
            ),

            SourcePosition::Right => (
                Rect::new(w as i32 / 2 + padding as i32, 0, w / 2 - padding, h),
                Rect::new(0, 0, w / 2 - padding, h),
            ),

            SourcePosition::Bottom => (
                Rect::new(0, h as i32 / 2 + padding as i32, w, h / 2 - padding),
                Rect::new(0, 0, w, h / 2 - padding),
            ),
        };

        self.source_view.set_clip_rect(source_rect);
        self.processed_view.set_clip_rect(processed_rect);
        self.fit_draw()?;

        Ok(())
    }

    /// Pans the image to the left.
    pub fn pan_left(&mut self) -> Result<(), String> {
        self.source_view.pan_left(50);
        self.draw()?;

        Ok(())
    }

    /// Pans the image to the right.
    pub fn pan_right(&mut self) -> Result<(), String> {
        self.source_view.pan_right(50);
        self.draw()?;

        Ok(())
    }

    /// Pans the image down.
    pub fn pan_down(&mut self) -> Result<(), String> {
        self.source_view.pan_down(50);
        self.draw()?;

        Ok(())
    }

    /// Pans the image up.
    pub fn pan_up(&mut self) -> Result<(), String> {
        self.source_view.pan_up(50);
        self.draw()?;

        Ok(())
    }

    /// Pans the virtual rectangle relative to mouse movement.
    pub fn pan_mouse_relative(&mut self, m_x: i32, m_y: i32) -> Result<(), String> {
        // let (w, h) = match self.settings.display_mode {
        //     DisplayMode::Continuous => self.window_size(),
        //     DisplayMode::Duplicate => self.source_view.clip_rect.size(),
        // };

        let (w, h) = self.window_size();
        let (v_w, v_h) = self.source_view.virt_rect.size();
        let v_x = if v_w > w {
            (w as i32 - m_x) - v_w as i32 * (w as i32 - m_x) / w as i32
        } else {
            m_x - v_w as i32 * m_x / w as i32
        };

        let v_y = if v_h > h {
            (h as i32 - m_y) - v_h as i32 * (h as i32 - m_y) / h as i32
        } else {
            m_y - v_h as i32 * m_y / h as i32
        };

        let mut v_rect = self.source_view.virt_rect;

        v_rect.set_x(v_x);
        v_rect.set_y(v_y);
        self.source_view.set_virt_rect(v_rect);
        self.draw()?;

        Ok(())
    }

    /// Sends the images close to the current position to be processed in other
    /// threads.
    ///
    /// This allows to process several images in parallel. It also prevents
    /// blocking the main thread which mannages the user interface.
    fn update_process_threads(&mut self) {
        // Start the process thread for the following images.
        //for (i, c) in (0..self.imgs.len()).flat_map(|i| (0..self.cmds.len()).map(move |c| (i, c))){
        // for (i, c) in VFirst2D::new(self.index, self.index.saturating_sub(5), usize::min(self.index + 5, self.imgs.len()-1),
        //                             self.cmd_index, self.cmd_index.saturating_sub(5), usize::min(self.cmd_index + 5, self.cmds.len()-1)) {
        for (i, c) in Closest2D::new(
            self.index,
            self.index.saturating_sub(5),
            usize::min(self.index + 5, self.imgs.len() - 1),
            self.cmd_index,
            self.cmd_index.saturating_sub(5),
            usize::min(self.cmd_index + 5, self.cmds.len() - 1),
        ) {
            if self.imgs[i].processed[c].is_some() {
                let mut p = self.imgs[i].processed[c].take().unwrap();
                if p.state == ProcessingState::NotProcessed {
                    let (tx, rx) = mpsc::channel();
                    self.rxs.push(rx);
                    let source_path = self.imgs[i].source.clone();
                    let output_directory = self.settings.processing_directory.clone();
                    let cmd = self.cmds[c].to_string();
                    thread::spawn(move || {
                        p.process(source_path, output_directory, cmd, c);

                        tx.send(((i, c), p)).unwrap();
                    });
                } else {
                    self.imgs[i].processed[c] = Some(p);
                }
            }
        }
    }

    fn load_source_at_index(&mut self) -> Result<(), String> {
        // Load image on screen.
        if let Some(v) = self.imgs[self.index].get_validated() {
            println!("load_source_is_validated");
            if let Some(d) = &self.imgs[self.index].deleted {
                self.source_texture = self.texture_creator.load_texture(d)?;
            }
        } else {
            println!("load_source_is_not_validated");
            self.source_texture = self
                .texture_creator
                .load_texture(&self.imgs[self.index].source)?;
        }

        let texture_info = self.source_texture.query();
        self.source_view
            .set_img_rect(Rect::new(0, 0, texture_info.width, texture_info.height));

        Ok(())
    }

    fn load_processed_at_index(&mut self) -> Result<(), String> {
        // Load processed picture
        if let Some(p) = self.imgs[self.index].get_validated() {
            println!("load_processed_is_validated");
            if let Some(o) = &p.processed_path {
                self.processed_texture = self.texture_creator.load_texture(&o)?;
            }
        } else if let Some(processed_img) = &self.imgs[self.index].processed[self.cmd_index] {
            println!("load_processed_is_not_validated_but_processed");
            if let Some(processed_path) = &processed_img.tmp_path {
                // println!("processed_path: {}", processed_path.display());
                self.processed_texture = self.texture_creator.load_texture(&processed_path)?;
            }
        }

        let texture_info = self.processed_texture.query();
        self.processed_view
            .set_img_rect(Rect::new(0, 0, texture_info.width, texture_info.height));

        self.update_process_threads();

        Ok(())
    }

    fn load_image_at_index(&mut self) -> Result<(), String> {
        self.load_source_at_index()?;
        self.load_processed_at_index()?;

        Ok(())
    }

    fn first_image(&mut self) -> Result<(), String> {
        self.index = 0;
        self.cmd_index = 0;
        // Processing first image here before other processes
        if !self.imgs.is_empty()
            && !self.cmds.is_empty()
            && self.imgs[self.index].processed[self.cmd_index].is_some()
        {
            let mut p = self.imgs[self.index].processed[self.cmd_index]
                .take()
                .unwrap();
            p.process(
                self.imgs[self.index].source.clone(),
                self.settings.processing_directory.clone(),
                self.cmds[self.cmd_index].to_string(),
                self.cmd_index,
            );
            self.imgs[self.index].processed[self.cmd_index] = Some(p);
        }

        self.load_image_at_index()?;
        self.fit_draw()?;

        Ok(())
    }

    pub fn next_image(&mut self) -> Result<(), String> {
        if self.index + 1 < self.imgs.len() {
            self.index += 1;
            self.load_image_at_index()?;
            self.fit_draw()?;
        }

        Ok(())
    }

    pub fn prev_image(&mut self) -> Result<(), String> {
        if self.index > 0 {
            self.index -= 1;
            self.load_image_at_index()?;
            self.fit_draw()?;
        }

        Ok(())
    }

    pub fn next_cmd(&mut self) -> Result<(), String> {
        if self.cmd_index + 1 < self.cmds.len() {
            self.cmd_index += 1;
            self.load_processed_at_index()?;
            self.draw()?;
        }

        Ok(())
    }

    pub fn prev_cmd(&mut self) -> Result<(), String> {
        if self.cmd_index > 0 {
            self.cmd_index -= 1;
            self.load_processed_at_index()?;
            self.draw()?;
        }

        Ok(())
    }

    pub fn validate_current(&mut self) -> Result<(), String> {
        let img = &mut self.imgs[self.index];

        if img.processed[self.cmd_index].is_some() {
            // Catch the error but don't panic.
            if let Err(s) = img.validate(self.cmd_index, &self.settings) {
                println!("Error: {s}");
            }
        }

        self.draw()?;

        Ok(())
    }

    pub fn undo_current(&mut self) -> Result<(), String> {
        let img = &mut self.imgs[self.index];

        // Catch the error but don't panic.
        if let Err(s) = img.undo() {
            println!("Error: {s}");
        }

        self.load_processed_at_index()?;
        self.draw()?;

        Ok(())
    }

    pub fn toggle_fullscreen(&mut self) -> Result<(), String> {
        let window = self.canvas.window_mut();

        if window.fullscreen_state() == FullscreenType::Off {
            window.set_fullscreen(FullscreenType::Desktop)?;
        } else {
            window.set_fullscreen(FullscreenType::Off)?;
        }

        self.update_views()?;
        Ok(())
    }

    /// This function must be ran in the main loop, it handles processing
    /// the images through multi threading.
    pub fn run(&mut self) -> Result<(), String> {
        let mut update_image = false;

        for k in (0..self.rxs.len()).rev() {
            if let Ok(((i, c), process_item)) = self.rxs[k].try_recv() {
                self.imgs[i].processed[c] = Some(process_item);
                if self.index == i && self.cmd_index == c {
                    update_image = true;
                }
                self.rxs.swap_remove(k);
            }
        }

        if update_image {
            self.load_processed_at_index()?;
            self.draw()?;
        }
        Ok(())
    }
}


/// Helper function to generate a text texture 
fn generate_text<T, U>(
    txt: &str,
    texture_creator: &TextureCreator<U>,
    canvas: &mut Canvas<T>,
    font: Font,
) -> Result<(), String>
where
    T: sdl2::render::RenderTarget,
{
    let s_text = font
        .render(txt)
        //.solid(Color::RGB(255,255,255))
        .blended(Color::RGB(255, 255, 255))
        //.shaded(Color::RGB(255,255,255), Color::RGB(0,128,128))
        .map_err(|e| format!("{e}"))?;

    let src_rect = s_text.rect();

    let t_text = s_text
        .as_texture(texture_creator)
        .map_err(|e| format!("{e}"))?;

    // let dst_rect = src_rect;

    let dst_rect = Rect::new(50, 0, src_rect.width(), src_rect.height());

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.fill_rect(dst_rect)?;
    canvas.copy(&t_text, Some(src_rect), Some(dst_rect))?;

    Ok(())
}


#[allow(unused)]
enum Anchor{
    TopLeft,
    Top,
    TopRight,

    Left,
    Center,
    Right,

    BottomLeft,
    Bottom,
    BottomRight,
}

/// Helper struct to generate a "textbox"
struct TextBox<'a, T> {
    texture_creator: &'a TextureCreator<T>,
    font: &'a Font<'a, 'a>,
    txt: &'a str,
    width: Option<u32>,
}

impl<'a, T> TextBox<'a, T>{
    fn new(txt: &'a str, font: &'a Font, texture_creator: &'a TextureCreator<T>) -> TextBox<'a, T> {
        TextBox{
            texture_creator,
            font,
            txt,
            width: None,
        }
    }

    fn wrapped(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    fn draw<C>(&self, canvas: &mut Canvas<C>, position: Point, anchor: Anchor) -> Result<(), String>
    where
        C: sdl2::render::RenderTarget,
    {
        let s_text = self.font
            .render(self.txt);
            //.solid(Color::RGB(255,255,255))
            //.blended(Color::RGB(255, 255, 255))
            //.shaded(Color::RGB(255,255,255), Color::RGB(0,128,128))
            //.map_err(|e| format!("{e}"))?;

        let s_text = match self.width {
            Some(width) => s_text.blended_wrapped(Color::RGB(255,255,255), width),
            None => s_text.blended(Color::RGB(255,255,255)),
        }.map_err(|e| format!("{e}"))?;

        

        let src_rect = s_text.rect();

        let t_text = s_text
            .as_texture(self.texture_creator)
            .map_err(|e| format!("{e}"))?;

        let (w, h) = src_rect.size();

        let position = match anchor {
            Anchor::TopLeft     => position,
            Anchor::Top         => position - Point::new(w as i32 / 2, 0),
            Anchor::TopRight    => position - Point::new(w as i32, 0),

            Anchor::Left        => position - Point::new(0, h as i32 / 2),
            Anchor::Center      => position - Point::new(w as i32 / 2, h as i32 / 2),
            Anchor::Right       => position - Point::new(w as i32, h as i32 / 2),

            Anchor::BottomLeft  => position - Point::new(0, h as i32),
            Anchor::Bottom      => position - Point::new(w as i32 / 2, h as i32),
            Anchor::BottomRight => position - Point::new(w as i32, h as i32),
        };
        let dst_rect = Rect::new(position.x, position.y, src_rect.width(), src_rect.height());


        let bg_rect = match self.width {
            Some(width) => Rect::new(position.x, position.y, width, src_rect.height()),
            None        => Rect::new(position.x, position.y, src_rect.width(), src_rect.height()),
        };

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.fill_rect(bg_rect)?;
        canvas.copy(&t_text, Some(src_rect), Some(dst_rect))?;

        Ok(())
    }
}
