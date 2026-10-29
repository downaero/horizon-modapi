//! Connected textures, as Create's `CTModel` draws its casings and framed
//! glass (and OptiFine's or Fusion's CTM): a block's faces drawn with one
//! sprite take instead a cell of a sheet, chosen by which neighbours in the
//! face's plane are the same block. Declared per block in
//! `assets/<ns>/horizon/connected/*.json`; each sheet's cells become sprite
//! layers of their own, so the mesher swaps a face's layer and keeps its UVs.
use crate::{Result, sprites::Sprites};
use horizon_assets::model::Direction;
use horizon_world::registry::{BlockRegistry, BlockStateId};
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path};

/// Create's `AllCTTypes`: how many cells a sheet has across, and which one a
/// face's neighbours pick.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Horizontal,
    HorizontalKryppers,
    Vertical,
    Omnidirectional,
    Roof,
    RoofStair,
    Cross,
    Rectangle,
}

/// Which neighbours in a face's plane connect (`CTContext`): a corner only
/// with both its sides.
#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub top_left: bool,
    pub top_right: bool,
    pub bottom_left: bool,
    pub bottom_right: bool,
}

impl Kind {
    /// Cells across (and down) its sheet.
    pub fn sheet_size(self) -> u32 {
        match self {
            Self::Horizontal | Self::HorizontalKryppers | Self::Vertical => 2,
            Self::Omnidirectional => 8,
            Self::Roof | Self::RoofStair | Self::Cross | Self::Rectangle => 4,
        }
    }
    /// `getTextureIndex`: the cell, row by row.
    pub fn index(self, c: &Context) -> usize {
        let b = usize::from;
        match self {
            Self::Horizontal => b(c.right) + 2 * b(c.left),
            Self::HorizontalKryppers => match (c.left, c.right) {
                (false, false) => 0,
                (_, false) => 3,
                (false, _) => 2,
                _ => 1,
            },
            Self::Vertical => b(c.up) + 2 * b(c.down),
            Self::Omnidirectional => omnidirectional(c),
            Self::Roof => roof(c),
            Self::RoofStair => {
                const MAPPING: [[usize; 4]; 4] =
                    [[1, 6, 9, 4], [14, 12, 13, 15], [2, 10, 8, 0], [5, 5, 5, 5]];
                MAPPING[2 * b(c.up) + b(c.right)][2 * b(c.left) + b(c.down)]
            }
            Self::Cross => b(c.up) + 2 * b(c.down) + 4 * b(c.left) + 8 * b(c.right),
            Self::Rectangle => {
                let x = match (c.left, c.right) {
                    (true, true) => 2,
                    (true, false) => 3,
                    (false, true) => 1,
                    _ => 0,
                };
                let y = match (c.up, c.down) {
                    (true, true) => 1,
                    (true, false) => 2,
                    (false, true) => 0,
                    _ => 3,
                };
                x + y * 4
            }
        }
    }
}

fn omnidirectional(c: &Context) -> usize {
    let b = i32::from;
    let borders = b(!c.up) + b(!c.down) + b(!c.left) + b(!c.right);
    let mut x = b(c.up) + 2 * b(c.down);
    let mut y = b(c.left) + 2 * b(c.right);
    if borders == 0 {
        x += b(c.top_right) + 2 * b(c.top_left);
        y += 2 * b(c.bottom_right) + b(c.bottom_left);
    }
    if borders == 1 {
        if !c.right && (c.top_left || c.bottom_left) {
            y = 4;
            x = -1 + b(c.bottom_left) + 2 * b(c.top_left);
        }
        if !c.left && (c.top_right || c.bottom_right) {
            y = 5;
            x = -1 + b(c.bottom_right) + 2 * b(c.top_right);
        }
        if !c.down && (c.top_left || c.top_right) {
            y = 6;
            x = -1 + b(c.top_left) + 2 * b(c.top_right);
        }
        if !c.up && (c.bottom_left || c.bottom_right) {
            y = 7;
            x = -1 + b(c.bottom_left) + 2 * b(c.bottom_right);
        }
    }
    if borders == 2
        && ((c.up && c.left && c.top_left)
            || (c.down && c.left && c.bottom_left)
            || (c.up && c.right && c.top_right)
            || (c.down && c.right && c.bottom_right))
    {
        x += 3;
    }
    (x + 8 * y).max(0) as usize
}

