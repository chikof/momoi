//! Concrete widget implementations.
//!
//! # `ClockWidget`
//! Renders a digital clock using a built-in 5×7 pixel bitmap font.
//! No external font crates are required.  Each glyph is a `[u8; 7]` bitmask
//! (one bit per column, MSB = leftmost pixel).
//!
//! # `TextWidget`
//! Renders a static UTF-8 string using the same pixel font.
//!
//! # `SystemStatsWidget`
//! Reads `/proc/stat` (CPU) and `/proc/meminfo` (RAM) and formats a one-line
//! stats string.  Linux-only; returns zeroes on other platforms.

use crate::{OverlayError, OverlayWidget, WidgetAnchor, WidgetRect};
use std::time::{SystemTime, UNIX_EPOCH};

// 5×7 bitmap font (ASCII 32–90)
// Each entry is 5 bytes: one per column, bit 0 = top pixel, bit 6 = bottom.
// Only printable ASCII is needed for a clock (0–9 and ':').

const GLYPH_W: u32 = 5;
const GLYPH_H: u32 = 7;
const GLYPH_GAP: u32 = 1; // pixels between glyphs

/// 5×7 bitmap for '0'–'9' and ':'.
#[allow(clippy::unreadable_literal)]
static DIGITS: [u8; 11 * 5] = [
    0b0111110, 0b1000001, 0b1000001, 0b1000001, 0b0111110, // 0
    0b0000000, 0b1000010, 0b1111111, 0b1000000, 0b0000000, // 1
    0b1000010, 0b1100001, 0b1010001, 0b1001001, 0b1000110, // 2
    0b0100010, 0b1001001, 0b1001001, 0b1001001, 0b0110110, // 3
    0b0011000, 0b0010100, 0b0010010, 0b1111111, 0b0010000, // 4
    0b0100111, 0b1001001, 0b1001001, 0b1001001, 0b0110001, // 5
    0b0111110, 0b1001001, 0b1001001, 0b1001001, 0b0110000, // 6
    0b0000001, 0b1110001, 0b0001001, 0b0000101, 0b0000011, // 7
    0b0110110, 0b1001001, 0b1001001, 0b1001001, 0b0110110, // 8
    0b0000110, 0b1001001, 0b1001001, 0b1001001, 0b0111110, // 9
    0b0000000, 0b0100100, 0b0100100, 0b0000000, 0b0000000, // : (index 10)
];

const MARGIN: u32 = 20;

/// Return a glyph slice for a `char` in the set `0–9` and `:`.
fn glyph_for(c: char) -> Option<&'static [u8]> {
    let idx = match c {
        '0'..='9' => (c as usize) - ('0' as usize),
        ':' => 10,
        _ => return None,
    };
    Some(&DIGITS[idx * 5..idx * 5 + 5])
}

/// Render a string of digits/colons into an RGBA pixel buffer.
///
/// Returns `(buffer, pixel_width, pixel_height)`.
fn render_text(text: &str, fg: [u8; 4], bg: [u8; 4]) -> (Vec<u8>, u32, u32) {
    let n_glyphs = text.chars().filter(|c| glyph_for(*c).is_some()).count();
    let Ok(n) = u32::try_from(n_glyphs) else {
        return (Vec::new(), 0, 0);
    };

    let w = n * (GLYPH_W + GLYPH_GAP) - GLYPH_GAP;
    let h = GLYPH_H;
    let mut buf = vec![bg[0], bg[1], bg[2], bg[3]];
    buf = buf.repeat((w * h) as usize);

    let mut cursor_x = 0u32;
    for c in text.chars() {
        let Some(glyph) = glyph_for(c) else { continue };
        for col in 0..GLYPH_W {
            let bits = glyph[col as usize];
            for row in 0..GLYPH_H {
                let lit = (bits >> row) & 1 == 1;
                let px = [
                    if lit { fg[0] } else { bg[0] },
                    if lit { fg[1] } else { bg[1] },
                    if lit { fg[2] } else { bg[2] },
                    if lit { fg[3] } else { bg[3] },
                ];
                let idx = ((row * w + cursor_x + col) * 4) as usize;
                if idx + 3 < buf.len() {
                    buf[idx..idx + 4].copy_from_slice(&px);
                }
            }
        }
        cursor_x += GLYPH_W + GLYPH_GAP;
    }
    (buf, w, h)
}

