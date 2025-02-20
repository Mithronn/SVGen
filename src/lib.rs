pub mod constants;
pub mod parsers;
pub mod structs;
pub mod utils;

use std::{
    collections::{HashMap, HashSet},
    io::{BufReader, Cursor},
    path::Path,
};

use imageproc::{
    contours::find_contours_with_threshold,
    image::{
        imageops::{resize, FilterType},
        GrayImage, ImageBuffer, ImageReader, Luma, Rgba,
    },
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

    // ------- Load the image -------
    let mut image_reader = ImageReader::new(BufReader::new(Cursor::new(image_byte)))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap()
        .to_rgba8();

    let (mut width, mut height) = image_reader.dimensions();
    info!("Image readed {}x{}", width, height);

    // ------- Upscale the image if necessary -------
    if width * height < 512 * 512 {
        let scale_factor = 3;
        width = width * scale_factor;
        height = height * scale_factor;

        image_reader = resize(&image_reader, width, height, FilterType::CatmullRom);

        warn!("Image size is small. Upscalled to {}x{}", width, height);
    }

    // --- Quantize the Image Colors ---
    // Here we reduce color variety by rounding each channel to the nearest multiple.
    let quantization_factor: u8 = 128;
    let mut quantized_image = ImageBuffer::new(width, height);
    let mut unique_colors = HashSet::new();

    for (x, y, pixel) in image_reader.enumerate_pixels() {
        let r = (pixel[0] / quantization_factor) * quantization_factor;
        let g = (pixel[1] / quantization_factor) * quantization_factor;
        let b = (pixel[2] / quantization_factor) * quantization_factor;
        // let r = pixel[0];
        // let g = pixel[1];
        // let b = pixel[2];
        let quant_pixel = Rgba([r, g, b, pixel[3]]);
        quantized_image.put_pixel(x, y, quant_pixel);
        unique_colors.insert((r, g, b));
    }

    // ------- SVG container created -------
    let mut document = Document::new()
        .set("width", width)
        .set("height", height)
        .set("viewBox", (0, 0, width, height));

    let mut defs = Definitions::new();
    let mut stroke_group = Group::new().set("stroke-width", "1px");
    let mut fill_group = Group::new();

    let mut strokes: HashMap<String, Vec<String>> = HashMap::new();
    let mut fills: HashMap<String, Vec<String>> = HashMap::new();

    // ------- Process each unique colors -------
    for color in unique_colors {
        // Build a binary mask for the current color
        let mut mask: GrayImage = GrayImage::new(width, height);
        for (x, y, pixel) in quantized_image.enumerate_pixels() {
            let (r, g, b, a) = (pixel[0], pixel[1], pixel[2], pixel[3]);
            if (r, g, b) == color && a > 0 {
                mask.put_pixel(x, y, Luma([255]));
            } else {
                mask.put_pixel(x, y, Luma([0]));
            }
        }

        // Extract contours from the mask
        let contours = find_contours_with_threshold::<u32>(&mask, 1);

        for (i, contour) in contours.iter().enumerate() {
            // Convert the color to a hex string for SVG.
            let fill_color = format!("#{:02X}{:02X}{:02X}", color.0, color.1, color.2);

            let simplified = contour
                .points
                .iter()
                .map(|&p| Point::new(p.x as f64, p.y as f64))
                .collect::<Vec<Point>>();

            let tolerance = 1 as f64;
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
                    .entry(fill_color.clone())
                    .or_insert_with(Vec::new)
                    .push(id.clone());

                fills.entry(fill_color).or_insert_with(Vec::new).push(id);
            }
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

    info!(
        "SVG created! Byte: {}",
        document.to_string().as_bytes().len()
    );

    document.to_string()
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
