#![allow(dead_code)]
use crate::math::{angle_diff, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntityKind {
    Creature,
    Food,
    Meat,
}

#[derive(Clone, Copy, Debug)]
pub struct SpatialEntry {
    pub kind: EntityKind,
    pub id: usize,
    pub pos: Vec2,
    pub radius: f32,
}

pub struct SpatialGrid {
    pub cell_size: f32,
    pub cols: usize,
    pub rows: usize,
    pub world_width: f32,
    pub world_height: f32,
    pub cells: Vec<Vec<SpatialEntry>>,
}

impl SpatialGrid {
    pub fn new(world_width: f32, world_height: f32, cell_size: f32) -> Self {
        let cols = (world_width / cell_size).ceil() as usize;
        let rows = (world_height / cell_size).ceil() as usize;
        let total_cells = cols * rows;
        SpatialGrid {
            cell_size,
            cols,
            rows,
            world_width,
            world_height,
            cells: vec![Vec::with_capacity(16); total_cells],
        }
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    #[inline]
    fn get_cell_coords(&self, pos: Vec2) -> (usize, usize) {
        let cx = ((pos.x / self.cell_size).floor() as isize).clamp(0, self.cols as isize - 1) as usize;
        let cy = ((pos.y / self.cell_size).floor() as isize).clamp(0, self.rows as isize - 1) as usize;
        (cx, cy)
    }

    pub fn insert(&mut self, entry: SpatialEntry) {
        let (cx, cy) = self.get_cell_coords(entry.pos);
        let idx = cy * self.cols + cx;
        self.cells[idx].push(entry);
    }

    // Query all entities within radius of center
    pub fn query_radius(&self, center: Vec2, radius: f32) -> Vec<SpatialEntry> {
        let mut results = Vec::with_capacity(32);
        let rad_sq = radius * radius;

        let min_cx = (((center.x - radius) / self.cell_size).floor() as isize).clamp(0, self.cols as isize - 1) as usize;
        let max_cx = (((center.x + radius) / self.cell_size).floor() as isize).clamp(0, self.cols as isize - 1) as usize;
        let min_cy = (((center.y - radius) / self.cell_size).floor() as isize).clamp(0, self.rows as isize - 1) as usize;
        let max_cy = (((center.y + radius) / self.cell_size).floor() as isize).clamp(0, self.rows as isize - 1) as usize;

        for cy in min_cy..=max_cy {
            let row_offset = cy * self.cols;
            for cx in min_cx..=max_cx {
                let cell = &self.cells[row_offset + cx];
                for entry in cell {
                    if entry.pos.distance_squared(center) <= rad_sq {
                        results.push(*entry);
                    }
                }
            }
        }

        results
    }

    // Raycast / cone query for eyes
    pub fn query_cone(
        &self,
        origin: Vec2,
        forward_angle: f32,
        fov: f32,
        range: f32,
        ignore_id: Option<usize>,
    ) -> Option<(SpatialEntry, f32)> {
        let range_sq = range * range;
        let half_fov = fov * 0.5;

        let min_cx = (((origin.x - range) / self.cell_size).floor() as isize).clamp(0, self.cols as isize - 1) as usize;
        let max_cx = (((origin.x + range) / self.cell_size).floor() as isize).clamp(0, self.cols as isize - 1) as usize;
        let min_cy = (((origin.y - range) / self.cell_size).floor() as isize).clamp(0, self.rows as isize - 1) as usize;
        let max_cy = (((origin.y + range) / self.cell_size).floor() as isize).clamp(0, self.rows as isize - 1) as usize;

        let mut closest: Option<(SpatialEntry, f32)> = None;
        let mut min_dist_sq = range_sq;

        for cy in min_cy..=max_cy {
            let row_offset = cy * self.cols;
            for cx in min_cx..=max_cx {
                let cell = &self.cells[row_offset + cx];
                for entry in cell {
                    if entry.kind == EntityKind::Creature && Some(entry.id) == ignore_id {
                        continue;
                    }

                    let d_sq = origin.distance_squared(entry.pos);
                    if d_sq > min_dist_sq {
                        continue;
                    }

                    // Check angle into cone
                    let to_entry = entry.pos - origin;
                    let angle_to_obj = to_entry.to_angle();
                    let diff = angle_diff(angle_to_obj, forward_angle).abs();

                    if diff <= half_fov {
                        min_dist_sq = d_sq;
                        closest = Some((*entry, d_sq.sqrt()));
                    }
                }
            }
        }

        closest
    }
}
