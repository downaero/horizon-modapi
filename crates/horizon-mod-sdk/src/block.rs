//! A server mod's blocks as types, as NeoForge's `Block` and `BlockEntity`
//! are: a [`Block`] has its own clicks and neighbour changes, a
//! [`BlockEntity`] its own fields, tick and save. A mod registers them at
//! `init` with [`register`] and [`register_entity`];
//! [`export_server_mod!`](crate::export_server_mod) then calls them for
//! their blocks before the mod's own hooks, which keep hearing every other
//! block (and a click a block passes on).
//!
//! A block entity is made when a player places its block, and is kept as
//! the bytes `block-entities` holds at its position (capability
//! `block-entities`): the game ticks it and tells it of neighbour changes
//! only where those bytes are. A block placed by the mod's own
//! [`Level::set_block`] gets its entity from [`Level::set_block_entity`] in
//! a later call, once the block stands.
use crate::level::{Axis, BlockState, Direction, Level};
use crate::server::{BlockPos, BlockTarget, Player, Pos, block_entities, items, visuals};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// What a click did, as NeoForge's `InteractionResult`: anything but
/// `Pass` takes the click, and nothing else happens with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InteractionResult {
    Success,
    Consume,
    Fail,
    Pass,
}

impl InteractionResult {
    pub fn consumes_action(self) -> bool {
        self != InteractionResult::Pass
    }
}

/// Where a player's click hit the block, and the face clicked.
#[derive(Clone, Copy, Debug)]
pub struct BlockHitResult {
    pub pos: BlockPos,
    pub direction: Option<Direction>,
}

/// A block of the mod's own (`mymod:gear`, declared in
/// `data/<id>/horizon/block/`). Players are given by UUID, as `players`
/// gives them.
pub trait Block: 'static {
    /// A player right-clicked it with an empty hand.
    fn use_without_item(
        &self,
        _state: &BlockState,
        _level: &Level,
        _pos: BlockPos,
        _player: &str,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        InteractionResult::Pass
    }
    /// A player right-clicked it holding `item`, before the item's own use
    /// (capability `block-entities`, as `item-events`).
    fn use_item_on(
        &self,
        _item: &str,
        _state: &BlockState,
        _level: &Level,
        _pos: BlockPos,
        _player: &str,
        _hit: &BlockHitResult,
    ) -> InteractionResult {
        InteractionResult::Pass
    }
    /// A player placed it, its block entity already made.
    fn set_placed_by(&self, _level: &Level, _pos: BlockPos, _state: &BlockState, _player: &Player) {
    }
    /// A player broke it: `state` is what stood there.
    fn player_will_destroy(
        &self,
        _level: &Level,
        _pos: BlockPos,
        _state: &BlockState,
        _player: &Player,
    ) {
    }
    /// The block at `neighbor_pos` became `neighbor`. Only blocks with a
    /// block entity are told.
    fn neighbor_changed(
        &self,
        _state: &BlockState,
        _level: &Level,
        _pos: BlockPos,
        _neighbor: &BlockState,
        _neighbor_pos: BlockPos,
    ) {
    }
}

/// What one of the mod's blocks keeps at its position, saved with the
/// world: at most 64 KiB, as `block-entities` holds.
pub trait BlockEntity: Sized + 'static {
    /// Ticked each server tick while its chunk is loaded.
    const TICKING: bool = true;
    /// Saved with `block-entities.set-synced`: the players whose games have
    /// its chunk get what it saves, for the mod's client half to read with
    /// `block_data` (NeoForge's update tag). Every change is sent, so a
    /// synced entity saves only what players need to see change.
    const SYNCED: bool = false;
    /// A new one, for a block just placed or before its data is read.
    fn new(pos: BlockPos, state: &BlockState) -> Self;
    /// Reads what [`BlockEntity::save_additional`] wrote.
    fn load_additional(&mut self, data: &[u8]);
    fn save_additional(&self, out: &mut Vec<u8>);
    /// Each server tick, when [`BlockEntity::TICKING`]; it is saved after
    /// when it changed.
    fn tick(&mut self, _level: &Level, _pos: BlockPos, _state: &BlockState) {}
    /// What it draws at its block, after it is placed and after each tick
    /// (capability `block-entities`, `visuals`): the parts go to players'
    /// games only when they differ from the last ones, and their games turn
    /// them every frame. A part's `angle` is its angle at game time 0
    /// (`visuals.set-in-phase`), so parts at one speed turn in step.
    fn visual(&self, _state: &BlockState, _visual: &mut Visual) {}
}

/// Degrees a tick for a speed in revolutions a minute, as Create gives its
/// shafts' speeds.
pub fn rpm(rpm: f32) -> f32 {
    rpm * 360.0 / 60.0 / 20.0
}

