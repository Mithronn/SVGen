use crate::{structs::TurnPolicy, vec2::IVec2};

const DIR_L: u8 = 1 << 0;
const DIR_R: u8 = 1 << 1;
const DIR_D: u8 = 1 << 2;
const DIR_U: u8 = 1 << 3;

#[inline(always)]
fn index(x: usize, y: usize, x_span: usize) -> usize {
    x + y * x_span
}

#[inline(always)]
fn is_filled_left(image: &[bool], size: &[usize; 2], x: usize, y: usize) -> bool {
    if x > 0 {
        image[index(x - 1, y, size[0])]
    } else {
        false
    }
}

#[inline(always)]
fn is_filled_right(image: &[bool], size: &[usize; 2], x: usize, y: usize) -> bool {
    if x + 1 < size[0] {
        image[index(x + 1, y, size[0])]
    } else {
        false
    }
}

#[inline(always)]
fn is_filled_down(image: &[bool], size: &[usize; 2], x: usize, y: usize) -> bool {
    if y > 0 {
        image[index(x, y - 1, size[0])]
    } else {
        false
    }
}

#[inline(always)]
fn is_filled_up(image: &[bool], size: &[usize; 2], x: usize, y: usize) -> bool {
    if y + 1 < size[1] {
        image[index(x, y + 1, size[0])]
    } else {
        false
    }
}

/// Moves (x, y) in the given direction.
fn step_move(dir: u8, x: &mut i32, y: &mut i32) {
    match dir {
        DIR_L => *x -= 1,
        DIR_R => *x += 1,
        DIR_D => *y -= 1,
        DIR_U => *y += 1,
        _ => unreachable!(),
    }
}

/// Returns the first matching direction from the cell without borrowing `pimage` for too long.
fn step_first_match(
    pimage: &[u8],
    idx: impl Fn(i32, i32) -> usize,
    d1: u8,
    d2: u8,
    d3: u8,
    x: &mut i32,
    y: &mut i32,
) -> u8 {
    let cell = pimage[idx(*x, *y)];
    if (cell & d1) != 0 {
        step_move(d1, x, y);
        d1
    } else if (cell & d2) != 0 {
        step_move(d2, x, y);
        d2
    } else if (cell & d3) != 0 {
        step_move(d3, x, y);
        d3
    } else {
        unreachable!()
    }
}

