//! Selection handling for the editor

/// Selection anchor and end points (line, col)
#[derive(Clone, Copy, Debug)]
pub(crate) struct Selection {
    /// The anchor point (where selection started)
    pub(crate) anchor_line: usize,
    pub(crate) anchor_col: usize,
    /// The end point (where selection currently ends)
    pub(crate) end_line: usize,
    pub(crate) end_col: usize,
}

impl Selection {
    pub(crate) fn new(line: usize, col: usize) -> Self {
        Self {
            anchor_line: line,
            anchor_col: col,
            end_line: line,
            end_col: col,
        }
    }

    pub(crate) fn update(&mut self, line: usize, col: usize) {
        self.end_line = line;
        self.end_col = col;
    }

    /// Get normalized start and end (start <= end)
    pub(crate) fn normalized(&self) -> ((usize, usize), (usize, usize)) {
        let start = (self.anchor_line, self.anchor_col);
        let end = (self.end_line, self.end_col);
        if start <= end {
            (start, end)
        } else {
            (end, start)
        }
    }

    /// Check if selection is empty (anchor == end)
    pub(crate) fn is_empty(&self) -> bool {
        self.anchor_line == self.end_line && self.anchor_col == self.end_col
    }
}
