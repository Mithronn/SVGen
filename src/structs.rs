use std::{error::Error, ffi::OsStr};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// An enumeration of supported image formats.
/// Not all formats support both encoding and decoding.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[non_exhaustive]
pub enum ImageFormat {
    /// An Image in PNG Format
    Png,

    /// An Image in JPEG Format
    Jpeg,

    /// An Image in GIF Format
    Gif,

    /// An Image in WEBP Format
    WebP,

    /// An Image in general PNM Format
    Pnm,

    /// An Image in TIFF Format
    Tiff,

    /// An Image in TGA Format
    Tga,

    /// An Image in DDS Format
    Dds,

    /// An Image in BMP Format
    Bmp,

    /// An Image in ICO Format
    Ico,

    /// An Image in Radiance HDR Format
    Hdr,

    /// An Image in OpenEXR Format
    OpenExr,

    /// An Image in farbfeld Format
    Farbfeld,

    /// An Image in AVIF Format
    Avif,

    /// An Image in QOI Format
    Qoi,

    /// An Image in PCX Format
    Pcx,
}

/// A best effort representation for image formats.
#[derive(Clone, Debug, Hash, PartialEq)]
#[non_exhaustive]
pub enum ImageFormatHint {
    /// The format is known exactly.
    Exact(ImageFormat),

    /// The format can be identified by a name.
    Name(String),

    /// A common path extension for the format is known.
    PathExtension(std::path::PathBuf),

    /// The format is not known or could not be determined.
    Unknown,
}

#[derive(Debug)]
pub struct DecodeError {
    pub format: ImageFormatHint,
    pub underlying: Option<Box<dyn Error + Send + Sync>>,
}

pub type DecodeResult<T> = Result<T, DecodeError>;

pub struct DecodeImage {
    pub pixels: Vec<Pixel>,
    pub width: u32,
    pub height: u32,
}

impl ImageFormat {
    /// Return the image format specified by a path's file extension.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let format = ImageFormat::from_extension("jpg");
    /// assert_eq!(format, Some(ImageFormat::Jpeg));
    /// ```
    #[inline]
    pub fn from_extension<S>(ext: S) -> Option<Self>
    where
        S: AsRef<OsStr>,
    {
        // thin wrapper function to strip generics
        fn inner(ext: &OsStr) -> Option<ImageFormat> {
            let ext = ext.to_str()?.to_ascii_lowercase();

            Some(match ext.as_str() {
                "avif" => ImageFormat::Avif,
                "jpg" | "jpeg" | "jfif" => ImageFormat::Jpeg,
                "png" | "apng" => ImageFormat::Png,
                "gif" => ImageFormat::Gif,
                "webp" => ImageFormat::WebP,
                "tif" | "tiff" => ImageFormat::Tiff,
                "tga" => ImageFormat::Tga,
                "dds" => ImageFormat::Dds,
                "bmp" => ImageFormat::Bmp,
                "ico" => ImageFormat::Ico,
                "hdr" => ImageFormat::Hdr,
                "exr" => ImageFormat::OpenExr,
                "pbm" | "pam" | "ppm" | "pgm" => ImageFormat::Pnm,
                "ff" => ImageFormat::Farbfeld,
                "qoi" => ImageFormat::Qoi,
                "pcx" => ImageFormat::Pcx,
                _ => return None,
            })
        }

        inner(ext.as_ref())
    }

    /// Return the image format specified by a MIME type.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let format = ImageFormat::from_mime_type("image/png").unwrap();
    /// assert_eq!(format, ImageFormat::Png);
    /// ```
    pub fn from_mime_type<M>(mime_type: M) -> Option<Self>
    where
        M: AsRef<str>,
    {
        match mime_type.as_ref() {
            "image/avif" => Some(ImageFormat::Avif),
            "image/jpeg" => Some(ImageFormat::Jpeg),
            "image/png" => Some(ImageFormat::Png),
            "image/gif" => Some(ImageFormat::Gif),
            "image/webp" => Some(ImageFormat::WebP),
            "image/tiff" => Some(ImageFormat::Tiff),
            "image/x-targa" | "image/x-tga" => Some(ImageFormat::Tga),
            "image/vnd-ms.dds" => Some(ImageFormat::Dds),
            "image/bmp" => Some(ImageFormat::Bmp),
            "image/x-icon" => Some(ImageFormat::Ico),
            "image/vnd.radiance" => Some(ImageFormat::Hdr),
            "image/x-exr" => Some(ImageFormat::OpenExr),
            "image/x-portable-bitmap"
            | "image/x-portable-graymap"
            | "image/x-portable-pixmap"
            | "image/x-portable-anymap" => Some(ImageFormat::Pnm),
            // Qoi's MIME type is being worked on.
            // See: https://github.com/phoboslab/qoi/issues/167
            "image/x-qoi" => Some(ImageFormat::Qoi),
            "image/vnd.zbrush.pcx" | "image/x-pcx" => Some(ImageFormat::Pcx),
            _ => None,
        }
    }