/// The parts a block entity draws, as Flywheel's instances: each a block
/// state's model, still or turning about an axis.
#[derive(Clone, Debug, Default)]
pub struct Visual {
    parts: Vec<visuals::Part>,
    scrolls: Vec<visuals::Scroll>,
    orientations: Vec<visuals::Orientation>,
}

/// A turn of `degrees` about `axis`, counterclockwise looking from its
/// positive end, as a unit quaternion (x, y, z, w): JOML's
/// `Axis.XP.rotationDegrees`, for [`Visual::oriented`].
pub fn rotation_degrees(axis: Axis, degrees: f32) -> [f32; 4] {
    let (s, c) = (degrees.to_radians() / 2.0).sin_cos();
    match axis {
        Axis::X => [s, 0.0, 0.0, c],
        Axis::Y => [0.0, s, 0.0, c],
        Axis::Z => [0.0, 0.0, s, c],
    }
}

/// `a * b`: `b`'s turn first, then `a`'s, as JOML's `a.mul(b)` chains a
/// renderer's turns (`Ry(base) * Rx(lower)`).
pub fn mul_quaternions(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let [ax, ay, az, aw] = a;
    let [bx, by, bz, bw] = b;
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}

impl Visual {
    /// `state`'s model turning about `axis` through the block's centre at
    /// `speed` degrees a tick ([`rpm`] converts), as Flywheel's
    /// `RotatingInstance`; the part is returned to move its pivot, offset
    /// or angle at game time 0 (its `rotationOffset`), or to give it a
    /// `velocity` in blocks a tick, from the offset it has now.
    pub fn rotating(&mut self, state: &str, axis: Axis, speed: f32) -> &mut visuals::Part {
        self.parts.push(visuals::Part {
            state: state.to_owned(),
            axis: axis as u8,
            pivot: (0.5, 0.5, 0.5),
            offset: (0.0, 0.0, 0.0),
            speed,
            angle: 0.0,
            velocity: (0.0, 0.0, 0.0),
            item: false,
            scale: 1.0,
        });
        self.parts.last_mut().expect("just pushed")
    }
    /// `item`'s model (`minecraft:iron_ingot`) as an item frame shows it,
    /// its centre at `centre` blocks from the block's corner, half size: an
    /// item lying on a depot. The part is returned to turn it (about x by 90
    /// degrees, `angle`, to lay it flat), scale it or move it.
    pub fn item(&mut self, item: &str, centre: [f32; 3]) -> &mut visuals::Part {
        let part = self.rotating(item, Axis::Y, 0.0);
        part.item = true;
        part.scale = 0.5;
        part.pivot = (centre[0], centre[1], centre[2]);
        part.offset = (centre[0] - 0.5, centre[1] - 0.5, centre[2] - 0.5);
        part
    }
    /// `state`'s model, still, its corner at `offset` blocks from the
    /// block's, as Flywheel's `TransformedInstance`.
    pub fn model(&mut self, state: &str, offset: [f32; 3]) -> &mut visuals::Part {
        let part = self.rotating(state, Axis::Y, 0.0);
        part.offset = (offset[0], offset[1], offset[2]);
        part
    }
    /// The last part's faces drawn with sprite `from` run down sprite `to`
    /// at `speed` of its heights a tick, starting again every `span` of it,
    /// as Create's `SpriteShiftEntry` scrolls a belt (`visuals.set-scrolls`).
    pub fn scrolling(&mut self, from: &str, to: &str, speed: f32, span: f32) -> &mut Self {
        if let Some(part) = self.parts.len().checked_sub(1) {
            self.scrolls.retain(|s| s.part as usize != part);
            self.scrolls.push(visuals::Scroll {
                part: part as u32,
                from: from.to_owned(),
                to: to.to_owned(),
                speed,
                span,
            });
        }
        self
    }
    /// The last part's model turned by unit quaternion `q` (x, y, z, w)
    /// about its pivot before its spin ([`rotation_degrees`],
    /// [`mul_quaternions`]), as a mechanical arm's segments are drawn
    /// (`visuals.set-orientations`).
    pub fn oriented(&mut self, q: [f32; 4]) -> &mut Self {
        if let Some(part) = self.parts.len().checked_sub(1) {
            self.orientations.retain(|o| o.part as usize != part);
            self.orientations.push(visuals::Orientation {
                part: part as u32,
                rotation: (q[0], q[1], q[2], q[3]),
            });
        }
        self
    }
    pub fn parts(&self) -> &[visuals::Part] {
        &self.parts
    }
    fn same_orientations(&self, other: &[visuals::Orientation]) -> bool {
        let key = |o: &visuals::Orientation| (o.part, o.rotation);
        self.orientations.len() == other.len()
            && self.orientations.iter().map(key).eq(other.iter().map(key))
    }
    fn same_scrolls(&self, other: &[visuals::Scroll]) -> bool {
        let key = |s: &visuals::Scroll| (s.part, s.from.clone(), s.to.clone(), s.speed, s.span);
        self.scrolls.len() == other.len() && self.scrolls.iter().map(key).eq(other.iter().map(key))
    }
    fn same(&self, other: &[visuals::Part]) -> bool {
        let key = |p: &visuals::Part| {
            (
                p.state.clone(),
                p.axis,
                p.pivot,
                p.offset,
                p.speed,
                p.angle,
                p.velocity,
                p.item,
                p.scale,
            )
        };
        self.parts.len() == other.len() && self.parts.iter().map(key).eq(other.iter().map(key))
    }
}

