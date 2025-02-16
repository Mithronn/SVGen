use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::structs::{CubicBezier, Point, Segment};

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

// Compute perpendicular distance from point p to the line (p0, p1)
fn distance_point_to_line(p: &Point, p0: &Point, p1: &Point) -> f64 {
    let numerator = ((p1.y - p0.y) * p.x - (p1.x - p0.x) * p.y + p1.x * p0.y - p1.y * p0.x).abs();
    let denominator = ((p1.y - p0.y).powi(2) + (p1.x - p0.x).powi(2)).sqrt();
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

// A simple implementation of the RDP algorithm to simplify a polyline.
pub fn rdp(points: &[Point], epsilon: f64) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let (index, dmax) = points
        .iter()
        .enumerate()
        .skip(1)
        .take(points.len() - 2)
        .map(|(i, p)| {
            (
                i,
                distance_point_to_line(p, &points[0], &points[points.len() - 1]),
            )
        })
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .unwrap();
    if dmax > epsilon {
        let rec_results1 = rdp(&points[0..=index], epsilon);
        let rec_results2 = rdp(&points[index..], epsilon);
        let mut result = rec_results1;
        result.pop(); // avoid duplicate point
        result.extend(rec_results2);
        result
    } else {
        vec![points[0].clone(), points[points.len() - 1].clone()]
    }
}

// Parameterize the given points using chord-length parameterization.
fn chord_length_parameterize(points: &[Point]) -> Vec<f64> {
    let n = points.len();
    let mut u = Vec::with_capacity(n);
    u.push(0.0);
    let mut total = 0.0;
    for i in 1..n {
        let d = points[i].distance(points[i - 1]);
        total += d;
        u.push(total);
    }
    if total.abs() < 1e-6 {
        return vec![0.0; n];
    }
    for val in u.iter_mut() {
        *val /= total;
    }
    u
}

// Generate a cubic Bézier curve approximating the given points.
// Uses the chord-length parameters 't' and the endpoints’ tangents.
fn generate_bezier(points: &[Point], t: &[f64], tangent0: Point, tangent1: Point) -> CubicBezier {
    let n = points.len();
    let p0 = points[0];
    let p3 = points[n - 1];

    let mut c = [[0.0, 0.0], [0.0, 0.0]];
    let mut x = [0.0, 0.0];

    for i in 0..n {
        let u = t[i];
        let u1 = 1.0 - u;
        let b0 = u1 * u1 * u1;
        let b3 = u * u * u;
        // Q_i is the linear part from endpoints.
        let q = p0.mul(b0).add(p3.mul(b3));
        let tmp = points[i].sub(q);

        let a1 = tangent0.mul(3.0 * u1 * u1 * u);
        let a2 = tangent1.mul(3.0 * u1 * u * u);

        c[0][0] += a1.dot(a1);
        c[0][1] += a1.dot(a2);
        c[1][1] += a2.dot(a2);

        x[0] += a1.dot(tmp);
        x[1] += a2.dot(tmp);
    }
    c[1][0] = c[0][1];

    let det = c[0][0] * c[1][1] - c[0][1] * c[1][0];
    let mut alpha_l;
    let mut alpha_r;
    if det.abs() > 1e-6 {
        alpha_l = (x[0] * c[1][1] - x[1] * c[0][1]) / det;
        alpha_r = (c[0][0] * x[1] - c[0][1] * x[0]) / det;
    } else {
        let dist = p0.distance(p3);
        alpha_l = dist / 3.0;
        alpha_r = dist / 3.0;
    }

    let seg_length = p0.distance(p3);
    let eps = 1e-6 * seg_length;
    if alpha_l < eps || alpha_r < eps {
        alpha_l = seg_length / 3.0;
        alpha_r = seg_length / 3.0;
    }

    let p1 = p0.add(tangent0.mul(alpha_l));
    let p2 = p3.add(tangent1.mul(alpha_r));

    CubicBezier { p0, p1, p2, p3 }
}

// Compute the maximum distance error between the points and the candidate Bézier curve.
fn compute_max_error(points: &[Point], bezier: &CubicBezier, t: &[f64]) -> (f64, usize) {
    let mut max_error = 0.0;
    let mut split_point = points.len() / 2;
    for (i, &param) in t.iter().enumerate() {
        let p_on_curve = bezier.evaluate(param);
        let d = points[i].distance(p_on_curve);
        if d > max_error {
            max_error = d;
            split_point = i;
        }
    }
    (max_error, split_point)
}

