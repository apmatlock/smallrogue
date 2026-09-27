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
    /// Scales brightness, e.g. `0.5` for half as bright. Used later for
    /// remembered-but-not-visible tiles.
    #[allow(dead_code)]
    pub fn dim(self, factor: f32) -> Rgb {
        let f = |c: u8| (c as f32 * factor).round().clamp(0.0, 255.0) as u8;
        Rgb(f(self.0), f(self.1), f(self.2))
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
        Cell { ch: ' ', fg: BLACK, bg: BLACK }
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