std::thread_local! {
    /// The parts last sent for each position (`BlockEntity::visual`).
    static DRAWN: RefCell<HashMap<(String, [i32; 3]), Visual>> = RefCell::new(HashMap::new());
}

/// Sends `entity`'s parts at `pos` when they differ from the last sent.
fn draw<T: BlockEntity>(entity: &T, level: &Level, pos: BlockPos, state: &BlockState) {
    let mut visual = Visual::default();
    entity.visual(state, &mut visual);
    let key = (level.dimension().to_owned(), [pos.x, pos.y, pos.z]);
    let last = DRAWN
        .with(|d| d.borrow().get(&key).cloned())
        .unwrap_or_default();
    let (x, y, z, d) = (pos.x, pos.y, pos.z, level.dimension());
    if !visual.same(&last.parts) && visuals::set_in_phase(d, x, y, z, &visual.parts).is_err() {
        return;
    }
    if !visual.parts.is_empty()
        && !visual.same_scrolls(&last.scrolls)
        && visuals::set_scrolls(d, x, y, z, &visual.scrolls).is_err()
    {
        return;
    }
    if !visual.parts.is_empty()
        && !visual.same_orientations(&last.orientations)
        && visuals::set_orientations(d, x, y, z, &visual.orientations).is_err()
    {
        return;
    }
    DRAWN.with(|d| {
        let mut d = d.borrow_mut();
        if visual.parts.is_empty() {
            d.remove(&key);
        } else {
            d.insert(key, visual);
        }
    });
}

/// A block entity type with its type forgotten: what the hooks call.
struct EntityType {
    block: String,
    ticking: bool,
    place: fn(&Level, BlockPos, &BlockState),
    tick: fn(&Level, BlockPos, &BlockState),
}

#[derive(Default)]
struct Registry {
    blocks: Vec<(String, Rc<dyn Block>)>,
    entities: Vec<EntityType>,
    /// `block-entities.clicked-face`, kept by `register`: a mod that
    /// registers no block never imports `block-entities` for it.
    clicked_face: Option<fn() -> Option<u8>>,
}

std::thread_local! {
    static REGISTRY: RefCell<Registry> = RefCell::new(Registry::default());
}

/// Makes `block` (`mymod:gear`, in one of the mod's namespaces) behave as
/// `behaviour` says. Once per block.
pub fn register(block: &str, behaviour: impl Block) -> Result<(), String> {
    REGISTRY.with(|r| {
        let mut r = r.borrow_mut();
        if r.blocks.iter().any(|(id, _)| id == block) {
            return Err(format!("{block} is registered already"));
        }
        r.blocks.push((block.to_owned(), Rc::new(behaviour)));
        r.clicked_face = Some(block_entities::clicked_face);
        Ok(())
    })
}

/// Gives `block` a block entity of type `T`, made when a player places it.
/// Once per block.
pub fn register_entity<T: BlockEntity>(block: &str) -> Result<(), String> {
    REGISTRY.with(|r| {
        let mut r = r.borrow_mut();
        if r.entities.iter().any(|e| e.block == block) {
            return Err(format!("{block} has a block entity already"));
        }
        r.entities.push(EntityType {
            block: block.to_owned(),
            ticking: T::TICKING,
            place: place::<T>,
            tick: tick::<T>,
        });
        Ok(())
    })
}

fn place<T: BlockEntity>(level: &Level, pos: BlockPos, state: &BlockState) {
    let entity = T::new(pos, state);
    let _ = level.set_block_entity(pos, &entity);
    draw(&entity, level, pos, state);
}

fn tick<T: BlockEntity>(level: &Level, pos: BlockPos, state: &BlockState) {
    let d = level.dimension();
    let Some(before) = block_entities::get(d, pos.x, pos.y, pos.z) else {
        return;
    };
    let mut entity = T::new(pos, state);
    // Empty data: a block the game gave data to as it appeared (placed by a
    // command or another mod), starting from `new`.
    if !before.is_empty() {
        entity.load_additional(&before);
    }
    entity.tick(level, pos, state);
    let mut after = Vec::new();
    entity.save_additional(&mut after);
    if after != before {
        let _ = if T::SYNCED {
            block_entities::set_synced(d, pos.x, pos.y, pos.z, &after)
        } else {
            block_entities::set(d, pos.x, pos.y, pos.z, &after)
        };
    }
    draw(&entity, level, pos, state);
}

