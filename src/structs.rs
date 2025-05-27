use wasm_bindgen::prelude::*;

#[derive(Copy, Clone)]
pub enum TurnPolicy {
    Black,
    White,
    Majority,
    Minority,
}

#[wasm_bindgen]
#[derive(Copy, Clone)]
pub enum ColorMode {
    Black,
    Colored,
}
