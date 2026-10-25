use crate::math::Vec2;
use serde::{Deserialize, Serialize};

pub const SCENT_PLANT: usize = 0;
pub const SCENT_DANGER: usize = 1;
pub const SCENT_MATE: usize = 2;
pub const CHANNELS: usize = 3;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChemicalGrid {
    pub width: usize,
    pub height: usize,
    pub world_width: f32,
    pub world_height: f32,
    pub cell_w: f32,
    pub cell_h: f32,
    // [channel][y * width + x]
    pub grid: Vec<Vec<f32>>,
    #[serde(skip)]
    scratch: Vec<Vec<f32>>,
}

impl ChemicalGrid {
    pub fn new(width: usize, height: usize, world_width: f32, world_height: f32) -> Self {
        let size = width * height;
        ChemicalGrid {
            width,
            height,
            world_width,
            world_height,
            cell_w: world_width / width as f32,
            cell_h: world_height / height as f32,
            grid: vec![vec![0.0; size]; CHANNELS],
            scratch: vec![vec![0.0; size]; CHANNELS],
        }
    }

    #[inline]
    fn world_to_cell(&self, pos: Vec2) -> (usize, usize) {
        let x = ((pos.x / self.cell_w).floor() as isize).clamp(0, self.width as isize - 1) as usize;
        let y = ((pos.y / self.cell_h).floor() as isize).clamp(0, self.height as isize - 1) as usize;
        (x, y)
    }

    pub fn deposit(&mut self, pos: Vec2, channel: usize, amount: f32) {
        if channel >= CHANNELS {
            return;
        }
        let (cx, cy) = self.world_to_cell(pos);
        let idx = cy * self.width + cx;
        self.grid[channel][idx] = (self.grid[channel][idx] + amount).min(10.0);
    }

    pub fn sample(&self, pos: Vec2, channel: usize) -> f32 {
        if channel >= CHANNELS {
            return 0.0;
        }
        let (cx, cy) = self.world_to_cell(pos);
        let idx = cy * self.width + cx;
        self.grid[channel][idx]
    }

    pub fn update(&mut self) {
        let w = self.width;
        let h = self.height;

        const DIFFUSION_RATES: [f32; CHANNELS] = [0.08, 0.12, 0.10];
        const DECAY_RATES: [f32; CHANNELS] = [0.015, 0.035, 0.025];

        for c in 0..CHANNELS {
            let diff = DIFFUSION_RATES[c];
            let decay = DECAY_RATES[c];
            let current = &self.grid[c];
            let scratch = &mut self.scratch[c];

            for y in 0..h {
                let ym1 = if y == 0 { 0 } else { y - 1 };
                let yp1 = if y + 1 >= h { h - 1 } else { y + 1 };
                let row_offset = y * w;

                for x in 0..w {
                    let xm1 = if x == 0 { 0 } else { x - 1 };
                    let xp1 = if x + 1 >= w { w - 1 } else { x + 1 };

                    let idx = row_offset + x;
                    let val = current[idx];

                    // 4-neighbor Laplacian
                    let left = current[row_offset + xm1];
                    let right = current[row_offset + xp1];
                    let up = current[ym1 * w + x];
                    let down = current[yp1 * w + x];

                    let laplacian = (left + right + up + down) * 0.25 - val;
                    let new_val = (val + diff * laplacian) * (1.0 - decay);

                    scratch[idx] = if new_val > 0.001 { new_val } else { 0.0 };
                }
            }

            self.grid[c].copy_from_slice(scratch);
        }
    }
}
