pub mod constants;
pub mod parsers;
pub mod structs;
pub mod utils;

use std::path::Path;

use svg::node::element::path::Data;
use svg::node::element::Path as SVGPath;
use svg::Document;

use constants::MARCHING_SQUARES;
use parsers::{bytes_to_pixels, parse_chunks, parse_ihdr, parse_plte, parse_trns, read_png};
use structs::{DecodeImage, DecodeResult, Pixel};
use utils::{catmull_rom_spline, decompress_idat, interpolate};

pub fn decode<P>(path: P) -> DecodeResult<DecodeImage>
where
    P: AsRef<Path>,
{
    let buffer = read_png(path.as_ref()).unwrap();
    let chunks = parse_chunks(&buffer);

    // Extract IHDR, PLTE, tRNS
    let ihdr_chunk = chunks.iter().find(|c| c.type_str == "IHDR").unwrap();
    let ihdr = parse_ihdr(&ihdr_chunk.data);
    let plte = parse_plte(&chunks);
    let trns = parse_trns(&chunks);

    println!("{ihdr:?}");

    let idat_data: Vec<u8> = chunks
        .iter()
        .filter(|c| c.type_str == "IDAT")
        .flat_map(|c| c.data.clone())
        .collect();

    let decompressed = decompress_idat(&idat_data);
    let pixels = bytes_to_pixels(&decompressed, &ihdr, &plte, &trns);

    Ok(DecodeImage {
        pixels,
        width: ihdr.width,
        height: ihdr.height,
    })
}

// pub fn create_svg<P>(pixels: &[Pixel], width: u32, height: u32, output_path: P) -> Result<(), ()>
// where
//     P: AsRef<Path>,
// {
//     // Track visited pixels to avoid overlap
//     let mut visited = vec![vec![false; height as usize]; width as usize];
//     let mut document = Document::new()
//         .set("viewBox", (0, 0, width, height))
//         .set("width", width)
//         .set("height", height);

//     for (i, pixel) in pixels.iter().enumerate() {
//         let x = i as u32 % width;
//         let y = i as u32 / width;

//         if pixel.a == 0 || visited[x as usize][y as usize] {
//             continue;
//         }

//         let idx = y * width + x;
//         let color = pixels[idx as usize];
//         // let contours = find_contours(&img, x, y, Some(color), false);

//         // let data = contours_to_path_data(&contours);

//         let path = SVGPath::new()
//             .set(
//                 "fill",
//                 // format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b),
//                 format!(
//                     "rgba({}, {}, {}, {})",
//                     color.r,
//                     color.g,
//                     color.b,
//                     color.a as f32 / 255.0
//                 ),
//             )
//             .set("stroke", "none") // Optional: removes border
//             .set("d", data);

//         document = document.add(path);

//         for contour in contours {
//             for point in contour.points {
//                 visited[(x + dx) as usize][(y + dy) as usize] = true
//             }
//         }
//     }

//     // Save the SVG document
//     svg::save(output_path, &document).unwrap();
//     Ok(())
// }

// // Convert detected contours to SVG path data
// fn contours_to_path_data(contours: &[Contour]) -> Data {
//     let mut data = Data::new();
//     for contour in contours {
//         data = data.move_to((contour.points[0].x, contour.points[0].y));
//         for point in &contour.points[1..] {
//             data = data.line_to((point.x, point.y));
//         }
//         data = data.close();
//     }
//     data
// }

// pub fn create_svg<P>(pixels: &[Pixel], width: u32, height: u32, output_path: P) -> Result<(), ()>
pub fn create_svg(
    // contours: &[(Vec<(usize, usize)>, (u8, u8, u8))],
    // contours: &Vec<Vec<(usize, usize)>>,
    contours: &Vec<Vec<(f64, f64)>>,
    pixels: &[Pixel],
    width: u32,
    height: u32,
) -> Result<String, ()> {
    let mut document = Document::new()
        .set("viewBox", (0, 0, width, height))
        .set("width", width)
        .set("height", height);

    for contour in contours {
        let mut data = Data::new();

        // Get the average color for this contour region
        let avg_color = average_color_from_pixels(contour, pixels, width as usize);
        let color = format!("rgb({}, {}, {})", avg_color.0, avg_color.1, avg_color.2);

        if let Some(&(x, y)) = contour.first() {
            data = data.move_to((x as f64, height as f64 - y as f64));
        }

        for &(x, y) in contour.iter().skip(1) {
            data = data.line_to((x as f64, height as f64 - y as f64));
        }
        // data = data.close();

        let path = SVGPath::new()
            .set("fill", color)
            .set("stroke", "none")
            // .set("stroke-width", 1)
            .set("d", data);

        document = document.add(path);
    }
    Ok(document.to_string())
}

