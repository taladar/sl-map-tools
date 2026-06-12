//! Pre-render occupancy analysis: which of nine fixed anchor positions on a map
//! are free of overlay content (routes, GLW shapes/labels) so additional,
//! position-flexible elements (a legend, logo or label) can be placed there.
//!
//! The occupancy is computed by drawing the overlays onto a transparent
//! [`crate::map_tiles::Map::blank`] map and treating any pixel with a non-zero
//! alpha as occupied (see [`OccupancyGrid::from_map`]); no base map tiles are
//! fetched, so this can run before the final render and be offered to a user
//! choosing where to place those extra elements.
//!
//! The image is reduced to a coarse boolean [`OccupancyGrid`] and each of the
//! nine [`PlacementSlot`]s is evaluated for the largest empty rectangle that can
//! be anchored within its own third of the map
//! ([`OccupancyGrid::evaluate_slots`]). The nine thirds tile the image exactly
//! (the centre third on each axis absorbs the division remainder so the two edge
//! thirds stay equal), so the per-slot rectangles never overlap and can be
//! assigned to independent elements. Adjacent anchors that share one contiguous
//! free area are reported in [`PlacementSlotInfo::connected_neighbours`], and a
//! `span_fill` element may grow across them ([`OccupancyGrid::spanned_region`])
//! into one larger rectangle that takes the minimum extent of the slots it
//! crosses on the perpendicular axis.

use crate::map_tiles::MapLike;

/// default number of grid cells along the longer image dimension used by
/// [`OccupancyGrid::from_map`]
pub const DEFAULT_COVERAGE_GRID: u32 = 64;

/// how a slot is anchored along one axis of the image
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AxisMode {
    /// against the low edge (left or top): grows toward the high edge
    Start,
    /// centred on the axis: grows symmetrically toward both edges
    Center,
    /// against the high edge (right or bottom): grows toward the low edge
    End,
}

/// horizontal alignment of content within an available span
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    /// against the left edge of the span
    Left,
    /// centred within the span
    Center,
    /// against the right edge of the span
    Right,
}

/// vertical alignment of content within an available span
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    /// against the top edge of the span
    Top,
    /// centred within the span
    Center,
    /// against the bottom edge of the span
    Bottom,
}

impl HAlign {
    /// pixel offset of `content`-wide content within an `available`-wide span for
    /// this alignment, clamped so the content never starts past the span end
    #[must_use]
    pub const fn offset(self, content: u32, available: u32) -> u32 {
        let slack = available.saturating_sub(content);
        match self {
            Self::Left => 0,
            Self::Center => slack / 2,
            Self::Right => slack,
        }
    }
}

impl VAlign {
    /// pixel offset of `content`-tall content within an `available`-tall span for
    /// this alignment, clamped so the content never starts past the span end
    #[must_use]
    pub const fn offset(self, content: u32, available: u32) -> u32 {
        let slack = available.saturating_sub(content);
        match self {
            Self::Top => 0,
            Self::Center => slack / 2,
            Self::Bottom => slack,
        }
    }
}

/// pixel origin of `content`-sized content within a `total`-sized image axis for
/// the given anchoring mode: `Start` hugs the low edge by `margin`, `End` hugs
/// the high edge by `margin`, `Center` centres; all clamped to stay inside
const fn axis_origin(mode: AxisMode, content: u32, total: u32, margin: u32) -> u32 {
    let slack = total.saturating_sub(content);
    match mode {
        // hug the low edge, but never start so far in that the content overflows
        AxisMode::Start => {
            if margin < slack {
                margin
            } else {
                slack
            }
        }
        AxisMode::Center => slack / 2,
        // hug the high edge, leaving `margin` if there is room
        AxisMode::End => slack.saturating_sub(margin),
    }
}

/// the widest contiguous run of `true` (free) entries in `col_free`, positioned
/// per the anchoring mode: `Start` hugs index `0`, `End` hugs the last index,
/// `Center` centres the run within the slice. Returned as `(offset, width)` into
/// the slice (`width == 0` when nothing is free in the required position).
fn run_for_mode(col_free: &[bool], mode: AxisMode) -> (u32, u32) {
    let n = u32::try_from(col_free.len()).unwrap_or(u32::MAX);
    match mode {
        AxisMode::Start => {
            let mut w = 0;
            while w < n && col_free.get(w as usize).copied().unwrap_or(false) {
                w += 1;
            }
            (0, w)
        }
        AxisMode::End => {
            let mut w = 0;
            while w < n && col_free.get((n - 1 - w) as usize).copied().unwrap_or(false) {
                w += 1;
            }
            (n - w, w)
        }
        AxisMode::Center => {
            for w in (1..=n).rev() {
                let c0 = (n - w) / 2;
                if (c0..c0 + w).all(|c| col_free.get(c as usize).copied().unwrap_or(false)) {
                    return (c0, w);
                }
            }
            (n / 2, 0)
        }
    }
}

/// one of the nine fixed candidate anchor positions on a map, laid out as a
/// conceptual 3x3 grid (the four corners, the four side midpoints and the
/// centre)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlacementSlot {
    /// top-left corner
    TopLeft,
    /// middle of the top edge
    TopCenter,
    /// top-right corner
    TopRight,
    /// middle of the left edge
    MiddleLeft,
    /// centre of the map
    Center,
    /// middle of the right edge
    MiddleRight,
    /// bottom-left corner
    BottomLeft,
    /// middle of the bottom edge
    BottomCenter,
    /// bottom-right corner
    BottomRight,
}