/// Drops `count` of `item` at the middle of `pos`, as NeoForge's
/// `Block.popResource` (capability `items`).
pub fn pop_resource(level: &Level, pos: BlockPos, item: &str, count: u32) -> Result<(), String> {
    items::drop(
        level.dimension(),
        f64::from(pos.x) + 0.5,
        f64::from(pos.y) + 0.5,
        f64::from(pos.z) + 0.5,
        item,
        count,
    )
}

fn block_of(state: &BlockState) -> Option<Rc<dyn Block>> {
    REGISTRY.with(|r| {
        r.borrow()
            .blocks
            .iter()
            .find(|(id, _)| state.is(id))
            .map(|(_, b)| b.clone())
    })
}

fn entity_of(
    state: &BlockState,
) -> Option<(
    bool,
    fn(&Level, BlockPos, &BlockState),
    fn(&Level, BlockPos, &BlockState),
)> {
    REGISTRY.with(|r| {
        r.borrow()
            .entities
            .iter()
            .find(|e| state.is(&e.block))
            .map(|e| (e.ticking, e.place, e.tick))
    })
}

fn handled(state: &BlockState) -> bool {
    block_of(state).is_some() || entity_of(state).is_some()
}

// What `export_server_mod!` calls. Each answers whether a registered block
// took the event; the mod's own hook hears it when not.

#[doc(hidden)]
pub fn __any() -> bool {
    REGISTRY.with(|r| {
        let r = r.borrow();
        !r.blocks.is_empty() || !r.entities.is_empty()
    })
}

#[doc(hidden)]
pub fn __ticking() -> Vec<String> {
    REGISTRY.with(|r| {
        r.borrow()
            .entities
            .iter()
            .filter(|e| e.ticking)
            .map(|e| e.block.clone())
            .collect()
    })
}

#[doc(hidden)]
pub fn __tick(dimension: &str, pos: Pos, state: &str) -> bool {
    let state = BlockState::parse(state);
    let Some((ticking, _, tick)) = entity_of(&state) else {
        return false;
    };
    if ticking {
        tick(&Level::new(dimension), pos.into(), &state);
    }
    true
}

#[doc(hidden)]
pub fn __used(player: &str, dimension: &str, pos: Pos, state: &str) -> bool {
    let state = BlockState::parse(state);
    let Some(block) = block_of(&state) else {
        return false;
    };
    let pos = BlockPos::from(pos);
    let hit = BlockHitResult {
        pos,
        direction: REGISTRY
            .with(|r| r.borrow().clicked_face)
            .and_then(|face| face())
            .and_then(Direction::from_3d_data_value),
    };
    block
        .use_without_item(&state, &Level::new(dimension), pos, player, &hit)
        .consumes_action()
}

#[doc(hidden)]
pub fn __item_used(player: &str, item: &str, target: Option<&BlockTarget>) -> bool {
    let Some(t) = target else {
        return false;
    };
    let state = BlockState::parse(&t.state);
    let Some(block) = block_of(&state) else {
        return false;
    };
    let pos = BlockPos::new(t.x, t.y, t.z);
    let hit = BlockHitResult {
        pos,
        direction: Direction::from_3d_data_value(t.face),
    };
    block
        .use_item_on(item, &state, &Level::new(&t.dimension), pos, player, &hit)
        .consumes_action()
}

#[doc(hidden)]
pub fn __placed(player: &Player, pos: BlockPos, state: &str) -> bool {
    let state = BlockState::parse(state);
    if !handled(&state) {
        return false;
    }
    let level = Level::new(&player.dimension);
    if let Some((_, place, _)) = entity_of(&state) {
        place(&level, pos, &state);
    }
    if let Some(block) = block_of(&state) {
        block.set_placed_by(&level, pos, &state, player);
    }
    true
}

#[doc(hidden)]
pub fn __broken(player: &Player, pos: BlockPos, state: &str) -> bool {
    let state = BlockState::parse(state);
    if !handled(&state) {
        return false;
    }
    if let Some(block) = block_of(&state) {
        block.player_will_destroy(&Level::new(&player.dimension), pos, &state, player);
    }
    true
}

#[doc(hidden)]
pub fn __neighbor_changed(
    dimension: &str,
    pos: Pos,
    state: &str,
    from: Pos,
    from_state: &str,
) -> bool {
    let state = BlockState::parse(state);
    if !handled(&state) {
        return false;
    }
    if let Some(block) = block_of(&state) {
        block.neighbor_changed(
            &state,
            &Level::new(dimension),
            pos.into(),
            &BlockState::parse(from_state),
            from.into(),
        );
    }
    true
}
