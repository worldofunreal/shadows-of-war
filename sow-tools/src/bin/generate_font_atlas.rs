//! Build the gameplay nameplate atlas from the supported Unicode glyph corpus.
//!
//! Usage:
//! cargo run -p sow-tools --bin generate-font-atlas -- \
//!   assets/gameplay/fonts/WorkSans-Black.ttf \
//!   sow-web/site/fonts/noto-sans-regular.ttf \
//!   sow-web/site/fonts/noto-sans-arabic-regular.ttf \
//!   sow-web/site/fonts/noto-sans-cjk-regular.ttc \
//!   sow-render/src/text/unicode_name_glyphs.txt \
//!   assets/gameplay/fonts/msdf-atlas.png \
//!   assets/gameplay/fonts/msdf-atlas.json
//!
//! The corpus sections select Vietnamese Latin diacritics, Cyrillic, Japanese,
//! Korean, Simplified and Traditional Chinese, and Arabic glyphs; they stay separate from the ghost-name list.

use ab_glyph::{Font, FontRef, GlyphId, PxScale, ScaleFont};
use rustybuzz::{Direction, Face as ShapeFace, UnicodeBuffer};
use std::collections::{BTreeMap, BTreeSet};

const FONT_SIZE: f32 = 48.0;
const DISTANCE_RANGE: f32 = 16.0;
const PADDING: u32 = 8;
const ATLAS_SIZE: u32 = 2048;
const UPSAMPLE: u32 = 4;

#[derive(Clone, Copy, Debug)]
enum FontSource {
    Latin,
    Cyrillic,
    Japanese,
    Korean,
    Chinese,
    ChineseTraditional,
    Arabic,
}

struct FontSet<'a> {
    latin: FontRef<'a>,
    cyrillic: FontRef<'a>,
    japanese: FontRef<'a>,
    korean: FontRef<'a>,
    chinese: FontRef<'a>,
    chinese_traditional: FontRef<'a>,
    arabic: FontRef<'a>,
}

impl FontSet<'_> {
    fn get(&self, source: FontSource) -> &FontRef<'_> {
        match source {
            FontSource::Latin => &self.latin,
            FontSource::Cyrillic => &self.cyrillic,
            FontSource::Japanese => &self.japanese,
            FontSource::Korean => &self.korean,
            FontSource::Chinese => &self.chinese,
            FontSource::ChineseTraditional => &self.chinese_traditional,
            FontSource::Arabic => &self.arabic,
        }
    }
}

struct GlyphInput {
    source: FontSource,
    glyph_id: u32,
    character: Option<char>,
    arabic_glyph_id: Option<u32>,
}

struct GlyphSheet {
    id: u32,
    arabic_glyph_id: Option<u32>,
    character: String,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    xoffset: i32,
    yoffset: i32,
    xadvance: u32,
    pixels: Vec<u8>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 8 {
        eprintln!(
            "Usage: {} <latin.ttf> <cyrillic.ttf> <arabic.ttf> <cjk.ttc> <unicode-glyphs.txt> <out.png> <out.json>",
            args.first()
                .map(String::as_str)
                .unwrap_or("generate-font-atlas")
        );
        std::process::exit(1);
    }

    let latin_data = std::fs::read(&args[1]).expect("Failed to read Latin font");
    let cyrillic_data = std::fs::read(&args[2]).expect("Failed to read Cyrillic font");
    let arabic_data = std::fs::read(&args[3]).expect("Failed to read Arabic font");
    let cjk_data = std::fs::read(&args[4]).expect("Failed to read CJK font collection");
    let glyphs_data =
        std::fs::read_to_string(&args[5]).expect("Failed to read Unicode glyph corpus");
    let latin = FontRef::try_from_slice(&latin_data).expect("Failed to parse Latin font");
    let cyrillic =
        FontRef::try_from_slice(&cyrillic_data).expect("Failed to parse Cyrillic font");
    let arabic = FontRef::try_from_slice(&arabic_data).expect("Failed to parse Arabic font");
    let japanese =
        FontRef::try_from_slice_and_index(&cjk_data, 0).expect("Failed to parse Japanese face");
    let korean =
        FontRef::try_from_slice_and_index(&cjk_data, 1).expect("Failed to parse Korean face");
    let chinese = FontRef::try_from_slice_and_index(&cjk_data, 2)
        .expect("Failed to parse Simplified Chinese face");
    let chinese_traditional = FontRef::try_from_slice_and_index(&cjk_data, 3)
        .expect("Failed to parse Traditional Chinese face");
    let fonts = FontSet {
        latin,
        cyrillic,
        japanese,
        korean,
        chinese,
        chinese_traditional,
        arabic,
    };
    let arabic_shaper =
        ShapeFace::from_slice(&arabic_data, 0).expect("Failed to parse Arabic shaping face");

