//! Pop-up boxes drawn over the map: item lists, item details, help.

use crate::frame::{Cell, Frame, Rgb};

const BOX_BG: Rgb = Rgb(16, 16, 20);
const BOX_BORDER: Rgb = Rgb(95, 95, 110);
const BOX_TITLE: Rgb = Rgb(220, 200, 150);

/// One line of text in a box, with its own color.
pub struct Line {
    pub text: String,
    pub color: Rgb,
}

impl Line {
    pub fn new(text: impl Into<String>, color: Rgb) -> Self {
        Self {
            text: text.into(),
            color,
        }
    }
}

/// Draws a bordered box with a title and lines of text, centered in the
/// area `(0, 0)` to `(area_w, area_h)`. Text too long for the box is
/// cut off, and so are lines that don't fit.
pub fn draw_box(frame: &mut Frame, area: (i32, i32), title: &str, lines: &[Line]) {
    let (area_w, area_h) = area;
    let longest = lines
        .iter()
        .map(|l| l.text.chars().count())
        .chain([title.chars().count()])
        .max()
        .unwrap_or(0) as i32;
    let width = (longest + 4).min(area_w);
    let height = (lines.len() as i32 + 4).min(area_h);
    let x0 = (area_w - width) / 2;
    let y0 = (area_h - height) / 2;
    let inner = (width - 4).max(0) as usize;

    for y in y0..y0 + height {
        for x in x0..x0 + width {
            let top_or_bottom = y == y0 || y == y0 + height - 1;
            let side = x == x0 || x == x0 + width - 1;
            let ch = match (top_or_bottom, side) {
                (true, true) => corner(x == x0, y == y0),
                (true, false) => '─',
                (false, true) => '│',
                _ => ' ',
            };
            frame.set(
                x,
                y,
                Cell {
                    ch,
                    fg: BOX_BORDER,
                    bg: BOX_BG,
                },
            );
        }
    }

    let clip = |text: &str| text.chars().take(inner).collect::<String>();
    print_on_box(frame, x0 + 2, y0 + 1, &clip(title), BOX_TITLE);
    let rows = (height - 4).max(0) as usize;
    for (i, line) in lines.iter().take(rows).enumerate() {
        print_on_box(
            frame,
            x0 + 2,
            y0 + 3 + i as i32,
            &clip(&line.text),
            line.color,
        );
    }
}

fn corner(left: bool, top: bool) -> char {
    match (left, top) {
        (true, true) => '┌',
        (false, true) => '┐',
        (true, false) => '└',
        (false, false) => '┘',
    }
}

/// Like `Frame::print`, but keeps the box's background color.
fn print_on_box(frame: &mut Frame, x: i32, y: i32, text: &str, fg: Rgb) {
    for (i, ch) in text.chars().enumerate() {
        frame.set(x + i as i32, y, Cell { ch, fg, bg: BOX_BG });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(frame: &Frame, y: u16) -> String {
        (0..frame.width).map(|x| frame.get(x, y).ch).collect()
    }

    #[test]
    fn box_has_border_title_and_lines() {
        let mut frame = Frame::new(30, 10);
        let lines = [Line::new("a) sword", BOX_TITLE)];
        draw_box(&mut frame, (30, 10), "Pack", &lines);
        let text: Vec<String> = (0..10).map(|y| row(&frame, y)).collect();
        assert!(text.iter().any(|r| r.contains("Pack")));
        assert!(text.iter().any(|r| r.contains("a) sword")));
        assert!(text.iter().any(|r| r.contains('┌')));
    }

    #[test]
    fn long_text_is_clipped_to_the_area() {
        let mut frame = Frame::new(40, 10);
        let long = "x".repeat(100);
        draw_box(&mut frame, (20, 10), "T", &[Line::new(long, BOX_TITLE)]);
        // Nothing drawn past the area's right edge.
        for y in 0..10 {
            for x in 20..40 {
                assert_eq!(frame.get(x, y).ch, ' ');
            }
        }
    }
}
