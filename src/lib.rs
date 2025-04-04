pub mod algo;
pub mod curve_fit_nd;
pub mod min_heap;
pub mod path_optimizer;
pub mod polygon_simplifier;
pub mod quantizer;
pub mod structs;
pub mod utils;
pub mod vec2;

use std::{
    collections::HashMap,
    io::{BufReader, Cursor},
};

use vec2::DVec2;
use wasm_bindgen::prelude::*;

use image::{
    imageops::{resize, FilterType},
    ImageReader, Rgba,
};

use log::{info, trace, warn};
use path_optimizer::OptimizedData;
use svg::{
    node::element::{
        path::{Command, Data, Position},
        Definitions, Group, Path as SVGPath, Use,
    },
    Document, Node,
};

use algo::extract_outline;
use polygon_simplifier::poly_list_simplify;
use quantizer::NeuQuant;
use structs::{ColorMode, TurnPolicy};
use utils::{generate_id, poly_list_subdivide, poly_list_subdivide_to_limit, trunc};

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

    let error_threshold = 1.0;
    let simplify_threshold = 2.5;
    let corner_threshold = 30.0_f64.to_radians();
    let use_optimize_exhaustive = true;
    let length_threshold = 0.75;
    let size: [usize; 2] = [width as usize, height as usize];
    let turn_policy = TurnPolicy::Majority;
    let scale = 1.0;
    let colors = 16;
    let color_mode = ColorMode::Black;

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

    // --- Quantize the Image Colors ---
    let quantizer = NeuQuant::new(10, colors, image_reader.as_raw());
    let palette = quantizer.color_map_rgba();

    // Iterate through each pixel, quantize its color, and write it to the output image.
    for pixel in image_reader.pixels_mut() {
        // Get the index in the palette corresponding to this color.
        let idx = quantizer.index_of(&pixel.0);
        // Each color in the palette is 4 bytes (RGBAs).
        let r = palette[idx * 4];
        let g = palette[idx * 4 + 1];
        let b = palette[idx * 4 + 2];
        // Write the quantized color; we keep the original alpha.
        *pixel = Rgba([r, g, b, pixel.0[3]]);
    }

    match color_mode {
        ColorMode::Black => {
            let mut image: Vec<bool> = Vec::with_capacity((width * height) as usize);
            let color_max: u8 = 255;
            let color_mid = ((color_max / 2) as u16) * 3;

            for pixel in image_reader.pixels() {
                let t = (pixel[0] as u16) + (pixel[1] as u16) + (pixel[2] as u16);

                if t < color_mid && pixel[3] == 255 {
                    image.push(true);
                } else {
                    image.push(false);
                }
            }

            let fill_color = format!("#000");

            let mut poly_list_to_fit = extract_outline(&image, &size, turn_policy, true)
                .iter_mut()
                .map(|x| {
                    (
                        x.0,
                        x.1.iter_mut().map(|x| x.as_dvec2()).collect::<Vec<DVec2>>(),
                    )
                })
                .collect::<Vec<(bool, Vec<DVec2>)>>();

            // Ensure we always have at least one knot between 'corners'
            // this means theres always a middle tangent, giving us more possible
            // tangents when fitting the curve.
            poly_list_subdivide(&mut poly_list_to_fit);
            poly_list_simplify(&mut poly_list_to_fit, simplify_threshold);
            poly_list_subdivide(&mut poly_list_to_fit);

            // While a little excessive, setting the `length_threshold` around 1.0
            // helps by ensure the density of the polygon is even
            // (without this diagonals will have many more points).
            poly_list_subdivide_to_limit(&mut poly_list_to_fit, length_threshold);

            let curve_list = curve_fit_nd::fit_poly_list(
                poly_list_to_fit,
                error_threshold,
                corner_threshold,
                use_optimize_exhaustive,
            );

            // Build SVG path data
            let mut data = Data::new();

            for &(_is_cyclic, ref p) in &curve_list {
                let mut v_prev = p.last().unwrap();
                let mut is_first = true;
                for v_curr in p {
                    debug_assert!(v_curr[0].is_finite());
                    debug_assert!(v_curr[1].is_finite());
                    debug_assert!(v_curr[2].is_finite());

                    let k0 = v_prev[1];
                    let h0 = v_prev[2];

                    let h1 = v_curr[0];
                    let k1 = v_curr[1];

                    // Could optimize this, but keep now for simplicity
                    if is_first {
                        data.append(Command::Move(
                            Position::Absolute,
                            vec![trunc(k0.x * scale), trunc(k0.y * scale)].into(),
                        ));
                    }
                    data.append(Command::CubicCurve(
                        Position::Absolute,
                        vec![
                            trunc(h0.x * scale),
                            trunc(h0.y * scale),
                            trunc(h1.x * scale),
                            trunc(h1.y * scale),
                            trunc(k1.x * scale),
                            trunc(k1.y * scale),
                        ]
                        .into(),
                    ));
                    v_prev = v_curr;
                    is_first = false;
                }
            }

            if !data.is_empty() {
                data.append(Command::Close);

                let id = generate_id(0);
                // id_num += 1;

                let mut optimized_data = OptimizedData::from(data);
                optimized_data.to_relative();

                let path = SVGPath::new()
                    .set("id", id.clone())
                    .set("d", optimized_data.optimize());
                defs.append(path);

                strokes
                    .entry(fill_color.clone())
                    .or_insert_with(Vec::new)
                    .push(id.clone());

                fills.entry(fill_color).or_insert_with(Vec::new).push(id);
            }
        }
        ColorMode::Colored => {
            let mut id_num = 0;
            // // Define a sharpening kernel
            // let kernel: [i32; 9] = [0, -1, 0, -1, 5, -1, 0, -1, 0];
            // image_reader = imageproc::filter::filter3x3(&image_reader, &kernel);

            let img_palette = palette
                .chunks(4)
                .into_iter()
                .map(|x| Rgba([x[0], x[1], x[2], x[3]]))
                .collect::<Vec<Rgba<u8>>>();

            // image_reader.save("assets/debug.png").unwrap();

            // ------- Process each unique colors -------
            for color in img_palette {
                // Build a binary mask for the current color
                let mut image: Vec<bool> = Vec::with_capacity(width as usize * height as usize);
                for pixel in image_reader.pixels() {
                    let a = pixel[3];

                    if (pixel[0], pixel[1], pixel[2]) == (color.0[0], color.0[1], color.0[2])
                        && a == 255
                    {
                        image.push(true);
                    } else {
                        image.push(false);
                    }
                }

                let fill_color = format!("#{:02X}{:02X}{:02X}", color.0[0], color.0[1], color.0[2]);

                let mut poly_list_to_fit = extract_outline(&image, &size, turn_policy, true)
                    .iter_mut()
                    .map(|x| {
                        (
                            x.0,
                            x.1.iter_mut().map(|x| x.as_dvec2()).collect::<Vec<DVec2>>(),
                        )
                    })
                    .collect::<Vec<(bool, Vec<DVec2>)>>();

                // Ensure we always have at least one knot between 'corners'
                // this means theres always a middle tangent, giving us more possible
                // tangents when fitting the curve.
                poly_list_subdivide(&mut poly_list_to_fit);
                poly_list_simplify(&mut poly_list_to_fit, simplify_threshold);
                poly_list_subdivide(&mut poly_list_to_fit);

                // While a little excessive, setting the `length_threshold` around 1.0
                // helps by ensure the density of the polygon is even
                // (without this diagonals will have many more points).
                poly_list_subdivide_to_limit(&mut poly_list_to_fit, length_threshold);

                let curve_list = curve_fit_nd::fit_poly_list(
                    poly_list_to_fit,
                    error_threshold,
                    corner_threshold,
                    use_optimize_exhaustive,
                );

                // Build SVG path data
                let mut data = Data::new();

                for &(_is_cyclic, ref p) in &curve_list {
                    let mut v_prev = p.last().unwrap();
                    let mut is_first = true;
                    for v_curr in p {
                        debug_assert!(v_curr[0].is_finite());
                        debug_assert!(v_curr[1].is_finite());
                        debug_assert!(v_curr[2].is_finite());

                        let k0 = v_prev[1];
                        let h0 = v_prev[2];

                        let h1 = v_curr[0];
                        let k1 = v_curr[1];

                        // Could optimize this, but keep now for simplicity
                        if is_first {
                            data.append(Command::Move(
                                Position::Absolute,
                                vec![trunc(k0.x * scale), trunc(k0.y * scale)].into(),
                            ));
                        }
                        data.append(Command::CubicCurve(
                            Position::Absolute,
                            vec![
                                trunc(h0.x * scale),
                                trunc(h0.y * scale),
                                trunc(h1.x * scale),
                                trunc(h1.y * scale),
                                trunc(k1.x * scale),
                                trunc(k1.y * scale),
                            ]
                            .into(),
                        ));
                        v_prev = v_curr;
                        is_first = false;
                    }
                }

                if !data.is_empty() {
                    data.append(Command::Close);

                    let id = generate_id(id_num);
                    id_num += 1;

                    let mut optimized_data = OptimizedData::from(data);
                    optimized_data.to_relative();

                    let path = SVGPath::new()
                        .set("id", id.clone())
                        .set("d", optimized_data.optimize());
                    defs.append(path);

                    strokes
                        .entry(fill_color.clone())
                        .or_insert_with(Vec::new)
                        .push(id.clone());

                    fills.entry(fill_color).or_insert_with(Vec::new).push(id);
                }
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

#[wasm_bindgen]
pub fn create_svg_wasm(image_byte: Box<[u8]>) -> JsValue {
    JsValue::from_str(&create_svg(&image_byte))
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Read};

    use super::*;

    fn init_logger() {
        let _ = env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .try_init();
    }

    #[test]
    fn decode_to_svg() {
        init_logger();

        let mut file = File::open("assets/BWC.png").unwrap();
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).unwrap();

        let svg_string = create_svg(&buffer);

        std::fs::write("assets/generated.svg", svg_string).expect("Unable to write file");
    }

    use image::{open, RgbaImage};

    /// Reduces a single channel value to the nearest available level.
    /// For example, with `levels = 4`, the only possible outputs are 0, 85, 170, and 255.
    fn posterize_channel(value: u8, levels: u8) -> u8 {
        // Calculate the step size between levels.
        let step = 255.0 / (levels - 1) as f32;
        // Normalize, round to the nearest level, then scale back.
        (((value as f32 / 255.0) * (levels - 1) as f32).round() * step) as u8
    }

    #[test]
    fn posterization() {
        // Open the input image and convert it to an RGBA image.
        let mut img: RgbaImage = open("assets/BWC.png")
            .expect("Failed to open image")
            .to_rgba8();

        // // Define how many levels per channel you want (e.g., 4 gives a blocky, less anti-aliased look).
        let levels: u8 = 6;

        // Iterate over every pixel and apply the posterization to R, G, and B channels.
        for pixel in img.pixels_mut() {
            pixel[0] = posterize_channel(pixel[0], levels); // Red
            pixel[1] = posterize_channel(pixel[1], levels); // Green
            pixel[2] = posterize_channel(pixel[2], levels); // Blue
                                                            // Optionally, leave the alpha channel unchanged.
        }

        // --- Quantize the Image Colors ---
        let quantizer = NeuQuant::new(10, 5, img.as_raw());
        let palette = quantizer.color_map_rgba();

        // Iterate through each pixel, quantize its color, and write it to the output image.
        for pixel in img.pixels_mut() {
            // Get the index in the palette corresponding to this color.
            let idx = quantizer.index_of(&pixel.0);
            // Each color in the palette is 3 bytes (RGB).
            let r = palette[idx * 4];
            let g = palette[idx * 4 + 1];
            let b = palette[idx * 4 + 2];
            // Write the quantized color; we keep the original alpha.
            *pixel = Rgba([r, g, b, pixel.0[3]]);
        }

        // Save the resulting image.
        img.save("assets/posterized_debug.png")
            .expect("Failed to save image");
    }
}
