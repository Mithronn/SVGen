use std::{
    num::ParseFloatError,
    ops::{Deref, DerefMut},
    str::FromStr,
};

use svg::node::{element::path::Data, Value};

use crate::utils::trunc;

#[derive(Clone, Debug)]
pub struct Parameters(pub Vec<f64>);

impl Deref for Parameters {
    type Target = [f64];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Parameters {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Parameters> for String {
    fn from(Parameters(inner): Parameters) -> Self {
        inner
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Position {
    Absolute,
    Relative,
}

#[derive(Debug, Clone)]
pub enum Command {
    M(Position, Parameters),
    L(Position, Parameters),
    H(Position, Parameters),
    V(Position, Parameters),
    C(Position, Parameters),
    S(Position, Parameters),
    Q(Position, Parameters),
    T(Position, Parameters),
    A(Position, Parameters),
    Z,
}

#[derive(Debug, Clone, Default)]
pub struct OptimizedData(Vec<Command>);

impl Deref for OptimizedData {
    type Target = [Command];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for OptimizedData {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl OptimizedData {
    pub fn new() -> Self {
        Default::default()
    }

    /// Add a command.
    #[inline]
    pub fn add(mut self, command: Command) -> Self {
        self.append(command);
        self
    }

    /// Append a command.
    #[inline]
    pub fn append(&mut self, command: Command) {
        self.0.push(command);
    }

    /// Convert all commands to relative.
    pub fn to_relative(&mut self) {
        let mut start = (0.0, 0.0);
        let mut cursor = (0.0, 0.0);

        for i in 0..self.0.len() {
            // Take ownership of the command using `std::mem::replace`.
            let command = std::mem::replace(&mut self.0[i], Command::Z);
            let new_command = match command {
                Command::M(pos, mut args) => {
                    if pos == Position::Absolute && i != 0 {
                        // Convert the coordinates to relative
                        args.0[0] -= cursor.0;
                        args.0[1] -= cursor.1;
                        // Update cursor after conversion
                        cursor.0 += args.0[0];
                        cursor.1 += args.0[1];
                        start = cursor;
                        Command::M(Position::Relative, args)
                    } else {
                        // For the first M (or if already relative)
                        cursor.0 += args.0[0];
                        cursor.1 += args.0[1];
                        start = cursor;
                        Command::M(pos, args)
                    }
                }
                Command::L(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.0;
                        args.0[1] -= cursor.1;
                        cursor.0 += args.0[0];
                        cursor.1 += args.0[1];
                        Command::L(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[0];
                        cursor.1 += args.0[1];
                        Command::L(pos, args)
                    }
                }
                Command::H(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.0;
                        cursor.0 += args.0[0];
                        Command::H(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[0];
                        Command::H(pos, args)
                    }
                }
                Command::V(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.1;
                        cursor.1 += args.0[0];
                        Command::V(Position::Relative, args)
                    } else {
                        cursor.1 += args.0[0];
                        Command::V(pos, args)
                    }
                }
                Command::C(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.0;
                        args.0[1] -= cursor.1;
                        args.0[2] -= cursor.0;
                        args.0[3] -= cursor.1;
                        args.0[4] -= cursor.0;
                        args.0[5] -= cursor.1;
                        cursor.0 += args.0[4];
                        cursor.1 += args.0[5];
                        Command::C(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[4];
                        cursor.1 += args.0[5];
                        Command::C(pos, args)
                    }
                }
                Command::S(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.0;
                        args.0[1] -= cursor.1;
                        args.0[2] -= cursor.0;
                        args.0[3] -= cursor.1;
                        cursor.0 += args.0[2];
                        cursor.1 += args.0[3];
                        Command::S(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[2];
                        cursor.1 += args.0[3];
                        Command::S(pos, args)
                    }
                }
                Command::Q(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.0;
                        args.0[1] -= cursor.1;
                        args.0[2] -= cursor.0;
                        args.0[3] -= cursor.1;
                        cursor.0 += args.0[2];
                        cursor.1 += args.0[3];
                        Command::Q(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[2];
                        cursor.1 += args.0[3];
                        Command::Q(pos, args)
                    }
                }
                Command::T(pos, mut args) => {
                    if pos == Position::Absolute {
                        args.0[0] -= cursor.0;
                        args.0[1] -= cursor.1;
                        cursor.0 += args.0[0];
                        cursor.1 += args.0[1];
                        Command::T(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[0];
                        cursor.1 += args.0[1];
                        Command::T(pos, args)
                    }
                }
                Command::A(pos, mut args) => {
                    if pos == Position::Absolute {
                        // For an elliptical arc, only the last two parameters are adjusted
                        args.0[5] -= cursor.0;
                        args.0[6] -= cursor.1;
                        cursor.0 += args.0[5];
                        cursor.1 += args.0[6];
                        Command::A(Position::Relative, args)
                    } else {
                        cursor.0 += args.0[5];
                        cursor.1 += args.0[6];
                        Command::A(pos, args)
                    }
                }
                Command::Z => {
                    // Close path: reset the cursor to the starting point.
                    cursor = start;
                    Command::Z
                }
            };
            // Put the processed command back into the vector.
            self.0[i] = new_command;
        }
    }

    pub fn optimize(&self) -> String {
        let mut output = String::with_capacity(self.0.len() * 4); // Preallocate estimated size
        let mut last_command: Option<char> = None;
        let mut last_char: Option<char> = None;

        for command in &self.0 {
            let (cmd_char, parameters, position) = match command {
                Command::M(pos, params) => ('M', params, pos),
                Command::L(pos, params) => ('L', params, pos),
                Command::H(pos, params) => ('H', params, pos),
                Command::V(pos, params) => ('V', params, pos),
                Command::C(pos, params) => ('C', params, pos),
                Command::S(pos, params) => ('S', params, pos),
                Command::Q(pos, params) => ('Q', params, pos),
                Command::T(pos, params) => ('T', params, pos),
                Command::A(pos, params) => ('A', params, pos),
                Command::Z => {
                    output.push('z');
                    last_char = Some('z');
                    continue;
                }
            };

            let letter = if *position == Position::Relative {
                cmd_char.to_ascii_lowercase()
            } else {
                cmd_char
            };

            // Append command letter only if different from the last command
            if Some(letter) != last_command {
                output.push(letter);
                last_command = Some(letter);
                last_char = Some(letter);
            }

            // Process parameters efficiently
            for (i, &num) in parameters.0.iter().enumerate() {
                let num_str = format_num(num);

                // Handle space insertion based on specific rules
                if i > 0 || last_char.map_or(false, |c| c != letter) {
                    // Only insert space when necessary:
                    // 1. If last char is a digit or '.' AND
                    // 2. Current number doesn't start with a minus sign AND
                    // 3. Current number doesn't start with '.' OR the previous char isn't '.'
                    if last_char.map_or(false, |c| (c.is_ascii_digit() || c == '.'))
                        && !num_str.starts_with('-')
                        && (!num_str.starts_with('.') || last_char != Some('.'))
                    {
                        output.push(' ');
                    }
                }

                output.push_str(&num_str);
                last_char = num_str.chars().last();
            }
        }
        output
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ParseDataError;

impl FromStr for OptimizedData {
    type Err = ParseDataError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Remove any leading/trailing whitespace.
        let s = s.trim();
        let mut commands = Vec::new();
        let mut chars = s.chars().peekable();

        while let Some(&ch) = chars.peek() {
            // Skip whitespace.
            if ch.is_whitespace() {
                chars.next();
                continue;
            }

            // The command letter must be one of the expected letters.
            let cmd_char = chars.next().ok_or_else(|| ParseDataError)?;

            // Special-case the Z/z command which takes no parameters.
            if cmd_char == 'Z' || cmd_char == 'z' {
                commands.push(Command::Z);
                continue;
            }

            // Determine position: uppercase means Absolute, lowercase means Relative.
            let position = if cmd_char.is_uppercase() {
                Position::Absolute
            } else {
                Position::Relative
            };

            // Accumulate characters that form the parameter part.
            let mut param_str = String::new();
            while let Some(&next_ch) = chars.peek() {
                // If the next character is alphabetic, it might be the next command.
                if next_ch.is_alphabetic() {
                    break;
                }
                param_str.push(next_ch);
                chars.next();
            }
            // Trim and split parameters on commas or whitespace.
            let param_str = param_str.trim();
            let numbers = if param_str.is_empty() {
                Vec::new()
            } else {
                param_str
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter(|s| !s.is_empty())
                    .map(|num_str| num_str.parse::<f64>())
                    .collect::<Result<Vec<f64>, ParseFloatError>>()
                    .map_err(|_| ParseDataError)?
            };
            let parameters = Parameters(numbers);

            // Depending on the command letter (normalized to uppercase) create the corresponding command.
            let command = match cmd_char.to_ascii_uppercase() {
                'M' => Command::M(position, parameters),
                'L' => Command::L(position, parameters),
                'H' => Command::H(position, parameters),
                'V' => Command::V(position, parameters),
                'C' => Command::C(position, parameters),
                'S' => Command::S(position, parameters),
                'Q' => Command::Q(position, parameters),
                'T' => Command::T(position, parameters),
                'A' => Command::A(position, parameters),
                _ => return Err(ParseDataError),
            };
            commands.push(command);
        }
        Ok(OptimizedData(commands))
    }
}

impl From<String> for OptimizedData {
    fn from(s: String) -> Self {
        // In this implementation we choose to panic on error.
        // Alternatively you could use a fallible conversion.
        s.parse()
            .expect("failed to parse OptimizedData from string")
    }
}

impl From<Data> for OptimizedData {
    fn from(data: Data) -> Self {
        let str_data: Value = data.into();
        OptimizedData::from(str_data.to_string())
    }
}

macro_rules! implement {
    ($($command:ident($position:ident) => $letter:expr,)*) => (
        impl From<Command> for String {
            fn from(command: Command) -> Self {
                use crate::path_optimizer::Command::*;
                use crate::path_optimizer::Position::*;
                match command {
                    $($command($position, parameters) => {
                        format!(concat!($letter, "{}"), String::from(parameters))
                    })*
                    Z => String::from("z"),
                }
            }
        }
    );
}

implement! {
    M(Absolute) => "M",
    M(Relative) => "m",
    L(Absolute) => "L",
    L(Relative) => "l",
    H(Absolute) => "H",
    H(Relative) => "h",
    V(Absolute) => "V",
    V(Relative) => "v",
    Q(Absolute) => "Q",
    Q(Relative) => "q",
    T(Absolute) => "T",
    T(Relative) => "t",
    C(Absolute) => "C",
    C(Relative) => "c",
    S(Absolute) => "S",
    S(Relative) => "s",
    A(Absolute) => "A",
    A(Relative) => "a",
}

/// Formats a number with a maximum of two decimal places, removing trailing zeros.
/// If the number is between -1 and 1 (excluding 0), the leading zero is removed.
/// Examples:
///   10.00 -> "10"
///   0.50  -> ".5"
///   -0.50 -> "-.5"
fn format_num(n: f64) -> String {
    // Format with two decimal places.
    let mut s = format!("{}", trunc(n));
    // Remove trailing zeros and the decimal point if unnecessary.
    if s.contains('.') {
        s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    }
    // Remove leading zero if between -1 and 1 and not zero.
    if s.starts_with("0.") {
        s = s.replacen("0", "", 1);
    } else if s.starts_with("-0.") {
        s = s.replacen("-0.", "-.", 1);
    }
    s
}