    let mut sources = BTreeMap::<char, FontSource>::new();
    for codepoint in 32u32..=126 {
        sources.insert(char::from_u32(codepoint).unwrap(), FontSource::Latin);
    }
    let mut arabic_chars = BTreeSet::new();
    let mut section = "";
    for line in glyphs_data.lines() {
        if let Some(name_section) = line.strip_prefix("# ") {
            section = name_section;
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let source = match section {
            "cyrillic" => FontSource::Cyrillic,
            "japanese_katakana" => FontSource::Japanese,
            "japanese_name_handles" => FontSource::Japanese,
            "korean_hangul" => FontSource::Korean,
            "chinese_simplified" => FontSource::Chinese,
            "chinese_traditional" => FontSource::ChineseTraditional,
            "vietnamese" => FontSource::Latin,
            "arabic" => {
                arabic_chars.extend(line.chars().filter(|character| !character.is_whitespace()));
                continue;
            }
            _ => panic!("unknown Unicode glyph section: {section}"),
        };
        for character in line.chars().filter(|character| !character.is_whitespace()) {
            if matches!(source, FontSource::Chinese | FontSource::ChineseTraditional) {
                sources.insert(character, source);
            } else {
                sources.entry(character).or_insert(source);
            }
        }
    }
    assert!(
        !arabic_chars.is_empty(),
        "Unicode glyph corpus has no Arabic characters"
    );

    let mut glyphs = Vec::<GlyphInput>::with_capacity(sources.len() + 256);
    for (character, source) in sources {
        let glyph_id = fonts.get(source).glyph_id(character).0 as u32;
        assert_ne!(
            glyph_id, 0,
            "selected font does not contain {character:?} from {source:?}"
        );
        glyphs.push(GlyphInput {
            source,
            glyph_id,
            character: Some(character),
            arabic_glyph_id: None,
        });
    }

    let mut arabic_glyph_ids = BTreeSet::new();
    let mut arabic_samples = Vec::new();
    for &character in &arabic_chars {
        let nominal_glyph = fonts.arabic.glyph_id(character).0 as u32;
        assert_ne!(nominal_glyph, 0, "Arabic font lacks {character:?}");
        arabic_samples.push(character.to_string());
        arabic_samples.push(format!("{character}{character}"));
        arabic_samples.push(format!("ب{character}ب"));
        for &next in &arabic_chars {
            arabic_samples.push(format!("{character}{next}"));
        }
    }
    for sample in arabic_samples {
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(&sample);
        buffer.set_direction(Direction::RightToLeft);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&arabic_shaper, &[], buffer);
        for info in shaped.glyph_infos() {
            arabic_glyph_ids.insert(info.glyph_id);
        }
    }
    assert!(
        !arabic_glyph_ids.is_empty(),
        "Unicode glyph corpus produced no Arabic glyphs"
    );
    for glyph_id in arabic_glyph_ids {
        assert!(
            glyph_id <= u16::MAX as u32,
            "Arabic font glyph id exceeds ab_glyph's supported range: {glyph_id}"
        );
        glyphs.push(GlyphInput {
            source: FontSource::Arabic,
            glyph_id,
            character: None,
            arabic_glyph_id: Some(glyph_id),
        });
    }

    eprintln!(
        "Rasterizing {} direct glyphs and {} Arabic shaped glyphs at size {}...",
        glyphs
            .iter()
            .filter(|glyph| glyph.character.is_some())
            .count(),
        glyphs
            .iter()
            .filter(|glyph| glyph.arabic_glyph_id.is_some())
            .count(),
        FONT_SIZE
    );

    let scale = PxScale::from(FONT_SIZE);
    let hi_scale = PxScale::from(FONT_SIZE * UPSAMPLE as f32);
    let latin_scaled = fonts.latin.as_scaled(scale);
    let ascent = latin_scaled.ascent();
    let descent = latin_scaled.descent();
    let line_gap = latin_scaled.line_gap();
    let line_height = (ascent - descent + line_gap).ceil() as u32;
    let base = ascent.ceil() as u32;

