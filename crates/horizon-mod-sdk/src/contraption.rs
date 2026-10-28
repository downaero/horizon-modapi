//! Moving structures for server mods: a box of blocks taken out of the world
//! as block entities (`entities::spawn_block`), moved together, and put back
//! as blocks where they stand. What a piston, a gantry, an elevator or a
//! simple flying machine needs. The structure only moves by whole steps of
//! its own choosing and is drawn from its blocks; players do not stand on it
//! or collide with it while it moves.
//!
//! Needs capabilities `world` and `entities` (`experimental = ["entities"]`).
use crate::server::{BlockChange, entities, world};

/// A structure taken out of the world.
pub struct Contraption {
    dimension: String,
    /// Where the box's minimum corner stood when it was assembled.
    origin: [i32; 3],
    /// How far it has moved since, in blocks.
    offset: [f64; 3],
    /// Each block: its place in the box, its state and its entity.
    blocks: Vec<([i32; 3], String, u32)>,
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
        for (rel, _, handle) in &self.blocks {
            let x = f64::from(self.origin[0] + rel[0]) + 0.5 + self.offset[0];
            let y = f64::from(self.origin[1] + rel[1]) + self.offset[1];
            let z = f64::from(self.origin[2] + rel[2]) + 0.5 + self.offset[2];
            entities::move_to(*handle, x, y, z, 0.0);
        }
    }

    /// Puts the blocks back into the world at the nearest whole position
    /// and takes the entities away. The blocks go where the world is;
    /// whatever stands there is replaced.
    pub fn disassemble(self) -> Result<[i32; 3], String> {
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
