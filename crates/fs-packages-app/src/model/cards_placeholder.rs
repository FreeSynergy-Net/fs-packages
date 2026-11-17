//! Pattern: none (plain functions: the stand-in logo of a program, from its name only).

/// The stand-in for a logo: the first letter on a coloured block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placeholder {
    /// The first letter of the name, upper case; a leading `fs-` is skipped, for it is on every name.
    pub letter: char,
    /// The colour at the top left, derived from the name only, so it is the same every time.
    pub rgb: (u8, u8, u8),
    /// The colour at the bottom right: the same hue, a little darker, for a soft gradient.
    pub rgb_end: (u8, u8, u8),
}

/// Letter and colour for `name`. The colour comes from a hash of the whole name (FNV-1a), turned into a hue with
/// fixed lightness, so every name keeps its colour and white letters stay readable.
#[must_use]
pub fn placeholder(name: &str) -> Placeholder {
    let plain = name.strip_prefix("fs-").unwrap_or(name);
    let letter = plain
        .chars()
        .find(|c| c.is_alphanumeric())
        .or_else(|| name.chars().next())
        .map_or('?', |c| c.to_uppercase().next().unwrap_or(c));
    let hash = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    #[allow(clippy::cast_precision_loss)]
    let hue = (hash % 360) as f32;
    Placeholder {
        letter,
        rgb: hsl(hue, 0.52, 0.52),
        rgb_end: hsl((hue + 16.0) % 360.0, 0.58, 0.36),
    }
}

/// Hue 0..360, saturation and lightness 0..1, to rgb.
fn hsl(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let mid = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let offset = lightness - chroma / 2.0;
    let (red, green, blue) = match hue {
        h if h < 60.0 => (chroma, mid, 0.0),
        h if h < 120.0 => (mid, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, mid),
        h if h < 240.0 => (0.0, mid, chroma),
        h if h < 300.0 => (mid, 0.0, chroma),
        _ => (chroma, 0.0, mid),
    };
    let byte = |value: f32| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let out = ((value + offset) * 255.0).round() as u8;
        out
    };
    (byte(red), byte(green), byte(blue))
}
