//! The goal of this program is batch image processing, mainly for compression 
//! but other types of processing are possible too. While it
//! is possible to use simple terminal commands to compress images without it,
//! the issue is that the terminal programs are blind, they simply process the
//! compression but the user doesn't know whether the compression has a positive 
//! or negative impact on the quality of the image or not.
//!
//! The solution proposed here is a program which can be fed a list of image
//! file names through stdin. These image will be compressed, and the
//! compressed result will be showed both the compressed and original images
//! side by side, and will be able to decide which image to keep with a simple
//! keyboard shotcut. Other features may include :
//!
//! [x] A setting to perform comparison continuously instead of duplicate image,
//!     i.e. the separation between left and right picure is the same column of 
//!     pixels for both pictures.
//!
//! [x] Possibility to have the split vertically or horizontally.
//!
//! [x] The stdout and stderr from external command should be output to stdout
//!     and stderr of the application.
//!
//! [x] Add padding option
//!
//! [x] The setting for the left/right top/down position should be merged with 
//!     the setting vertical/horizontal split. It could be an enum for the 
//!     position of the source file : Top/Bottom/Left/Right
//!
//! [x] Fix zoom bug when configured in SourcePosition::Bottom or 
//!     SourcePosition::Right
//!
//! [x] Ability to swap source and processed sides
//!
//! [x] The compression level can be changed on the fly so that, if it degrades
//!     the image or does not compress enough, this can easily be adapted per
//!     picture. However the level will remain to what it is for the following
//!     pictures, as when several pictures are stored in a folder it is likely 
//!     that they have the same format and would be affected more or less the
//!     same way by a same compression method. However bimgo will not
//!     attempt to sort the image by folder, they will be processed in the order
//!     fed through stdin. It is the responsability of the user to feed them in the
//!     order they want. We suggest using find or fd-find to feed path names.
//!
//! [x] Have an iterator provide the next index (i,j) for the next picture to 
//!     send to thread ? 
//!
//! [x] feh is much faster at loading image than us, what's up with that?
//!     -> Actually not really, it's because we load 2 images at once instead of 
//!     one. However, speed can be improved by preloading the next images.
//!
//! [x] Create a "nearest" iterator, which, given a grid and a position, yields 
//!     the index closest to given position on each iteration. Closest is 
//!     determined by the number of keystroke to reach it. This should create 
//!     a form of "spiral", with priority to non-diagonal cases.
//!
//! [x] Add ability to "rename" (i.e. move) across file systems, because 
//!     std rename doesn't allow that, and /tmp/ is in ram, which is another
//!     file system.
//!
//! [x] Allow a configuration switch to either use ram (via /tmp/) or hard disk 
//!     for processed file output. => The user can choose the tmp folder.
//!
//! [x] Multithreading could be used to process files faster.
//!
//! [x] When multithreading, when the buffer (e.g. 10 images) of files ahead are
//!     processed, start processing with the next (and previous) command in line, 
//!     so that if the user changes compression method, the changes appear
//!     instantaneous. => An iterator decides which are the command and image to 
//!     process next based on position.
//!
//! [x] possibility to undo or something
//!
//! [x] Add license
//!
//! [x] Nice border around the canvas that is kept to denote selection (e.g., 
//!     selected image's canvas is reduced and centered, and a colored border is 
//!     added. -> No reduction of image, simply a border around the selected 
//!     image.
//!
//! [-] Shortcut to zoom on pictures and ability to move with the mouse (when on
//!     top left corner, should zoom on top left, etc) or with keyboard shortcuts.
//!     zoom on both pictures at the same time so we can compare the quality.
//!
//! [-] Add the possibility to have a list of compression commands in a file.
//!     Keyboard shortcuts will move between compression commands. This makes it
//!     easy to customize compression levels, rather than to try and guess
//!     compression commands for the user.
//!
//! [-] Between image switch, option to : fit width, fit height, fit whole image, 
//!     fill screen (i.e. fit best?), keep zoom level, reset zoom level to 1.
//!
//! [ ] As a safety measure, when the compressed image is kept, the original
//!     image will be stored in a folder instead. The user will have to manually
//!     delete these pictures.
//!
//! [ ] It would be interesting to have a feature to go back to previous image
//!     and change the decision without having to manually delete the pictures.
//!
//! [ ] A geometry settings for the window size. Could be absolute pixel, or 
//!     screen ratio
//!
//! [ ] A position settings for the window, could be absolute value or screen
//!     ratio
//! 
//! [ ] Shift + hjkl moves 5 or 10 times as fast.
//!
//! [ ] Move speed depends on window size.
//!
//! [ ] Add a switch to reverse hjkl direction (image moves, or view moves).
//!
//! [ ] Option to enable processing of all available commands at once (maybe with
//!     a warning if commmand number is greater than 10 or so).
//!
//! [ ] Some statistical information on how fast the processing happenned for each
//!     image
//!
//! [ ] The file path below each image.
//!
//! [ ] Some feedback on actions.
//!
//! [ ] Holding space sets a second zoom level with the image location following 
//!     the pointer. Releasing space sets the image exactly to where it was.
//!
//! [ ] Zooming in and out while space is held changes the zoom factor of the
//!     space key, and it does not reset on any occasion.
//!
//! [ ] If the currently displayed image has not yet been processed, the other 
//!     half must have a loading symbol instead of the picture, and when it is 
//!     complete, the processed image must be loaded without user interaction.
//!
//! [ ] Ability to move the split bar left and right (or top and bottom)
//!
//! [ ] Preload the pictures in ram in addition to processing them, so that
//!     switching is faster.
//!
//! [ ] Functionnality to enable chess like background for transparent pictures
//!
//! [ ] Set threadpriority higher for pictures closer to current picture.
//!
//! [ ] A setting for how many images to process at once
//!
//! [ ] A setting for how many commands to process at once
//!
//! [ ] A setting for how many horizontal images to process at once (in the img list)
//!
//! [ ] A setting for how many vertical images to process at once (in the cmd list)
//!
//! [ ] Setting to allow mouse following zoom (like loop) to be always on, or
//!     or to be toggled by a press on space bar (or other).
//!
//! [ ] Make it so that, once all the closest images are processed, the iterator
//!     is allowed to move furhter to start preloading following images. This 
//!     may be done by counting how many images have been loaded up til now, how
//!     many images are currently loading, or something similar.
//!
//! [ ] For the color of the border that shows which image was validated, use
//!     the average color of the image below and then choose a color opposite
//!     to that? (might be uggly) or use two colors, an inner and an outer,
//!     this way we should at least see the outer.
//!
//! [ ] Store a list of files already validated, so that if the user uses the 
//!     same list of files, those are automatically filtered out. A command 
//!     should allow clearing the list, maybe with confirmation, or the user can 
//!     clear it manually. This is because otherwise, if some files have already 
//!     been processed, they will make the user spend more time for no reason.
//!     Instead of clearing the list, the user may simply decide to ignore it 
//!     for one session with a flag such as --ignored-processed.
//!
//! [ ] Change the behaviour so that, moving the images is only done at the end 
//!     before exiting, and after a double confirmation, instead of doing it on 
//!     the fly image after image. This seems safer as it does nothing until the 
//!     user has explicitly given his confirmation.
//!
//! [ ] Delete the tmp images when the user leaves the application. Alternatively 
//!     give the processed image an exhaustive name that includes the command used 
//!     to process the image, and then on the next run, we don't need to process 
//!     the image if it has already been processed (sort of cache)
//!
//! [ ] Implement some basic image processing in-app (e.g. compression), while 
//!     this would break a little the spirit of doing only one thing, and 
//!     relying on other applications for the rest, this would allow to use the 
//!     GPU and possibly yield huge performance gains. Though this is very far 
//!     down the road as my knowledge of GPU programming and image compression 
//!     is currently non-existent.
//!
//! [ ] Instead of trying to implement some of the processing commands, it would
//!     be interesting to look for command line image processing tools that work 
//!     with the GPU. Possibly changing the current command configuration method 
//!     to allow commands that already work in batch mode.
//! 
//! [ ] File in trash should be named based on folder location, using the "%" 
//!     separator instead of "/" (?), there is a small possibility that this fails 
//!     if filename already has % in itself (could maybe be fixed by adding an 
//!     escaping % sign if there is already a percent in the original path name, 
//!     as it is unlikely to have two slashes in the filename.)
//!
//! [ ] Fix issue where views don't get updated properly after toggling fullscreen
//!     because the windows parameter are not yet updated at the time of calling
//!     update_views.
//!
//! Other notes:
//! This program is based on sdl2 image example.
//! An example of usage would be :
//! fd /directory/*.png | bimgo

