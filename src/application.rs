use sdl2::rect::Point;
use sdl2::rect::Rect;
use sdl2::ttf::Font;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::video::FullscreenType;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
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
use crate::sdl_utils::*;
use crate::img::*;

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
    font: Font<'a, 'a>,
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
        //
        // Load font
        let font_path = expand_tilde("~/bimgo/fonts/FiraMono-Medium.ttf")
                .map_err(|e| format!("{e}"))?;
        let font = ttf_context.load_font(font_path, 30)?;

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
            font,
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

        // Draw at correct position
        let (w, h) = self.window_size();

        let (position, anchor) = match self.settings.source_position {
            SourcePosition::Top     => (Point::new(0, 0), Anchor::TopLeft),
            SourcePosition::Bottom  => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Left    => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Right   => (Point::new(w as i32 / 2, h as i32), Anchor::BottomLeft),
        };

        let txt = TextBox::new(&info_str, &self.font, self.texture_creator)
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

        // Draw at correct position
        let (w, h) = self.window_size();

        let (position, anchor) = match self.settings.source_position {
            SourcePosition::Bottom  => (Point::new(0, 0), Anchor::TopLeft),
            SourcePosition::Top     => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Right   => (Point::new(0, h as i32), Anchor::BottomLeft),
            SourcePosition::Left    => (Point::new(w as i32 / 2, h as i32), Anchor::BottomLeft),
        };

        let txt = TextBox::new(&info_str, &self.font, self.texture_creator)
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
                if !p.is_processed(){
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


    /// Switch processed pane image to image processed with next command in 
    /// the list
    ///
    /// If we reached the end of the list, the function does nothing and returns 
    /// Ok(())
    pub fn next_cmd(&mut self) -> Result<(), String> {
        if self.cmd_index + 1 < self.cmds.len() {
            self.cmd_index += 1;
            self.load_processed_at_index()?;
            self.draw()?;
        }

        Ok(())
    }


    /// Switch processed pane image to image processed with previous command in 
    /// the list
    ///
    /// If we reached the begining of the list, the function does nothing and 
    /// returns Ok(())
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


    /// Undo the selection/validation of currently selected image
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


    /// Switches the application between fullscreen and normal
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


    /// Function to be ran in the main loop, it handles processing
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

