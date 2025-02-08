use std::io::Read;

use flate2::read::ZlibDecoder;

pub fn decompress_idat(data: &[u8]) -> Vec<u8> {
    let mut decoder = ZlibDecoder::new(data);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).unwrap();
    decompressed
}

pub fn paeth_predictor(left: u8, above: u8, upper_left: u8) -> u8 {
    let p = left as i32 + above as i32 - upper_left as i32;
    let pa = (p - left as i32).abs();
    let pb = (p - above as i32).abs();
    let pc = (p - upper_left as i32).abs();

    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        above
    } else {
        upper_left
    }
}

pub fn defilter_scanline(
    filter_type: u8,
    current: &mut [u8],
    previous: &[u8],
    bytes_per_pixel: usize,
) {
    match filter_type {
        0 => {} // None
        1 => {
            // Sub filter
            for i in bytes_per_pixel..current.len() {
                current[i] = current[i].wrapping_add(current[i - bytes_per_pixel]);
            }
        }
        2 => {
            // Up filter
            for i in 0..current.len() {
                current[i] = current[i].wrapping_add(previous[i]);
            }
        }
        3 => {
            // Average filter
            for i in 0..current.len() {
                let left = if i >= bytes_per_pixel {
                    current[i - bytes_per_pixel]
                } else {
                    0
                };
                let above = previous[i];
                let avg = ((left as u16 + above as u16) / 2) as u8;
                current[i] = current[i].wrapping_add(avg);
            }
        }
        4 => {
            // Paeth filter
            for i in 0..current.len() {
                let left = if i >= bytes_per_pixel {
                    current[i - bytes_per_pixel]
                } else {
                    0
                };
                let above = previous[i];
                let upper_left = if i >= bytes_per_pixel {
                    previous[i - bytes_per_pixel]
                } else {
                    0
                };
                let prediction = paeth_predictor(left, above, upper_left);
                current[i] = current[i].wrapping_add(prediction);
            }
        }
        _ => panic!("Unknown filter type: {}", filter_type),
    }
}

// - Color Type    Allowed Bit Depths    Interpretation
//
//       0             1,2,4,8,16        Each pixel is a grayscale sample.
//       2             8,16              Each pixel is an R,G,B triple.
//       3             1,2,4,8           Each pixel is a palette index;
//                                       a PLTE chunk must appear.
//       4             8,16              Each pixel is a grayscale sample,
//                                       followed by an alpha sample.
//       6             8,16              Each pixel is an R,G,B triple,
//                                       followed by an alpha sample.
pub fn get_bytes_per_pixel(color_type: u8, bit_depth: u8) -> usize {
    match (color_type, bit_depth) {
        (0, 1 | 2 | 4 | 8 | 16) => 1,
        (2, 8 | 16) => 3,
        (3, 1 | 2 | 4 | 8) => 1,
        (4, 8 | 16) => 2,
        (6, 8 | 16) => 4,
        _ => panic!("Unsupported color type/bit depth"),
    }
}

// Unpack bits for grayscale/indexed pixels (bit depths 1, 2, 4, 8)
pub fn unpack_bits(packed: &[u8], bit_depth: u8, width: u32) -> Vec<u8> {
    let width = width as usize;
    let bits_per_pixel = bit_depth as usize;
    let mut values = Vec::with_capacity(width);
    let mut buffer = 0u16;
    let mut remaining_bits = 0;

    for &byte in packed {
        buffer = (buffer << 8) | byte as u16;
        remaining_bits += 8;

        while remaining_bits >= bits_per_pixel && values.len() < width {
            remaining_bits -= bits_per_pixel;
            let shift = 16 - bits_per_pixel - remaining_bits;
            let mask = (1u16 << bits_per_pixel) - 1;
            let value = ((buffer >> shift) & mask) as u8;
            values.push(value);
        }
    }

    values
}

// Scale lower bit depths (1/2/4) to 8-bit grayscale
pub fn scale_to_8bit(value: u8, bit_depth: u8) -> u8 {
    let max_value = (1 << bit_depth) - 1;
    ((value as f32 / max_value as f32) * 255.0) as u8
}

pub fn interpolate(p1: (usize, usize), p2: (usize, usize), value1: u8, value2: u8) -> (f64, f64) {
    // Linear interpolation formula
    if value1 == value2 {
        return (p1.0 as f64, p1.1 as f64); // No interpolation needed if both values are equal
    }
    let t = 0.5 * (value1 as f64 + value2 as f64);
    let x = p1.0 as f64 + (p2.0 as f64 - p1.0 as f64) * (t / (value2 as f64 - value1 as f64));
    let y = p1.1 as f64 + (p2.1 as f64 - p1.1 as f64) * (t / (value2 as f64 - value1 as f64));
    (x, y)
}

pub fn catmull_rom_spline(points: &[(f64, f64)], tension: f64) -> Vec<(f64, f64)> {
    let mut smoothed_points = Vec::new();

    for i in 0..points.len() - 3 {
        let p0 = points[i];
        let p1 = points[i + 1];
        let p2 = points[i + 2];
        let p3 = points[i + 3];

        for t in 0..10 {
            let t = t as f64 / 10.0;
            let t2 = t * t;
            let t3 = t2 * t;

            let x = 0.5
                * ((2.0 * p1.0)
                    + (-p0.0 + p2.0) * t
                    + (2.0 * p0.0 - 5.0 * p1.0 + 4.0 * p2.0 - p3.0) * t2
                    + (-p0.0 + 3.0 * p1.0 - 3.0 * p2.0 + p3.0) * t3);

            let y = 0.5
                * ((2.0 * p1.1)
                    + (-p0.1 + p2.1) * t
                    + (2.0 * p0.1 - 5.0 * p1.1 + 4.0 * p2.1 - p3.1) * t2
                    + (-p0.1 + 3.0 * p1.1 - 3.0 * p2.1 + p3.1) * t3);

            smoothed_points.push((x, y));
        }
    }

    smoothed_points
}