// pub fn find_contours_with_colors(
//     pixels: &[Pixel],
//     width: usize,
//     height: usize,
// ) -> Vec<(Vec<(usize, usize)>, (u8, u8, u8))> {
//     let binary_mask = pixels_to_binary_mask(pixels, width, height, 128);
//     let mut visited = vec![false; width * height];
//     let mut contours = Vec::new();

//     for y in 0..height {
//         for x in 0..width {
//             let idx = y * width + x;
//             if binary_mask[idx] == 255 && !visited[idx] {
//                 let mut contour = Vec::new();
//                 trace_contour(
//                     &binary_mask,
//                     width,
//                     height,
//                     x,
//                     y,
//                     &mut visited,
//                     &mut contour,
//                 );

//                 // Get the average color of the contour region
//                 let avg_color = average_color_from_pixels(&contour, pixels, width);
//                 contours.push((contour, avg_color));
//             }
//         }
//     }

//     contours
// }

fn smooth_contours(contours: Vec<Vec<(f64, f64)>>) -> Vec<Vec<(f64, f64)>> {
    contours
        .into_iter()
        .map(|contour| {
            if contour.len() > 3 {
                catmull_rom_spline(&contour, 0.5)
            } else {
                contour
            }
        })
        .collect()
}

pub fn marching_squares(pixels: &[Pixel], width: usize, height: usize) -> Vec<Vec<(usize, usize)>> {
    let mut contours = Vec::new();

    for y in 0..height - 1 {
        for x in 0..width - 1 {
            let idx1 = y * width + x;
            let idx2 = (y + 1) * width + x;
            let idx3 = y * width + (x + 1);
            let idx4 = (y + 1) * width + (x + 1);

            // Create a binary map for the 2x2 block
            let binary = [
                if pixels[idx1].a > 128 { 1 } else { 0 },
                if pixels[idx2].a > 128 { 1 } else { 0 },
                if pixels[idx3].a > 128 { 1 } else { 0 },
                if pixels[idx4].a > 128 { 1 } else { 0 },
            ];

            let square_index = binary[0] * 8 + binary[1] * 4 + binary[2] * 2 + binary[3];

            if square_index != 0 && square_index != 15 {
                let contour = match MARCHING_SQUARES[square_index] {
                    (0.5, 0.0) => vec![(x, y)],         // Simple case for demonstration
                    (1.0, 0.5) => vec![(x + 1, y + 1)], // Another case
                    _ => vec![],                        // Add other cases for more complex contours
                };

                contours.push(contour);
            }
        }
    }

    contours
}

fn marching_squares_interpolated(
    pixels: &[Pixel],
    width: usize,
    height: usize,
) -> Vec<Vec<(f64, f64)>> {
    let mut contours = Vec::new();

    for y in 0..height - 1 {
        for x in 0..width - 1 {
            let idx1 = y * width + x;
            let idx2 = (y + 1) * width + x;
            let idx3 = y * width + (x + 1);
            let idx4 = (y + 1) * width + (x + 1);

            // Create a binary map for the 2x2 block
            let binary = [
                if pixels[idx1].a > 128 { 1 } else { 0 },
                if pixels[idx2].a > 128 { 1 } else { 0 },
                if pixels[idx3].a > 128 { 1 } else { 0 },
                if pixels[idx4].a > 128 { 1 } else { 0 },
            ];

            let square_index = binary[0] * 8 + binary[1] * 4 + binary[2] * 2 + binary[3];

            if square_index != 0 && square_index != 15 {
                let mut contour = Vec::new();

                // Interpolate between the edges of the 2x2 square
                match square_index {
                    1 => {
                        contour.push(interpolate((x, y), (x, y + 1), binary[0], binary[1]));
                    }
                    2 => {
                        contour.push(interpolate((x, y), (x + 1, y), binary[0], binary[2]));
                    }
                    3 => {
                        contour.push(interpolate((x, y), (x + 1, y), binary[0], binary[2]));
                        contour.push(interpolate(
                            (x, y + 1),
                            (x + 1, y + 1),
                            binary[1],
                            binary[3],
                        ));
                    }
                    // Add more cases based on the marching squares lookup table
                    _ => {}
                }

                contours.push(contour);
            }
        }
    }

    contours
}

