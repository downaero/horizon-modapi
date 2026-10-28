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
use crate::level::{BlockState, Direction, Level};
use crate::server::{BlockPos, BlockTarget, Player, Pos, block_entities, items};
use std::cell::RefCell;
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

/// Where a player's click hit the block: the face is known for clicks with
/// an item, not for those with an empty hand.
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
    let _ = level.set_block_entity(pos, &T::new(pos, state));
}

fn tick<T: BlockEntity>(level: &Level, pos: BlockPos, state: &BlockState) {
    let d = level.dimension();
    let Some(before) = block_entities::get(d, pos.x, pos.y, pos.z) else {
        return;
    };
    let mut entity = T::new(pos, state);
    entity.load_additional(&before);
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
        direction: None,
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
