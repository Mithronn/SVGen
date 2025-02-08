use std::{fs::File, io::Read, path::Path};

use crate::constants::PNG_SIGNATURE;
use crate::structs::{Chunk, Pixel, IHDR};
use crate::utils::{defilter_scanline, get_bytes_per_pixel, scale_to_8bit, unpack_bits};

pub fn read_png(file_path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Validate signature
    if buffer[0..8] != PNG_SIGNATURE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid PNG signature",
        ));
    }

    Ok(buffer)
}

pub fn parse_chunks(buffer: &[u8]) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut offset = 8; // Skip signature

    while offset < buffer.len() {
        let length = u32::from_be_bytes(buffer[offset..offset + 4].try_into().unwrap());
        let type_str = String::from_utf8_lossy(&buffer[offset + 4..offset + 8]).to_string();
        let data_start = offset + 8;
        let data_end = data_start + length as usize;
        let data = buffer[data_start..data_end].to_vec();
        let crc = u32::from_be_bytes(buffer[data_end..data_end + 4].try_into().unwrap());

        chunks.push(Chunk {
            length,
            type_str,
            data,
            crc,
        });

        offset = data_end + 4; // Move to next chunk
    }

    chunks
}

// Parse PLTE chunk (palette for indexed images)
pub fn parse_plte(chunks: &[Chunk]) -> Vec<(u8, u8, u8)> {
    chunks
        .iter()
        .find(|chunk| chunk.type_str == "PLTE")
        .map(|chunk| {
            chunk
                .data
                .chunks(3)
                .map(|rgb| (rgb[0], rgb[1], rgb[2]))
                .collect()
        })
        .unwrap_or_default()
}

// Parse tRNS chunk (transparency for indexed/grayscale)
pub fn parse_trns(chunks: &[Chunk]) -> Vec<u8> {
    chunks
        .iter()
        .find(|chunk| chunk.type_str == "tRNS")
        .map(|chunk| chunk.data.clone())
        .unwrap_or_default()
}

pub fn parse_ihdr(data: &[u8]) -> IHDR {
    IHDR {
        width: u32::from_be_bytes(data[0..4].try_into().unwrap()),
        height: u32::from_be_bytes(data[4..8].try_into().unwrap()),
        bit_depth: data[8],
        color_type: data[9],
        compression_method: data[10],
        filter_method: data[11],
        interlace_method: data[12],
    }
}

// pub fn parse_actl(data: &[u8]) -> AnimationControl {
//     AnimationControl {
//         num_frames: u32::from_be_bytes(data[0..4].try_into().unwrap()),
//         num_plays: u32::from_be_bytes(data[4..8].try_into().unwrap()),
//     }
// }

// pub fn parse_fctl(data: &[u8]) -> FrameControl {
//     FrameControl {
//         sequence_number: u32::from_be_bytes(data[0..4].try_into().unwrap()),
//         width: u32::from_be_bytes(data[4..8].try_into().unwrap()),
//         height: u32::from_be_bytes(data[8..12].try_into().unwrap()),
//         x_offset: u32::from_be_bytes(data[12..16].try_into().unwrap()),
//         y_offset: u32::from_be_bytes(data[16..20].try_into().unwrap()),
//         delay_num: u16::from_be_bytes(data[20..22].try_into().unwrap()),
//         delay_den: u16::from_be_bytes(data[22..24].try_into().unwrap()),
//         dispose_op: data[24],
//         blend_op: data[25],
//     }
// }

// pub fn process_fdat_chunks(chunks: &[Chunk]) -> Vec<Vec<u8>> {
//     let mut frame_data = Vec::new();
//     let mut current_frame = Vec::new();

//     for chunk in chunks {
//         if chunk.type_str == "fdAT" {
//             // Skip the 4-byte sequence number
//             current_frame.extend_from_slice(&chunk.data[4..]);
//         } else if chunk.type_str == "fcTL" {
//             if !current_frame.is_empty() {
//                 frame_data.push(current_frame);
//                 current_frame = Vec::new();
//             }
//         }
//     }

