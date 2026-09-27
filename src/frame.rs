//! A backend-independent picture of one screen.
//!
//! The UI code paints into a `Frame`, and a backend (currently the
//! terminal in `term.rs`) turns it into real output. Swapping in a
//! tile renderer later means writing a new backend, not touching the
//! game or the UI layout.

/// A 24-bit color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// A dark, cold version of this color, used for remembered tiles
    /// that are out of sight. It keeps the brightness but drains the
    /// warmth, so memory reads as "old information" at a glance.
    pub fn remembered(self) -> Rgb {
        let brightness = (self.0 as u32 + self.1 as u32 + self.2 as u32) / 3;
        let scale = |percent: u32| (brightness * percent / 100) as u8;
        Rgb(scale(35), scale(38), scale(50))
    }
}

/// One character cell on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Rgb,
    pub bg: Rgb,
}

pub const BLACK: Rgb = Rgb(0, 0, 0);

impl Default for Cell {
    fn default() -> Self {
        Cell {
            ch: ' ',
            fg: BLACK,
            bg: BLACK,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: u16,
    pub height: u16,
    cells: Vec<Cell>,
}

impl Frame {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            cells: vec![Cell::default(); width as usize * height as usize],
        }
    }

    pub fn get(&self, x: u16, y: u16) -> Cell {
        self.cells[y as usize * self.width as usize + x as usize]
    }

    /// Sets one cell. Writes outside the frame are silently ignored,
    /// which keeps drawing code simple when the terminal is small.
    pub fn set(&mut self, x: i32, y: i32, cell: Cell) {
        if x >= 0 && y >= 0 && x < self.width as i32 && y < self.height as i32 {
            self.cells[y as usize * self.width as usize + x as usize] = cell;
        }
    }

    /// Writes text starting at (x, y), clipped at the frame edge.
    pub fn print(&mut self, x: i32, y: i32, text: &str, fg: Rgb) {
        for (i, ch) in text.chars().enumerate() {
            self.set(x + i as i32, y, Cell { ch, fg, bg: BLACK });
        }
    }
}