fn trace_contour(
    image: &[u8],
    width: usize,
    height: usize,
    start_x: usize,
    start_y: usize,
    visited: &mut [bool],
    contour: &mut Vec<(usize, usize)>,
) {
    let directions = [(-1, 0), (1, 0), (0, -1), (0, 1)];
    let mut x = start_x;
    let mut y = start_y;

    loop {
        contour.push((x, y));
        visited[y * width + x] = true;

        let mut found_next = false;
        for &(dx, dy) in &directions {
            let nx = x as isize + dx;
            let ny = y as isize + dy;

            if nx >= 0 && nx < width as isize && ny >= 0 && ny < height as isize {
                let nidx = ny as usize * width + nx as usize;
                if image[nidx] == 255 && !visited[nidx] {
                    x = nx as usize;
                    y = ny as usize;
                    found_next = true;
                    break;
                }
            }
        }

        if !found_next {
            break;
        }
    }
}

fn average_color_from_pixels(
    // contour: &[(usize, usize)],
    contour: &[(f64, f64)],
    pixels: &[Pixel],
    width: usize,
) -> (u8, u8, u8) {
    let mut r_sum = 0;
    let mut g_sum = 0;
    let mut b_sum = 0;
    let mut count = 0;

    for &(x, y) in contour {
        let idx = y as usize * width + x as usize;
        let pixel = pixels[idx];

        r_sum += pixel.r as u32;
        g_sum += pixel.g as u32;
        b_sum += pixel.b as u32;
        count += 1;
    }

    if count > 0 {
        (
            (r_sum / count) as u8,
            (g_sum / count) as u8,
            (b_sum / count) as u8,
        )
    } else {
        (0, 0, 0) // Default black if no valid pixels
    }
}

fn pixels_to_binary_mask(pixels: &[Pixel], width: usize, height: usize, threshold: u8) -> Vec<u8> {
    let mut mask = vec![0; width * height];

    for (i, pixel) in pixels.iter().enumerate() {
        let gray = (0.299 * pixel.r as f32 + 0.587 * pixel.g as f32 + 0.114 * pixel.b as f32) as u8;
        mask[i] = if gray > threshold && pixel.a > 128 {
            255
        } else {
            0
        };
    }

    mask
}

#[cfg(test)]
mod tests {
    use crate::structs::Pixel;

    use super::*;
    use image::{ImageBuffer, RgbaImage};

    #[test]
    fn decode_to_file() {
        let decoded_image = decode("assets/image.png").unwrap();
        save_as_png(
            &decoded_image.pixels,
            decoded_image.width,
            decoded_image.height,
            "assets/output.bmp",
        );

        let img = image::ImageReader::open("assets/image.png")
            .unwrap()
            .decode()
            .unwrap();
        img.save("assets/output_2.bmp").unwrap();
    }

    #[test]
    fn decode_to_svg() {
        let decoded_image = decode("assets/image.png").unwrap();
        // let contours = find_contours_with_colors(
        //     &decoded_image.pixels,
        //     decoded_image.width as usize,
        //     decoded_image.height as usize,
        // );

        // let contours = marching_squares(
        //     &decoded_image.pixels,
        //     decoded_image.width as usize,
        //     decoded_image.height as usize,
        // );
        let contours = marching_squares_interpolated(
            &decoded_image.pixels,
            decoded_image.width as usize,
            decoded_image.height as usize,
        );
        let smoothed_contours = smooth_contours(contours);

        let svg_string = create_svg(
            &smoothed_contours,
            &decoded_image.pixels,
            decoded_image.width,
            decoded_image.height,
        )
        .unwrap();

        std::fs::write("assets/generated.svg", svg_string).expect("Unable to write file");
    }

    fn save_as_png(pixels: &[Pixel], width: u32, height: u32, path: &str) {
        let mut img: RgbaImage = ImageBuffer::new(width, height);
        for (i, pixel) in pixels.iter().enumerate() {
            let x = i as u32 % width;
            let y = i as u32 / width;
            img.put_pixel(x, y, image::Rgba([pixel.r, pixel.g, pixel.b, pixel.a]));
        }
        img.save(path).unwrap();
    }
}
