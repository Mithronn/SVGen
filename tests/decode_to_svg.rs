use std::{env, fs::File, io::Read};

use svgen::{create_svg, structs::ColorMode};

fn init_logger() {
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();
}

fn main() {
    init_logger();

    let mut args = env::args();

    let file_name = args.nth(3).unwrap_or("assets/BWC.png".to_string());
    let color_mode = match args
        .nth(0)
        .unwrap_or("colored".to_string())
        .to_lowercase()
        .as_str()
    {
        "black" => ColorMode::Black,
        _ => ColorMode::Colored,
    };

    let mut file = File::open(file_name).unwrap();
    let mut buffer = Vec::new();

    file.read_to_end(&mut buffer).unwrap();

    let svg_string = create_svg(&buffer, color_mode);

    std::fs::write("assets/generated.svg", svg_string).expect("Unable to write file");
}
