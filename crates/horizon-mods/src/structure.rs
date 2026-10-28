//! Capability `entities`' structures: blocks a server mod takes out of the
//! grid as one object, moved and turned as a whole, drawn by players' games
//! as one model. This is the form both ends share: the blocks, the pose and
//! the payloads that carry them.

/// The most blocks in one structure.
pub const BLOCKS: usize = 4096;
/// The most structures one mod has at once.
pub const PER_MOD: usize = 64;

/// Where a structure is: its local (0, 0, 0) corner in the world, and its
/// turn about that corner as a unit quaternion (x, y, z, w).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub pos: [f64; 3],
    pub rotation: [f32; 4],
}

impl Pose {
    /// Checks a mod's pose: finite, in the world's bounds, a quaternion
    /// that is not zero; it comes back with the quaternion made unit.
    pub fn checked(self) -> Result<Self, String> {
        if !self.pos.iter().all(|v| v.is_finite() && v.abs() < 3.0e7) {
            return Err("a position within the world".into());
        }
        let r = self.rotation;
        let len = r.iter().map(|v| v * v).sum::<f32>().sqrt();
        if !len.is_finite() || len < 1e-6 {
            return Err("a rotation quaternion that is not zero".into());
        }
        Ok(Self {
            pos: self.pos,
            rotation: r.map(|v| v / len),
        })
    }
    /// Where a local point is in the world.
    pub fn apply(&self, local: [f64; 3]) -> [f64; 3] {
        let [x, y, z, w] = self.rotation.map(f64::from);
        let v = local;
        // v + 2w (q × v) + 2 q × (q × v)
        let t = [
            2.0 * (y * v[2] - z * v[1]),
            2.0 * (z * v[0] - x * v[2]),
            2.0 * (x * v[1] - y * v[0]),
        ];
        let c = [
            y * t[2] - z * t[1],
            z * t[0] - x * t[2],
            x * t[1] - y * t[0],
        ];
        [0, 1, 2].map(|i| self.pos[i] + v[i] + w * t[i] + c[i])
    }
}

/// One block of a structure, at its local position.
#[derive(Clone, Debug, PartialEq)]
pub struct StructureBlock {
    pub pos: [i32; 3],
    pub state: String,
}

/// Where each block lands when a structure is put back into the grid: its
/// turn goes to the nearest quarter turns about the middle of its blocks,
/// which stays where it is, so every block gets a place of its own (a
/// structure stopped halfway through a turn would otherwise put two blocks
/// in one place).
pub fn settle(blocks: &[StructureBlock], pose: &Pose) -> Vec<([i32; 3], String)> {
    if blocks.is_empty() {
        return Vec::new();
    }
    let n = blocks.len() as f64;
    let middle = [0, 1, 2].map(|i| {
        blocks
            .iter()
            .map(|b| f64::from(b.pos[i]) + 0.5)
            .sum::<f64>()
            / n
    });
    let at = pose.apply(middle);
    let origin = pose.apply([0.0; 3]);
    let turned = |v: [f64; 3]| {
        let p = pose.apply(v);
        [0, 1, 2].map(|i| p[i] - origin[i])
    };
    // Each local axis to the world axis nearest it, x first, then y among
    // the others; z follows from them (a turn, never a mirror).
    let nearest = |v: [f64; 3], taken: Option<usize>| -> [i32; 3] {
        let axis = (0..3)
            .filter(|i| Some(*i) != taken)
            .max_by(|a, b| v[*a].abs().total_cmp(&v[*b].abs()))
            .expect("an axis");
        let mut out = [0; 3];
        out[axis] = if v[axis] < 0.0 { -1 } else { 1 };
        out
    };
    let x = nearest(turned([1.0, 0.0, 0.0]), None);
    let y = nearest(turned([0.0, 1.0, 0.0]), x.iter().position(|v| *v != 0));
    let z = [
        x[1] * y[2] - x[2] * y[1],
        x[2] * y[0] - x[0] * y[2],
        x[0] * y[1] - x[1] * y[0],
    ];
    blocks
        .iter()
        .map(|b| {
            let d = [0, 1, 2].map(|i| f64::from(b.pos[i]) + 0.5 - middle[i]);
            let c = [0, 1, 2].map(|i| {
                at[i] + f64::from(x[i]) * d[0] + f64::from(y[i]) * d[1] + f64::from(z[i]) * d[2]
            });
            (c.map(|v| v.floor() as i32), b.state.clone())
        })
        .collect()
}

