pub mod constants;
pub mod parsers;
pub mod structs;
pub mod utils;

use std::{
    collections::HashMap,
    io::{BufReader, Cursor},
    path::Path,
};

use image::{GrayImage, ImageReader};
use imageproc::{
    contours::{find_contours_with_threshold, Contour},
    edges::canny,
};
use log::{info, trace, warn};
use svg::{
    node::element::{
        path::{Command, Data, Position},
        Definitions, Group, Path as SVGPath, Use,
    },
    Document, Node,
};

use parsers::{bytes_to_pixels, parse_chunks, parse_ihdr, parse_plte, parse_trns, read_png};
use structs::{DecodeImage, DecodeResult, ImageFormat, Point, Segment};
use utils::{decompress_idat, fit_curve, generate_id, rdp, trunc};

pub fn decode<P>(path: P) -> DecodeResult
where
    P: AsRef<Path>,
{
    trace!("Image decode");

    let buffer = read_png(path.as_ref()).unwrap();
    let chunks = parse_chunks(&buffer);

    info!("Parsed {} image chunks", chunks.len());

    // Extract IHDR, PLTE, tRNS
    let ihdr_chunk = chunks.iter().find(|c| c.type_str == "IHDR").unwrap();
    let ihdr = parse_ihdr(&ihdr_chunk.data);
    let plte = parse_plte(&chunks);
    let trns = parse_trns(&chunks);

    info!("Image Header(IHDR): {ihdr:?}");

    // TODO: Other formats need to check "jpeg, bmp, webp"
    let format = ImageFormat::Png;

    let idat_data: Vec<u8> = chunks
        .iter()
        .filter(|c| c.type_str == "IDAT")
        .flat_map(|c| c.data.clone())
        .collect();

    let decompressed = decompress_idat(&idat_data);
    let pixels = bytes_to_pixels(&decompressed, &ihdr, &plte, &trns);

    info!("Decoded {} image {}x{}", format, ihdr.width, ihdr.height);

    Ok(DecodeImage {
        pixels,
        format,
        width: ihdr.width,
        height: ihdr.height,
    })
}

pub fn create_svg(image_byte: &[u8]) -> String {
    trace!("SVG Creation");

    let image_reader = ImageReader::new(BufReader::new(Cursor::new(image_byte)))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap();

    info!(
        "Image readed {}x{}",
        image_reader.width(),
        image_reader.height()
    );

    let mut gray_image = image_reader.to_luma8();

    if gray_image.width() * gray_image.height() < 512 * 512 {
        gray_image = upscale_image(&gray_image, 3);
        warn!(
            "Image size is small. Upscalled to {}x{}",
            gray_image.width(),
            gray_image.height()
        );
    }

    let low_threshold = 10.0 as f32;
    let high_threshold = 60.0 as f32;

    // TODO: FEATURE maybe ?
    // let otsu_thresh = otsu_level(&gray_image);
    // let low_threshold = otsu_thresh as f32 * 0.2;
    // let high_threshold = otsu_thresh as f32;

    gray_image = canny(&gray_image, low_threshold, high_threshold);
    // gray_image = close_gaps(&gray_image);

    let mut document = Document::new()
        .set("width", gray_image.width())
        .set("height", gray_image.height())
        .set("viewBox", (0, 0, gray_image.width(), gray_image.height()));

    gray_image.save("assets/binary_image.png").unwrap();

    let contours = find_contours_with_threshold::<u32>(&gray_image, 128);

    // Set an error tolerance (in pixels)
    let tolerance = 0.4;
    contour_to_svg(&mut document, &contours, tolerance as f64);

    info!(
        "SVG created! Byte: {}",
        document.to_string().as_bytes().len()
    );

    document.to_string()
}

fn upscale_image(img: &GrayImage, scale_factor: u32) -> GrayImage {
    let (width, height) = (img.width() * scale_factor, img.height() * scale_factor);
    image::imageops::resize(img, width, height, image::imageops::FilterType::CatmullRom)
}