mod rect_utils;
mod application;
mod settings;
mod processing_order;
mod utils;

use std::path::PathBuf;

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Keycode;
use sdl2::image::InitFlag;
use sdl2::pixels::Color;

use itertools::Itertools;


//use std::env;

use application::App;
use settings::*;
use clap::Parser;

fn main() -> Result<(), String> {

    /* CLI initialization */ 
    let cli = Cli::parse();


    /* Initialization of SDL libary components. */
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let _image_context = sdl2::image::init(InitFlag::PNG | InitFlag::JPG)?;
    let ttf_context = sdl2::ttf::init().map_err(|e| format!("{e}"))?;
    
    


    // Create a window.
    let window = video_subsystem
        //.window("b*tchimgc", 1920, 1080) // Create a window with title and give geometry.
        .window("bimgo", 1600, 1200) // Create a window with title and give geometry.
        .resizable()
        .position_centered() // Centered on screen.
        .build() // Apply and build window.
        .map_err(|e| e.to_string())?; // Store in window variable or return error as a string.


    // Consumes window and build a canvas.
    let mut canvas = window
        .into_canvas()
        .software() // Enable software fallback renderer flag.
        .build() // Apply and build canvas.
        .map_err(|e| e.to_string())?; // Store in canvas variable or return error as string.
    let texture_creator = canvas.texture_creator();

    let mut evt_pump = sdl_context.event_pump()?;


    /* Here starts the application code */

    //let mut first_file = String::new();
    //stdin().read_line(&mut first_file).expect("Could not read stdin");

    // Temporary list of img for testing. In final version this will come from 
    // stdin
    use utils::*;
    let img_list_file = expand_tilde("~/bimgo/img_list")
        .expect("img_list file not found");
    let img_list: Vec<PathBuf> = 
        read_file_lines(&img_list_file)
        .expect("Unable to parse image list").into_iter()
        .map(PathBuf::from)
        .collect();

    let mut app = App::new(&mut canvas, &texture_creator, &ttf_context, img_list)?;

    'mainloop: loop {
        app.run()?;

        // We skip events that are of same variant and only keep one (here the
        // first even though it would be preferable to only keep the last).
        let evts = evt_pump
            .poll_iter()
            .dedup_by(|a, b| std::mem::discriminant(a) == std::mem::discriminant(b));

        for event in evts {
            //println!("Event received: {event:?}");
            match event {
                Event::Quit { .. }
                | Event::KeyDown {keycode: Option::Some(Keycode::Escape), .. }
                | Event::KeyDown {keycode: Option::Some(Keycode::Q), .. } 
                    => break 'mainloop,

                Event::KeyDown {keycode: Option::Some(Keycode::Semicolon), .. } 
                    => app.next_image()?,
                    
                Event::KeyDown {keycode: Option::Some(Keycode::Comma), .. } 
                    => app.prev_image()?,

                Event::KeyDown {keycode: Option::Some(Keycode::N), .. } 
                    => app.next_cmd()?,
                    
                Event::KeyDown {keycode: Option::Some(Keycode::P), .. } 
                    => app.prev_cmd()?,

                Event::KeyDown {keycode: Option::Some(Keycode::Space), .. } 
                    => app.validate_current()?,

                Event::KeyDown {keycode: Option::Some(Keycode::U), .. } 
                    => app.undo_current()?,

                Event::KeyDown {keycode: Option::Some(Keycode::O), .. } 
                    => app.zoom_in()?,

                Event::KeyDown {keycode: Option::Some(Keycode::I), .. } 
                    => app.zoom_out()?,

                Event::KeyDown {keycode: Option::Some(Keycode::H), .. } 
                    => app.pan_left()?,

                Event::KeyDown {keycode: Option::Some(Keycode::J), .. } 
                    => app.pan_down()?,

                Event::KeyDown {keycode: Option::Some(Keycode::K), .. } 
                    => app.pan_up()?,

                Event::KeyDown {keycode: Option::Some(Keycode::L), .. } 
                    => app.pan_right()?,

                Event::KeyDown {keycode: Option::Some(Keycode::F), .. } 
                    => app.toggle_fullscreen()?,
                    
                Event::Window  {win_event: WindowEvent::SizeChanged(_, _), .. } 
                    => app.update_views()?,

                Event::KeyDown {keycode: Option::Some(Keycode::S), .. } 
                    => app.update_views()?,

                Event::MouseMotion { x, y, .. }
                    // => app.pan_mouse_relative(x, y)?,
                    => (),

                _ => (),
            }
        }
    }

    Ok(())
}

