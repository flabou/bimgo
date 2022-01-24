//! This modules extends the sdl2::Rect object with custom functionnalities.

use sdl2::rect::{Rect, Point};

trait RectExt {
    fn aspect_ratio(&self) -> f32;
    fn scale(&mut self, scale: f32);
    fn scaled(&self, scale: f32) -> Rect;
}

impl RectExt for Rect {
    /// Returns the aspect ratio (= width/height) of the rectangle as an f32.
    fn aspect_ratio(&self) -> f32 {
        self.width() as f32 / self.height() as f32
    }

    /// Resizes the rectangle according to scale factor.
    fn scale(&mut self, scale: f32) {
        self.set_width((self.width() as f32 * scale) as u32);
        self.set_height((self.height() as f32 * scale) as u32);
    }

    /// Resizes the rectangle according to scale factor.
    fn scaled(&self, scale: f32) -> Rect {
        let mut scaled_rect = *self;
        scaled_rect.set_width((self.width() as f32 * scale) as u32);
        scaled_rect.set_height((self.height() as f32 * scale) as u32);

        scaled_rect
    }
}

/// This struct facilitates the positionning, moving, clipping, and zooming 
/// of textures that get copied to the sdl2 canvas with sdl2 copy.
/// This is achieved through a set of sdl2 Rectangles.
/// - img_rect  must contain the width and size of the image in pixels. 
///   Positions doesn't matter.
/// - clip_rect must contain the section of the window where the texture may 
///   be displayed. 
/// - virt_rect will contains the location where the image would be 
///   displayed without taking clipping into account (i.e. the location of 
///   the image if clip_rect was infinite).
/// - src_rect  will contain the src_rect to pass to the copy function.
/// - dst_rect  will contain the dst_rect to pass to the copy function.
///
/// A set of methods is provided to facilitate actions on the many 
/// rectangles.
#[allow(dead_code)]
pub struct ViewRect {
    /// Image rectangle, used to find the aspect ratio.
    img_rect: Rect,     

    /// Section of the window where image may be dipslayed.
    pub clip_rect: Rect,    

    /// Location of the image if clip_rect was inifinite.
    pub virt_rect: Rect,    
    
    /// src Rect of the texture copy function.
    pub src_rect: Rect,     

    /// dst Rect of the texture copy function.
    pub dst_rect: Rect,     
    
}

impl Default for ViewRect {
    fn default() -> ViewRect {
        let empty_rect = Rect::new(0,0,1,1);
        ViewRect {
            img_rect: empty_rect,
            clip_rect: empty_rect,
            virt_rect: empty_rect,
            src_rect: empty_rect,
            dst_rect: empty_rect,
        }
    }
}

#[allow(dead_code)]
impl ViewRect {
    pub fn new(img_size: (u32, u32), clip_rect: Rect) -> ViewRect {
        let (w, h) = img_size;

        let img_rect = Rect::new(0, 0, w, h);

        let mut view = ViewRect {
            img_rect,
            clip_rect,
            virt_rect: img_rect,

            src_rect: img_rect,
            dst_rect: clip_rect, 
        };

        view.set_img_rect(img_rect);

        view.virt_rect.set_x(0);
        view.virt_rect.set_y(0);

        view.update();

        view
    }

    /// Synchronize, zoom factor and relative position with other ViewRect.
    /// Does not synchronize clip_rect.
    pub fn sync_duplicate_with(&mut self, other: &ViewRect){
        let pt = self.clip_rect.top_left() - other.clip_rect.top_left();
        let (x, y) = (pt.x, pt.y);

        let mut new_virt_rect = other.virt_rect;
        new_virt_rect.offset(x, y);
        self.virt_rect = new_virt_rect;
        self.update();
    }

    /// Synchronize in a way that makes the view continuous left, to write.
    pub fn sync_continuous_with(&mut self, other: &ViewRect) {
        self.virt_rect = other.virt_rect;
        self.update();
    }

    pub fn set_img_rect(&mut self, img_rect: Rect){
        self.img_rect = img_rect;
        self.virt_rect = self.img_rect;
        self.virt_rect.reposition(Point::new(0,0));
        self.update();
    }

    pub fn set_virt_rect(&mut self, virt_rect: Rect){
        self.virt_rect = virt_rect;
        self.update();
    }

    pub fn set_clip_rect(&mut self, clip_rect: Rect) {
        self.clip_rect = clip_rect;
    }

    /// Returns the zoom factor
    fn zoom_factor(&self) -> f32 {
        self.virt_rect.width() as f32 / self.img_rect.width() as f32
    }

    pub fn fit_width_to_rect(&mut self, fit_rect: Rect){
        self.virt_rect.set_width(fit_rect.width());
        self.set_height_from_width();
        self.virt_rect.center_on(fit_rect.center());
        self.update();
    }

    /// Fit the width of the image to the width of the 
    fn fit_width(&mut self){
        self.fit_width_to_rect(self.clip_rect);
    }

    pub fn fit_height_to_rect(&mut self, fit_rect: Rect) {
        self.virt_rect.set_height(fit_rect.height());
        self.set_width_from_height();
        self.virt_rect.center_on(fit_rect.center());
        self.update();
    }

    fn fit_height(&mut self){
        todo!();
    }

    pub fn fit_best_to_rect(&mut self, fit_rect: Rect) {
        if self.img_rect.aspect_ratio() > fit_rect.aspect_ratio() {
            self.fit_width_to_rect(fit_rect);
        } else {
            self.fit_height_to_rect(fit_rect);
        }
    }