/// A structure payload for a client.
#[derive(Clone, Debug, PartialEq)]
pub enum Payload {
    /// It appeared (or the player's game first sees it): its blocks and pose.
    Create {
        mod_id: String,
        id: u32,
        blocks: Vec<StructureBlock>,
        pose: Pose,
    },
    /// It moved.
    Pose { mod_id: String, id: u32, pose: Pose },
    /// It is gone.
    Remove { mod_id: String, id: u32 },
}

fn put_str(out: &mut Vec<u8>, s: &str) {
    out.extend((s.len() as u16).to_le_bytes());
    out.extend(s.as_bytes());
}
fn put_pose(out: &mut Vec<u8>, p: &Pose) {
    for v in p.pos {
        out.extend(v.to_le_bytes());
    }
    for v in p.rotation {
        out.extend(v.to_le_bytes());
    }
}

impl Payload {
    /// A kind byte (0 create, 1 pose, 2 remove), the mod id (u16 length and
    /// bytes) and the structure's id (u32); a create then has its palette
    /// (u16 count, each state as the id), its blocks (u32 count, each x y z
    /// i32 and a u16 palette index) and the pose; a pose the pose (x y z
    /// f64, the quaternion f32), little-endian.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let (kind, mod_id, id) = match self {
            Payload::Create { mod_id, id, .. } => (0, mod_id, id),
            Payload::Pose { mod_id, id, .. } => (1, mod_id, id),
            Payload::Remove { mod_id, id } => (2, mod_id, id),
        };
        out.push(kind);
        put_str(&mut out, mod_id);
        out.extend(id.to_le_bytes());
        match self {
            Payload::Create { blocks, pose, .. } => {
                let mut palette: Vec<&str> = Vec::new();
                let index: Vec<u16> = blocks
                    .iter()
                    .map(|b| {
                        let at = palette
                            .iter()
                            .position(|s| *s == b.state)
                            .unwrap_or_else(|| {
                                palette.push(&b.state);
                                palette.len() - 1
                            });
                        at as u16
                    })
                    .collect();
                out.extend((palette.len() as u16).to_le_bytes());
                for s in &palette {
                    put_str(&mut out, s);
                }
                out.extend((blocks.len() as u32).to_le_bytes());
                for (b, i) in blocks.iter().zip(index) {
                    for v in b.pos {
                        out.extend(v.to_le_bytes());
                    }
                    out.extend(i.to_le_bytes());
                }
                put_pose(&mut out, pose);
            }
            Payload::Pose { pose, .. } => put_pose(&mut out, pose),
            Payload::Remove { .. } => {}
        }
        out
    }

    /// [`Payload::encode`]'s inverse; none for damaged bytes.
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let mut c = Cursor(bytes);
        let kind = c.take(1)?[0];
        let mod_id = c.string()?;
        let id = u32::from_le_bytes(c.array()?);
        let out = match kind {
            0 => {
                let count = u16::from_le_bytes(c.array()?) as usize;
                let palette = (0..count).map(|_| c.string()).collect::<Option<Vec<_>>>()?;
                let count = u32::from_le_bytes(c.array()?) as usize;
                if count > BLOCKS {
                    return None;
                }
                let mut blocks = Vec::with_capacity(count);
                for _ in 0..count {
                    let pos = [(); 3].map(|_| c.array().map(i32::from_le_bytes));
                    let i = u16::from_le_bytes(c.array()?) as usize;
                    blocks.push(StructureBlock {
                        pos: [pos[0]?, pos[1]?, pos[2]?],
                        state: palette.get(i)?.clone(),
                    });
                }
                Payload::Create {
                    mod_id,
                    id,
                    blocks,
                    pose: c.pose()?,
                }
            }
            1 => Payload::Pose {
                mod_id,
                id,
                pose: c.pose()?,
            },
            2 => Payload::Remove { mod_id, id },
            _ => return None,
        };
        c.0.is_empty().then_some(out)
    }
}

