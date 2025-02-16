use core::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct Chunk {
    pub length: u32,
    pub type_str: String,
    pub data: Vec<u8>,
    pub crc: u32,
}

#[derive(Debug)]
pub struct IHDR {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub color_type: u8,
    pub compression_method: u8,
    pub filter_method: u8,
    pub interlace_method: u8,
}

#[derive(Debug)]
pub struct AnimationControl {
    pub num_frames: u32,
    pub num_plays: u32, // 0 = infinite loop
}

#[derive(Debug)]
pub struct FrameControl {
    pub sequence_number: u32,
    pub width: u32,
    pub height: u32,
    pub x_offset: u32,
    pub y_offset: u32,
    pub delay_num: u16, // Delay numerator (in 1/100 seconds)
    pub delay_den: u16, // Delay denominator
    pub dispose_op: u8, // Disposal operation (0-3)
    pub blend_op: u8,   // Blend operation (0-1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

// A simple 2D point with basic arithmetic.
#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    pub fn add(&self, other: Point) -> Point {
        Point::new(self.x + other.x, self.y + other.y)
    }

    pub fn sub(&self, other: Point) -> Point {
        Point::new(self.x - other.x, self.y - other.y)
    }

    pub fn mul(&self, scalar: f64) -> Point {
        Point::new(self.x * scalar, self.y * scalar)
    }

    pub fn div(&self, scalar: f64) -> Point {
        Point::new(self.x / scalar, self.y / scalar)
    }

    pub fn dot(&self, other: Point) -> f64 {
        self.x * other.x + self.y * other.y
    }

    pub fn norm(&self) -> f64 {
        self.dot(*self).sqrt()
    }

    pub fn distance(&self, other: Point) -> f64 {
        self.sub(other).norm()
    }

    pub fn normalize(&self) -> Point {
        let n = self.norm();
        if n.abs() < 1e-6 {
            *self
        } else {
            self.div(n)
        }
    }
}

// A cubic Bézier curve represented by four control points.
#[derive(Debug, Clone)]
pub struct CubicBezier {
    pub p0: Point,
    pub p1: Point,
    pub p2: Point,
    pub p3: Point,
}

impl CubicBezier {
    // Evaluate the Bézier curve at parameter t (0 <= t <= 1)
    pub fn evaluate(&self, t: f64) -> Point {
        let u = 1.0 - t;
        // Bernstein basis form:
        self.p0
            .mul(u * u * u)
            .add(self.p1.mul(3.0 * u * u * t))
            .add(self.p2.mul(3.0 * u * t * t))
            .add(self.p3.mul(t * t * t))
    }
}

#[derive(Debug, Clone)]
pub enum Segment {
    Line { start: Point, end: Point },
    Cubic(CubicBezier),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum ImageFormat {
    Png,
    Jpeg,
    Bmp,
    WebP,
    /// The format is not known or could not be determined.
    #[default]
    Unknown,
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ImageFormat::Png => write!(f, "png"),
            ImageFormat::Jpeg => write!(f, "jpeg"),
            ImageFormat::Bmp => write!(f, "bmp"),
            ImageFormat::WebP => write!(f, "webp"),
            ImageFormat::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

#[derive(Debug)]
pub struct DecodeError {
    pub format: ImageFormat,
    pub underlying: Option<Box<dyn Error + Send + Sync>>,
}

pub type DecodeResult = Result<DecodeImage, DecodeError>;

#[derive(Clone, Default)]
pub struct DecodeImage {
    pub pixels: Vec<Pixel>,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
}

impl DecodeImage {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn new_with(width: u32, height: u32) -> Self {
        Self {
            pixels: Vec::with_capacity((width * height) as usize),
            width,
            height,
            format: Default::default(),
        }
    }

    pub fn get_pixel(&self, x: usize, y: usize) -> Pixel {
        let index = y * self.width as usize + x;
        self.get_pixel_at(index)
    }

    pub fn get_pixel_at(&self, index: usize) -> Pixel {
        self.pixels[index]
    }

    pub fn try_get_pixel(&self, x: usize, y: usize) -> Option<Pixel> {
        let index = y * self.width as usize + x;
        self.try_get_pixel_at(index)
    }

    pub fn try_get_pixel_at(&self, index: usize) -> Option<Pixel> {
        self.pixels.get(index).copied()
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, pixel: &Pixel) {
        let index = y * self.width as usize + x;
        self.set_pixel_at(index, pixel);
    }

    pub fn set_pixel_at(&mut self, index: usize, pixel: &Pixel) {
        self.pixels[index] = pixel.clone();
    }

    pub fn try_set_pixel(&mut self, x: usize, y: usize, pixel: &Pixel) {
        let index = y * self.width as usize + x;
        self.try_set_pixel_at(index, pixel);
    }

    pub fn try_set_pixel_at(&mut self, index: usize, pixel: &Pixel) {
        if let Some(pixel_at) = self.pixels.get_mut(index) {
            *pixel_at = pixel.clone();
        }
    }
}
