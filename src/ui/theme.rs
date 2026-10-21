use ratatui::style::Color;

/// The UI palette. A near-black background with light text. The values
/// complement the syntect `base16-eighties.dark` theme.
pub const BACKGROUND: Color = Color::Rgb(0x0f, 0x11, 0x15);
pub const FOREGROUND: Color = Color::Rgb(0xe6, 0xe9, 0xef);
pub const MUTED: Color = Color::Rgb(0xaa, 0xb4, 0xc0);
pub const BORDER: Color = Color::Rgb(0x6b, 0x76, 0x84);
pub const HEADER: Color = Color::Rgb(0xff, 0xd1, 0x8a);
pub const SELECTED_FG: Color = Color::Rgb(0x0f, 0x11, 0x15);
pub const SELECTED_BG: Color = Color::Rgb(0xa9, 0xc1, 0xe8);

/// Parse a Linguist hex color and raise dark values until they are readable on
/// the dark background.
pub fn language_color(hex: &str) -> Option<Color> {
    let hex = hex.trim().strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|character| character.is_ascii_hexdigit()) {
        return None;
    }

    let red = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let green = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let blue = u8::from_str_radix(&hex[4..6], 16).ok()?;
    let (red, green, blue) = readable(red, green, blue);
    Some(Color::Rgb(red, green, blue))
}

fn readable(red: u8, green: u8, blue: u8) -> (u8, u8, u8) {
    let luminance =
        0.2126 * red as f32 + 0.7152 * green as f32 + 0.0722 * blue as f32;
    if luminance >= 110.0 {
        return (red, green, blue);
    }

    let factor = (170.0 - luminance) / (255.0 - luminance);
    let blend = |channel: u8| {
        (channel as f32 + (255.0 - channel as f32) * factor)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    (blend(red), blend(green), blend(blue))
}