fn roof(c: &Context) -> usize {
    let up_drops = c.down && !c.up && (c.left || c.right);
    let down_drops = !c.down && c.up && (c.left || c.right);
    let left_drops = !c.left && c.right && (c.up || c.down);
    let right_drops = c.left && !c.right && (c.up || c.down);
    if up_drops {
        return match (left_drops, right_drops) {
            (true, _) => {
                if c.bottom_right {
                    0
                } else {
                    5
                }
            }
            (_, true) => {
                if c.bottom_left {
                    2
                } else {
                    5
                }
            }
            _ => 1,
        };
    }
    if down_drops {
        return match (left_drops, right_drops) {
            (true, _) => {
                if c.top_right {
                    8
                } else {
                    5
                }
            }
            (_, true) => {
                if c.top_left {
                    10
                } else {
                    5
                }
            }
            _ => 9,
        };
    }
    if left_drops {
        return 4;
    }
    if right_drops {
        return 6;
    }
    if !c.up || !c.down || !c.left || !c.right {
        return 5;
    }
    if c.bottom_left && c.top_right {
        match (c.top_left, c.bottom_right) {
            (true, false) => return 12,
            (false, true) => return 15,
            (false, false) => return 7,
            _ => {}
        }
    }
    if c.bottom_right && c.top_left {
        match (c.top_right, c.bottom_left) {
            (true, false) => return 13,
            (false, true) => return 14,
            (false, false) => return 11,
            _ => {}
        }
    }
    5
}

/// A face's right and up in the world, as Create's `buildContext` turns
/// them (by `Direction`, down to east).
const AXES: [([i32; 3], [i32; 3]); 6] = [
    ([1, 0, 0], [0, 0, 1]),
    ([1, 0, 0], [0, 0, -1]),
    ([-1, 0, 0], [0, 1, 0]),
    ([1, 0, 0], [0, 1, 0]),
    ([0, 0, 1], [0, 1, 0]),
    ([0, 0, -1], [0, 1, 0]),
];
const NORMALS: [[i32; 3]; 6] = [
    [0, -1, 0],
    [0, 1, 0],
    [0, 0, -1],
    [0, 0, 1],
    [-1, 0, 0],
    [1, 0, 0],
];

/// One sprite of a block drawn from a sheet's cells.
#[derive(Clone, Debug)]
pub struct Rule {
    /// The layer the faces it changes are drawn with.
    pub sprite: u32,
    pub kind: Kind,
    /// Each cell's layer, and whether all its texels are opaque.
    pub cells: Vec<(u32, bool)>,
    /// The faces it changes, a bit per `Direction`.
    pub faces: u8,
    /// The block's first and last state: a neighbour connects when it is
    /// one of them.
    pub block: (u32, u32),
}

