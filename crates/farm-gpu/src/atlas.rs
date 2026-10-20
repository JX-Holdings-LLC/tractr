//! Shelf packing for the glyph and shape atlases.
//!
//! Rectangles go left to right onto shelves (rows) of a fixed height; a rectangle opens a new
//! shelf (its height rounded up to 8 pixels) when no shelf of its height class has room. Nothing
//! is freed one by one: a full atlas is cleared and refilled with what the frame needs.

/// A packed rectangle, in texels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy)]
struct Shelf {
    y: u32,
    height: u32,
    x: u32,
}

#[derive(Debug)]
pub struct Packer {
    width: u32,
    height: u32,
    shelves: Vec<Shelf>,
    next_y: u32,
}

impl Packer {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height, shelves: Vec::new(), next_y: 0 }
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn clear(&mut self) {
        self.shelves.clear();
        self.next_y = 0;
    }

    /// Room for `width`×`height` texels, or `None` when the atlas is full.
    pub fn alloc(&mut self, width: u32, height: u32) -> Option<Slot> {
        if width == 0 || height == 0 || width > self.width || height > self.height {
            return None;
        }
        let class = height.div_ceil(8) * 8;
        for shelf in &mut self.shelves {
            if shelf.height == class && shelf.x + width <= self.width {
                let slot = Slot { x: shelf.x, y: shelf.y };
                shelf.x += width;
                return Some(slot);
            }
        }
        let class = class.min(self.height);
        if self.next_y + class > self.height {
            return None;
        }
        let shelf = Shelf { y: self.next_y, height: class, x: width };
        self.next_y += class;
        self.shelves.push(shelf);
        Some(Slot { x: 0, y: shelf.y })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packs_rows_and_reports_a_full_atlas() {
        let mut packer = Packer::new(64, 32);
        assert_eq!(packer.alloc(30, 10), Some(Slot { x: 0, y: 0 }));
        assert_eq!(packer.alloc(30, 12), Some(Slot { x: 30, y: 0 }), "same height class");
        assert_eq!(packer.alloc(30, 5), Some(Slot { x: 0, y: 16 }), "a new shelf");
        assert_eq!(packer.alloc(10, 9), None, "no room for another 16-pixel shelf");
        assert_eq!(packer.alloc(65, 1), None, "wider than the atlas");
        packer.clear();
        assert_eq!(packer.alloc(64, 32), Some(Slot { x: 0, y: 0 }));
    }
}
