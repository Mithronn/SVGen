use crate::vec2::DVec2;

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

// Subdivide
pub fn poly_subdivide(is_cyclic: bool, poly_src: &Vec<DVec2>) -> Vec<DVec2> {
    let mut poly_dst: Vec<DVec2> = Vec::with_capacity(poly_src.len() * 2);
    let mut v_orig_prev = &poly_src[if is_cyclic { poly_src.len() - 1 } else { 0 }];
    if !is_cyclic {
        poly_dst.push(*v_orig_prev);
    }

    for v_orig_curr in &poly_src[(if is_cyclic { 0 } else { 1 })..] {
        // subdivided point
        poly_dst.push(v_orig_prev.mid(*v_orig_curr));
        // regular point
        poly_dst.push(*v_orig_curr);
        v_orig_prev = v_orig_curr;
    }
    return poly_dst;
}

pub fn poly_list_subdivide(poly_list_src: &mut Vec<(bool, Vec<DVec2>)>) {
    poly_list_src
        .iter_mut()
        .for_each(|(is_cyclic, poly_src)| *poly_src = poly_subdivide(*is_cyclic, &poly_src))
}

// Subdivide until segments are smaller then the limit
pub fn poly_subdivide_to_limit(is_cyclic: bool, poly_src: &Vec<DVec2>, limit: f64) -> Vec<DVec2> {
    // target size isn't known. but will be at least as big as the source
    let mut poly_dst: Vec<DVec2> = Vec::with_capacity(poly_src.len());

    let limit_sq = DVec2::sq(limit);
    let mut v_orig_prev = &poly_src[if is_cyclic { poly_src.len() - 1 } else { 0 }];
    if !is_cyclic {
        poly_dst.push(*v_orig_prev);
    }

    for v_orig_curr in &poly_src[(if is_cyclic { 0 } else { 1 })..] {
        // subdivided point(s)
        let len_sq = v_orig_prev.len_squared_with(*v_orig_curr);
        if len_sq > limit_sq {
            let len = len_sq.sqrt();
            let sub = (len / limit).floor();
            let inc = 1.0 / sub;
            let mut step = inc;
            for _ in 0..((sub as usize) - 1) {
                poly_dst.push(v_orig_prev.interp(*v_orig_curr, step));
                debug_assert!(step > 0.0 && step < 1.0);
                step += inc;
            }
        }
        // regular point
        poly_dst.push(*v_orig_curr);
        v_orig_prev = v_orig_curr;
    }

    return poly_dst;
}

pub fn poly_list_subdivide_to_limit(poly_list_src: &mut Vec<(bool, Vec<DVec2>)>, limit: f64) {
    poly_list_src.iter_mut().for_each(|(is_cyclic, poly_src)| {
        *poly_src = poly_subdivide_to_limit(*is_cyclic, &poly_src, limit)
    })
}