impl PlacementSlot {
    /// all nine anchors, in reading order (top row left-to-right, then middle,
    /// then bottom)
    pub const ALL: [Self; 9] = [
        Self::TopLeft,
        Self::TopCenter,
        Self::TopRight,
        Self::MiddleLeft,
        Self::Center,
        Self::MiddleRight,
        Self::BottomLeft,
        Self::BottomCenter,
        Self::BottomRight,
    ];

    /// the stable snake_case name for this slot (`top_left` … `bottom_right`).
    /// This is the inverse of the [`FromStr`](std::str::FromStr) impl and the
    /// value used in JSON; [`Display`](std::fmt::Display) yields the same text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::MiddleLeft => "middle_left",
            Self::Center => "center",
            Self::MiddleRight => "middle_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
        }
    }

    /// the position of this anchor in the conceptual 3x3 layout as
    /// `(horizontal_index, vertical_index)`, each in `0..3` with `0` against the
    /// low edge
    #[must_use]
    const fn cell_3x3(self) -> (u8, u8) {
        match self {
            Self::TopLeft => (0, 0),
            Self::TopCenter => (1, 0),
            Self::TopRight => (2, 0),
            Self::MiddleLeft => (0, 1),
            Self::Center => (1, 1),
            Self::MiddleRight => (2, 1),
            Self::BottomLeft => (0, 2),
            Self::BottomCenter => (1, 2),
            Self::BottomRight => (2, 2),
        }
    }

    /// how this anchor is positioned along the horizontal axis
    #[must_use]
    const fn horizontal(self) -> AxisMode {
        match self.cell_3x3().0 {
            0 => AxisMode::Start,
            1 => AxisMode::Center,
            _ => AxisMode::End,
        }
    }

    /// how this anchor is positioned along the vertical axis
    #[must_use]
    const fn vertical(self) -> AxisMode {
        match self.cell_3x3().1 {
            0 => AxisMode::Start,
            1 => AxisMode::Center,
            _ => AxisMode::End,
        }
    }

    /// the natural alignment for content placed at this anchor: the slot's own
    /// 3x3 position, biased to the outside of the image (e.g. `TopLeft` →
    /// left/top, `Center` → center/center, `BottomRight` → right/bottom). Used
    /// as the default alignment for text labels, overridable per axis.
    #[must_use]
    pub const fn default_alignment(self) -> (HAlign, VAlign) {
        let (h, v) = self.cell_3x3();
        let ha = match h {
            0 => HAlign::Left,
            1 => HAlign::Center,
            _ => HAlign::Right,
        };
        let va = match v {
            0 => VAlign::Top,
            1 => VAlign::Center,
            _ => VAlign::Bottom,
        };
        (ha, va)
    }

    /// pixel origin (top-left) at which to place a `content_w` × `content_h` box
    /// anchored at this slot within a `img_w` × `img_h` image: corners hug their
    /// edges leaving `margin` px, edge-midpoints and the centre centre the box on
    /// the relevant axis. All clamped to keep the box inside the image. This is
    /// the single source of truth for positioning a fixed-size element (e.g. the
    /// legend) at a slot.
    #[must_use]
    pub const fn anchored_origin(
        self,
        content_w: u32,
        content_h: u32,
        img_w: u32,
        img_h: u32,
        margin: u32,
    ) -> (u32, u32) {
        let x = axis_origin(self.horizontal(), content_w, img_w, margin);
        let y = axis_origin(self.vertical(), content_h, img_h, margin);
        (x, y)
    }

    /// the anchors orthogonally adjacent to this one in the 3x3 layout (the
    /// up/down/left/right neighbours, never the diagonals)
    #[must_use]
    pub fn neighbours(self) -> Vec<Self> {
        let (h, v) = self.cell_3x3();
        Self::ALL
            .into_iter()
            .filter(|other| {
                let (oh, ov) = other.cell_3x3();
                let dh = (i16::from(h) - i16::from(oh)).abs();
                let dv = (i16::from(v) - i16::from(ov)).abs();
                dh + dv == 1
            })
            .collect()
    }
}

impl std::fmt::Display for PlacementSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned by [`PlacementSlot`]'s [`FromStr`](std::str::FromStr) impl
/// when the string is not one of the nine snake_case slot names.
#[derive(Debug, Clone, thiserror::Error)]
#[error("`{0}` is not a valid placement slot name")]
pub struct ParsePlacementSlotError(String);

impl std::str::FromStr for PlacementSlot {
    type Err = ParsePlacementSlotError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "top_left" => Self::TopLeft,
            "top_center" => Self::TopCenter,
            "top_right" => Self::TopRight,
            "middle_left" => Self::MiddleLeft,
            "center" => Self::Center,
            "middle_right" => Self::MiddleRight,
            "bottom_left" => Self::BottomLeft,
            "bottom_center" => Self::BottomCenter,
            "bottom_right" => Self::BottomRight,
            other => return Err(ParsePlacementSlotError(other.to_owned())),
        })
    }
}

/// an axis-aligned rectangle in image pixel coordinates (origin top-left)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    /// x coordinate of the left edge in pixels
    pub x: u32,
    /// y coordinate of the top edge in pixels
    pub y: u32,
    /// width in pixels
    pub width: u32,
    /// height in pixels
    pub height: u32,
}