// Compute a tangent vector at a given index in the points array.
// For the first point, use the vector to the second point;
// for the last point, use the vector from the second-to-last point;
// otherwise, use the difference between the next and previous points.
fn compute_tangent(points: &[Point], index: usize) -> Point {
    if index == 0 {
        points[1].sub(points[0]).normalize()
    } else if index == points.len() - 1 {
        points[points.len() - 1]
            .sub(points[points.len() - 2])
            .normalize()
    } else {
        points[index + 1].sub(points[index - 1]).normalize()
    }
}

fn is_almost_line(points: &[Point], tol: f64) -> bool {
    let p0 = points.first().unwrap();
    let p_end = points.last().unwrap();
    let line_vec = p_end.sub(*p0);
    let line_len = line_vec.norm();
    if line_len < 1e-6 {
        return true;
    }
    let mut max_dist = 0.0;
    for p in points.iter() {
        let proj = p.sub(*p0).dot(line_vec) / (line_len * line_len);
        let closest = p0.add(line_vec.mul(proj));
        let d = p.distance(closest);
        if d > max_dist {
            max_dist = d;
        }
    }
    max_dist < tol
}

// Recursively fit the curve. Returns a vector of FittedSegment (Line or Cubic).
fn fit_curve_recursive(
    points: &[Point],
    tol: f64,
    tangent0: Point,
    tangent1: Point,
) -> Vec<Segment> {
    if is_almost_line(points, tol) {
        return vec![Segment::Line {
            start: points[0],
            end: *points.last().unwrap(),
        }];
    }

    let u = chord_length_parameterize(points);
    let bezier = generate_bezier(points, &u, tangent0, tangent1);
    let (max_error, split_point) = compute_max_error(points, &bezier, &u);

    if max_error < tol {
        // Even if acceptable, if the points are very nearly collinear, choose a line.
        if is_almost_line(points, tol / 2.0) {
            return vec![Segment::Line {
                start: points[0],
                end: *points.last().unwrap(),
            }];
        } else {
            return vec![Segment::Cubic(bezier)];
        }
    }

    let center_tangent = compute_tangent(points, split_point);
    let left = fit_curve_recursive(&points[0..=split_point], tol, tangent0, center_tangent);
    let right = fit_curve_recursive(
        &points[split_point..],
        tol,
        center_tangent.mul(-1.0),
        tangent1,
    );
    [left, right].concat()
}

// Fit curve entry point.
pub fn fit_curve(points: &[Point], tol: f64) -> Vec<Segment> {
    if points.len() < 2 {
        return Vec::new();
    }
    let tangent0 = compute_tangent(points, 0);
    let tangent1 = compute_tangent(points, points.len() - 1);
    fit_curve_recursive(points, tol, tangent0, tangent1)
}

// // Recursively fit a cubic Bézier to the given points.
// fn fit_cubic(points: &[Point], error: f64, tangent0: Point, tangent1: Point) -> Vec<CubicBezier> {
//     // If there are only two points, return the line segment as a cubic Bézier.
//     if points.len() == 2 {
//         let p0 = points[0];
//         let p3 = points[1];
//         let dist = p0.distance(p3);
//         let p1 = p0.add(tangent0.mul(dist / 3.0));
//         let p2 = p3.add(tangent1.mul(dist / 3.0));
//         return vec![CubicBezier { p0, p1, p2, p3 }];
//     }

//     // Parameterize points and generate a candidate curve.
//     let u = chord_length_parameterize(points);
//     let bezier = generate_bezier(points, &u, tangent0, tangent1);
//     let (max_error, split_point) = compute_max_error(points, &bezier, &u);

//     // If the error is within tolerance, return the curve.
//     if max_error < error {
//         return vec![bezier];
//     }

//     // Otherwise, split the points at the point of maximum error and fit recursively.
//     // Compute the center tangent at the split point.
//     let center_tangent = compute_tangent(points, split_point);
//     // Recursively fit the left segment.
//     let left = fit_cubic(&points[0..=split_point], error, tangent0, center_tangent);
//     // For the right segment, reverse the center tangent.
//     let right = fit_cubic(
//         &points[split_point..],
//         error,
//         center_tangent.mul(-1.0),
//         tangent1,
//     );

//     [left, right].concat()
// }

// // Fit a cubic Bézier curve (or curves) to the given set of points with the specified error tolerance.
// pub fn fit_curve(points: &[Point], error: f64) -> Vec<CubicBezier> {
//     if points.len() < 2 {
//         return Vec::new();
//     }
//     let tangent0 = compute_tangent(points, 0);
//     let tangent1 = compute_tangent(points, points.len() - 1);
//     fit_cubic(points, error, tangent0, tangent1)
// }
