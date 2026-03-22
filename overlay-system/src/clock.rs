//! Digital clock overlay widget.
//!
//! Renders `HH:MM:SS` using a built-in 5×7 pixel bitmap font.
//! No external font crates required.
//!
//! Call `update()` once per frame; the internal buffer is only re-rendered
//! when the second changes, keeping CPU overhead minimal.

use crate::{OverlayError, OverlayWidget, WidgetRect};
use std::time::{SystemTime, UNIX_EPOCH};

// 5×7 bitmap font for digits 0–9 and ':'
// Each glyph is 5 bytes (one per column). Bit 0 = top row, bit 6 = bottom row.

const GLYPH_W: u32 = 5;
const GLYPH_H: u32 = 7;
const GLYPH_GAP: u32 = 1;

#[rustfmt::skip]
static GLYPHS: [[u8; 5]; 11] = [
    [0x3E, 0x51, 0x49, 0x45, 0x3E], // 0
    [0x00, 0x42, 0x7F, 0x40, 0x00], // 1
    [0x42, 0x61, 0x51, 0x49, 0x46], // 2
    [0x22, 0x49, 0x49, 0x49, 0x36], // 3
    [0x18, 0x14, 0x12, 0x7F, 0x10], // 4
    [0x27, 0x45, 0x45, 0x45, 0x39], // 5
    [0x3E, 0x49, 0x49, 0x49, 0x30], // 6
    [0x01, 0x71, 0x09, 0x05, 0x03], // 7
    [0x36, 0x49, 0x49, 0x49, 0x36], // 8
    [0x06, 0x49, 0x49, 0x49, 0x3E], // 9
    [0x00, 0x24, 0x24, 0x00, 0x00], // : (index 10)
];

fn glyph_index(c: char) -> Option<usize> {
    match c {
        '0'..='9' => Some(c as usize - '0' as usize),
        ':' => Some(10),
        _ => None,
    }
}

fn render_string(text: &str, scale: u32) -> (Vec<u8>, u32, u32) {
    let glyph_count = text.chars().filter(|c| glyph_index(*c).is_some()).count();
    let Ok(n) = u32::try_from(glyph_count) else {
        return (Vec::new(), 0, 0);
    };

    if n == 0 {
        return (Vec::new(), 0, 0);
    }

    let gw = GLYPH_W * scale;
    let gh = GLYPH_H * scale;
    let gap = GLYPH_GAP * scale;
    let w = n * (gw + gap) - gap;
    let h = gh;
    let mut buf = vec![0u8; (w * h * 4) as usize];

    let mut cx = 0u32;
    for c in text.chars() {
        let Some(idx) = glyph_index(c) else { continue };
        let glyph = &GLYPHS[idx];
        for col in 0..GLYPH_W {
            let bits = glyph[col as usize];
            for row in 0..GLYPH_H {
                let lit = (bits >> row) & 1 == 1;
                if !lit {
                    continue;
                }
                // Scale the logical pixel up to `scale × scale` screen pixels.
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = cx + col * scale + sx;
                        let py = row * scale + sy;
                        let i = ((py * w + px) * 4) as usize;
                        if i + 3 < buf.len() {
                            buf[i] = 255;
                            buf[i + 1] = 255;
                            buf[i + 2] = 255;
                            buf[i + 3] = 220;
                        }
                    }
                }
            }
        }
        cx += gw + gap;
    }
    (buf, w, h)
}

/// Digital clock rendered with a built-in pixel font.
#[derive(Debug)]
pub struct ClockWidget {
    /// Integer scale factor (1 = tiny 5×7 px, 3 = comfortable desktop size).
    scale: u32,
    /// Cached pixel buffer — regenerated once per second.
    cache: Option<(Vec<u8>, u32, u32)>,
    /// Unix second at which the cache was last built.
    last_second: u64,
}

impl ClockWidget {
    /// Create a clock with `scale` (try 3 for a readable desktop widget).
    #[must_use]
    pub fn new(scale: u32) -> Self {
        Self {
            scale: scale.max(1),
            cache: None,
            last_second: 0,
        }
    }

    fn time_string() -> (String, u64) {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        let s = secs % 60;
        (format!("{h:02}:{m:02}:{s:02}"), secs)
    }
}

impl Default for ClockWidget {
    fn default() -> Self {
        Self::new(3)
    }
}

impl OverlayWidget for ClockWidget {
    fn update(&mut self) {
        let (text, secs) = Self::time_string();
        if secs != self.last_second {
            self.last_second = secs;
            self.cache = Some(render_string(&text, self.scale));
        }
    }

    fn render(&self, _width: u32, _height: u32) -> Result<Vec<u8>, OverlayError> {
        Ok(self
            .cache
            .as_ref()
            .map(|(b, _, _)| b.clone())
            .unwrap_or_default())
    }

    fn bounds(&self) -> WidgetRect {
        let (w, h) = self.cache.as_ref().map_or(
            (5 * 8 * self.scale, GLYPH_H * self.scale), // format
            |(_, w, h)| (*w, *h),
        );
        WidgetRect {
            x: 20,
            y: 20,
            width: w,
            height: h,
        }
    }

    fn name(&self) -> &'static str {
        "clock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_widget_update_populates_cache() {
        let mut w = ClockWidget::default();
        w.update();
        assert!(w.cache.is_some());
    }

    #[test]
    fn render_string_correct_buffer_size() {
        // "00:00" = 5 glyphs × (5+1)*scale - gap*scale at scale=1
        let (buf, w, h) = render_string("00:00", 1);
        assert_eq!(h, GLYPH_H);
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }
}