    /// Return the MIME type for this image format or "application/octet-stream" if no MIME type
    /// exists for the format.
    ///
    /// Some notes on a few of the MIME types:
    ///
    /// - The portable anymap format has a separate MIME type for the pixmap, graymap and bitmap
    ///   formats, but this method returns the general "image/x-portable-anymap" MIME type.
    /// - The Targa format has two common MIME types, "image/x-targa"  and "image/x-tga"; this
    ///   method returns "image/x-targa" for that format.
    /// - The QOI MIME type is still a work in progress. This method returns "image/x-qoi" for
    ///   that format.
    ///
    /// # Example
    ///
    /// ```
    /// use image::ImageFormat;
    ///
    /// let mime_type = ImageFormat::Png.to_mime_type();
    /// assert_eq!(mime_type, "image/png");
    /// ```
    #[must_use]
    pub fn to_mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Avif => "image/avif",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
            ImageFormat::Tiff => "image/tiff",
            // the targa MIME type has two options, but this one seems to be used more
            ImageFormat::Tga => "image/x-targa",
            ImageFormat::Dds => "image/vnd-ms.dds",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Hdr => "image/vnd.radiance",
            ImageFormat::OpenExr => "image/x-exr",
            // return the most general MIME type
            ImageFormat::Pnm => "image/x-portable-anymap",
            // Qoi's MIME type is being worked on.
            // See: https://github.com/phoboslab/qoi/issues/167
            ImageFormat::Qoi => "image/x-qoi",
            // farbfeld's MIME type taken from https://www.wikidata.org/wiki/Q28206109
            ImageFormat::Farbfeld => "application/octet-stream",
            ImageFormat::Pcx => "image/vnd.zbrush.pcx",
        }
    }

    /// Return if the `ImageFormat` can be decoded by the lib.
    #[inline]
    #[must_use]
    pub fn can_read(&self) -> bool {
        // Needs to be updated once a new variant's decoder is added to free_functions.rs::load
        match self {
            ImageFormat::Png => true,
            ImageFormat::Gif => true,
            ImageFormat::Jpeg => true,
            ImageFormat::WebP => true,
            ImageFormat::Tiff => true,
            ImageFormat::Tga => true,
            ImageFormat::Dds => false,
            ImageFormat::Bmp => true,
            ImageFormat::Ico => true,
            ImageFormat::Hdr => true,
            ImageFormat::OpenExr => true,
            ImageFormat::Pnm => true,
            ImageFormat::Farbfeld => true,
            ImageFormat::Avif => true,
            ImageFormat::Qoi => true,
            ImageFormat::Pcx => true,
        }
    }

    /// Return if the `ImageFormat` can be encoded by the lib.
    #[inline]
    #[must_use]
    pub fn can_write(&self) -> bool {
        // Needs to be updated once a new variant's encoder is added to free_functions.rs::save_buffer_with_format_impl
        match self {
            ImageFormat::Gif => true,
            ImageFormat::Ico => true,
            ImageFormat::Jpeg => true,
            ImageFormat::Png => true,
            ImageFormat::Bmp => true,
            ImageFormat::Tiff => true,
            ImageFormat::Tga => true,
            ImageFormat::Pnm => true,
            ImageFormat::Farbfeld => true,
            ImageFormat::Avif => true,
            ImageFormat::WebP => true,
            ImageFormat::Hdr => true,
            ImageFormat::OpenExr => true,
            ImageFormat::Dds => false,
            ImageFormat::Qoi => true,
            ImageFormat::Pcx => false,
        }
    }

    /// Return a list of applicable extensions for this format.
    ///
    /// All currently recognized image formats specify at least on extension but for future
    /// compatibility you should not rely on this fact. The list may be empty if the format has no
    /// recognized file representation, for example in case it is used as a purely transient memory
    /// format.
    ///
    /// The method name `extensions` remains reserved for introducing another method in the future
    /// that yields a slice of `OsStr` which is blocked by several features of const evaluation.
    #[must_use]
    pub fn extensions_str(self) -> &'static [&'static str] {
        match self {
            ImageFormat::Png => &["png"],
            ImageFormat::Jpeg => &["jpg", "jpeg"],
            ImageFormat::Gif => &["gif"],
            ImageFormat::WebP => &["webp"],
            ImageFormat::Pnm => &["pbm", "pam", "ppm", "pgm"],
            ImageFormat::Tiff => &["tiff", "tif"],
            ImageFormat::Tga => &["tga"],
            ImageFormat::Dds => &["dds"],
            ImageFormat::Bmp => &["bmp"],
            ImageFormat::Ico => &["ico"],
            ImageFormat::Hdr => &["hdr"],
            ImageFormat::OpenExr => &["exr"],
            ImageFormat::Farbfeld => &["ff"],
            // According to: https://aomediacodec.github.io/av1-avif/#mime-registration
            ImageFormat::Avif => &["avif"],
            ImageFormat::Qoi => &["qoi"],
            ImageFormat::Pcx => &["pcx"],
        }
    }

    /// Return all `ImageFormat`s
    pub fn all() -> impl Iterator<Item = ImageFormat> {
        [
            ImageFormat::Gif,
            ImageFormat::Ico,
            ImageFormat::Jpeg,
            ImageFormat::Png,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
            ImageFormat::Tga,
            ImageFormat::Pnm,
            ImageFormat::Farbfeld,
            ImageFormat::Avif,
            ImageFormat::WebP,
            ImageFormat::OpenExr,
            ImageFormat::Qoi,
            ImageFormat::Dds,
            ImageFormat::Hdr,
            ImageFormat::Pcx,
        ]
        .iter()
        .copied()
    }
}