    let mut sheets = Vec::<GlyphSheet>::with_capacity(glyphs.len());
    for input in glyphs {
        let font = fonts.get(input.source);
        let glyph_id = GlyphId(input.glyph_id as u16);
        let scaled = font.as_scaled(scale);
        let xadvance = scaled.h_advance(glyph_id).round().max(0.0) as u32;
        let bounds = font
            .outline_glyph(glyph_id.with_scale(scale))
            .map(|outline| outline.px_bounds());
        let (cell_w, cell_h, xoffset, yoffset) = match bounds {
            Some(bounds) => {
                let width = (bounds.width().ceil() as u32).max(1) + PADDING * 2;
                let height = (bounds.height().ceil() as u32).max(1) + PADDING * 2;
                let xoffset = bounds.min.x.round() as i32 - PADDING as i32;
                let yoffset = base as i32 + bounds.min.y.round() as i32 - PADDING as i32;
                (width, height, xoffset, yoffset)
            }
            None => (
                PADDING * 2 + 1,
                PADDING * 2 + 1,
                -(PADDING as i32),
                base as i32 - PADDING as i32,
            ),
        };

        let raster_w = cell_w as usize * UPSAMPLE as usize;
        let raster_h = cell_h as usize * UPSAMPLE as usize;
        let mut mask = vec![0u8; raster_w * raster_h];
        if let Some(outlined) = font.outline_glyph(glyph_id.with_scale(hi_scale)) {
            let offset = (PADDING * UPSAMPLE) as i32;
            outlined.draw(|px, py, coverage| {
                let x = px as i32 + offset;
                let y = py as i32 + offset;
                if x >= 0 && y >= 0 && (x as usize) < raster_w && (y as usize) < raster_h {
                    let index = y as usize * raster_w + x as usize;
                    mask[index] = mask[index].max((coverage * 255.0) as u8);
                }
            });
        }

        let sdf = compute_sdf(&mask, raster_w, raster_h, DISTANCE_RANGE * UPSAMPLE as f32);
        let mut pixels = Vec::with_capacity((cell_w * cell_h * 3) as usize);
        for y in 0..cell_h as usize {
            for x in 0..cell_w as usize {
                let mut sum = 0.0f32;
                for dy in 0..UPSAMPLE as usize {
                    for dx in 0..UPSAMPLE as usize {
                        sum += sdf
                            [(y * UPSAMPLE as usize + dy) * raster_w + x * UPSAMPLE as usize + dx];
                    }
                }
                let value =
                    (sum / (UPSAMPLE * UPSAMPLE) as f32 / (DISTANCE_RANGE * UPSAMPLE as f32) + 0.5)
                        .clamp(0.0, 1.0);
                let byte = (value * 255.0) as u8;
                pixels.extend_from_slice(&[byte, byte, byte]);
            }
        }

        let id = input
            .arabic_glyph_id
            .map(|glyph_id| 0x110000 + glyph_id)
            .or_else(|| input.character.map(u32::from))
            .expect("each atlas glyph needs a stable id");
        sheets.push(GlyphSheet {
            id,
            arabic_glyph_id: input.arabic_glyph_id,
            character: input
                .character
                .map_or_else(String::new, |ch| ch.to_string()),
            x: 0,
            y: 0,
            width: cell_w,
            height: cell_h,
            xoffset,
            yoffset,
            xadvance,
            pixels,
        });
    }

    // Large glyphs first keeps the shelf pack dense across the mixed font faces.
    sheets.sort_by(|left, right| {
        right
            .height
            .cmp(&left.height)
            .then_with(|| right.width.cmp(&left.width))
            .then_with(|| left.id.cmp(&right.id))
    });
    eprintln!("Packing glyphs into {ATLAS_SIZE}x{ATLAS_SIZE} atlas...");
    let mut atlas_pixels = vec![0u8; ATLAS_SIZE as usize * ATLAS_SIZE as usize * 3];
    let (mut cursor_x, mut cursor_y, mut row_height) = (0u32, 0u32, 0u32);
    for sheet in &mut sheets {
        if sheet.width > ATLAS_SIZE || sheet.height > ATLAS_SIZE {
            panic!(
                "glyph exceeds atlas dimensions: {}x{}",
                sheet.width, sheet.height
            );
        }
        if cursor_x + sheet.width > ATLAS_SIZE {
            cursor_x = 0;
            cursor_y += row_height;
            row_height = 0;
        }
        if cursor_y + sheet.height > ATLAS_SIZE {
            panic!("font atlas overflow; increase ATLAS_SIZE");
        }
        sheet.x = cursor_x;
        sheet.y = cursor_y;
        for y in 0..sheet.height {
            for x in 0..sheet.width {
                let src = ((y * sheet.width + x) * 3) as usize;
                let dst = (((cursor_y + y) * ATLAS_SIZE + cursor_x + x) * 3) as usize;
                atlas_pixels[dst..dst + 3].copy_from_slice(&sheet.pixels[src..src + 3]);
            }
        }
        cursor_x += sheet.width;
        row_height = row_height.max(sheet.height);
    }

