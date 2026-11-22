const TILE_STEP: u32 = 16;
pub const TABLETOP_Y_SCALE: f32 = 0.866_025_4; // cos(30°)
const HEIGHT_SCALE: f32 = 0.5; // sin(30°)

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, blade_macros::Vertex)]
pub(super) struct TabletopVertex {
    pub world: [f32; 2],
    pub board_y: f32,
}

/// The static triangles and CPU picking use the same grid and diagonal.
pub struct Tabletop {
    cells_x: usize,
    cells_y: usize,
    step_x: f32,
    step_y: f32,
    heights: Vec<f32>,
}

impl Tabletop {
    pub(super) fn new(width: u32, height: u32, terrain: &[u8]) -> Self {
        let cells_x = width.div_ceil(TILE_STEP) as usize;
        let cells_y = height.div_ceil(TILE_STEP) as usize;
        let step_x = width as f32 / cells_x as f32;
        let step_y = height as f32 / cells_y as f32;
        let stride = cells_x + 1;
        let mut heights = vec![0.0; stride * (cells_y + 1)];
        let max_height = step_y * 0.5;
        for y in 1..cells_y {
            for x in 1..cells_x {
                let col = (x as f32 * step_x) as usize;
                let row = (y as f32 * step_y) as usize;
                heights[y * stride + x] =
                    f32::from((terrain[row * width as usize + col] & 31).min(30)) / 30.0 * max_height;
            }
        }

        // Any triangle covering water stays at sea level. Flatten the four vertices
        // of each coarse cell touched by a water tile, including a shoreline crossing.
        for (i, tile) in terrain.iter().enumerate() {
            if tile & 128 != 0 && tile & 31 != 31 {
                continue;
            }
            let x = i % width as usize;
            let y = i / width as usize;
            let coarse = |p: usize, step: f32, cells: usize| {
                let first = ((p as f32 / step) as usize).min(cells - 1);
                let last = (((p + 1) as f32 / step) as usize).min(cells - 1);
                first..=last
            };
            for cy in coarse(y, step_y, cells_y) {
                for cx in coarse(x, step_x, cells_x) {
                    let i = cy * stride + cx;
                    heights[i..i + 2].fill(0.0);
                    heights[i + stride..i + stride + 2].fill(0.0);
                }
            }
        }
        Self { cells_x, cells_y, step_x, step_y, heights }
    }

    pub(super) fn mesh(&self) -> (Vec<TabletopVertex>, Vec<u32>) {
        let stride = self.cells_x + 1;
        let mut vertices = Vec::with_capacity(self.heights.len());
        let mut indices = Vec::with_capacity(self.cells_x * self.cells_y * 6);
        for y in 0..=self.cells_y {
            for x in 0..stride {
                let i = y * stride + x;
                let world = [x as f32 * self.step_x, y as f32 * self.step_y];
                vertices.push(TabletopVertex {
                    world,
                    board_y: world[1] * TABLETOP_Y_SCALE - self.heights[i] * HEIGHT_SCALE,
                });
                if x < self.cells_x && y < self.cells_y {
                    let a = i as u32;
                    let c = (i + stride) as u32;
                    indices.extend_from_slice(&[a, a + 1, c + 1, a, c + 1, c]);
                }
            }
        }
        (vertices, indices)
    }

    pub fn project(&self, [x, y]: [f32; 2]) -> [f32; 2] {
        let gx = (x / self.step_x).clamp(0.0, self.cells_x as f32);
        let gy = (y / self.step_y).clamp(0.0, self.cells_y as f32);
        let cx = (gx as usize).min(self.cells_x - 1);
        let cy = (gy as usize).min(self.cells_y - 1);
        let fx = gx - cx as f32;
        let fy = gy - cy as f32;
        let stride = self.cells_x + 1;
        let i = cy * stride + cx;
        let a = self.heights[i];
        let d = self.heights[i + stride + 1];
        let h = if fx >= fy {
            a * (1.0 - fx) + self.heights[i + 1] * (fx - fy) + d * fy
        } else {
            a * (1.0 - fy) + self.heights[i + stride] * (fy - fx) + d * fx
        };
        [x, y * TABLETOP_Y_SCALE - h * HEIGHT_SCALE]
    }

    pub fn unproject(&self, [x, board_y]: [f32; 2]) -> [f32; 2] {
        // Height is at most half a step, so projected Y increases along every
        // triangle. Invert the surface without an extra render or GPU readback.
        let flat = board_y / TABLETOP_Y_SCALE;
        let max_y = self.step_y * self.cells_y as f32;
        if flat <= 0.0 || flat >= max_y {
            return [x, flat];
        }
        let mut lo = flat;
        let mut hi = (flat + self.step_y).min(max_y);
        for _ in 0..24 {
            let y = (lo + hi) * 0.5;
            if self.project([x, y])[1] < board_y {
                lo = y;
            } else {
                hi = y;
            }
        }
        [x, (lo + hi) * 0.5]
    }
}