/// the result of evaluating one [`PlacementSlot`] against an [`OccupancyGrid`]
#[derive(Debug, Clone, PartialEq)]
pub struct PlacementSlotInfo {
    /// which slot this describes
    pub slot: PlacementSlot,
    /// whether there is any free space at this anchor at all (i.e. the anchor
    /// itself is not covered by overlay content)
    pub available: bool,
    /// the largest empty rectangle that can be placed anchored at this slot,
    /// confined to the slot's own third of the map so the nine slot rectangles
    /// never overlap, in pixel coordinates, or `None` if the slot's third has no
    /// free space at the anchor
    pub free_rect: Option<PixelRect>,
    /// convenience `(width, height)` of [`Self::free_rect`] in pixels, `(0, 0)`
    /// when there is no free rectangle
    pub free_size: (u32, u32),
    /// fraction (`0.0..=1.0`) of the local third-of-the-map region around this
    /// anchor that is covered by overlay content, as a density hint
    pub occupied_fraction: f32,
    /// the orthogonally adjacent anchors that share one contiguous free area
    /// with this anchor, so they could be combined for a larger element
    pub connected_neighbours: Vec<PlacementSlot>,
}

/// a coarse boolean occupancy grid downsampled from an overlay coverage mask
///
/// Cells are stored row-major with row `0` at the top of the image (pixel
/// `y == 0`). A cell is occupied if *any* pixel inside it is covered, so the
/// grid never reports covered space as free.
#[derive(Debug, Clone)]
pub struct OccupancyGrid {
    /// number of cell columns
    cols: u32,
    /// number of cell rows
    rows: u32,
    /// side length of a (square) cell in pixels
    cell_size: u32,
    /// image width in pixels
    img_width: u32,
    /// image height in pixels
    img_height: u32,
    /// row-major occupancy, length `cols * rows`
    occupied: Vec<bool>,
}

impl OccupancyGrid {
    /// derive the cell grid dimensions for an image of the given size, aiming
    /// for roughly `grid_resolution` cells along the longer dimension with
    /// square cells
    fn grid_params(img_width: u32, img_height: u32, grid_resolution: u32) -> (u32, u32, u32) {
        if img_width == 0 || img_height == 0 {
            return (0, 0, 1);
        }
        let resolution = grid_resolution.max(1);
        let longer = img_width.max(img_height);
        let cell_size = longer.div_ceil(resolution).max(1);
        let cols = img_width.div_ceil(cell_size).max(1);
        let rows = img_height.div_ceil(cell_size).max(1);
        (cols, rows, cell_size)
    }

    /// build an occupancy grid from a map by treating any pixel with a non-zero
    /// alpha channel as covered
    ///
    /// Intended to be used with a [`crate::map_tiles::Map::blank`] map onto
    /// which the overlay content (route, GLW shapes and labels) has been drawn,
    /// so that exactly the overlay pixels are occupied and the (absent) base map
    /// is not.
    #[must_use]
    pub fn from_map<M: MapLike + ?Sized>(map: &M, grid_resolution: u32) -> Self {
        let rgba = map.image().to_rgba8();
        let (img_width, img_height) = rgba.dimensions();
        let (cols, rows, cell_size) = Self::grid_params(img_width, img_height, grid_resolution);
        let mut occupied = vec![false; (cols * rows) as usize];
        if cols > 0 && rows > 0 {
            for (x, y, pixel) in rgba.enumerate_pixels() {
                if pixel[3] != 0 {
                    let col = (x / cell_size).min(cols - 1);
                    let row = (y / cell_size).min(rows - 1);
                    if let Some(cell) = occupied.get_mut((row * cols + col) as usize) {
                        *cell = true;
                    }
                }
            }
        }
        Self {
            cols,
            rows,
            cell_size,
            img_width,
            img_height,
            occupied,
        }
    }

    /// whether the cell at `(row, col)` is free (out-of-bounds cells count as
    /// occupied)
    fn is_free(&self, row: u32, col: u32) -> bool {
        !self
            .occupied
            .get((row * self.cols + col) as usize)
            .copied()
            .unwrap_or(true)
    }

    /// convert an exclusive cell rectangle `[r0, r1) x [c0, c1)` to a pixel
    /// rectangle, clamped to the image bounds
    fn cell_rect_to_pixels(&self, r0: u32, r1: u32, c0: u32, c1: u32) -> PixelRect {
        let x = c0 * self.cell_size;
        let y = r0 * self.cell_size;
        let width = (c1 * self.cell_size).min(self.img_width).saturating_sub(x);
        let height = (r1 * self.cell_size).min(self.img_height).saturating_sub(y);
        PixelRect {
            x,
            y,
            width,
            height,
        }
    }

    /// the half-open range `[lo, hi)` of grid columns covered by the third-of-
    /// the-grid at horizontal index `h3` (`0..3`). The two edge thirds each get
    /// `cols / 3` columns and the centre third absorbs the division remainder,
    /// so the left and right thirds are always equal in size. The three ranges
    /// tile `0..cols` exactly, sharing boundaries with no gap or overlap.
    const fn col_band(&self, h3: u32) -> (u32, u32) {
        let edge = self.cols / 3;
        match h3 {
            0 => (0, edge),
            1 => (edge, self.cols - edge),
            _ => (self.cols - edge, self.cols),
        }
    }

    /// the half-open range `[lo, hi)` of grid rows covered by the third-of-the-
    /// grid at vertical index `v3` (`0..3`); the centre third absorbs the
    /// remainder so the top and bottom thirds stay equal. See [`Self::col_band`].
    const fn row_band(&self, v3: u32) -> (u32, u32) {
        let edge = self.rows / 3;
        match v3 {
            0 => (0, edge),
            1 => (edge, self.rows - edge),
            _ => (self.rows - edge, self.rows),
        }
    }

