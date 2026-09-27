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

const PAGE_HINT: Rgb = Rgb(120, 120, 130);

/// Draws a bordered box with a title and lines of text, centered in the
/// area `(0, 0)` to `(area_w, area_h)`. Text too long for the box is
/// cut off. When the lines don't all fit, they are split into pages and
/// page number `page` (counting from 0) is shown, with a hint on the
/// last row. Returns how many pages there are.
pub fn draw_box(
    frame: &mut Frame,
    area: (i32, i32),
    title: &str,
    lines: &[Line],
    page: usize,
) -> usize {
    let (area_w, area_h) = area;
    let hint = "space: more";
    let longest = lines
        .iter()
        .map(|l| l.text.chars().count())
        .chain([title.chars().count(), hint.len() + 6])
        .max()
        .unwrap_or(0) as i32;
    let width = (longest + 4).min(area_w);
    let height = (lines.len() as i32 + 4).min(area_h);
    let x0 = (area_w - width) / 2;
    let y0 = (area_h - height) / 2;
    let inner = (width - 4).max(0) as usize;

    // Rows for text: everything inside the border except the title and
    // the gap under it. If paging, the last one holds the page hint.
    let rows = (height - 4).max(0) as usize;
    let paged = lines.len() > rows;
    let per_page = if paged {
        rows.saturating_sub(1).max(1)
    } else {
        rows.max(1)
    };
    let pages = lines.len().div_ceil(per_page).max(1);
    let page = page.min(pages - 1);

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
    let shown = lines.iter().skip(page * per_page).take(per_page);
    for (i, line) in shown.enumerate() {
        let y = y0 + 3 + i as i32;
        print_on_box(frame, x0 + 2, y, &clip(&line.text), line.color);
    }
    if paged && rows > 1 {
        let text = format!("{}/{pages}  {hint}", page + 1);
        print_on_box(
            frame,
            x0 + 2,
            y0 + 3 + per_page as i32,
            &clip(&text),
            PAGE_HINT,
        );
    }
    pages
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
        draw_box(&mut frame, (30, 10), "Pack", &lines, 0);
        let text: Vec<String> = (0..10).map(|y| row(&frame, y)).collect();
        assert!(text.iter().any(|r| r.contains("Pack")));
        assert!(text.iter().any(|r| r.contains("a) sword")));
        assert!(text.iter().any(|r| r.contains('┌')));
    }

    #[test]
    fn long_lists_are_split_into_pages() {
        // 10 rows of area leave 6 text rows: 5 lines per page + a hint.
        let lines: Vec<Line> = (0..12)
            .map(|i| Line::new(format!("line {i}"), BOX_TITLE))
            .collect();
        let mut frame = Frame::new(30, 10);
        assert_eq!(draw_box(&mut frame, (30, 10), "T", &lines, 0), 3);
        let text: String = (0..10).map(|y| row(&frame, y)).collect();
        assert!(text.contains("line 4") && !text.contains("line 5"));
        assert!(text.contains("1/3"));

        let mut frame = Frame::new(30, 10);
        draw_box(&mut frame, (30, 10), "T", &lines, 2);
        let text: String = (0..10).map(|y| row(&frame, y)).collect();
        assert!(text.contains("line 11") && text.contains("3/3"));
    }

    #[test]
    fn long_text_is_clipped_to_the_area() {
        let mut frame = Frame::new(40, 10);
        let long = "x".repeat(100);
        draw_box(&mut frame, (20, 10), "T", &[Line::new(long, BOX_TITLE)], 0);
        // Nothing drawn past the area's right edge.
        for y in 0..10 {
            for x in 20..40 {
                assert_eq!(frame.get(x, y).ch, ' ');
            }
        }
    }
}
