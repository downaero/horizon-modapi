//! A dimension of the server's world for server mods, as NeoForge's `Level`
//! is to its mods: block states at positions, the signal reaching them,
//! sounds and level events there. Every call goes to the game as the
//! interfaces under it do (`world`, `redstone`, `effects`, `players`), so a
//! mod declares the capabilities of the methods it uses; changes are made
//! after the call, as `world.set-blocks` makes them.
use crate::block::BlockEntity;
use crate::server::{
    BlockChange, BlockPos, Player, Pos, block_entities, effects, players, redstone, world,
};
use std::fmt;

/// One of a block's six faces, in the order the game numbers them (0 down,
/// 1 up, 2 north, 3 south, 4 west, 5 east), as NeoForge's `Direction`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Direction {
    Down,
    Up,
    North,
    South,
    West,
    East,
}

/// The axis a direction runs along.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Axis {
    X,
    Y,
    Z,
}

impl Axis {
    /// `x`, `y` or `z`, as an `axis` property writes it.
    pub fn get_name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Y => "y",
            Axis::Z => "z",
        }
    }
    pub fn by_name(name: &str) -> Option<Self> {
        match name {
            "x" => Some(Axis::X),
            "y" => Some(Axis::Y),
            "z" => Some(Axis::Z),
            _ => None,
        }
    }
}

impl Direction {
    pub const ALL: [Direction; 6] = [
        Direction::Down,
        Direction::Up,
        Direction::North,
        Direction::South,
        Direction::West,
        Direction::East,
    ];
    /// The face numbered as `item-events` and `world.use-item` number them.
    pub fn from_3d_data_value(face: u8) -> Option<Self> {
        Self::ALL.get(usize::from(face)).copied()
    }
    pub fn get_3d_data_value(self) -> u8 {
        self as u8
    }
    /// `down`, `up`, `north`, ..., as a `facing` property writes it.
    pub fn get_name(self) -> &'static str {
        match self {
            Direction::Down => "down",
            Direction::Up => "up",
            Direction::North => "north",
            Direction::South => "south",
            Direction::West => "west",
            Direction::East => "east",
        }
    }
    pub fn by_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.get_name() == name)
    }
    pub fn get_opposite(self) -> Self {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }
    pub fn get_axis(self) -> Axis {
        match self {
            Direction::Down | Direction::Up => Axis::Y,
            Direction::North | Direction::South => Axis::Z,
            Direction::West | Direction::East => Axis::X,
        }
    }
    /// The horizontal way a yaw (degrees, as F3 shows it) faces, as
    /// NeoForge's `Direction.fromYRot`: 0 south, 90 west, 180 north, 270 east.
    pub fn from_y_rot(yaw: f32) -> Self {
        let quarter = ((f64::from(yaw) / 90.0 + 0.5).floor() as i64).rem_euclid(4);
        [
            Direction::South,
            Direction::West,
            Direction::North,
            Direction::East,
        ][quarter as usize]
    }
    /// The way a player with this view looks most nearly, as NeoForge's
    /// `getNearestLookingDirection`: up or down past 45 degrees of pitch,
    /// else the horizontal way.
    pub fn nearest_looking(yaw: f32, pitch: f32) -> Self {
        if pitch < -45.0 {
            Direction::Up
        } else if pitch > 45.0 {
            Direction::Down
        } else {
            Self::from_y_rot(yaw)
        }
    }
    /// The step one block this way: north is -z, east +x.
    pub fn get_normal(self) -> [i32; 3] {
        match self {
            Direction::Down => [0, -1, 0],
            Direction::Up => [0, 1, 0],
            Direction::North => [0, 0, -1],
            Direction::South => [0, 0, 1],
            Direction::West => [-1, 0, 0],
            Direction::East => [1, 0, 0],
        }
    }
}

impl BlockPos {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
    pub fn offset(self, dx: i32, dy: i32, dz: i32) -> Self {
        Self::new(self.x + dx, self.y + dy, self.z + dz)
    }
    /// The next position `n` blocks towards `direction`.
    pub fn relative(self, direction: Direction, n: i32) -> Self {
        let [dx, dy, dz] = direction.get_normal();
        self.offset(dx * n, dy * n, dz * n)
    }
    pub fn above(self) -> Self {
        self.offset(0, 1, 0)
    }
    pub fn below(self) -> Self {
        self.offset(0, -1, 0)
    }
    pub fn north(self) -> Self {
        self.offset(0, 0, -1)
    }
    pub fn south(self) -> Self {
        self.offset(0, 0, 1)
    }
    pub fn west(self) -> Self {
        self.offset(-1, 0, 0)
    }
    pub fn east(self) -> Self {
        self.offset(1, 0, 0)
    }
}