//     if !current_frame.is_empty() {
//         frame_data.push(current_frame);
//     }

//     frame_data
// }

pub fn bytes_to_pixels(data: &[u8], ihdr: &IHDR, plte: &[(u8, u8, u8)], trns: &[u8]) -> Vec<Pixel> {
    let mut pixels = Vec::new();
    let bytes_per_pixel = get_bytes_per_pixel(ihdr.color_type, ihdr.bit_depth);
    let bytes_per_line = match ihdr.color_type {
        0 | 4 => (ihdr.width as usize * ihdr.bit_depth as usize + 7) / 8 + 1,
        3 => (ihdr.width as usize * ihdr.bit_depth as usize + 7) / 8 + 1,
        _ => (ihdr.width as usize * bytes_per_pixel) + 1,
    };

    for y in 0..ihdr.height as usize {
        let filter_type = data[y * bytes_per_line];
        let line_data = &data[y * bytes_per_line + 1..(y + 1) * bytes_per_line];
        let mut current_line = line_data.to_vec();

        // Apply defiltering using previous scanline
        if y > 0 {
            let prev_line = &data[(y - 1) * bytes_per_line + 1..y * bytes_per_line];
            defilter_scanline(filter_type, &mut current_line, prev_line, bytes_per_pixel);
        }

        match ihdr.color_type {
            // Grayscale (color type 0)
            0 => {
                let grays = unpack_bits(&current_line, ihdr.bit_depth, ihdr.width);
                for gray in grays {
                    let scaled_gray = match ihdr.bit_depth {
                        1 | 2 | 4 => scale_to_8bit(gray, ihdr.bit_depth),
                        _ => gray,
                    };
                    let alpha = if !trns.is_empty() && scaled_gray == trns[0] {
                        0
                    } else {
                        255
                    };
                    pixels.push(Pixel {
                        r: scaled_gray,
                        g: scaled_gray,
                        b: scaled_gray,
                        a: alpha,
                    });
                }
            }
            // Indexed (color type 3)
            3 => {
                let indexes = unpack_bits(&current_line, ihdr.bit_depth, ihdr.width);
                for idx in indexes {
                    if let Some(&(r, g, b)) = plte.get(idx as usize) {
                        let alpha = trns.get(idx as usize).copied().unwrap_or(255);
                        pixels.push(Pixel { r, g, b, a: alpha });
                    } else {
                        // Invalid index: fallback to black
                        pixels.push(Pixel {
                            r: 0,
                            g: 0,
                            b: 0,
                            a: 255,
                        });
                    }
                }
            }
            2 | 6 | 4 => {}
            _ => unimplemented!("Unsupported color type"),
        }

        // Convert bytes to pixels (with alpha)
        for x in 0..ihdr.width as usize {
            let offset = x * bytes_per_pixel;
            match ihdr.color_type {
                2 => {
                    // RGB (3 bytes, no alpha)
                    pixels.push(Pixel {
                        r: current_line[offset],
                        g: current_line[offset + 1],
                        b: current_line[offset + 2],
                        a: 255, // Opaque
                    });
                }
                6 => {
                    // RGBA (4 bytes)
                    pixels.push(Pixel {
                        r: current_line[offset],
                        g: current_line[offset + 1],
                        b: current_line[offset + 2],
                        a: current_line[offset + 3],
                    });
                }
                4 => {
                    // Grayscale + Alpha (2 bytes)
                    pixels.push(Pixel {
                        r: current_line[offset],
                        g: current_line[offset],
                        b: current_line[offset],
                        a: current_line[offset + 1],
                    });
                }
                0 | 3 => {}
                _ => unimplemented!("Unsupported color type"),
            }
        }
    }

    pixels
}