    pub fn fit_fill_to_rect(&mut self, fit_rect: Rect) {
        if self.img_rect.aspect_ratio() > fit_rect.aspect_ratio() {
            self.fit_height_to_rect(fit_rect);
        } else {
            self.fit_width_to_rect(fit_rect);
        }
    }

    fn set_height_from_width(&mut self){
        self.virt_rect.set_height((self.virt_rect.width() as f32 / self.img_rect.aspect_ratio()).round() as u32);
    }

    fn set_width_from_height(&mut self){
        self.virt_rect.set_width((self.virt_rect.height() as f32 * self.img_rect.aspect_ratio()).round() as u32);
    }

    /// Updates the src and dst rectangles.
    fn update(&mut self){

        // Determine what part of the virtual scaled image is visible.
        if let Some(intersecting_rect) = self.clip_rect.intersection(self.virt_rect) {
            self.dst_rect = intersecting_rect;
            // let mut src_rect = intersecting_rect.scaled(1./self.zoom_factor());

            let mut src_rect = Rect::new(0, 0, 
                (intersecting_rect.width() as f32 / self.virt_rect.width() as f32 * self.img_rect.width() as f32) as u32,
                (intersecting_rect.height() as f32 / self.virt_rect.height() as f32 * self.img_rect.height() as f32) as u32,
            );
            //src_rect.set_x(((self.clip_rect.left() - self.virt_rect.left()) as f32 / self.virt_rect.width() as f32 * self.img_rect.width() as f32) as i32);
            //src_rect.set_y(((self.clip_rect.top() - self.virt_rect.top()) as f32 / self.virt_rect.height() as f32 * self.img_rect.height() as f32) as i32);

            src_rect.set_x(((self.clip_rect.left() - self.virt_rect.left()) as f32 / self.virt_rect.width() as f32 * self.img_rect.width() as f32) as i32);
            src_rect.set_y(((self.clip_rect.top() - self.virt_rect.top()) as f32 / self.virt_rect.height() as f32 * self.img_rect.height() as f32) as i32);

            if src_rect.x <= 0 {
                src_rect.set_x(0);
            }

            if src_rect.y <= 0 {
                src_rect.set_y(0);
            }


            self.src_rect = src_rect;
        }

    }
    
    fn pan_xy(&mut self, x: i32, y: i32){
        self.virt_rect.offset(-x,-y);
        if self.virt_rect.left() > self.clip_rect.right(){
            self.virt_rect.set_x(self.clip_rect.right()-1);
        }

        if self.virt_rect.right() < self.clip_rect.left(){
            self.virt_rect.set_right(self.clip_rect.left()+1);
        }

        if self.virt_rect.top() > self.clip_rect.bottom(){
            self.virt_rect.set_y(self.clip_rect.bottom()-1);
        }

        if self.virt_rect.bottom() < self.clip_rect.top(){
            self.virt_rect.set_bottom(self.clip_rect.top()+1);
        }

        self.update();
    }

    fn pan_x(&mut self, x: i32){
       self.pan_xy(x, 0);
       self.update();
    }

    fn pan_y(&mut self, y: i32){
       self.pan_xy(0, y);
       self.update();
    }

    /// Move left by n pixels. It is the view that moves and not the image (like
    /// if the view is a camera that is moving to the left and showing the left
    /// side of the picture).
    pub fn pan_left(&mut self, x: u32){
        self.pan_x(x as i32);
    }

    pub fn pan_right(&mut self, x: u32){
        self.pan_x(-(x as i32));
    }

    pub fn pan_up(&mut self, y: u32){
        self.pan_y(y as i32);
    }

    pub fn pan_down(&mut self, y: u32){
        self.pan_y(-(y as i32));
    }
    
    /// Zoom in on texture, while attempting to keep point at the same 
    /// coordinates. Point coordinates are relative to provided Rect.
    pub fn zoom_towards_point_on_rect(&mut self, pt: Point, rect: Rect, scale: f32){
        
        // Compute the position of the point relative to virt_rect.
        let point_virt_rect_distance = pt  + rect.top_left() - self.virt_rect.top_left();
        
        // Guess what the next position of the virtual rectangle should be after scaling
        let next_distance_x = (point_virt_rect_distance.x as f32 * scale).round() as i32;
        let next_distance_y = (point_virt_rect_distance.y as f32 * scale).round() as i32;
        let next_point_virt_rect_distance = Point::new(next_distance_x, next_distance_y);

        let offset = point_virt_rect_distance - next_point_virt_rect_distance;

        self.virt_rect.set_width((self.virt_rect.width() as f32 * scale).round() as u32);
        self.set_height_from_width();

        // Now correct by offseting rectangle with the difference between what 
        // is and what should be.
        self.virt_rect.offset(offset.x, offset.y);
        self.update();
    }

    /// Zoom in on texture, while attempting to keep point at the same 
    /// coordinates. Point coordinates are relative to clip_rect.
    pub fn zoom_towards_point(&mut self, pt: Point, scale: f32){
        self.zoom_towards_point_on_rect(pt, self.clip_rect, scale);   
    }

    // Zoom towards center of the canvas
    pub fn zoom_towards_view_center(&mut self, scale: f32){
        self.zoom_towards_point(self.clip_rect.center(), scale);
    }
}