impl From<Pos> for BlockPos {
    fn from(p: Pos) -> Self {
        Self::new(p.x, p.y, p.z)
    }
}

impl PartialEq for BlockPos {
    fn eq(&self, other: &Self) -> bool {
        [self.x, self.y, self.z] == [other.x, other.y, other.z]
    }
}

/// A block and its properties, `minecraft:oak_log[axis=y]` as the game
/// writes it, as NeoForge's `BlockState`. Properties left out are the
/// block's defaults when the game reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BlockState {
    block: String,
    properties: Vec<(String, String)>,
}

impl BlockState {
    /// Reads `mymod:gear[axis=x,powered=false]`; a block alone has no
    /// properties set.
    pub fn parse(state: &str) -> Self {
        let (block, rest) = state.split_once('[').unwrap_or((state, ""));
        let properties = rest
            .trim_end_matches(']')
            .split(',')
            .filter_map(|p| p.split_once('='))
            .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
            .collect();
        Self {
            block: block.trim().to_owned(),
            properties,
        }
    }
    pub fn air() -> Self {
        Self::parse("minecraft:air")
    }
    /// The block's id, `mymod:gear`, with no properties.
    pub fn get_block(&self) -> &str {
        &self.block
    }
    pub fn is(&self, block: &str) -> bool {
        self.block == block
    }
    pub fn is_air(&self) -> bool {
        matches!(
            self.block.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
    pub fn has_property(&self, property: &str) -> bool {
        self.get_value(property).is_some()
    }
    /// A property's value as written: `x` for `axis`, `true` for `powered`.
    pub fn get_value(&self, property: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(k, _)| k == property)
            .map(|(_, v)| v.as_str())
    }
    /// The state with `property` set to `value`, added when it was left out.
    pub fn set_value(mut self, property: &str, value: &str) -> Self {
        match self.properties.iter_mut().find(|(k, _)| k == property) {
            Some((_, v)) => *v = value.to_owned(),
            None => self
                .properties
                .push((property.to_owned(), value.to_owned())),
        }
        self
    }
    /// `axis`, as `x`, `y` or `z`.
    pub fn get_axis(&self) -> Option<Axis> {
        self.get_value("axis").and_then(Axis::by_name)
    }
    /// `facing`, the way the block faces.
    pub fn get_facing(&self) -> Option<Direction> {
        self.get_value("facing").and_then(Direction::by_name)
    }
}

impl fmt::Display for BlockState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.block)?;
        if self.properties.is_empty() {
            return Ok(());
        }
        f.write_str("[")?;
        for (i, (k, v)) in self.properties.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{k}={v}")?;
        }
        f.write_str("]")
    }
}

/// One dimension (`minecraft:overworld`, ...) of the server's world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    dimension: String,
}