/// Extract the outline from an image.
/// Returns a Vec of (flag, polygon) pairs.
pub fn extract_outline(
    image: &[bool],
    size: &[usize; 2],
    turn_policy: TurnPolicy,
    use_simplify: bool,
) -> Vec<(bool, Vec<IVec2>)> {
    let padded_size = [size[0] + 1, size[1] + 1];
    let mut pimage = vec![0u8; padded_size[0] * padded_size[1]];

    // Populate the padded image with directional flags.
    let mut steps_total = 0;
    for y in 0..size[1] {
        for x in 0..size[0] {
            if image[index(x, y, size[0])] {
                if !is_filled_left(image, size, x, y) {
                    pimage[index(x, y, padded_size[0])] |= DIR_U;
                    steps_total += 1;
                }
                if !is_filled_right(image, size, x, y) {
                    pimage[index(x + 1, y + 1, padded_size[0])] |= DIR_D;
                    steps_total += 1;
                }
                if !is_filled_down(image, size, x, y) {
                    pimage[index(x + 1, y, padded_size[0])] |= DIR_L;
                    steps_total += 1;
                }
                if !is_filled_up(image, size, x, y) {
                    pimage[index(x, y + 1, padded_size[0])] |= DIR_R;
                    steps_total += 1;
                }
            }
        }
    }

    let mut poly_list = Vec::new();

    // The inner function for following a polygon from a starting point.
    fn poly_from_direction_mask(
        pimage: &mut [u8],
        x_init: i32,
        y_init: i32,
        x_span: i32,
        image_data: (&[bool], IVec2),
        turn_policy: TurnPolicy,
        use_simplify: bool,
        initial_dir: u8,
    ) -> (Vec<IVec2>, usize) {
        let mut poly = Vec::new();
        let (mut x, mut y) = (x_init, y_init);
        let mut prev_dir = initial_dir;
        let mut handled = 0;

        let idx = |x: i32, y: i32| -> usize { (x as usize) + (y as usize) * (x_span as usize) };

        // Check whether the majority of the neighborhood is filled.
        let is_majority = |x: i32, y: i32, data: (&[bool], IVec2)| -> bool {
            let (img, dims) = data;
            let xy_or = |x: i32, y: i32, default: bool| -> bool {
                if x >= 0 && x < dims.x && y >= 0 && y < dims.y {
                    img[index(x as usize, y as usize, dims.x as usize)]
                } else {
                    default
                }
            };
            for i in 2..5 {
                let mut ct = 0;
                for a in (-i + 1)..i {
                    ct += if xy_or(x + a, y + i - 1, false) {
                        1
                    } else {
                        -1
                    };
                    ct += if xy_or(x + i - 1, y + a - 1, false) {
                        1
                    } else {
                        -1
                    };
                    ct += if xy_or(x + a - 1, y - i, false) {
                        1
                    } else {
                        -1
                    };
                    ct += if xy_or(x - i, y + a, false) { 1 } else { -1 };
                }
                if ct > 0 {
                    return true;
                } else if ct < 0 {
                    return false;
                }
            }
            false
        };

        loop {
            // Simplify collinear points if requested.
            if use_simplify && poly.len() > 1 {
                let a: IVec2 = poly[poly.len() - 2];
                let b: IVec2 = poly[poly.len() - 1];
                if (x == a.x && x == b.x) || (y == a.y && y == b.y) {
                    if let Some(last) = poly.last_mut() {
                        last.x = x;
                        last.y = y;
                    }
                } else {
                    poly.push(IVec2 {
                        x,
                        y,
                        ..IVec2::ZERO
                    });
                }
            } else {
                poly.push(IVec2 {
                    x,
                    y,
                    ..IVec2::ZERO
                });
            }

            // End the loop when we return to the starting point.
            if handled != 0 && x == x_init && y == y_init {
                poly.pop();
                break;
            }

            let cell_index = idx(x, y);
            let cell = pimage[cell_index];

            // Decide on the next move.
            let next_dir = if [DIR_L, DIR_R, DIR_D, DIR_U].contains(&cell) {
                // Non-ambiguous case.
                step_move(cell, &mut x, &mut y);
                cell
            } else {
                // Ambiguous: choose turn based on policy.
                let turn_ccw = match turn_policy {
                    TurnPolicy::Black => true,
                    TurnPolicy::White => false,
                    TurnPolicy::Majority => is_majority(x, y, image_data),
                    TurnPolicy::Minority => !is_majority(x, y, image_data),
                };

                if !turn_ccw {
                    match prev_dir {
                        DIR_L => {
                            step_first_match(&pimage, &idx, DIR_D, DIR_L, DIR_U, &mut x, &mut y)
                        }
                        DIR_U => {
                            step_first_match(&pimage, &idx, DIR_L, DIR_U, DIR_R, &mut x, &mut y)
                        }
                        DIR_R => {
                            step_first_match(&pimage, &idx, DIR_U, DIR_R, DIR_D, &mut x, &mut y)
                        }
                        DIR_D => {
                            step_first_match(&pimage, &idx, DIR_R, DIR_D, DIR_L, &mut x, &mut y)
                        }
                        _ => unreachable!(),
                    }
                } else {
                    match prev_dir {
                        DIR_L => {
                            step_first_match(&pimage, &idx, DIR_U, DIR_L, DIR_D, &mut x, &mut y)
                        }
                        DIR_U => {
                            step_first_match(&pimage, &idx, DIR_R, DIR_U, DIR_L, &mut x, &mut y)
                        }
                        DIR_R => {
                            step_first_match(&pimage, &idx, DIR_D, DIR_R, DIR_U, &mut x, &mut y)
                        }
                        DIR_D => {
                            step_first_match(&pimage, &idx, DIR_L, DIR_D, DIR_R, &mut x, &mut y)
                        }
                        _ => unreachable!(),
                    }
                }
            };

            // Now that any immutable borrows are done, update the cell.
            pimage[cell_index] &= !next_dir;
            prev_dir = next_dir;
            handled += 1;
        }
        (poly, handled)
    }

    let image_data = (image, IVec2::new(size[0] as i32, size[1] as i32));
    let mut steps_handled = 0;

    'outer: for y in 0..padded_size[1] {
        for x in 0..padded_size[0] {
            let cell_index = index(x, y, padded_size[0]);
            if pimage[cell_index] & DIR_U != 0 {
                let (poly, handled) = poly_from_direction_mask(
                    &mut pimage,
                    x as i32,
                    y as i32,
                    padded_size[0] as i32,
                    image_data,
                    turn_policy,
                    use_simplify,
                    DIR_L,
                );
                poly_list.push((true, poly));
                steps_handled += handled;
                if steps_handled >= steps_total {
                    break 'outer;
                }
            }
        }
    }

    poly_list
}
