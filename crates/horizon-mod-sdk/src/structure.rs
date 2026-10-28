//! Moving structures for server mods as one object (capability `entities`'
//! `structures`): a box of blocks taken out of the world, moved and turned
//! as a whole, and put back as blocks. Players' games draw it as one model,
//! smoothly between ticks, whatever its angle; it has none of
//! [`Contraption`](crate::contraption::Contraption)'s limit of 1024 blocks
//! and entity per block. Players do not collide with it yet.
//!
//! Needs capabilities `world` and `entities` (`experimental = ["entities"]`).
use crate::server::{BlockChange, structures, world};

/// A structure taken out of the world.
pub struct Structure {
    dimension: String,
    id: u32,
    /// Where the box's minimum corner stood when it was assembled.
    origin: [i32; 3],
    /// How far it has moved since, in blocks.
    offset: [f64; 3],
    /// Each block: its place in the box and its state.
    blocks: Vec<([i32; 3], String)>,
    /// The size of the box, for the centre it turns about.
    size: [i32; 3],
    /// Its turn, in degrees: pitch about x, then yaw about y.
    rotation: [f32; 2],
}

/// `q * r`, quaternions as (x, y, z, w).
fn mul(q: [f64; 4], r: [f64; 4]) -> [f64; 4] {
    let [ax, ay, az, aw] = q;
    let [bx, by, bz, bw] = r;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

/// `v` turned by the unit quaternion `q`.
fn turn(q: [f64; 4], v: [f64; 3]) -> [f64; 3] {
    let p = mul(mul(q, [v[0], v[1], v[2], 0.0]), [-q[0], -q[1], -q[2], q[3]]);
    [p[0], p[1], p[2]]
}

impl Structure {
    /// Takes the non-air blocks from `min` to `max` (both included, at most
    /// 32768 positions and 4096 blocks) out of the world as one structure
    /// standing where they stood. Fails, and changes nothing, when the box
    /// is too large or holds too many blocks.
    pub fn assemble(dimension: &str, min: [i32; 3], max: [i32; 3]) -> Result<Self, String> {
        let region = world::scan(dimension, min[0], min[1], min[2], max[0], max[1], max[2])?;
        let blocks: Vec<([i32; 3], String)> = region
            .blocks
            .iter()
            .map(|b| {
                let state = region.palette[b.state as usize].clone();
                ([b.x - min[0], b.y - min[1], b.z - min[2]], state)
            })
            .collect();
        let mut this = Self {
            dimension: dimension.to_owned(),
            id: 0,
            origin: min,
            offset: [0.0; 3],
            blocks,
            size: [0, 1, 2].map(|i| max[i] - min[i] + 1),
            rotation: [0.0; 2],
        };
        let listed: Vec<structures::StructureBlock> = this
            .blocks
            .iter()
            .map(|(p, state)| structures::StructureBlock {
                x: p[0],
                y: p[1],
                z: p[2],
                state: state.clone(),
            })
            .collect();
        this.id = structures::create(dimension, &listed, this.pose())?;
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
            if let Err(e) = world::set_blocks(dimension, chunk) {
                structures::remove(this.id);
                return Err(e);
            }
        }
        Ok(this)
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

    /// Moves it by `delta` blocks (a fraction for a smooth move, a tick at
    /// a time).
    pub fn move_by(&mut self, delta: [f64; 3]) {
        for i in 0..3 {
            self.offset[i] += delta[i];
        }
        structures::set_pose(self.id, self.pose());
    }

    /// Turns it about the centre of its box, in degrees: first `pitch`
    /// about x, then `yaw` about y, any angle.
    pub fn set_rotation(&mut self, yaw: f32, pitch: f32) {
        self.rotation = [yaw, pitch];
        structures::set_pose(self.id, self.pose());
    }

    /// Its turn as a quaternion (x, y, z, w): yaw about -y after pitch
    /// about x, as [`Contraption`](crate::contraption::Contraption) turns.
    fn quaternion(&self) -> [f64; 4] {
        let half = |deg: f32| f64::from(deg).to_radians() / 2.0;
        let (y, p) = (half(-self.rotation[0]), half(self.rotation[1]));
        mul([0.0, y.sin(), 0.0, y.cos()], [p.sin(), 0.0, 0.0, p.cos()])
    }

    /// Where its (0, 0, 0) corner is and how it is turned, turning about
    /// the box's centre.
    fn pose(&self) -> structures::Pose {
        let q = self.quaternion();
        let centre = self.size.map(|s| f64::from(s) / 2.0);
        let turned = turn(q, centre);
        let at = |i: usize| f64::from(self.origin[i]) + self.offset[i] + centre[i] - turned[i];
        structures::Pose {
            x: at(0),
            y: at(1),
            z: at(2),
            rotation: (q[0] as f32, q[1] as f32, q[2] as f32, q[3] as f32),
        }
    }

    /// Puts the blocks back into the world at the nearest whole position,
    /// turned to the nearest quarter turns, and takes the structure away.
    /// Whatever stands where they go is replaced. Returns how far the box
    /// moved.
    pub fn disassemble(mut self) -> Result<[i32; 3], String> {
        let quarter = |deg: f32| (deg / 90.0).round() * 90.0;
        self.rotation = self.rotation.map(quarter);
        self.offset = self.offset.map(f64::round);
        let pose = self.pose();
        let q = self.quaternion();
        let changes: Vec<BlockChange> = self
            .blocks
            .iter()
            .map(|(rel, state)| {
                let c = turn(q, rel.map(|v| f64::from(v) + 0.5));
                BlockChange {
                    x: (pose.x + c[0]).floor() as i32,
                    y: (pose.y + c[1]).floor() as i32,
                    z: (pose.z + c[2]).floor() as i32,
                    state: state.clone(),
                }
            })
            .collect();
        for chunk in changes.chunks(4096) {
            world::set_blocks(&self.dimension, chunk)?;
        }
        structures::remove(self.id);
        Ok(self.offset.map(|o| o as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quarter_pitch_turns_up_to_south() {
        let s = Structure {
            dimension: String::new(),
            id: 0,
            origin: [0; 3],
            offset: [0.0; 3],
            blocks: Vec::new(),
            size: [1, 3, 3],
            rotation: [0.0, 90.0],
        };
        // Pitch is about +x: up (+y) turns to +z.
        let v = turn(s.quaternion(), [0.0, 1.0, 0.0]);
        assert!(
            v[0].abs() < 1e-9 && v[1].abs() < 1e-9 && (v[2] - 1.0).abs() < 1e-9,
            "{v:?}"
        );
        // The box's centre stays where it was.
        let p = s.pose();
        let centre = [0.5, 1.5, 1.5];
        let c = turn(s.quaternion(), centre);
        let back = [p.x + c[0], p.y + c[1], p.z + c[2]];
        for i in 0..3 {
            assert!((back[i] - centre[i]).abs() < 1e-9, "{back:?}");
        }
    }
}
