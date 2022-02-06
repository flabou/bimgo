//! Module with generic helping code related to SDL

use sdl2::rect::{Rect,Point};
use sdl2::pixels::Color;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::ttf::Font;


#[allow(unused)]
pub enum Anchor{
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
pub struct TextBox<'a, T> {
    texture_creator: &'a TextureCreator<T>,
    font: &'a Font<'a, 'a>,
    txt: &'a str,
    width: Option<u32>,
}


impl<'a, T> TextBox<'a, T>{
    pub fn new(txt: &'a str, font: &'a Font, texture_creator: &'a TextureCreator<T>) -> TextBox<'a, T> {
        TextBox{
            texture_creator,
            font,
            txt,
            width: None,
        }
    }

    pub fn wrapped(mut self, width: u32) -> Self {
        self.width = Some(width);
        self
    }

    pub fn draw<C>(&self, canvas: &mut Canvas<C>, position: Point, anchor: Anchor) -> Result<(), String>
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