    /// the largest all-free cell rectangle anchored per the given axis modes,
    /// confined to the column range `[c_lo, c_hi)` and row range `[r_lo, r_hi)`,
    /// returned as an exclusive cell rectangle `(r0, r1, c0, c1)`. Confining the
    /// search to a slot's own third (see [`Self::col_band`] / [`Self::row_band`])
    /// keeps the nine per-slot rectangles from overlapping one another.
    fn largest_free_rect_in(
        &self,
        c_lo: u32,
        c_hi: u32,
        r_lo: u32,
        r_hi: u32,
        hmode: AxisMode,
        vmode: AxisMode,
    ) -> Option<(u32, u32, u32, u32)> {
        if c_lo >= c_hi || r_lo >= r_hi {
            return None;
        }
        let band_rows = r_hi - r_lo;
        let mut best: Option<(u32, u32, u32, u32, u32)> = None;
        for h in 1..=band_rows {
            let (r0, r1) = match vmode {
                AxisMode::Start => (r_lo, r_lo + h),
                AxisMode::End => (r_hi - h, r_hi),
                AxisMode::Center => {
                    let r0 = r_lo + (band_rows - h) / 2;
                    (r0, r0 + h)
                }
            };
            let col_free: Vec<bool> = (c_lo..c_hi)
                .map(|c| (r0..r1).all(|r| self.is_free(r, c)))
                .collect();
            let (off, w) = run_for_mode(&col_free, hmode);
            if w == 0 {
                continue;
            }
            let area = w * h;
            if best.is_none_or(|b| area > b.0) {
                best = Some((area, r0, r1, c_lo + off, c_lo + off + w));
            }
        }
        best.map(|(_, r0, r1, c0, c1)| (r0, r1, c0, c1))
    }

    /// the largest all-free cell rectangle anchored per the given axis modes,
    /// searched over the whole grid (the basis for a `span_fill` placement, which
    /// may grow across slot boundaries), returned as an exclusive cell rectangle
    /// `(r0, r1, c0, c1)`. Because it is a single contiguous rectangle it spans
    /// neighbouring thirds only where their free space touches and is limited to
    /// the minimum extent of those thirds on the perpendicular axis.
    fn largest_free_rect(&self, hmode: AxisMode, vmode: AxisMode) -> Option<(u32, u32, u32, u32)> {
        self.largest_free_rect_in(0, self.cols, 0, self.rows, hmode, vmode)
    }

    /// the cell `(row, col)` that the given anchor sits in
    fn anchor_cell(&self, anchor: PlacementSlot) -> (u32, u32) {
        let col = match anchor.horizontal() {
            AxisMode::Start => 0,
            AxisMode::Center => self.img_width / 2 / self.cell_size,
            AxisMode::End => self.img_width.saturating_sub(1) / self.cell_size,
        }
        .min(self.cols.saturating_sub(1));
        let row = match anchor.vertical() {
            AxisMode::Start => 0,
            AxisMode::Center => self.img_height / 2 / self.cell_size,
            AxisMode::End => self.img_height.saturating_sub(1) / self.cell_size,
        }
        .min(self.rows.saturating_sub(1));
        (row, col)
    }

    /// the fraction of the local third-of-the-map block around the anchor that
    /// is occupied
    fn local_occupied_fraction(&self, anchor: PlacementSlot) -> f32 {
        if self.cols == 0 || self.rows == 0 {
            return 0f32;
        }
        let (hi, vi) = anchor.cell_3x3();
        let (c0, c1) = self.col_band(u32::from(hi));
        let (r0, r1) = self.row_band(u32::from(vi));
        let mut total = 0u32;
        let mut occ = 0u32;
        for r in r0..r1 {
            for c in c0..c1 {
                total += 1;
                if !self.is_free(r, c) {
                    occ += 1;
                }
            }
        }
        if total == 0 {
            0f32
        } else {
            f32::from(u16::try_from(occ).unwrap_or(u16::MAX))
                / f32::from(u16::try_from(total).unwrap_or(u16::MAX))
        }
    }

    /// label every free cell with a connected-component id (4-connectivity),
    /// occupied cells get `-1`
    fn free_components(&self) -> Vec<i32> {
        let mut comp = vec![-1i32; (self.cols * self.rows) as usize];
        let mut next = 0i32;
        for start_row in 0..self.rows {
            for start_col in 0..self.cols {
                if !self.is_free(start_row, start_col) {
                    continue;
                }
                let start_idx = (start_row * self.cols + start_col) as usize;
                if comp.get(start_idx).copied().unwrap_or(0) != -1 {
                    continue;
                }
                if let Some(cell) = comp.get_mut(start_idx) {
                    *cell = next;
                }
                let mut stack = vec![(start_row, start_col)];
                while let Some((r, c)) = stack.pop() {
                    let mut neighbours: Vec<(u32, u32)> = Vec::with_capacity(4);
                    if r > 0 {
                        neighbours.push((r - 1, c));
                    }
                    if r + 1 < self.rows {
                        neighbours.push((r + 1, c));
                    }
                    if c > 0 {
                        neighbours.push((r, c - 1));
                    }
                    if c + 1 < self.cols {
                        neighbours.push((r, c + 1));
                    }
                    for (rr, cc) in neighbours {
                        let i = (rr * self.cols + cc) as usize;
                        if self.is_free(rr, cc) && comp.get(i).copied().unwrap_or(0) == -1 {
                            if let Some(cell) = comp.get_mut(i) {
                                *cell = next;
                            }
                            stack.push((rr, cc));
                        }
                    }
                }
                next += 1;
            }
        }
        comp
    }