impl Level {
    pub fn new(dimension: &str) -> Self {
        Self {
            dimension: dimension.to_owned(),
        }
    }
    pub fn dimension(&self) -> &str {
        &self.dimension
    }
    /// Whether the server has the chunk at `pos` loaded (capability `world`).
    pub fn is_loaded(&self, pos: BlockPos) -> bool {
        world::get_block(&self.dimension, pos.x, pos.y, pos.z).is_some()
    }
    /// The state at `pos` as of this call, air where no chunk is loaded
    /// (capability `world`).
    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        world::get_block(&self.dimension, pos.x, pos.y, pos.z)
            .map_or_else(BlockState::air, |s| BlockState::parse(&s))
    }
    /// Sets the state at `pos` as `/setblock` does, after the call
    /// (capability `world`); a read in the same call still sees the old one.
    pub fn set_block(&self, pos: BlockPos, state: &BlockState) -> Result<(), String> {
        world::set_blocks(
            &self.dimension,
            &[BlockChange {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                state: state.to_string(),
            }],
        )
    }
    /// Air at `pos`, after the call (capability `world`).
    pub fn remove_block(&self, pos: BlockPos) -> Result<(), String> {
        self.set_block(pos, &BlockState::air())
    }
    /// Breaks the block at `pos` with its particles and sound, after the
    /// call: with `drop`, its drops as a bare hand gets them, else none
    /// (capability `world`).
    pub fn destroy_block(&self, pos: BlockPos, drop: bool) -> Result<(), String> {
        if drop {
            world::break_block(&self.dimension, pos.x, pos.y, pos.z, None, &[], None)
        } else {
            self.remove_block(pos)
        }
    }
    /// The biome at `pos`, `minecraft:plains`, ... (capability `world`).
    pub fn get_biome(&self, pos: BlockPos) -> Option<String> {
        world::get_biome(&self.dimension, pos.x, pos.y, pos.z)
    }
    /// The strongest signal (0 to 15) reaching `pos` from its neighbours, as
    /// of the start of this tick; 0 where the game does not tell it: only
    /// positions where the mod keeps data or has a container (capability
    /// `redstone`).
    pub fn get_best_neighbor_signal(&self, pos: BlockPos) -> u8 {
        redstone::power(&self.dimension, pos.x, pos.y, pos.z).unwrap_or(0)
    }
    pub fn has_neighbor_signal(&self, pos: BlockPos) -> bool {
        self.get_best_neighbor_signal(pos) > 0
    }
    /// Plays a sound event at `pos` for every player near it, after the
    /// call (capability `effects`).
    pub fn play_sound(
        &self,
        pos: BlockPos,
        sound: &str,
        volume: f32,
        pitch: f32,
    ) -> Result<(), String> {
        effects::play_sound(&self.dimension, pos.x, pos.y, pos.z, sound, volume, pitch)
    }
    /// Vanilla's `levelEvent` at `pos`, those `effects.level-event` takes
    /// (capability `effects`).
    pub fn level_event(&self, id: i32, pos: BlockPos, data: i32) -> Result<(), String> {
        effects::level_event(&self.dimension, pos.x, pos.y, pos.z, id, data)
    }
    /// The players online in this dimension (capability `players`).
    pub fn players(&self) -> Vec<Player> {
        players::online()
            .into_iter()
            .filter(|p| p.dimension == self.dimension)
            .collect()
    }
    /// The block entity at `pos`, read from what the mod keeps there
    /// (capability `block-entities`): none where it keeps nothing.
    pub fn get_block_entity<T: BlockEntity>(&self, pos: BlockPos) -> Option<T> {
        let data = block_entities::get(&self.dimension, pos.x, pos.y, pos.z)?;
        let mut entity = T::new(pos, &self.get_block_state(pos));
        // Empty data: a block given data as it appeared, still as `new` made it.
        if !data.is_empty() {
            entity.load_additional(&data);
        }
        Some(entity)
    }
    /// Keeps `entity` at `pos` (capability `block-entities`), synced to
    /// players when its type is ([`BlockEntity::SYNCED`]): refused where the
    /// block there is not one of the mod's.
    pub fn set_block_entity<T: BlockEntity>(
        &self,
        pos: BlockPos,
        entity: &T,
    ) -> Result<(), String> {
        let mut data = Vec::new();
        entity.save_additional(&mut data);
        if T::SYNCED {
            block_entities::set_synced(&self.dimension, pos.x, pos.y, pos.z, &data)
        } else {
            block_entities::set(&self.dimension, pos.x, pos.y, pos.z, &data)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_read_and_write_as_the_game_does() {
        let s = BlockState::parse("mymod:gear[axis=x,powered=false]");
        assert_eq!(s.get_block(), "mymod:gear");
        assert_eq!(s.get_axis(), Some(Axis::X));
        assert_eq!(s.get_value("powered"), Some("false"));
        let s = s.set_value("powered", "true").set_value("facing", "north");
        assert_eq!(
            s.to_string(),
            "mymod:gear[axis=x,powered=true,facing=north]"
        );
        assert_eq!(s.get_facing(), Some(Direction::North));
        assert_eq!(
            BlockState::parse("minecraft:stone").to_string(),
            "minecraft:stone"
        );
        assert!(BlockState::air().is_air());
    }

    #[test]
    fn directions_step_and_turn_round() {
        let p = BlockPos::new(1, 2, 3);
        assert_eq!(p.relative(Direction::East, 2), BlockPos::new(3, 2, 3));
        assert_eq!(p.north(), BlockPos::new(1, 2, 2));
        assert_eq!(Direction::from_y_rot(0.0), Direction::South);
        assert_eq!(Direction::from_y_rot(-90.0), Direction::East);
        assert_eq!(Direction::from_y_rot(181.0), Direction::North);
        assert_eq!(Direction::from_y_rot(-46.0), Direction::East);
        assert_eq!(Direction::nearest_looking(0.0, 60.0), Direction::Down);
        for d in Direction::ALL {
            assert_eq!(d.get_opposite().get_opposite(), d);
            assert_eq!(
                Direction::from_3d_data_value(d.get_3d_data_value()),
                Some(d)
            );
            assert_eq!(Direction::by_name(d.get_name()), Some(d));
            assert_eq!(d.get_axis(), d.get_opposite().get_axis());
        }
    }
}
