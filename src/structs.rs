#[derive(Copy, Clone)]
pub enum TurnPolicy {
    Black,
    White,
    Majority,
    Minority,
}

#[derive(Copy, Clone)]
pub enum ColorMode {
    Black,
    Colored,
}
