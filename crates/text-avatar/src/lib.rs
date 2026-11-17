use std::io::Cursor;

use ab_glyph::{Font, FontRef, OutlinedGlyph, PxScale, Rect, ScaleFont, point};
use image::{ImageFormat, Rgb, RgbImage};

const ROBOTO_CAPITALS_AND_DIGITS: &[u8] = include_bytes!("roboto-regular-capitals.ttf");
/// `wa-avatar` draws its initials at 40% of the avatar size.
const TEXT_HEIGHT: f32 = 0.4;
/// Android crops the image to a circle, so wider text must shrink to stay inside it.
const MAX_TEXT_WIDTH: f32 = 0.7;

/// A `size`×`size` PNG of `text` centred on `background`. `None` when the font
/// lacks one of the characters.
pub fn text_avatar_png(
    text: &str,
    size: u32,
    background: Rgb<u8>,
    foreground: Rgb<u8>,
) -> Option<Vec<u8>> {
    let font =
        FontRef::try_from_slice(ROBOTO_CAPITALS_AND_DIGITS).expect("the embedded font parses");
    if text.is_empty() || text.chars().any(|c| font.glyph_id(c).0 == 0) {
        return None;
    }
    let size_px = size as f32;
    let mut glyphs = layout(&font, text, size_px * TEXT_HEIGHT);
    let mut bounds = text_bounds(&glyphs)?;
    if bounds.width() > size_px * MAX_TEXT_WIDTH {
        let shrunk_px = size_px * TEXT_HEIGHT * size_px * MAX_TEXT_WIDTH / bounds.width();
        glyphs = layout(&font, text, shrunk_px);
        bounds = text_bounds(&glyphs)?;
    }
    let offset_x = (size_px - bounds.width()) / 2.0 - bounds.min.x;
    let offset_y = (size_px - bounds.height()) / 2.0 - bounds.min.y;
    let mut image = RgbImage::from_pixel(size, size, background);
    for glyph in &glyphs {
        let glyph_bounds = glyph.px_bounds();
        glyph.draw(|x, y, coverage| {
            let px = (glyph_bounds.min.x + offset_x) as i64 + x as i64;
            let py = (glyph_bounds.min.y + offset_y) as i64 + y as i64;
            blend(&mut image, px, py, foreground, coverage);
        });
    }
    let mut png = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("encoding a PNG in memory cannot fail");
    Some(png)
}

fn layout(font: &FontRef, text: &str, px: f32) -> Vec<OutlinedGlyph> {
    let scaled = font.as_scaled(PxScale::from(px));
    let mut caret = 0.0;
    text.chars()
        .filter_map(|c| {
            let glyph = scaled.scaled_glyph(c);
            let advance = scaled.h_advance(glyph.id);
            let positioned = ab_glyph::Glyph {
                position: point(caret, 0.0),
                ..glyph
            };
            caret += advance;
            font.outline_glyph(positioned)
        })
        .collect()
}

fn text_bounds(glyphs: &[OutlinedGlyph]) -> Option<Rect> {
    glyphs.iter().map(OutlinedGlyph::px_bounds).reduce(union)
}

fn union(a: Rect, b: Rect) -> Rect {
    Rect {
        min: point(a.min.x.min(b.min.x), a.min.y.min(b.min.y)),
        max: point(a.max.x.max(b.max.x), a.max.y.max(b.max.y)),
    }
}

fn blend(image: &mut RgbImage, x: i64, y: i64, color: Rgb<u8>, coverage: f32) {
    let (Ok(x), Ok(y)) = (u32::try_from(x), u32::try_from(y)) else {
        return;
    };
    let Some(pixel) = image.get_pixel_mut_checked(x, y) else {
        return;
    };
    let coverage = coverage.clamp(0.0, 1.0);
    for (channel, target) in pixel.0.iter_mut().zip(color.0) {
        *channel = (*channel as f32 * (1.0 - coverage) + target as f32 * coverage).round() as u8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GREY: Rgb<u8> = Rgb([0x8e, 0x8e, 0x93]);
    const WHITE: Rgb<u8> = Rgb([0xff, 0xff, 0xff]);

    #[test]
    fn draws_latin_greek_and_cyrillic_capitals() {
        for text in ["GC", "ÖL", "ΩΑ", "ΆΝ", "ЖБ", "42"] {
            let png = text_avatar_png(text, 192, GREY, WHITE).expect(text);
            let image = image::load_from_memory(&png).unwrap();
            assert_eq!((image.width(), image.height()), (192, 192));
        }
    }

    #[test]
    fn keeps_wide_text_inside_the_circle() {
        let png = text_avatar_png("WWW", 192, GREY, WHITE).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgb8();
        let margin = (192.0 * (1.0 - MAX_TEXT_WIDTH) / 2.0) as u32 - 1;
        let mut touched_columns =
            (0..192).filter(|&x| (0..192).any(|y| image.get_pixel(x, y) != &GREY));
        assert!(touched_columns.all(|x| x >= margin && x < 192 - margin));
    }

    #[test]
    fn refuses_text_the_font_lacks() {
        assert_eq!(text_avatar_png("مر", 192, GREY, WHITE), None);
        assert_eq!(text_avatar_png("", 192, GREY, WHITE), None);
    }
}
