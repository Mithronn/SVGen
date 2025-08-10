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
use wasm_bindgen::prelude::*;

use image::{
    imageops::{resize, FilterType},
    ImageReader, Rgba,
};
use log::{info, trace, warn};
use svg::{
    node::element::{
        path::{Command, Data, Position},
        Definitions, Group, Path as SVGPath, Use,
    },
    Document, Node,
};

use algo::extract_outline;
use path_optimizer::OptimizedData;
use polygon_simplifier::poly_list_simplify;
use quantizer::NeuQuant;
use structs::{ColorMode, TurnPolicy};
use utils::{generate_id, poly_list_subdivide, poly_list_subdivide_to_limit, trunc};
use vec2::DVec2;

pub fn create_svg(image_byte: &[u8], color_mode: ColorMode) -> String {
    trace!("SVG Creation");

    // ------- Load the image -------
    let image_reader = ImageReader::new(BufReader::new(Cursor::new(image_byte)))
        .with_guessed_format()
        .unwrap()
        .decode()
        .unwrap()
        .to_rgba8();

    let (mut width, mut height) = image_reader.dimensions();
    info!("Image readed {}x{}", width, height);

    let mut image_reader = preprocess_image(&image_reader);

    // ------- Upscale the image if necessary -------
    if width * height < 512 * 512 {
        let scale_factor = 3;
        width = width * scale_factor;
        height = height * scale_factor;

        image_reader = resize(&image_reader, width, height, FilterType::CatmullRom);

        warn!("Image size is small. Upscalled to {}x{}", width, height);
    }

    let error_threshold = 1.5; // 1.0
    let simplify_threshold = 2.0; // 2.5
    let corner_threshold = 30.0_f64.to_radians(); // 30
    let use_optimize_exhaustive = true;
    let length_threshold = 0.75; // 0.75
    let size: [usize; 2] = [width as usize, height as usize];
    let turn_policy = TurnPolicy::Majority;
    let scale = 1.0;

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

    let mut hist: HashMap<[u8; 4], usize> = HashMap::new();
    for pix in image_reader.pixels() {
        let key = [pix[0], pix[1], pix[2], pix[3]];
        *hist.entry(key).or_default() += 1;
    }

    let colors = 5;

    // --- Quantize the Image Colors ---
    let quantizer = NeuQuant::new(1, colors, image_reader.as_raw());
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
pub fn create_svg_wasm(image_byte: Box<[u8]>, color_mode: ColorMode) -> JsValue {
    JsValue::from_str(&create_svg(&image_byte, color_mode))
}

fn preprocess_image(
    img: &image::ImageBuffer<Rgba<u8>, Vec<u8>>,
) -> image::ImageBuffer<Rgba<u8>, Vec<u8>> {
    // Adaptive Kuwahara filter: adapts the window radius per-pixel based on
    // local edge strength (Sobel gradient magnitude). Flat regions use larger
    // windows; edge regions use smaller windows to preserve detail.
    pub fn adaptive_kuwahara_filter(
        src: &image::ImageBuffer<Rgba<u8>, Vec<u8>>,
        r_min: f64,
        r_max: f64,
        gamma: f32,
    ) -> image::ImageBuffer<Rgba<u8>, Vec<u8>> {
        use image::{ImageBuffer, Rgba};

        let (width, height) = src.dimensions();
        let mut dst = ImageBuffer::new(width, height);

        let w = width as usize;
        let h = height as usize;

        // 1) Build grayscale (luminance) buffer
        let mut lum: Vec<f32> = vec![0.0; w * h];
        for y in 0..height {
            for x in 0..width {
                let p = src.get_pixel(x, y).0;
                let l = 0.299f32 * p[0] as f32 + 0.587f32 * p[1] as f32 + 0.114f32 * p[2] as f32;
                lum[(y as usize) * w + (x as usize)] = l;
            }
        }

        // Helper to clamp coordinates and fetch luminance
        let get_lum = |xx: i32, yy: i32| -> f32 {
            let cx = xx.clamp(0, (width as i32) - 1) as usize;
            let cy = yy.clamp(0, (height as i32) - 1) as usize;
            lum[cy * w + cx]
        };

        // 2) Compute Sobel gradient magnitude per pixel, track global max
        let mut grad_mag: Vec<f32> = vec![0.0; w * h];
        let mut max_mag: f32 = 0.0;
        for y in 0..(height as i32) {
            for x in 0..(width as i32) {
                // Sobel kernels
                let gx = -1.0 * get_lum(x - 1, y - 1)
                    + 1.0 * get_lum(x + 1, y - 1)
                    + -2.0 * get_lum(x - 1, y)
                    + 2.0 * get_lum(x + 1, y)
                    + -1.0 * get_lum(x - 1, y + 1)
                    + 1.0 * get_lum(x + 1, y + 1);
                let gy = 1.0 * get_lum(x - 1, y - 1)
                    + 2.0 * get_lum(x, y - 1)
                    + 1.0 * get_lum(x + 1, y - 1)
                    + -1.0 * get_lum(x - 1, y + 1)
                    - 2.0 * get_lum(x, y + 1)
                    - 1.0 * get_lum(x + 1, y + 1);
                let m = (gx * gx + gy * gy).sqrt();
                let idx = (y as usize) * w + (x as usize);
                grad_mag[idx] = m;
                if m > max_mag {
                    max_mag = m;
                }
            }
        }

        // 3) Map gradient magnitude to adaptive radius per pixel
        // Normalized edge strength in [0,1]. Strong edges -> near 1.
        // Use (1 - edge)^gamma to favor larger windows in flat regions.
        let denom = if max_mag > 0.0 { max_mag } else { 1.0 };
        let r_min_c = r_min.max(0.0);
        let r_max_c = r_max.max(r_min_c);
        let range = (r_max_c - r_min_c) as f64;
        let mut r_map: Vec<u32> = vec![r_min_c.round() as u32; w * h];
        for i in 0..grad_mag.len() {
            let e = (grad_mag[i] / denom).clamp(0.0, 1.0);
            let inv = (1.0 - e).powf(gamma);
            let r = r_min_c + range * (inv as f64);
            let r_clamped = r.clamp(0.0, r_max_c);
            r_map[i] = r_clamped.round() as u32;
        }

        // 4) Apply classic Kuwahara per pixel with its own radius
        for y in 0..height {
            for x in 0..width {
                let idx = (y as usize) * w + (x as usize);
                let r = r_map[idx];

                let mut best_var = f64::MAX;
                let mut best_mean = [0f64; 4];

                // four sub-windows: (0,0), (0,r), (r,0), (r,r)
                for (dy, dx) in &[(0, 0), (0, r), (r, 0), (r, r)] {
                    let mut sum = [0u64; 4];
                    let mut sum_sq = [0u64; 4];
                    let mut count = 0u64;

                    let y0 = y.saturating_sub(*dy);
                    let x0 = x.saturating_sub(*dx);
                    for yy in y0..=(y0 + r).min(height - 1) {
                        for xx in x0..=(x0 + r).min(width - 1) {
                            let pix = src.get_pixel(xx, yy).0;
                            for c in 0..4 {
                                let v = pix[c] as u64;
                                sum[c] += v;
                                sum_sq[c] += v * v;
                            }
                            count += 1;
                        }
                    }

                    let mut var = 0f64;
                    let mut mean = [0f64; 4];
                    for c in 0..4 {
                        let s = sum[c] as f64;
                        let ss = sum_sq[c] as f64;
                        mean[c] = s / count as f64;
                        var += (ss / count as f64) - (mean[c] * mean[c]);
                    }
                    var /= 4.0;

                    if var < best_var {
                        best_var = var;
                        best_mean = mean;
                    }
                }

                let out_pix = Rgba([
                    best_mean[0].round().clamp(0.0, 255.0) as u8,
                    best_mean[1].round().clamp(0.0, 255.0) as u8,
                    best_mean[2].round().clamp(0.0, 255.0) as u8,
                    best_mean[3].round().clamp(0.0, 255.0) as u8,
                ]);
                dst.put_pixel(x, y, out_pix);
            }
        }

        dst
    }

    // Reasonable defaults: r in [1, 5], gamma = 1.2 (more weight to edges)
    let a = adaptive_kuwahara_filter(&img, 1.0, 1.5, 1.2);

    a.save("assets/preprocessed.png").expect("save");
    a
}

#[cfg(test)]
mod tests {}