pub fn contour_to_svg(document: &mut Document, contours: &[Contour<u32>], tolerance: f64) {
    let mut defs = Definitions::new();
    let mut stroke_group = Group::new().set("stroke-width", "1px");
    let mut fill_group = Group::new();

    let mut strokes: HashMap<String, Vec<String>> = HashMap::new();
    let mut fills: HashMap<String, Vec<String>> = HashMap::new();

    for (i, contour) in contours.iter().enumerate() {
        let simplified = contour
            .points
            .iter()
            .map(|&p| Point::new(p.x as f64, p.y as f64))
            .collect::<Vec<Point>>();

        let simplify_tolerance = 2 as f64;
        let simplified = rdp(&simplified, simplify_tolerance);
        let segments = fit_curve(&simplified, tolerance);

        // Build SVG path data
        let mut data = Data::new();

        if let Some(first) = segments.first() {
            let move_to = match first {
                Segment::Line { start, .. } => (trunc(start.x), trunc(start.y)),
                Segment::Cubic(curve) => (trunc(curve.p0.x), trunc(curve.p0.y)),
            };
            data.append(Command::Move(
                Position::Absolute,
                vec![move_to.0, move_to.1].into(),
            ));
        }

        for segment in &segments {
            match segment {
                Segment::Line { end, .. } => {
                    data.append(Command::Line(
                        Position::Absolute,
                        vec![trunc(end.x), trunc(end.y)].into(),
                    ));
                }
                Segment::Cubic(curve) => {
                    data.append(Command::CubicCurve(
                        Position::Absolute,
                        vec![
                            trunc(curve.p1.x),
                            trunc(curve.p1.y),
                            trunc(curve.p2.x),
                            trunc(curve.p2.y),
                            trunc(curve.p3.x),
                            trunc(curve.p3.y),
                        ]
                        .into(),
                    ));
                }
            }
        }

        if !data.is_empty() {
            data.append(Command::Close);

            let id = generate_id(i);

            let path = SVGPath::new().set("id", id.clone()).set("d", data);
            defs.append(path);

            strokes
                .entry("black".to_string())
                .or_insert_with(Vec::new)
                .push(id.clone());

            fills
                .entry("none".to_string())
                .or_insert_with(Vec::new)
                .push(id);
        }
    }

    for (stroke, ids) in strokes.iter() {
        let mut group = Group::new().set("stroke", stroke.clone());

        for id in ids {
            let stroke_use = Use::new().set("href", format!("#{id}"));
            group.append(stroke_use);
        }

        stroke_group.append(group);
    }

    for (fill, ids) in fills.iter() {
        let mut group = Group::new().set("fill", fill.clone());

        for id in ids {
            let stroke_use = Use::new().set("href", format!("#{id}"));
            group.append(stroke_use);
        }

        fill_group.append(group);
    }

    document.append(defs);
    document.append(stroke_group);
    document.append(fill_group);
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read};

    use crate::structs::Pixel;

    use super::*;
    use image::ImageBuffer;

    fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .try_init();
    }

    fn save_as_png(pixels: &[Pixel], width: u32, height: u32, path: &str) {
        let mut img = ImageBuffer::new(width, height);
        for (i, pixel) in pixels.iter().enumerate() {
            let x = i as u32 % width;
            let y = i as u32 / width;
            img.put_pixel(x, y, image::Rgba([pixel.r, pixel.g, pixel.b, pixel.a]));
        }
        img.save(path).unwrap();
    }

    #[test]
    fn decode_to_file() {
        init_logger();

        let decoded_image = decode("assets/hurricane.png").unwrap();
        save_as_png(
            &decoded_image.pixels,
            decoded_image.width,
            decoded_image.height,
            "assets/hurricane.bmp",
        );

        let img = image::ImageReader::open("assets/image.png")
            .unwrap()
            .decode()
            .unwrap();
        img.save("assets/output_2.bmp").unwrap();
    }

    #[test]
    fn decode_to_svg() {
        init_logger();

        let mut file = File::open("assets/hurricane.png").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();

        let svg_string = create_svg(&buffer);

        std::fs::write("assets/generated.svg", svg_string).expect("Unable to write file");
    }
}