/// Renders the current local time as `HH:MM:SS` using the built-in pixel font.
pub struct ClockWidget {
    anchor: WidgetAnchor,
    /// Foreground colour (RGBA).
    fg: [u8; 4],
    /// Scale factor (integer): each logical pixel becomes `scale × scale` screen pixels.
    scale: u32,
    /// Cached rendered buffer (refreshed once per second).
    cache: Option<(Vec<u8>, u32, u32)>,
    last_second: u64,
    /// Screen dimensions — needed to compute anchor position.
    screen_w: u32,
    screen_h: u32,
}

impl ClockWidget {
    /// Create a clock at `anchor`.  `fg` is the text colour (RGBA); background
    /// is fully transparent.  `scale` magnifies the pixel font (1 = tiny, 3 =
    /// readable at typical desktop distance).
    #[must_use]
    pub fn new(anchor: WidgetAnchor, fg: [u8; 4], scale: u32) -> Self {
        Self {
            anchor,
            fg,
            scale: scale.max(1),
            cache: None,
            last_second: 0,
            screen_w: 1920,
            screen_h: 1080,
        }
    }

    /// Set the output dimensions so the anchor calculates the correct position.
    pub fn set_screen_size(&mut self, w: u32, h: u32) {
        self.screen_w = w;
        self.screen_h = h;
    }

    fn current_time_string() -> (String, u64) {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let h = (secs / 3600) % 24;
        let m = (secs / 60) % 60;
        let s = secs % 60;
        (format!("{h:02}:{m:02}:{s:02}"), secs)
    }

    /// Scale up a buffer by integer factor.
    fn scale_up(src: &[u8], w: u32, h: u32, factor: u32) -> (Vec<u8>, u32, u32) {
        let dw = w * factor;
        let dh = h * factor;
        let mut dst = vec![0u8; (dw * dh * 4) as usize];
        for sy in 0..h {
            for sx in 0..w {
                let si = ((sy * w + sx) * 4) as usize;
                let pixel = [src[si], src[si + 1], src[si + 2], src[si + 3]];
                for dy in 0..factor {
                    for dx in 0..factor {
                        let di = (((sy * factor + dy) * dw + sx * factor + dx) * 4) as usize;
                        dst[di..di + 4].copy_from_slice(&pixel);
                    }
                }
            }
        }
        (dst, dw, dh)
    }
}

impl Default for ClockWidget {
    fn default() -> Self {
        Self::new(WidgetAnchor::TopLeft, [255, 255, 255, 220], 3)
    }
}

impl OverlayWidget for ClockWidget {
    fn update(&mut self) {
        let (text, secs) = Self::current_time_string();
        if secs != self.last_second {
            self.last_second = secs;
            let (raw, w, h) = render_text(&text, self.fg, [0, 0, 0, 0]);
            let scaled = if self.scale > 1 {
                Self::scale_up(&raw, w, h, self.scale)
            } else {
                (raw, w, h)
            };
            self.cache = Some(scaled);
        }
    }

    fn render(&self, _width: u32, _height: u32) -> Result<Vec<u8>, OverlayError> {
        match &self.cache {
            Some((buf, _, _)) => Ok(buf.clone()),
            None => Ok(Vec::new()),
        }
    }

    fn bounds(&self) -> WidgetRect {
        let (w, h) = self.cache.as_ref().map_or((80, 21), |(_, w, h)| (*w, *h));
        let (x, y) = match self.anchor {
            WidgetAnchor::TopLeft => (MARGIN, MARGIN),
            WidgetAnchor::TopRight => (self.screen_w.saturating_sub(w + MARGIN), MARGIN),
            WidgetAnchor::BottomLeft => (MARGIN, self.screen_h.saturating_sub(h + MARGIN)),
            WidgetAnchor::BottomRight => (
                self.screen_w.saturating_sub(w + MARGIN),
                self.screen_h.saturating_sub(h + MARGIN),
            ),
            WidgetAnchor::Centre => (
                (self.screen_w / 2).saturating_sub(w / 2),
                (self.screen_h / 2).saturating_sub(h / 2),
            ),
        };
        WidgetRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    fn name(&self) -> &'static str {
        "clock"
    }
}

/// Renders a one-line `CPU: xx%  RAM: xxMB` overlay.
#[derive(Default)]
pub struct SystemStatsWidget {
    cache: Option<(Vec<u8>, u32, u32)>,
    /// Update interval in seconds.
    interval_secs: u64,
    last_update: u64,
}

