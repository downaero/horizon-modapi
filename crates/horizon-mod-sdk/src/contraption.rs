//! Moving structures for server mods: a box of blocks taken out of the world
//! as block entities (`entities::spawn_block`), moved together, and put back
//! as blocks where they stand. What a piston, a gantry, an elevator or a
//! simple flying machine needs. The structure only moves by whole steps of
//! its own choosing and is drawn from its blocks; players do not stand on it
//! or collide with it while it moves.
//!
//! Needs capabilities `world` and `entities` (`experimental = ["entities"]`).
use crate::server::{BlockChange, entities, world};

/// The smallest turn clients are sent (an angle is a byte on the wire):
/// turns a whole number of these keep every tick's step the same and the
/// blocks where their turn puts them.
pub const ROTATION_STEP: f32 = 360.0 / 256.0;

/// `angle` on the grid of [`ROTATION_STEP`].
pub fn snap_angle(angle: f32) -> f32 {
    (angle / ROTATION_STEP).round() * ROTATION_STEP
}

/// A structure taken out of the world.
pub struct Contraption {
    dimension: String,
    /// Where the box's minimum corner stood when it was assembled.
    origin: [i32; 3],
    /// How far it has moved since, in blocks.
    offset: [f64; 3],
    /// Each block: its place in the box, its state and its entity.
    blocks: Vec<([i32; 3], String, u32)>,
    /// The size of the box, for the centre it turns about.
    size: [i32; 3],
    /// Its turn, in degrees: pitch about x, then yaw about y.
    rotation: [f32; 2],
}

impl Contraption {
    /// Takes the non-air blocks from `min` to `max` (both included, at most
    /// 32768 positions and 1024 blocks) out of the world: each becomes a
    /// block entity where it stood and its place turns to air. Fails, and
    /// changes nothing, when the box is too large or holds too many blocks.
    pub fn assemble(dimension: &str, min: [i32; 3], max: [i32; 3]) -> Result<Self, String> {
        let region = world::scan(dimension, min[0], min[1], min[2], max[0], max[1], max[2])?;
        if region.blocks.len() > 1024 {
            return Err(format!("{} blocks: at most 1024", region.blocks.len()));
        }
        let mut blocks = Vec::with_capacity(region.blocks.len());
        for b in &region.blocks {
            let state = region.palette[b.state as usize].clone();
            let at = [f64::from(b.x) + 0.5, f64::from(b.y), f64::from(b.z) + 0.5];
            match entities::spawn_block(dimension, at[0], at[1], at[2], &state) {
                Ok(handle) => {
                    blocks.push(([b.x - min[0], b.y - min[1], b.z - min[2]], state, handle))
                }
                Err(e) => {
                    for (_, _, handle) in blocks {
                        entities::remove(handle);
                    }
                    return Err(e);
                }
            }
        }
        let air: Vec<BlockChange> = region
            .blocks
            .iter()
            .map(|b| BlockChange {
                x: b.x,
                y: b.y,
                z: b.z,
                state: "minecraft:air".into(),
            })
            .collect();
        for chunk in air.chunks(4096) {
            world::set_blocks(dimension, chunk)?;
        }
        Ok(Self {
            dimension: dimension.to_owned(),
            origin: min,
            offset: [0.0; 3],
            blocks,
            size: [
                max[0] - min[0] + 1,
                max[1] - min[1] + 1,
                max[2] - min[2] + 1,
            ],
            rotation: [0.0; 2],
        })
    }

    /// How many blocks it carries.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether it carries none.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// How far it has moved since it was assembled, in blocks.
    pub fn offset(&self) -> [f64; 3] {
        self.offset
    }

    /// Moves every block by `delta` blocks (a fraction for a smooth move, a
    /// tick at a time).
    pub fn move_by(&mut self, delta: [f64; 3]) {
        for i in 0..3 {
            self.offset[i] += delta[i];
        }
        self.place();
    }

    /// Turns the whole structure about the centre of its box, in degrees:
    /// first `pitch` about x, then `yaw` about y (a mill's sails, a door
    /// swinging, a ship turning). Each block turns with it.
    pub fn set_rotation(&mut self, yaw: f32, pitch: f32) {
        // On the grid clients turn blocks by, so places and turns agree.
        let (yaw, pitch) = (snap_angle(yaw), snap_angle(pitch));
        self.rotation = [yaw, pitch];
        for (_, _, handle) in &self.blocks {
            entities::rotate(*handle, yaw, pitch);
        }
        self.place();
    }

    /// Makes every block solid for players, or not (`entities::set_solid`).
    pub fn set_solid(&self, solid: bool) {
        for (_, _, handle) in &self.blocks {
            entities::set_solid(*handle, solid);
        }
    }

    /// Puts each block entity where its place in the box is now, moved and
    /// turned about the box's centre.
    fn place(&self) {
        let (yaw, pitch) = (
            f64::from(-self.rotation[0]).to_radians(),
            f64::from(self.rotation[1]).to_radians(),
        );
        let half = self.size.map(|s| f64::from(s) / 2.0);
        let pivot = [
            f64::from(self.origin[0]) + half[0] + self.offset[0],
            f64::from(self.origin[1]) + half[1] + self.offset[1],
            f64::from(self.origin[2]) + half[2] + self.offset[2],
        ];
        for (rel, _, handle) in &self.blocks {
            // The block's centre from the pivot, turned about x, then y.
            let v = [
                f64::from(rel[0]) + 0.5 - half[0],
                f64::from(rel[1]) + 0.5 - half[1],
                f64::from(rel[2]) + 0.5 - half[2],
            ];
            let (y1, z1) = (
                v[1] * pitch.cos() - v[2] * pitch.sin(),
                v[1] * pitch.sin() + v[2] * pitch.cos(),
            );
            let (x2, z2) = (
                v[0] * yaw.cos() + z1 * yaw.sin(),
                -v[0] * yaw.sin() + z1 * yaw.cos(),
            );
            entities::move_to(
                *handle,
                pivot[0] + x2,
                pivot[1] + y1 - 0.5,
                pivot[2] + z2,
                self.rotation[0],
            );
        }
    }

    /// Puts the blocks back into the world at the nearest whole position
    /// and takes the entities away. The blocks go where the world is;
    /// whatever stands there is replaced.
    pub fn disassemble(self) -> Result<[i32; 3], String> {
        // Put back unturned: a turned structure lands as it was assembled.
        let shift = self.offset.map(|o| o.round() as i32);
        let changes: Vec<BlockChange> = self
            .blocks
            .iter()
            .map(|(rel, state, _)| BlockChange {
                x: self.origin[0] + rel[0] + shift[0],
                y: self.origin[1] + rel[1] + shift[1],
                z: self.origin[2] + rel[2] + shift[2],
                state: state.clone(),
            })
            .collect();
        for chunk in changes.chunks(4096) {
            world::set_blocks(&self.dimension, chunk)?;
        }
        for (_, _, handle) in self.blocks {
            entities::remove(handle);
        }
        Ok(shift)
    }
}