    /// the connected-component id of the cell the anchor sits in, or `None` if
    /// that cell is occupied (or the grid is empty)
    fn anchor_component(&self, anchor: PlacementSlot, comp: &[i32]) -> Option<i32> {
        if self.cols == 0 || self.rows == 0 {
            return None;
        }
        let (row, col) = self.anchor_cell(anchor);
        let id = comp
            .get((row * self.cols + col) as usize)
            .copied()
            .unwrap_or(-1);
        if id < 0 { None } else { Some(id) }
    }

    /// the orthogonally adjacent anchors that share a contiguous free area with
    /// the given anchor
    fn connected_neighbours(&self, anchor: PlacementSlot, comp: &[i32]) -> Vec<PlacementSlot> {
        let Some(my) = self.anchor_component(anchor, comp) else {
            return Vec::new();
        };
        anchor
            .neighbours()
            .into_iter()
            .filter(|&nb| self.anchor_component(nb, comp) == Some(my))
            .collect()
    }

    /// evaluate all nine anchors against this grid
    #[must_use]
    pub fn evaluate_slots(&self) -> Vec<PlacementSlotInfo> {
        let components = self.free_components();
        PlacementSlot::ALL
            .into_iter()
            .map(|anchor| {
                let (h3, v3) = anchor.cell_3x3();
                let (c_lo, c_hi) = self.col_band(u32::from(h3));
                let (r_lo, r_hi) = self.row_band(u32::from(v3));
                let free = self.largest_free_rect_in(
                    c_lo,
                    c_hi,
                    r_lo,
                    r_hi,
                    anchor.horizontal(),
                    anchor.vertical(),
                );
                let free_rect =
                    free.map(|(r0, r1, c0, c1)| self.cell_rect_to_pixels(r0, r1, c0, c1));
                let free_size = free_rect.map_or((0, 0), |r| (r.width, r.height));
                let available = free_size.0 > 0 && free_size.1 > 0;
                PlacementSlotInfo {
                    slot: anchor,
                    available,
                    free_rect,
                    free_size,
                    occupied_fraction: self.local_occupied_fraction(anchor),
                    connected_neighbours: self.connected_neighbours(anchor, &components),
                }
            })
            .collect()
    }