impl Rule {
    /// The cell a face facing `facing` drawn with `layer` takes, and whether
    /// it is opaque, `at` reading the state at an offset from the block;
    /// none when the rule does not change it. A neighbour connects as
    /// Create's `connectsTo` says: the same block, not covered on the face's
    /// side by the same block again.
    pub fn cell(
        &self,
        layer: u32,
        facing: Direction,
        at: impl Fn([i32; 3]) -> u32,
    ) -> Option<(u32, bool)> {
        let d = facing as usize;
        if layer != self.sprite || self.faces & (1 << d) == 0 {
            return None;
        }
        let ((h, v), n) = (AXES[d], NORMALS[d]);
        let same = |p: [i32; 3]| (self.block.0..=self.block.1).contains(&at(p));
        let connects = |sh: i32, sv: i32| {
            let p = [0, 1, 2].map(|k| h[k] * sh + v[k] * sv);
            same(p) && !same([0, 1, 2].map(|k| p[k] + n[k]))
        };
        let mut c = Context {
            up: connects(0, 1),
            down: connects(0, -1),
            left: connects(-1, 0),
            right: connects(1, 0),
            ..Context::default()
        };
        c.top_left = c.up && c.left && connects(-1, 1);
        c.top_right = c.up && c.right && connects(1, 1);
        c.bottom_left = c.down && c.left && connects(-1, -1);
        c.bottom_right = c.down && c.right && connects(1, -1);
        self.cells.get(self.kind.index(&c)).copied()
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    blocks: Vec<String>,
    sprite: String,
    sheet: String,
    #[serde(rename = "type")]
    kind: Kind,
    faces: Option<Vec<String>>,
    /// Only states with these properties.
    #[serde(default)]
    when: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum Files {
    One(File),
    Many(Vec<File>),
}

/// Every block's connected textures, by state.
#[derive(Default)]
pub struct Connected {
    /// By state id: its rules in `rules` plus one, 0 for none.
    by_state: Vec<u32>,
    rules: Vec<Vec<Rule>>,
}

impl Connected {
    /// The rules of every namespace's `horizon/connected/` files, their
    /// sheets cut into `sprites` layers. A file that does not read, or a
    /// sheet that does not load, is logged and left out.
    pub fn load(root: &Path, registry: &BlockRegistry, sprites: &mut Sprites) -> Self {
        let mut out = Self::default();
        let Ok(namespaces) = fs::read_dir(root.join("assets")) else {
            return out;
        };
        let mut files = Vec::new();
        for ns in namespaces.flatten() {
            let dir = ns.path().join("horizon/connected");
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            files.extend(
                entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|e| e == "json")),
            );
        }
        files.sort();
        if files.is_empty() {
            return out;
        }
        let mut by_name: BTreeMap<&str, Vec<BlockStateId>> = BTreeMap::new();
        for i in 0..registry.len() {
            let id = BlockStateId(i as u32);
            if let Some(s) = registry.state(id) {
                by_name.entry(s.name.as_str()).or_default().push(id);
            }
        }
        let mut per_state: BTreeMap<u32, Vec<Rule>> = BTreeMap::new();
        for path in files {
            let read = fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Files>(&t).map_err(|e| e.to_string()));
            let entries = match read {
                Ok(Files::One(f)) => vec![f],
                Ok(Files::Many(f)) => f,
                Err(e) => {
                    log::warn!("Horizon: connected textures {}: {e}", path.display());
                    continue;
                }
            };
            for f in entries {
                if let Err(e) = out.add(root, registry, sprites, &by_name, &mut per_state, &f) {
                    log::warn!("Horizon: connected textures {}: {e}", path.display());
                }
            }
        }
        out.by_state = vec![0; registry.len()];
        for (state, rules) in per_state {
            out.rules.push(rules);
            out.by_state[state as usize] = out.rules.len() as u32;
        }
        out
    }
    fn add(
        &self,
        root: &Path,
        registry: &BlockRegistry,
        sprites: &mut Sprites,
        by_name: &BTreeMap<&str, Vec<BlockStateId>>,
        per_state: &mut BTreeMap<u32, Vec<Rule>>,
        f: &File,
    ) -> Result<()> {
        // Only a sprite some model draws can be swapped.
        let Some(&sprite) = sprites.layers.get(&f.sprite) else {
            return Err(format!("no model draws {}", f.sprite).into());
        };
        let n = f.kind.sheet_size();
        let layers = sprites.insert_cells(root, &f.sheet, n)?;
        let cells = layers
            .into_iter()
            .map(|l| (l, sprites.opaque[l as usize]))
            .collect();
        let faces = match &f.faces {
            None => 0b11_1111,
            Some(names) => names.iter().try_fold(0u8, |bits, name| {
                let d = Direction::ALL
                    .into_iter()
                    .position(|d| d.name() == name)
                    .ok_or_else(|| format!("{name:?} is not a face"))?;
                Ok::<_, String>(bits | 1 << d)
            })?,
        };
        let mut rule = Rule {
            sprite,
            kind: f.kind,
            cells,
            faces,
            block: (0, 0),
        };
        for block in &f.blocks {
            let Some(states) = by_name.get(block.as_str()) else {
                return Err(format!("{block} is not a block").into());
            };
            let (first, last) = (states[0].0, states[states.len() - 1].0);
            rule.block = (first, last);
            for &id in states {
                let props = &registry.state(id).expect("indexed state").properties;
                if f.when.iter().all(|(k, v)| props.get(k) == Some(v)) {
                    per_state.entry(id.0).or_default().push(rule.clone());
                }
            }
        }
        Ok(())
    }
    /// The rules of a state's faces; empty for most.
    pub fn rules(&self, state: u32) -> &[Rule] {
        match self.by_state.get(state as usize) {
            Some(&i) if i > 0 => &self.rules[i as usize - 1],
            _ => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(up: bool, down: bool, left: bool, right: bool) -> Context {
        Context {
            up,
            down,
            left,
            right,
            ..Context::default()
        }
    }

    /// Create's indices: alone, joined on every side with its corners, and
    /// the axis-aligned kinds.
    #[test]
    fn indices_follow_creates_sheets() {
        let alone = Context::default();
        assert_eq!(Kind::Omnidirectional.index(&alone), 0);
        let mut all = ctx(true, true, true, true);
        assert_eq!(Kind::Omnidirectional.index(&all), 3 + 8 * 3);
        all.top_left = true;
        all.top_right = true;
        all.bottom_left = true;
        all.bottom_right = true;
        assert_eq!(Kind::Omnidirectional.index(&all), 6 + 8 * 6);
        // Joined above only: the column's top cell.
        assert_eq!(
            Kind::Omnidirectional.index(&ctx(true, false, false, false)),
            1
        );
        assert_eq!(Kind::Rectangle.index(&alone), 3 * 4);
        assert_eq!(Kind::Rectangle.index(&ctx(true, true, true, true)), 2 + 4);
        assert_eq!(Kind::Vertical.index(&ctx(true, true, false, false)), 3);
        assert_eq!(Kind::Horizontal.index(&ctx(false, false, true, false)), 2);
        assert_eq!(
            Kind::HorizontalKryppers.index(&ctx(false, false, true, false)),
            3
        );
        assert_eq!(Kind::Cross.index(&ctx(false, false, false, true)), 8);
    }

    /// Three of a block in a row along x: the middle one's north face is
    /// joined left and right, its top face too, its east face not at all.
    #[test]
    fn neighbours_in_the_faces_plane_connect() {
        let rule = Rule {
            sprite: 7,
            kind: Kind::Cross,
            cells: (0..16).map(|i| (100 + i, true)).collect(),
            faces: 0b11_1111,
            block: (5, 6),
        };
        let row = |p: [i32; 3]| {
            if p[1] == 0 && p[2] == 0 && p[0].abs() <= 1 {
                5
            } else {
                0
            }
        };
        // North: right is west, so both sides join: up 0, down 0, left 4, right 8.
        assert_eq!(rule.cell(7, Direction::North, row), Some((112, true)));
        assert_eq!(rule.cell(7, Direction::Up, row), Some((112, true)));
        assert_eq!(rule.cell(7, Direction::East, row), Some((100, true)));
        // Another sprite is left as it is.
        assert_eq!(rule.cell(8, Direction::North, row), None);
        // A neighbour covered on the face's side by the block is no join.
        let covered = |p: [i32; 3]| match p {
            [1, 0, 0] | [1, 1, 0] => 5,
            _ => 0,
        };
        assert_eq!(rule.cell(7, Direction::Up, covered), Some((100, true)));
    }
}