    let image = image::RgbImage::from_raw(ATLAS_SIZE, ATLAS_SIZE, atlas_pixels)
        .expect("Failed to create atlas image");
    image.save(&args[6]).expect("Failed to save atlas PNG");

    let chars_json: Vec<serde_json::Value> = sheets
        .iter()
        .enumerate()
        .map(|(index, sheet)| {
            let mut record = serde_json::json!({
                "id": sheet.id,
                "index": index,
                "char": sheet.character,
                "width": sheet.width,
                "height": sheet.height,
                "xoffset": sheet.xoffset,
                "yoffset": sheet.yoffset,
                "xadvance": sheet.xadvance,
                "chnl": 15,
                "x": sheet.x,
                "y": sheet.y,
                "page": 0,
            });
            if let Some(glyph_id) = sheet.arabic_glyph_id {
                record["glyphId"] = serde_json::json!(glyph_id);
            }
            record
        })
        .collect();
    let charset: Vec<String> = sheets
        .iter()
        .filter_map(|sheet| sheet.character.chars().next().map(|ch| ch.to_string()))
        .collect();
    let atlas_json = serde_json::json!({
        "pages": ["msdf-atlas.png"],
        "chars": chars_json,
        "info": {
            "face": "Work Sans + Noto Sans Cyrillic + Noto Sans Arabic + Noto Sans CJK JP/KR/SC/TC",
            "size": FONT_SIZE as u32,
            "bold": 0,
            "italic": 0,
            "charset": charset,
            "unicode": 1,
            "stretchH": 100,
            "smooth": 1,
            "aa": 1,
            "padding": [PADDING, PADDING, PADDING, PADDING],
            "spacing": [0, 0],
            "outline": 0,
        },
        "common": {
            "lineHeight": line_height,
            "base": base,
            "scaleW": ATLAS_SIZE,
            "scaleH": ATLAS_SIZE,
            "pages": 1,
            "packed": 0,
            "alphaChnl": 0,
            "redChnl": 0,
            "greenChnl": 0,
            "blueChnl": 0,
        },
        "distanceField": {
            "fieldType": "msdf",
            "distanceRange": DISTANCE_RANGE,
        },
        "kernings": [],
    });
    std::fs::write(
        &args[7],
        serde_json::to_string_pretty(&atlas_json).expect("Failed to serialize atlas JSON"),
    )
    .expect("Failed to write atlas JSON");
    eprintln!("Done! {} glyphs packed.", sheets.len());
}

fn compute_sdf(mask: &[u8], width: usize, height: usize, spread: f32) -> Vec<f32> {
    let total = width * height;
    let mut outside = vec![f32::INFINITY; total];
    let mut inside = vec![f32::INFINITY; total];

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let index = y * width + x;
            let filled = mask[index] > 127;
            let edge = if filled {
                mask[index - 1] <= 127
                    || mask[index + 1] <= 127
                    || mask[index - width] <= 127
                    || mask[index + width] <= 127
            } else {
                mask[index - 1] > 127
                    || mask[index + 1] > 127
                    || mask[index - width] > 127
                    || mask[index + width] > 127
            };
            if edge {
                outside[index] = 0.0;
                inside[index] = 0.0;
            }
        }
    }

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let index = y * width + x;
            if mask[index] > 127 {
                inside[index] = inside[index]
                    .min(inside[index - 1] + 1.0)
                    .min(inside[index - width] + 1.0)
                    .min(inside[index - width - 1] + 1.414)
                    .min(inside[index - width + 1] + 1.414);
            } else {
                outside[index] = outside[index]
                    .min(outside[index - 1] + 1.0)
                    .min(outside[index - width] + 1.0)
                    .min(outside[index - width - 1] + 1.414)
                    .min(outside[index - width + 1] + 1.414);
            }
        }
    }

    for y in (1..height - 1).rev() {
        for x in (1..width - 1).rev() {
            let index = y * width + x;
            if mask[index] > 127 {
                inside[index] = inside[index]
                    .min(inside[index + 1] + 1.0)
                    .min(inside[index + width] + 1.0)
                    .min(inside[index + width - 1] + 1.414)
                    .min(inside[index + width + 1] + 1.414);
            } else {
                outside[index] = outside[index]
                    .min(outside[index + 1] + 1.0)
                    .min(outside[index + width] + 1.0)
                    .min(outside[index + width - 1] + 1.414)
                    .min(outside[index + width + 1] + 1.414);
            }
        }
    }

    let mut sdf = vec![0.0f32; total];
    for (index, value) in sdf.iter_mut().enumerate() {
        *value = if mask[index] > 127 {
            inside[index].min(spread)
        } else {
            -outside[index].min(spread)
        };
    }
    sdf
}