    /// The slots and pixel rectangle a *spanning* element anchored at
    /// `anchor` would occupy.
    ///
    /// Returns every [`PlacementSlot`] whose anchor cell lies in the same
    /// connected free region as `anchor` (so all three bottom slots are
    /// returned when the whole bottom edge is free), paired with the largest
    /// all-free rectangle anchored at `anchor` — which extends across that
    /// region. Returns `None` if the anchor's own cell is covered.
    ///
    /// Unlike [`Self::evaluate_slots`], which reports each slot independently,
    /// this is the basis for *reserving* a contiguous block of slots for one
    /// element so neighbouring placements cannot overlap it.
    #[must_use]
    pub fn spanned_region(&self, anchor: PlacementSlot) -> Option<(Vec<PlacementSlot>, PixelRect)> {
        let components = self.free_components();
        let my = self.anchor_component(anchor, &components)?;
        let slots: Vec<PlacementSlot> = PlacementSlot::ALL
            .into_iter()
            .filter(|&slot| self.anchor_component(slot, &components) == Some(my))
            .collect();
        let (r0, r1, c0, c1) = self.largest_free_rect(anchor.horizontal(), anchor.vertical())?;
        Some((slots, self.cell_rect_to_pixels(r0, r1, c0, c1)))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// build an occupancy grid directly from a boolean cell layout (row-major,
    /// row 0 at the top) with a fixed cell size, for testing the slot evaluation
    /// in isolation from any image
    fn grid_from_cells(cols: u32, rows: u32, cell_size: u32, occupied: Vec<bool>) -> OccupancyGrid {
        assert_eq!(occupied.len(), (cols * rows) as usize);
        OccupancyGrid {
            cols,
            rows,
            cell_size,
            img_width: cols * cell_size,
            img_height: rows * cell_size,
            occupied,
        }
    }

    fn slot(
        slots: &[PlacementSlotInfo],
        anchor: PlacementSlot,
    ) -> Result<PlacementSlotInfo, String> {
        slots
            .iter()
            .find(|s| s.slot == anchor)
            .cloned()
            .ok_or_else(|| format!("anchor {anchor:?} should be evaluated"))
    }

    #[test]
    fn empty_grid_confines_each_slot_to_its_third() -> Result<(), Box<dyn std::error::Error>> {
        // 8 cells per side, 10 px each = 80 px. 8 / 3 = 2 edge cells, so the
        // column/row bands are 2 | 4 | 2 cells -> 20 | 40 | 20 px.
        let grid = grid_from_cells(8, 8, 10, vec![false; 64]);
        let slots = grid.evaluate_slots();
        assert_eq!(slots.len(), 9);
        for s in &slots {
            assert!(s.available, "{:?} should be available", s.slot);
            assert!(s.occupied_fraction.abs() < f32::EPSILON);
        }
        // on an empty map each slot fills exactly its own third (no overlap into
        // neighbouring thirds), the centre third being the wider one.
        let size = |a| slot(&slots, a).map(|s| s.free_size);
        assert_eq!(size(PlacementSlot::TopLeft)?, (20, 20));
        assert_eq!(size(PlacementSlot::TopCenter)?, (40, 20));
        assert_eq!(size(PlacementSlot::TopRight)?, (20, 20));
        assert_eq!(size(PlacementSlot::MiddleLeft)?, (20, 40));
        assert_eq!(size(PlacementSlot::Center)?, (40, 40));
        assert_eq!(size(PlacementSlot::BottomRight)?, (20, 20));
        Ok(())
    }

    #[test]
    fn empty_grid_slot_rects_tile_without_overlap() -> Result<(), Box<dyn std::error::Error>> {
        // the nine free rectangles of an empty map must partition the whole
        // image: pairwise disjoint and their areas summing to the full image.
        let grid = grid_from_cells(8, 8, 10, vec![false; 64]);
        let rects: Vec<PixelRect> = grid
            .evaluate_slots()
            .into_iter()
            .filter_map(|s| s.free_rect)
            .collect();
        assert_eq!(rects.len(), 9);
        let overlaps = |a: &PixelRect, b: &PixelRect| {
            a.x < b.x + b.width
                && b.x < a.x + a.width
                && a.y < b.y + b.height
                && b.y < a.y + a.height
        };
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                assert!(!overlaps(a, b), "{a:?} overlaps {b:?}");
            }
        }
        let total: u32 = rects.iter().map(|r| r.width * r.height).sum();
        assert_eq!(total, 80 * 80, "the nine thirds tile the whole image");
        Ok(())
    }

    #[test]
    fn full_grid_no_slot_available() {
        let grid = grid_from_cells(8, 8, 10, vec![true; 64]);
        let slots = grid.evaluate_slots();
        for s in &slots {
            assert!(!s.available, "{:?} should not be available", s.slot);
            assert_eq!(s.free_rect, None);
            assert_eq!(s.free_size, (0, 0));
            assert!(s.connected_neighbours.is_empty());
        }
    }

    #[test]
    fn central_vertical_stripe_frees_the_sides() -> Result<(), Box<dyn std::error::Error>> {
        // occupy the two middle columns (col 3 and 4) of an 8-wide grid
        let occupied: Vec<bool> = (0..64u32).map(|i| matches!(i % 8, 3 | 4)).collect();
        let grid = grid_from_cells(8, 8, 10, occupied);
        let slots = grid.evaluate_slots();
        assert!(slot(&slots, PlacementSlot::MiddleLeft)?.available);
        assert!(slot(&slots, PlacementSlot::MiddleRight)?.available);
        // the centre anchor sits on the occupied stripe
        assert!(!slot(&slots, PlacementSlot::Center)?.available);
        // confined to its third, the free left block is the 2-cell left band -> 20 px
        assert_eq!(slot(&slots, PlacementSlot::MiddleLeft)?.free_size.0, 20);
        assert_eq!(slot(&slots, PlacementSlot::MiddleRight)?.free_size.0, 20);
        Ok(())
    }

    #[test]
    fn horizontal_mirror_swaps_left_and_right() -> Result<(), Box<dyn std::error::Error>> {
        // occupy one interior cell of the top-left third (row 1, col 1 -> 9)
        let occupied: Vec<bool> = (0..64u32).map(|i| i == 9).collect();
        let grid = grid_from_cells(8, 8, 10, occupied);
        let slots = grid.evaluate_slots();
        let top_left = slot(&slots, PlacementSlot::TopLeft)?.free_size;
        assert_ne!(top_left, (0, 0), "the top-left third still has free space");

        // mirror horizontally (col -> 7 - col): the occupied cell lands in the
        // top-right third (row 1, col 6 -> 14) and TopRight must report the
        // mirror-image free size
        let mirrored: Vec<bool> = (0..64u32).map(|i| i == 14).collect();
        let mirrored_grid = grid_from_cells(8, 8, 10, mirrored);
        let mirrored_slots = mirrored_grid.evaluate_slots();
        let top_right = slot(&mirrored_slots, PlacementSlot::TopRight)?.free_size;

        assert_eq!(top_left, top_right);
        Ok(())
    }

    #[test]
    fn adjacency_links_free_neighbours_across_free_band() -> Result<(), Box<dyn std::error::Error>>
    {
        // entirely free grid: every anchor connects to all its 3x3 neighbours
        let grid = grid_from_cells(9, 9, 10, vec![false; 81]);
        let slots = grid.evaluate_slots();
        let center = slot(&slots, PlacementSlot::Center)?;
        let mut neighbours = center.connected_neighbours.clone();
        neighbours.sort_by_key(|a| format!("{a:?}"));
        let mut expected = PlacementSlot::Center.neighbours();
        expected.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(neighbours, expected);
        Ok(())
    }

    #[test]
    fn adjacency_broken_by_occupied_band() -> Result<(), Box<dyn std::error::Error>> {
        // occupy the entire middle column (col 4) so left and right are separate
        // free components
        let occupied: Vec<bool> = (0..81u32).map(|i| i % 9 == 4).collect();
        let grid = grid_from_cells(9, 9, 10, occupied);
        let slots = grid.evaluate_slots();
        // TopLeft and TopRight live in different components (split by the band),
        // and neither connects to TopCenter (which sits on the band)
        let top_left = slot(&slots, PlacementSlot::TopLeft)?;
        assert!(
            !top_left
                .connected_neighbours
                .contains(&PlacementSlot::TopCenter)
        );
        // TopLeft still connects downward to MiddleLeft (same left component)
        assert!(
            top_left
                .connected_neighbours
                .contains(&PlacementSlot::MiddleLeft)
        );
        Ok(())
    }

    #[test]
    fn neighbours_are_orthogonal_only() {
        assert_eq!(
            PlacementSlot::TopLeft.neighbours(),
            vec![PlacementSlot::TopCenter, PlacementSlot::MiddleLeft]
        );
        let center = PlacementSlot::Center.neighbours();
        assert_eq!(center.len(), 4);
        assert!(!center.contains(&PlacementSlot::TopLeft));
    }

    #[test]
    fn as_str_and_from_str_round_trip() {
        for slot in PlacementSlot::ALL {
            assert_eq!(slot.as_str().parse::<PlacementSlot>().ok(), Some(slot));
            // Display yields the same text as as_str
            assert_eq!(slot.to_string(), slot.as_str());
        }
        assert_eq!("nonsense".parse::<PlacementSlot>().ok(), None);
    }

    #[test]
    fn default_alignment_matches_slot_outward() {
        assert_eq!(
            PlacementSlot::TopLeft.default_alignment(),
            (HAlign::Left, VAlign::Top)
        );
        assert_eq!(
            PlacementSlot::TopCenter.default_alignment(),
            (HAlign::Center, VAlign::Top)
        );
        assert_eq!(
            PlacementSlot::MiddleRight.default_alignment(),
            (HAlign::Right, VAlign::Center)
        );
        assert_eq!(
            PlacementSlot::Center.default_alignment(),
            (HAlign::Center, VAlign::Center)
        );
        assert_eq!(
            PlacementSlot::BottomRight.default_alignment(),
            (HAlign::Right, VAlign::Bottom)
        );
    }

    #[test]
    fn align_offset_clamps_and_centres() {
        // content fits: left/top = 0, centre = half slack, right/bottom = full slack
        assert_eq!(HAlign::Left.offset(20, 100), 0);
        assert_eq!(HAlign::Center.offset(20, 100), 40);
        assert_eq!(HAlign::Right.offset(20, 100), 80);
        assert_eq!(VAlign::Bottom.offset(30, 100), 70);
        // content larger than span: never negative/wraps
        assert_eq!(HAlign::Right.offset(120, 100), 0);
        assert_eq!(VAlign::Center.offset(120, 100), 0);
    }

    #[test]
    fn anchored_origin_hugs_edges_and_centres() {
        // 200x100 image, a 40x20 box, 8px margin
        assert_eq!(
            PlacementSlot::TopLeft.anchored_origin(40, 20, 200, 100, 8),
            (8, 8)
        );
        assert_eq!(
            PlacementSlot::TopRight.anchored_origin(40, 20, 200, 100, 8),
            (200 - 40 - 8, 8)
        );
        assert_eq!(
            PlacementSlot::BottomRight.anchored_origin(40, 20, 200, 100, 8),
            (200 - 40 - 8, 100 - 20 - 8)
        );
        assert_eq!(
            PlacementSlot::Center.anchored_origin(40, 20, 200, 100, 8),
            ((200 - 40) / 2, (100 - 20) / 2)
        );
        assert_eq!(
            PlacementSlot::TopCenter.anchored_origin(40, 20, 200, 100, 8),
            ((200 - 40) / 2, 8)
        );
        // box larger than image clamps to origin without underflow
        assert_eq!(
            PlacementSlot::BottomRight.anchored_origin(400, 200, 200, 100, 8),
            (0, 0)
        );
    }

    #[test]
    fn spanned_region_covers_whole_free_bottom_edge() -> Result<(), Box<dyn std::error::Error>> {
        // free the entire bottom row (row 2), occupy everything above it
        let occupied: Vec<bool> = (0..27u32).map(|i| i / 9 != 2).collect();
        let grid = grid_from_cells(9, 3, 10, occupied);
        let (mut slots, rect) = grid
            .spanned_region(PlacementSlot::BottomCenter)
            .ok_or("bottom_center anchor is free, must span")?;
        slots.sort_by_key(|a| format!("{a:?}"));
        let mut expected = vec![
            PlacementSlot::BottomLeft,
            PlacementSlot::BottomCenter,
            PlacementSlot::BottomRight,
        ];
        expected.sort_by_key(|a| format!("{a:?}"));
        assert_eq!(slots, expected, "all three bottom slots are reserved");
        // the combined rectangle spans the full 9-cell width
        assert_eq!(rect.width, 90);
        assert_eq!(rect.height, 10);
        Ok(())
    }

    #[test]
    fn spanned_region_none_when_anchor_covered() {
        // occupy the entire bottom row so the bottom_center anchor is covered
        let occupied: Vec<bool> = (0..27u32).map(|i| i / 9 == 2).collect();
        let grid = grid_from_cells(9, 3, 10, occupied);
        assert_eq!(grid.spanned_region(PlacementSlot::BottomCenter), None);
    }

    #[test]
    fn spanned_region_stops_at_occupied_band() -> Result<(), Box<dyn std::error::Error>> {
        // free only the left two columns of the bottom row; occupy the rest.
        // bottom_left's component must exclude bottom_right.
        let occupied: Vec<bool> = (0..27u32)
            .map(|i| {
                let (row, col) = (i / 9, i % 9);
                !(row == 2 && col < 2)
            })
            .collect();
        let grid = grid_from_cells(9, 3, 10, occupied);
        let (slots, _rect) = grid
            .spanned_region(PlacementSlot::BottomLeft)
            .ok_or("bottom_left anchor is free")?;
        assert!(slots.contains(&PlacementSlot::BottomLeft));
        assert!(!slots.contains(&PlacementSlot::BottomRight));
        Ok(())
    }

    #[test]
    fn centre_third_absorbs_remainder_keeping_edges_equal() -> Result<(), Box<dyn std::error::Error>>
    {
        // 11 cells per side is not divisible by 3: 11 / 3 = 3 edge cells, so the
        // bands are 3 | 5 | 3. The two edge thirds must stay equal and the centre
        // third must be the wider one (it absorbs the remainder).
        let grid = grid_from_cells(11, 11, 10, vec![false; 121]);
        let slots = grid.evaluate_slots();
        let size = |a| slot(&slots, a).map(|s| s.free_size);
        assert_eq!(size(PlacementSlot::TopLeft)?, (30, 30));
        assert_eq!(size(PlacementSlot::TopRight)?, (30, 30));
        assert_eq!(size(PlacementSlot::BottomLeft)?, (30, 30));
        assert_eq!(size(PlacementSlot::BottomRight)?, (30, 30));
        assert_eq!(size(PlacementSlot::Center)?, (50, 50));
        Ok(())
    }

    #[test]
    fn spanned_region_uses_minimum_perpendicular_extent() -> Result<(), Box<dyn std::error::Error>>
    {
        // left third (cols 0..3) is free 4 rows deep; the rest of the top is free
        // only 2 rows deep. A span from top-left can either stay narrow-and-tall
        // (3 cols x 4 rows) or grow full-width-and-short (9 cols x 2 rows). The
        // wider rectangle wins on area, and its height is the *minimum* depth
        // across the thirds it crosses (2 rows), not the left third's 4.
        let occupied: Vec<bool> = (0..54u32)
            .map(|i| {
                let (row, col) = (i / 9, i % 9);
                let free = if col < 3 { row < 4 } else { row < 2 };
                !free
            })
            .collect();
        let grid = grid_from_cells(9, 6, 10, occupied);
        let (_slots, rect) = grid
            .spanned_region(PlacementSlot::TopLeft)
            .ok_or("top_left anchor is free")?;
        assert_eq!(rect.width, 90, "the span grows across the full width");
        assert_eq!(
            rect.height, 20,
            "the span is clipped to the shallower thirds"
        );
        Ok(())
    }

    mod with_map {
        use super::*;
        use crate::map_tiles::Map;
        use image::GenericImageView as _;
        use sl_types::map::{GridCoordinates, GridRectangle, ZoomLevel};

        fn test_rectangle() -> GridRectangle {
            // a 4x4 region rectangle
            GridRectangle::new(
                GridCoordinates::new(1000, 1000),
                GridCoordinates::new(1003, 1003),
            )
        }

        #[test]
        fn blank_dimensions_match_zoom_times_regions() -> Result<(), Box<dyn std::error::Error>> {
            let zoom = ZoomLevel::try_new(4)?;
            let map = Map::blank(test_rectangle(), zoom);
            // zoom 4 -> 32 pixels per region, 4 regions per side
            let expected = u32::from(zoom.pixels_per_region()) * 4;
            assert_eq!(map.dimensions(), (expected, expected));
            assert_eq!(map.dimensions(), (128, 128));
            Ok(())
        }

        #[test]
        fn blank_fit_matches_real_render_sizing() -> Result<(), Box<dyn std::error::Error>> {
            let rect = test_rectangle();
            let map = Map::blank_fit(rect.clone(), 200, 200)?;
            let zoom = ZoomLevel::max_zoom_level_to_fit_regions_into_output_image(4, 4, 200, 200)?;
            let expected = u32::from(zoom.pixels_per_region()) * 4;
            assert_eq!(map.dimensions(), (expected, expected));
            Ok(())
        }

        #[test]
        fn stamped_rectangle_blocks_its_corner() -> Result<(), Box<dyn std::error::Error>> {
            let mut map = Map::blank(test_rectangle(), ZoomLevel::try_new(4)?);
            // opaque rectangle covering the top-left 40x40 pixels
            imageproc::drawing::draw_filled_rect_mut(
                map.image_mut(),
                imageproc::rect::Rect::at(0, 0).of_size(40, 40),
                image::Rgba([255, 0, 0, 255]),
            );
            let grid = OccupancyGrid::from_map(&map, DEFAULT_COVERAGE_GRID);
            let slots = grid.evaluate_slots();
            // the covered top-left corner has no free space
            assert!(!slot(&slots, PlacementSlot::TopLeft)?.available);
            assert!(slot(&slots, PlacementSlot::TopLeft)?.occupied_fraction > 0f32);
            // the far corner is wide open: free across its whole third (the
            // 128 px image splits into thirds of ~42 px, so the bottom-right
            // third is unobstructed end to end)
            let bottom_right = slot(&slots, PlacementSlot::BottomRight)?;
            assert!(bottom_right.available);
            assert!(bottom_right.free_size.0 >= 40);
            assert!(bottom_right.free_size.1 >= 40);
            Ok(())
        }

        #[test]
        fn diagonal_route_blocks_centre_but_not_off_diagonal_corner()
        -> Result<(), Box<dyn std::error::Error>> {
            let mut map = Map::blank(test_rectangle(), ZoomLevel::try_new(4)?);
            map.draw_pixel_waypoint_route(
                &[(10f32, 10f32), (64f32, 64f32), (118f32, 118f32)],
                image::Rgba([0, 0, 255, 255]),
            )?;
            let grid = OccupancyGrid::from_map(&map, DEFAULT_COVERAGE_GRID);
            let slots = grid.evaluate_slots();
            // the route runs through the middle of the map
            assert!(!slot(&slots, PlacementSlot::Center)?.available);
            // the top-right corner is far from the descending diagonal
            assert!(slot(&slots, PlacementSlot::TopRight)?.available);
            Ok(())
        }
    }
}