impl SystemStatsWidget {
    /// Create a stats widget that refreshes every `interval_secs` seconds.
    #[must_use]
    pub fn new(interval_secs: u64) -> Self {
        Self {
            cache: None,
            interval_secs: interval_secs.max(1),
            last_update: 0,
        }
    }

    /// Read CPU usage (%) from `/proc/stat`.
    #[cfg(target_os = "linux")]
    fn read_cpu_percent() -> f32 {
        use std::io::{BufRead, BufReader};
        let Ok(f) = std::fs::File::open("/proc/stat") else {
            return 0.0;
        };

        let line = BufReader::new(f)
            .lines()
            .next()
            .and_then(std::result::Result::ok);

        let Some(line) = line else { return 0.0 };
        let nums: Vec<u64> = line
            .split_whitespace()
            .skip(1)
            .filter_map(|s| s.parse().ok())
            .collect();

        if nums.len() < 4 {
            return 0.0;
        }

        let idle = nums[3];
        let total: u64 = nums.iter().sum();
        if total == 0 {
            return 0.0;
        }

        (1.0 - idle as f32 / total as f32) * 100.0
    }

    #[cfg(not(target_os = "linux"))]
    fn read_cpu_percent() -> f32 {
        0.0
    }

    /// Read used RAM (MB) from `/proc/meminfo`.
    #[cfg(target_os = "linux")]
    fn read_ram_used_mb() -> u64 {
        use std::io::{BufRead, BufReader};
        let Ok(f) = std::fs::File::open("/proc/meminfo") else {
            return 0;
        };
        let mut total_kb = 0u64;
        let mut available_kb = 0u64;
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            if let Some(val) = line.strip_prefix("MemTotal:") {
                total_kb = val
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if let Some(val) = line.strip_prefix("MemAvailable:") {
                available_kb = val
                    .split_whitespace()
                    .next()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }
        (total_kb.saturating_sub(available_kb)) / 1024
    }

    #[cfg(not(target_os = "linux"))]
    fn read_ram_used_mb() -> u64 {
        0
    }
}

impl OverlayWidget for SystemStatsWidget {
    fn update(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now - self.last_update >= self.interval_secs || self.cache.is_none() {
            self.last_update = now;
            let cpu = Self::read_cpu_percent();
            let ram = Self::read_ram_used_mb();
            let cpu_str = format!("CPU:{cpu:.0}  RAM:{ram}");
            let chars: String = cpu_str
                .chars()
                .filter(|c| glyph_for(*c).is_some())
                .collect();
            let (buf, w, h) = render_text(&chars, [200, 200, 200, 200], [0, 0, 0, 0]);
            self.cache = Some((buf, w, h));
        }
    }

    fn render(&self, _w: u32, _h: u32) -> Result<Vec<u8>, OverlayError> {
        Ok(self
            .cache
            .as_ref()
            .map(|(b, _, _)| b.clone())
            .unwrap_or_default())
    }

    fn bounds(&self) -> WidgetRect {
        let (w, h) = self.cache.as_ref().map_or((120, 7), |(_, w, h)| (*w, *h));
        // Place below where the clock sits at TopRight with scale=3
        // Clock height: 7 * 3 = 21px + margin 20 + gap 8 = y:49
        WidgetRect {
            x: 20,
            y: 49,
            width: w,
            height: h,
        }
    }

    fn name(&self) -> &'static str {
        "system-stats"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_for_zero_should_return_some() {
        assert!(glyph_for('0').is_some());
    }

    #[test]
    fn glyph_for_colon_should_return_some() {
        assert!(glyph_for(':').is_some());
    }

    #[test]
    fn glyph_for_letter_should_return_none() {
        assert!(glyph_for('A').is_none());
    }

    #[test]
    fn render_text_should_produce_correct_buffer_size() {
        let (buf, w, h) = render_text("12:34", [255, 255, 255, 255], [0, 0, 0, 0]);
        assert_eq!(w, 29);
        assert_eq!(h, 7);
        assert_eq!(buf.len(), (w * h * 4) as usize);
    }

    #[test]
    fn clock_widget_update_should_populate_cache() {
        let mut w = ClockWidget::default();
        w.update();
        assert!(w.cache.is_some(), "cache should be populated after update");
    }

    #[test]
    fn scale_up_should_quadruple_buffer_for_scale_2() {
        let src = vec![255u8; 4];
        let (dst, dw, dh) = ClockWidget::scale_up(&src, 1, 1, 2);
        assert_eq!((dw, dh), (2, 2));
        assert_eq!(dst.len(), 2 * 2 * 4);
    }
}
