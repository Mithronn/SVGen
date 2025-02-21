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

pub fn generate_id(input: usize) -> String {
    let mut id = String::new();
    let mut num = input;

    loop {
        let remainder = num % 52;
        num /= 52;

        let char_to_append = if remainder < 26 {
            (remainder as u8 + b'a') as char
        } else {
            (remainder as u8 - 26 + b'A') as char
        };
        id.push(char_to_append);

        if num == 0 {
            break;
        }
    }

    id.chars().rev().collect()
}

pub fn rgba_to_hex(r: u8, g: u8, b: u8, a: u8) -> String {
    // Produces a string in the form "#RRGGBBAA"
    format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
}

pub fn trunc(value: f64) -> f32 {
    (f64::trunc(value * 100.0) / 100.0) as f32
}