/// Reads [`Payload::encode`]'s bytes from the front.
struct Cursor<'a>(&'a [u8]);

impl Cursor<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let (head, rest) = self.0.split_at_checked(n)?;
        self.0 = rest;
        Some(head)
    }
    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        self.take(N)?.try_into().ok()
    }
    fn string(&mut self) -> Option<String> {
        let n = u16::from_le_bytes(self.array()?) as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    fn pose(&mut self) -> Option<Pose> {
        let pos = [(); 3].map(|_| self.array().map(f64::from_le_bytes));
        let rotation = [(); 4].map(|_| self.array().map(f32::from_le_bytes));
        Pose {
            pos: [pos[0]?, pos[1]?, pos[2]?],
            rotation: [rotation[0]?, rotation[1]?, rotation[2]?, rotation[3]?],
        }
        .checked()
        .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quarter_turn_about_y() -> [f32; 4] {
        let h = std::f32::consts::FRAC_PI_4;
        [0.0, h.sin(), 0.0, h.cos()]
    }

    #[test]
    fn payloads_round_trip_and_damage_is_refused() {
        let pose = Pose {
            pos: [10.5, 64.0, -3.25],
            rotation: quarter_turn_about_y(),
        }
        .checked()
        .unwrap();
        let blocks = vec![
            StructureBlock {
                pos: [0, 0, 0],
                state: "minecraft:oak_planks".into(),
            },
            StructureBlock {
                pos: [1, 0, 0],
                state: "minecraft:oak_planks".into(),
            },
            StructureBlock {
                pos: [0, 1, 0],
                state: "gears:shaft[axis=y]".into(),
            },
        ];
        for p in [
            Payload::Create {
                mod_id: "gears".into(),
                id: 7,
                blocks: blocks.clone(),
                pose,
            },
            Payload::Pose {
                mod_id: "gears".into(),
                id: 7,
                pose,
            },
            Payload::Remove {
                mod_id: "gears".into(),
                id: 7,
            },
        ] {
            let bytes = p.encode();
            let back = Payload::decode(&bytes).unwrap();
            assert_eq!(back.encode(), bytes);
            assert_eq!(Payload::decode(&bytes[..bytes.len() - 1]), None);
            assert_eq!(Payload::decode(&[bytes.clone(), vec![0]].concat()), None);
        }
    }

    #[test]
    fn a_quarter_turn_settles_blocks_on_the_grid() {
        let pose = Pose {
            pos: [0.0; 3],
            rotation: quarter_turn_about_y(),
        }
        .checked()
        .unwrap();
        // +x turns to -z about y.
        let p = pose.apply([1.0, 0.0, 0.0]);
        assert!((p[0]).abs() < 1e-6 && (p[2] + 1.0).abs() < 1e-6, "{p:?}");
        let blocks = [StructureBlock {
            pos: [2, 0, 0],
            state: "minecraft:stone".into(),
        }];
        assert_eq!(
            settle(&blocks, &pose),
            [([0, 0, -3], "minecraft:stone".into())]
        );
        // A 3 x 3 wheel stopped at an eighth of a turn about x: nine blocks,
        // nine places.
        let e = std::f32::consts::PI / 8.0;
        let pose = Pose {
            pos: [10.0, 64.0, 10.0],
            rotation: [e.sin(), 0.0, 0.0, e.cos()],
        }
        .checked()
        .unwrap();
        let wheel: Vec<StructureBlock> = (0..3)
            .flat_map(|y| {
                (0..3).map(move |z| StructureBlock {
                    pos: [0, y, z],
                    state: "minecraft:oak_planks".into(),
                })
            })
            .collect();
        let mut places: Vec<[i32; 3]> = settle(&wheel, &pose).into_iter().map(|(p, _)| p).collect();
        places.sort();
        places.dedup();
        assert_eq!(places.len(), 9);
        assert!(
            Pose {
                pos: [0.0; 3],
                rotation: [0.0; 4]
            }
            .checked()
            .is_err()
        );
    }
}
