#![allow(dead_code)]

use std::path::{Path, PathBuf};
use clap::Parser;
use serde::Deserialize;
use std::fs;
use std::io;

use crate::utils::expand_tilde;

/// Setting to select the image fitting method, applied when switching image. 
/// - FitWidth  fits the image to the width of the window/split (depends 
///   on DisplayMode).
/// - FitHeight fits the image to the height of the window/split (depends 
///   on DisplayMode).
/// - FitBest   automatically selects FitWidth or FitHeight in order to view
///   the whole image in window/split.
/// - Fill      automatically selects FitWidth or FitHeight in order fill the 
///   whole window/split with image.
/// - ClearZoom resets the zoom to 1, showing the real size of the image.
/// - KeepZoom  keeps the same zoom level.
/// - NoFit     Does nothing.
#[derive(Deserialize)]
pub enum FitMode {
    FitWidth,
    FitHeight,
    FitBest,
    Fill,
    KeepZoom,
    ClearZoom,
    NoFit,
}

impl Default for FitMode { fn default() -> Self { FitMode::FitBest } }

/// Setting to select whether the image is duplicated on both sections or 
/// continued from one section to the next.
#[derive(Deserialize)]
pub enum DisplayMode {
    Duplicate,
    Continuous,
}

impl Default for DisplayMode { fn default() -> Self { DisplayMode::Continuous } }


/// The position of the source image on the screen.
///
/// This controls wether the screen is split vertically or horizontally as
/// well.
#[derive(Deserialize)]
pub enum SourcePosition {
    Top,
    Bottom,
    Left,
    Right,
}
impl Default for SourcePosition { fn default() -> Self { SourcePosition::Left } }


/// Setting to choose whether movement key move the image, or the view (i.e.
/// in image mode, up moves image up, while in View mode, up moves image down).
#[derive(Deserialize)]
pub enum MoveMode {
    Image,
    View,
}
impl Default for MoveMode { fn default() -> Self { MoveMode::Image } }


#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(long)]
    /// Location of the configuration file.
    #[clap(default_value_t = String::from("~/.config/bimgo/bimgo.toml"))]
    config: String,
    
}

/// Struct that stores the commands, which are loaded from a file.
pub struct Commands {
    pub cmds: Vec<String>,
}

/// Settings of the app, some of these will be loaded from the config file, 
/// possibly overwritten from command line arguments.
#[derive(Default, Deserialize)]
pub struct AppSettings{

    #[serde(default = "default_processing_directory")]
    pub processing_directory: PathBuf,

    #[serde(default = "default_trash_directory")]
    pub trash_directory: PathBuf,

    #[serde(default = "default_cmd_file")]
    pub cmds_file: PathBuf,

    #[serde(default)]
    pub display_mode: DisplayMode,

    #[serde(default)]
    pub source_position: SourcePosition,

    #[serde(default)]
    pub fit_mode: FitMode,

    #[serde(default)]
    pub padding: u32,

    #[serde(default)]
    pub move_mode: MoveMode,
}

impl AppSettings {

    pub fn new() -> io::Result<AppSettings> {
        let config_path = expand_tilde("~/.config/bimgo/bimgo.toml")?;
        let mut settings = Self::from_file(&config_path)?;
        
        settings.expand_home()?;

        Ok(settings)
    }


    /// Expands ~ to home in settings
    fn expand_home(&mut self) -> io::Result<()> {
        self.processing_directory = expand_tilde(&self.processing_directory)?;
        self.trash_directory = expand_tilde(&self.trash_directory)?;
        self.cmds_file = expand_tilde(&self.cmds_file)?;

        Ok(())
    }

    /// Atempts to read config file at provided path.
    pub fn from_file(config_file: &Path) -> io::Result<AppSettings> {
        let config_string = fs::read_to_string(config_file)?;

        toml::from_str(&config_string)
            .map_err(|e| 
                     io::Error::new(io::ErrorKind::InvalidData, format!("Unable to parse config file: {e}")))
    }
}

fn default_processing_directory() -> PathBuf { PathBuf::from("/tmp/") }
fn default_trash_directory() -> PathBuf { PathBuf::from("~/.local/share/bimgo/trash")}
fn default_cmd_file() -> PathBuf { PathBuf::from("~/.config/bimgo/cmds")}


#[test]
fn verify_app() {
    use clap::IntoApp;
    Cli::into_app().debug_assert()
}
