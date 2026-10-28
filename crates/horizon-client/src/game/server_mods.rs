//! Server mods in the game (`docs/mods.md`, "Server mods"): the game that
//! simulates the world (in-frame single player, the server thread, a LAN
//! host, the dedicated server) runs them. At the end of each tick it tells
//! them who joined and left, then what players said, broke and placed since
//! the last one, then ticks them; a player's command reaches them at once.
//! Their lines go to the players they name. With no server mod on, each hook
//! here is one check.
use super::{Game, lan::LanHost};
#[cfg(not(feature = "mods"))]
use horizon_world::registry::BlockStateId;

/// Who did something a server mod hears about, resolved at the tick's end.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) enum Who {
    /// The window's own player, in-frame.
    Local,
    /// A connection's player (the server thread's owner is one).
    Conn(u32),
    Uuid(u128),
}

/// What players did during a tick, for the server mods at its end.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) enum ModEvent {
    Chat(Who, String),
    Block {
        who: Who,
        pos: [i32; 3],
        state: String,
        placed: bool,
    },
}

#[cfg(not(feature = "mods"))]
impl Game<'_> {
    pub(super) fn tick_server_mods(&mut self) {}
    pub(super) fn mod_block_event(&mut self, _pos: [i32; 3], _state: BlockStateId, _placed: bool) {}
    pub(super) fn mod_player_chat(&mut self, _who: Who, _text: &str) {}
    pub(super) fn mod_block_use(&mut self, _pos: [i32; 3]) -> bool {
        false
    }
    pub(super) fn mod_block_use_by_guest(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _pos: [i32; 3],
    ) -> bool {
        false
    }
    pub(super) fn mod_item_use_by_guest(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _target: Option<([i32; 3], u8)>,
    ) -> bool {
        false
    }
    pub(super) fn local_menu_message(&mut self, _message: horizon_mods::menu_layout::MenuMessage) {}
    pub(super) fn guest_mod_net(
        &mut self,
        _host: &LanHost,
        _conn: u32,
        _channel: &str,
        _data: &[u8],
    ) {
    }
    pub(super) fn remote_mod_net(&mut self, _channel: &str, _data: Vec<u8>) {}
    pub(super) fn local_server_mod_command(&mut self, _line: &str) -> bool {
        false
    }
    pub(super) fn guest_server_mod_command(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _line: &str,
    ) -> bool {
        false
    }
    pub(super) fn apply_mod_block_writes(&mut self) {}
    pub(super) fn is_mod_entity(&self, _id: u32) -> bool {
        false
    }
    pub(super) fn mod_container_entity(
        &self,
        _block: &str,
    ) -> Option<horizon_client::containers::BlockEntity> {
        None
    }
    pub(super) fn flush_mod_block_data(&mut self) {}
}

/// Capability `net`: a mod's messages travel as custom payloads on
/// `horizon:mod/<mod id>`.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) const NET_CHANNEL: &str = "horizon:mod/";

/// Where a world keeps what server mods store at block positions
/// (capability `block-entities`): `<world>/horizon-mod-data/<mod id>.blocks`,
/// beside the vanilla save and never part of it.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) const BLOCK_DATA_DIR: &str = "horizon-mod-data";

/// One mod's block data as a file: per position, the dimension (u16 length
/// and bytes), x y z (i32), the data (u32 length and bytes), little-endian.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) fn encode_block_data<'a>(
    entries: impl Iterator<Item = (&'a str, [i32; 3], &'a [u8])>,
) -> Vec<u8> {
    let mut out = Vec::new();
    for (dimension, pos, data) in entries {
        out.extend((dimension.len() as u16).to_le_bytes());
        out.extend(dimension.as_bytes());
        for c in pos {
            out.extend(c.to_le_bytes());
        }
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
    }
    out
}

/// [`encode_block_data`]'s inverse; a damaged tail is dropped.
#[cfg_attr(not(feature = "mods"), allow(dead_code))]
pub(super) fn decode_block_data(mut bytes: &[u8]) -> Vec<(String, [i32; 3], Vec<u8>)> {
    fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
        let (head, rest) = bytes.split_at_checked(n)?;
        *bytes = rest;
        Some(head)
    }
    let mut out = Vec::new();
    while !bytes.is_empty() {
        let Some(entry) = (|| {
            let n = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().ok()?) as usize;
            let dimension = String::from_utf8(take(&mut bytes, n)?.to_vec()).ok()?;
            let mut pos = [0i32; 3];
            for c in &mut pos {
                *c = i32::from_le_bytes(take(&mut bytes, 4)?.try_into().ok()?);
            }
            let n = u32::from_le_bytes(take(&mut bytes, 4)?.try_into().ok()?) as usize;
            Some((dimension, pos, take(&mut bytes, n)?.to_vec()))
        })() else {
            break;
        };
        out.push(entry);
    }
    out
}

#[cfg(test)]
mod block_data_tests {
    use super::*;

    #[test]
    fn block_data_round_trips_and_drops_a_damaged_tail() {
        let entries = [
            ("minecraft:overworld", [1, -64, 3], &[1u8, 2, 3][..]),
            ("minecraft:the_nether", [-5, 70, 9], &[][..]),
        ];
        let mut bytes = encode_block_data(entries.iter().copied());
        let back = decode_block_data(&bytes);
        assert_eq!(back.len(), 2);
        assert_eq!(
            back[0],
            ("minecraft:overworld".into(), [1, -64, 3], vec![1, 2, 3])
        );
        assert_eq!(back[1].2, Vec::<u8>::new());
        bytes.extend([9, 9, 9]);
        assert_eq!(decode_block_data(&bytes).len(), 2);
    }
}

#[cfg(feature = "mods")]
mod run {
    use super::{Game, LanHost, ModEvent, Who};
    use horizon_client::chunk_map::ChunkMap;
    use horizon_client::{
        command::args::GameMode as Mode,
        lan::data::component_nbt,
        text::{self, Text},
    };
    use horizon_mods::{
        ArgumentKind, BlockPos, GameMode, Output, ServerCommand, ServerPlayer, WorldView,
    };
    use horizon_protocol::packets::configuration::CustomPayload;
    use horizon_protocol::packets::play::{self, Clientbound as C};
    use horizon_world::registry::{BlockRegistry, BlockStateId};
    use serde_json::{Value, json};

    /// Positions whose redstone signal is read for mods each tick.
    const MAX_POWER_READS: usize = 4096;
    /// Block changes a tick that server mods are told about.
    const MAX_NEIGHBOR_CHANGES: usize = 4096;
    /// A stack as capability `stacks` shows it: every mod's own bytes from
    /// `minecraft:custom_data` under `horizon:<mod id>`.
    fn stack_info(s: &horizon_client::items::ItemStack) -> horizon_mods::StackInfo {
        use horizon_world::nbt::Tag;
        let data = s.extra.as_deref();
        let custom = data
            .and_then(|d| d.other.iter().find(|(k, _)| k == "minecraft:custom_data"))
            .map(|(_, t)| t);
        let mut mods = Vec::new();
        if let Some(Tag::Compound(map)) = custom {
            for (k, v) in map {
                let Ok(k) = k.to_string() else { continue };
                if let (Some(id), Tag::ByteArray(bytes)) = (k.strip_prefix("horizon:"), v) {
                    mods.push((id.to_owned(), bytes.clone()));
                }
            }
        }
        horizon_mods::StackInfo {
            item: s.item.to_string(),
            count: s.count,
            damage: s.damage,
            name: s.custom_name().map(str::to_owned),
            enchantments: data
                .map(|d| {
                    d.enchantments
                        .iter()
                        .map(|(e, l)| (e.to_string(), *l))
                        .collect()
                })
                .unwrap_or_default(),
            data: mods,
        }
    }

    /// The stack mod `mod_id` wrote: on `base` when it is the same item, so
    /// what other mods and vanilla keep on it stays; the mod's own bytes set
    /// (or taken off).
    fn stack_with(
        base: Option<&horizon_client::items::ItemStack>,
        info: horizon_mods::StackInfo,
        mod_id: &str,
    ) -> horizon_client::items::ItemStack {
        use horizon_world::nbt::Tag;
        let mut s = match base {
            Some(b) if *b.item == *info.item => b.clone(),
            _ => horizon_client::items::ItemStack::new(info.item.as_str(), info.count),
        };
        s.count = info.count;
        s.damage = info.damage;
        {
            let d = s.data_mut();
            d.custom_name = info.name.clone();
            d.enchantments = info
                .enchantments
                .iter()
                .map(|(e, l)| (std::sync::Arc::from(e.as_str()), *l))
                .collect();
            let key = format!("horizon:{mod_id}");
            let own = info
                .data
                .into_iter()
                .find(|(m, _)| m == mod_id)
                .map(|(_, b)| b);
            let at = d
                .other
                .iter()
                .position(|(k, _)| k == "minecraft:custom_data");
            let mut map = match at.map(|i| d.other.remove(i).1) {
                Some(Tag::Compound(map)) => map,
                _ => Default::default(),
            };
            match own {
                Some(bytes) => {
                    map.insert(key.as_str().into(), Tag::ByteArray(bytes));
                }
                None => {
                    map.remove(&key.as_str().into());
                }
            }
            if !map.is_empty() {
                d.other
                    .push(("minecraft:custom_data".into(), Tag::Compound(map)));
                d.other.sort_by(|a, b| a.0.cmp(&b.0));
            }
        }
        // A stack with nothing beyond its item compares as a plain one.
        if s.extra.as_deref().is_some_and(|d| *d == Default::default()) {
            s.extra = None;
        }
        s
    }

    /// The slots of an inventory capability `stacks` reaches: a chest's (any
    /// container's) or a furnace's.
    fn inventory_slots(
        entity: &mut horizon_client::containers::BlockEntity,
    ) -> Option<&mut [Option<horizon_client::items::ItemStack>]> {
        use horizon_client::containers::BlockEntity;
        match entity {
            BlockEntity::Chest(items) => Some(items.as_mut_slice()),
            BlockEntity::Furnace(f) => Some(f.items.as_mut_slice()),
            _ => None,
        }
    }

    /// Item entities lent to mods each tick.
    const MAX_LENT_ITEMS: usize = 4096;
    /// `net` messages waiting for a tick are kept up to this many.
    const MAX_QUEUED_NET: usize = 4096;
    /// Entity changes waiting for a tick are kept up to this many.
    const MAX_QUEUED_ENTITY_OPS: usize = 1 << 16;

    /// Blocks queued for levels that don't tick (another dimension, in-frame)
    /// are kept up to this many; the oldest go first.
    const MAX_QUEUED_WRITES: usize = 1 << 16;

    /// Capability `world`'s view during a call: the levels' chunks, moved out
    /// of the game for the call and back after it (no copy).
    struct LentWorld {
        registry: &'static BlockRegistry,
        biomes: std::sync::Arc<horizon_client::biome::Biomes>,
        dims: Vec<(&'static str, ChunkMap)>,
        /// The simulated dimension's mod containers (capability
        /// `containers`) as of the call's start, by position.
        containers: std::collections::HashMap<[i32; 3], Vec<Option<horizon_mods::SlotStack>>>,
        container_dim: &'static str,
        /// The item entities lying in the simulated dimension at the call's
        /// start (capability `items`).
        dropped: Vec<horizon_mods::DroppedStack>,
        /// The redstone signal at the positions of the mod blocks that keep
        /// data or items in the simulated dimension, as of the call's start
        /// (capability `redstone`).
        power: std::collections::HashMap<[i32; 3], u8>,
        /// The simulated dimension's block entities, moved out of the game
        /// for the call (capability `transfer` moves items in and out).
        entities: horizon_client::block_entities::BlockEntities,
        res: std::sync::Arc<crate::game::Resources>,
        /// Chunks whose block entities `transfer` changed, to save.
        touched: Vec<(i32, i32)>,
    }
    /// The slots of an inventory a hopper reaches through `face` (0 down,
    /// 1 up, 2 to 5 the sides; none: all), to put in or to take out, as
    /// `WorldlyContainer.getSlotsForFace` and its checks have them for a
    /// furnace; every slot of any other.
    fn reachable(
        entity: &mut horizon_client::containers::BlockEntity,
        face: Option<u8>,
        insert: bool,
    ) -> Option<(&mut [Option<horizon_client::items::ItemStack>], Vec<usize>)> {
        use horizon_client::containers::BlockEntity;
        match entity {
            BlockEntity::Chest(items) => {
                let all = (0..items.len()).collect();
                Some((items.as_mut_slice(), all))
            }
            BlockEntity::Furnace(f) => {
                let slots = match (insert, face) {
                    (true, Some(1)) => vec![0],
                    (true, Some(0)) => vec![],
                    (true, Some(_)) => vec![1],
                    (true, None) => vec![0, 1],
                    (false, Some(0) | None) => vec![2],
                    (false, Some(_)) => vec![],
                };
                Some((f.items.as_mut_slice(), slots))
            }
            _ => None,
        }
    }
    impl WorldView for LentWorld {
        fn craft(
            &self,
            grid: &[Option<(String, u32)>],
            width: u32,
        ) -> Option<horizon_mods::Crafted> {
            use horizon_client::items::ItemStack;
            let size = width as usize;
            let grid: Vec<Option<ItemStack>> = grid
                .iter()
                .map(|c| {
                    c.as_ref()
                        .map(|(item, count)| ItemStack::new(item.as_str(), *count))
                })
                .collect();
            let recipes = self.res.recipes();
            let (name, result) = recipes.find(&grid, size)?;
            let full = |n: &str| match n.contains(':') {
                true => n.to_owned(),
                false => format!("minecraft:{n}"),
            };
            Some(horizon_mods::Crafted {
                recipe: full(name),
                result: (result.item.to_string(), result.count),
                remainders: recipes
                    .remainders(name, &grid, size)
                    .into_iter()
                    .map(|r| r.map(|s| (s.item.to_string(), s.count)))
                    .collect(),
            })
        }
        fn cook(&self, kind: &str, item: &str) -> Option<horizon_mods::Cooked> {
            let kind = match kind.contains(':') {
                true => kind.to_owned(),
                false => format!("minecraft:{kind}"),
            };
            let stack = horizon_client::items::ItemStack::new(item, 1);
            let c = self.res.recipes().cook(&kind, &stack)?;
            Some(horizon_mods::Cooked {
                recipe: match c.id.contains(':') {
                    true => c.id.clone(),
                    false => format!("minecraft:{}", c.id),
                },
                result: (c.result.item.to_string(), c.result.count),
                experience: c.experience,
                time: c.time,
            })
        }
        fn stonecut(&self, item: &str) -> Vec<(String, (String, u32))> {
            let stack = horizon_client::items::ItemStack::new(item, 1);
            self.res
                .stations()
                .recipes
                .stonecutter(&stack)
                .into_iter()
                .map(|r| {
                    let id = match r.id.contains(':') {
                        true => r.id.clone(),
                        false => format!("minecraft:{}", r.id),
                    };
                    (id, (r.result.item.to_string(), r.result.count))
                })
                .collect()
        }
        fn recipes_of_type(&self, kind: &str) -> Vec<(String, String)> {
            self.res
                .recipes()
                .custom
                .iter()
                .filter(|(_, k, _)| k == kind)
                .map(|(id, _, json)| (id.clone(), json.clone()))
                .collect()
        }
        fn test_ingredient(&self, ingredient: &str, item: &str) -> bool {
            // A bare id or tag as written in a file, or JSON.
            let value = serde_json::from_str(ingredient)
                .unwrap_or_else(|_| serde_json::Value::String(ingredient.to_owned()));
            horizon_client::crafting::Ingredient::parse(&value, self.res.items())
                .is_some_and(|i| i.0.contains(item))
        }
        fn stack_at(
            &self,
            dimension: &str,
            pos: [i32; 3],
            slot: u32,
        ) -> Option<horizon_mods::StackInfo> {
            if dimension != self.container_dim {
                return None;
            }
            let mut entity = self.entities.get(&(pos[0], pos[1], pos[2]))?.clone();
            let slots = inventory_slots(&mut entity)?;
            slots.get(slot as usize)?.as_ref().map(stack_info)
        }
        fn set_stack_at(
            &mut self,
            dimension: &str,
            pos: [i32; 3],
            slot: u32,
            stack: Option<horizon_mods::StackInfo>,
            mod_id: &str,
        ) -> bool {
            if dimension != self.container_dim {
                return false;
            }
            let Some(entity) = self.entities.get_mut(&(pos[0], pos[1], pos[2])) else {
                return false;
            };
            let Some(cell) = inventory_slots(entity).and_then(|s| s.get_mut(slot as usize)) else {
                return false;
            };
            *cell = stack.map(|info| stack_with(cell.as_ref(), info, mod_id));
            self.touched.push((pos[0] >> 4, pos[2] >> 4));
            true
        }
        fn item_slots(
            &self,
            dimension: &str,
            pos: [i32; 3],
            face: Option<u8>,
        ) -> Option<Vec<Option<horizon_mods::SlotStack>>> {
            if dimension != self.container_dim {
                return None;
            }
            // `reachable` wants it mutable; a copy keeps this a read. The
            // slots reachable are those to put in and those to take from.
            let mut entity = self.entities.get(&(pos[0], pos[1], pos[2]))?.clone();
            let mut open = reachable(&mut entity, face, true)?.1;
            let (slots, out) = reachable(&mut entity, face, false)?;
            open.extend(out);
            open.sort_unstable();
            open.dedup();
            Some(
                open.into_iter()
                    .map(|i| slots[i].as_ref().map(|s| (s.item.to_string(), s.count)))
                    .collect(),
            )
        }
        fn insert_item(
            &mut self,
            dimension: &str,
            pos: [i32; 3],
            face: Option<u8>,
            item: &str,
            count: u32,
            simulate: bool,
        ) -> u32 {
            if dimension != self.container_dim {
                return count;
            }
            let items = self.res.items();
            let Some(kind) = items.get(item) else {
                return count;
            };
            let max = kind.max_stack.max(1);
            let fuel = kind.fuel.is_some();
            let Some(entity) = self.entities.get_mut(&(pos[0], pos[1], pos[2])) else {
                return count;
            };
            let furnace = matches!(entity, horizon_client::containers::BlockEntity::Furnace(_));
            let Some((slots, open)) = reachable(entity, face, true) else {
                return count;
            };
            let mut left = count;
            // Like stacks first, then empty slots (`HopperBlockEntity.addItem`).
            for merge in [true, false] {
                for &i in &open {
                    if left == 0 {
                        break;
                    }
                    // A furnace's fuel slot takes fuel only.
                    if furnace && i == 1 && !fuel {
                        continue;
                    }
                    match &mut slots[i] {
                        Some(s)
                            if merge && &*s.item == item && s.damage == 0 && s.extra.is_none() =>
                        {
                            let put = left.min(max.saturating_sub(s.count));
                            if !simulate {
                                s.count += put;
                            }
                            left -= put;
                        }
                        cell @ None if !merge => {
                            let put = left.min(max);
                            if !simulate {
                                *cell = Some(horizon_client::items::ItemStack::new(item, put));
                            }
                            left -= put;
                        }
                        _ => {}
                    }
                }
            }
            if left != count && !simulate {
                self.touched.push((pos[0] >> 4, pos[2] >> 4));
            }
            left
        }
        fn extract_item(
            &mut self,
            dimension: &str,
            pos: [i32; 3],
            face: Option<u8>,
            count: u32,
            simulate: bool,
        ) -> Option<horizon_mods::SlotStack> {
            if dimension != self.container_dim {
                return None;
            }
            let entity = self.entities.get_mut(&(pos[0], pos[1], pos[2]))?;
            let (slots, open) = reachable(entity, face, false)?;
            for i in open {
                let Some(s) = &mut slots[i] else {
                    continue;
                };
                let take = count.min(s.count);
                let out = (s.item.to_string(), take);
                if !simulate {
                    s.count -= take;
                    if s.count == 0 {
                        slots[i] = None;
                    }
                    self.touched.push((pos[0] >> 4, pos[2] >> 4));
                }
                return Some(out);
            }
            None
        }
        fn dropped(&self, dimension: &str) -> Vec<horizon_mods::DroppedStack> {
            if dimension != self.container_dim {
                return Vec::new();
            }
            self.dropped.clone()
        }
        fn power(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<u8> {
            if dimension != self.container_dim {
                return None;
            }
            self.power.get(&[x, y, z]).copied()
        }
        fn container(
            &self,
            dimension: &str,
            x: i32,
            y: i32,
            z: i32,
        ) -> Option<Vec<Option<horizon_mods::SlotStack>>> {
            if dimension != self.container_dim {
                return None;
            }
            self.containers.get(&[x, y, z]).cloned()
        }
        fn block(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            let chunk = chunks.get(&(x >> 4, z >> 4))?;
            Some(state_string(self.registry, chunk.block(x & 15, y, z & 15)))
        }
        fn biome(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
            let (_, chunks) = self.dims.iter().find(|(d, _)| *d == dimension)?;
            let chunk = chunks.get(&(x >> 4, z >> 4))?;
            Some(self.biomes.get(chunk.biome(x, y, z)).name.clone())
        }
        fn has_dimension(&self, dimension: &str) -> bool {
            matches!(
                dimension,
                "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
            )
        }
        fn check_state(&self, state: &str) -> Result<(), String> {
            crate::game::render_control::parse_state(self.registry, state)
                .map(drop)
                .map_err(|e| format!("{state}: {e}"))
        }
    }

    /// Server mods' commands as a root of the commands report
    /// (`lan::commands`): a literal per command, then its arguments in a
    /// chain, each node runnable where only optional arguments follow.
    fn command_tree<'a>(commands: impl Iterator<Item = &'a ServerCommand>) -> Value {
        let mut root = serde_json::Map::new();
        for c in commands {
            let mut next: Option<(String, Value)> = None;
            for (i, a) in c.arguments.iter().enumerate().rev() {
                let executable = c.arguments.get(i + 1).is_none_or(|b| b.optional);
                let (parser, properties) = match a.kind {
                    ArgumentKind::Word => ("brigadier:string", json!({"type": "word"})),
                    ArgumentKind::Text => ("brigadier:string", json!({"type": "greedy"})),
                    ArgumentKind::Integer((min, max)) => {
                        let mut p = serde_json::Map::new();
                        if let Some(min) = min {
                            p.insert("min".into(), min.into());
                        }
                        if let Some(max) = max {
                            p.insert("max".into(), max.into());
                        }
                        ("brigadier:integer", Value::Object(p))
                    }
                    ArgumentKind::Number => ("brigadier:double", Value::Null),
                    ArgumentKind::Boolean => ("brigadier:bool", Value::Null),
                    ArgumentKind::Player => (
                        "minecraft:entity",
                        json!({"amount": "single", "type": "players"}),
                    ),
                    ArgumentKind::BlockPos => ("minecraft:block_pos", Value::Null),
                };
                let mut node = json!({
                    "type": "argument",
                    "parser": parser,
                    "properties": properties,
                    "executable": executable,
                });
                if let Some((name, child)) = next.take() {
                    node["children"] = json!({ name: child });
                }
                next = Some((a.name.clone(), node));
            }
            let executable = c.arguments.first().is_none_or(|a| a.optional);
            let mut literal = json!({"type": "literal", "executable": executable});
            if let Some((name, child)) = next {
                literal["children"] = json!({ name: child });
            }
            root.insert(c.name.clone(), literal);
        }
        if root.is_empty() {
            return Value::Null;
        }
        json!({"type": "root", "children": root})
    }

    /// `minecraft:oak_log[axis=y]`.
    fn state_string(registry: &BlockRegistry, state: BlockStateId) -> String {
        let Some(s) = registry.state(state) else {
            return "minecraft:air".into();
        };
        if s.properties.is_empty() {
            return s.name.clone();
        }
        let props: Vec<String> = s
            .properties
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{}[{}]", s.name, props.join(","))
    }

    /// `UUID.toString()`.
    fn uuid_string(u: u128) -> String {
        let h = format!("{u:032x}");
        format!(
            "{}-{}-{}-{}-{}",
            &h[..8],
            &h[8..12],
            &h[12..16],
            &h[16..20],
            &h[20..]
        )
    }

    fn mode(id: i32) -> GameMode {
        match id {
            1 => GameMode::Creative,
            2 => GameMode::Adventure,
            3 => GameMode::Spectator,
            _ => GameMode::Survival,
        }
    }

    impl Game<'_> {
        /// The players on this game's server: the window's own player
        /// in-frame, then every connection's. `host` is the hub when it is
        /// out of `self.lan`.
        pub(in crate::game) fn server_players(&self, host: Option<&LanHost>) -> Vec<ServerPlayer> {
            let mut out = Vec::new();
            if self.options.execution_role.is_client()
                && self.remote.is_none()
                && let Some(p) = &self.player
            {
                out.push(ServerPlayer {
                    id: self.player_uuid_string(),
                    name: crate::game::chat::PLAYER_NAME.to_owned(),
                    x: p.position.x,
                    y: p.position.y,
                    z: p.position.z,
                    dimension: self.dim.id().to_owned(),
                    game_mode: match self.mode {
                        Mode::Survival => GameMode::Survival,
                        Mode::Creative => GameMode::Creative,
                        Mode::Adventure => GameMode::Adventure,
                        Mode::Spectator => GameMode::Spectator,
                    },
                    health: p.health.health(),
                });
            }
            let guests = host
                .or(self.lan.as_ref())
                .into_iter()
                .flat_map(|h| &h.hub.guests);
            out.extend(guests.filter(|g| g.placed).map(|g| ServerPlayer {
                id: uuid_string(g.uuid),
                name: g.name.clone(),
                x: g.position[0],
                y: g.position[1],
                z: g.position[2],
                dimension: g.dimension.clone(),
                game_mode: mode(g.game_mode),
                health: g.health,
            }));
            out
        }

        /// Whether what players do now reaches server mods: one is running
        /// and this game simulates the world.
        fn hears_players(&self) -> bool {
            self.mods.server && self.remote.is_none()
        }

        /// The tick's end: joins and leaves, the tick's events, the tick hook.
        pub(in crate::game) fn tick_server_mods(&mut self) {
            if !self.options.execution_role.is_client() {
                // A server has no frames to check its dev mods in.
                self.reload_dev_mods();
            }
            self.send_server_mod_commands();
            if !self.mods.server {
                self.world.neighbor_log = None;
                return;
            }
            self.world.neighbor_log.get_or_insert_with(Vec::new);
            self.sync_typed_entities();
            self.tick_typed_entities();
            let players = self.server_players(None);
            let mut snapshot = self.mod_snapshot_with(players.clone());
            self.fill_mod_inventories(&mut snapshot, None);
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            let before = std::mem::take(&mut self.mods.players);
            for p in before
                .iter()
                .filter(|p| !players.iter().any(|q| q.id == p.id))
            {
                self.mods.mods.player_left(p);
            }
            for p in players
                .iter()
                .filter(|p| !before.iter().any(|q| q.id == p.id))
            {
                self.mods.mods.player_joined(p);
            }
            for event in std::mem::take(&mut self.mods.events) {
                let who = |w: &Who| {
                    let id = match w {
                        Who::Local => self.player_uuid_string(),
                        Who::Uuid(u) => uuid_string(*u),
                        Who::Conn(c) => {
                            let guests = self.lan.iter().flat_map(|h| &h.hub.guests);
                            let g = guests.into_iter().find(|g| g.conn == *c);
                            g.map_or_else(String::new, |g| uuid_string(g.uuid))
                        }
                    };
                    players.iter().chain(&before).find(|p| p.id == id).cloned()
                };
                match event {
                    ModEvent::Chat(w, text) => {
                        if let Some(p) = who(&w) {
                            self.mods.mods.player_chat(&p, &text);
                        }
                    }
                    ModEvent::Block {
                        who: w,
                        pos,
                        state,
                        placed,
                    } => {
                        let Some(p) = who(&w) else { continue };
                        let pos = BlockPos {
                            x: pos[0],
                            y: pos[1],
                            z: pos[2],
                        };
                        if placed {
                            self.mods.mods.block_placed(&p, pos, &state);
                        } else {
                            self.mods.mods.block_broken(&p, pos, &state);
                        }
                    }
                }
            }
            self.sync_mod_block_data();
            for (mod_id, player, data) in std::mem::take(&mut self.mods.net_to_server) {
                self.mods.mods.net_message(&mod_id, Some(&player), &data);
            }
            let mut changed = self.world.neighbor_log.take().unwrap_or_default();
            self.world.neighbor_log = Some(Vec::new());
            changed.sort_unstable();
            changed.dedup();
            changed.truncate(MAX_NEIGHBOR_CHANGES);
            if !changed.is_empty() {
                let changed: Vec<[i32; 3]> = changed.into_iter().map(|p| [p.0, p.1, p.2]).collect();
                self.mods.mods.neighbors_changed(self.dim.id(), &changed);
            }
            // In order: a button's action sees the texts sent before it.
            for (mod_id, player, event) in std::mem::take(&mut self.mods.menu_events) {
                use horizon_mods::menu_layout::MenuMessage;
                match event {
                    MenuMessage::Button { id, .. } => {
                        self.mods.mods.menu_button(&mod_id, &player, id)
                    }
                    MenuMessage::Text { id, value, .. } => {
                        self.mods.mods.menu_text(&mod_id, &player, id, &value)
                    }
                    MenuMessage::Closed { .. } => self.mods.mods.menu_closed(&mod_id, &player),
                    _ => {}
                }
            }
            for (from, to, data) in std::mem::take(&mut self.mods.peer_messages) {
                self.mods.mods.peer_message(&to, &from, &data);
            }
            self.deliver_typed_events();
            self.mods.mods.server_tick(self.ticks);
            self.mods.mods.tick_blocks();
            self.return_world();
            self.mods.players = players;
            self.show_mod_output();
            self.apply_mod_block_writes();
            self.apply_mod_entities();
            self.apply_typed_ops();
            self.apply_mod_containers();
            self.apply_mod_takes();
            self.apply_mod_pushes();
            self.apply_mod_breaks();
            self.apply_mod_item_uses();
            self.apply_mod_menus();
            self.apply_mod_console();
            self.apply_mod_player_stacks();
            self.apply_mod_signals();
            self.apply_mod_drops();
            self.apply_mod_effects();
            if self.ticks.is_multiple_of(200) {
                if self.mods.block_data_dirty {
                    self.flush_mod_block_data();
                }
                self.flush_typed_entities();
            }
        }

        /// Hands the mods the block data saved with this world: when the
        /// world or the set of running mods changed since the last tick.
        fn sync_mod_block_data(&mut self) {
            let key = (self.world_root.clone(), self.mods.mods.generation());
            if self.mods.block_data_key.as_ref() == Some(&key) {
                return;
            }
            if self.mods.block_data_key.as_ref().map(|k| &k.0) != Some(&key.0) {
                self.mods.block_data.clear();
                self.mods.block_data_dirty = false;
                if !key.0.as_os_str().is_empty()
                    && let Ok(dir) = std::fs::read_dir(key.0.join(super::BLOCK_DATA_DIR))
                {
                    for file in dir.flatten() {
                        let path = file.path();
                        let id = path.file_stem().and_then(|s| s.to_str());
                        if path.extension().is_none_or(|e| e != "blocks") {
                            continue;
                        }
                        let (Some(id), Ok(bytes)) = (id, std::fs::read(&path)) else {
                            continue;
                        };
                        for (dimension, pos, data) in super::decode_block_data(&bytes) {
                            self.mods
                                .block_data
                                .insert((id.to_owned(), dimension, pos), data);
                        }
                    }
                }
            }
            for ((id, dimension, pos), data) in &self.mods.block_data {
                self.mods
                    .mods
                    .load_block_data(id, dimension, *pos, data.clone());
            }
            self.mods.block_data_key = Some(key);
        }

        /// Records what a mod stored at (or removed from) a block position.
        fn store_mod_block_data(
            &mut self,
            mod_id: String,
            dimension: String,
            pos: [i32; 3],
            data: Option<Vec<u8>>,
        ) {
            let key = (mod_id, dimension, pos);
            let changed = match data {
                Some(data) => self.mods.block_data.insert(key, data.clone()) != Some(data),
                None => self.mods.block_data.remove(&key).is_some(),
            };
            self.mods.block_data_dirty |= changed;
        }

        /// Writes the block data of every mod to the world's folder, and
        /// removes the files of mods that keep none.
        pub(in crate::game) fn flush_mod_block_data(&mut self) {
            if !self.mods.block_data_dirty || self.world_root.as_os_str().is_empty() {
                return;
            }
            let dir = self.world_root.join(super::BLOCK_DATA_DIR);
            let mut ids: std::collections::BTreeMap<&str, Vec<(&str, [i32; 3], &[u8])>> =
                Default::default();
            for ((id, dimension, pos), data) in &self.mods.block_data {
                ids.entry(id).or_default().push((dimension, *pos, data));
            }
            let result = (|| -> std::io::Result<()> {
                if !ids.is_empty() {
                    std::fs::create_dir_all(&dir)?;
                }
                for (id, entries) in &ids {
                    // A mod's id names a file: nothing that could leave the folder.
                    if id.is_empty() || id.contains(['/', '\\', ':', '.']) {
                        continue;
                    }
                    let bytes = super::encode_block_data(entries.iter().copied());
                    let tmp = dir.join(format!("{id}.blocks.tmp"));
                    std::fs::write(&tmp, bytes)?;
                    std::fs::rename(&tmp, dir.join(format!("{id}.blocks")))?;
                }
                if let Ok(files) = std::fs::read_dir(&dir) {
                    for file in files.flatten() {
                        let path = file.path();
                        let stale = path.extension().is_some_and(|e| e == "blocks")
                            && path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .is_some_and(|id| !ids.contains_key(id));
                        if stale {
                            std::fs::remove_file(path)?;
                        }
                    }
                }
                Ok(())
            })();
            match result {
                Ok(()) => self.mods.block_data_dirty = false,
                Err(e) => log::warn!("Horizon: could not save server mods' block data: {e}"),
            }
        }

        /// Players' command trees list the running server mods' commands
        /// after vanilla's, whenever they change (or a hub opens).
        fn send_server_mod_commands(&mut self) {
            let key = self.mods.mods.generation();
            let Some(host) = self.lan.as_mut() else {
                return;
            };
            if host.hub.mod_commands_key() == Some(key) {
                return;
            }
            let tree = command_tree(self.mods.mods.server_commands());
            host.hub.set_mod_commands(key, tree);
        }

        /// Lends the levels' chunks to capability `world` for the calls up to
        /// [`Game::return_world`], when a running server mod has it.
        fn lend_world(&mut self) {
            if !self.mods.world {
                return;
            }
            // Before the chunks leave: the slots of the mod containers.
            let mut containers = std::collections::HashMap::new();
            if !self.mods.container_blocks.is_empty() {
                let positions: Vec<_> = self
                    .world
                    .block_entities
                    .iter_mut()
                    .map(|(p, _)| *p)
                    .collect();
                for p in positions {
                    let block = self.world.block(p.0, p.1, p.2);
                    let name = &self.res.facts().get(block).name;
                    if !self.mods.container_blocks.contains_key(&**name) {
                        continue;
                    }
                    if let Some(horizon_client::containers::BlockEntity::Chest(items)) =
                        self.world.block_entities.get(&p)
                    {
                        let slots = items
                            .iter()
                            .map(|s| s.as_ref().map(|s| (s.item.to_string(), s.count)))
                            .collect();
                        containers.insert([p.0, p.1, p.2], slots);
                    }
                }
            }
            let power = self.mod_block_power(containers.keys().copied());
            let dropped = self
                .dropped
                .iter()
                .take(MAX_LENT_ITEMS)
                .map(|e| horizon_mods::DroppedStack {
                    id: e.id,
                    item: e.stack.item.to_string(),
                    count: e.stack.count,
                    pos: [e.position.x, e.position.y, e.position.z],
                })
                .collect();
            let entities = std::mem::take(&mut self.world.block_entities);
            let mut dims = vec![(self.dim.id(), std::mem::take(&mut self.world.chunks))];
            for (dim, chunks) in self.levels.parked_chunks() {
                dims.push((dim.id(), std::mem::take(chunks)));
            }
            let registry = self.pack.assets.registry;
            let biomes = self.res.biomes().clone();
            self.mods.mods.lend_world(Box::new(LentWorld {
                registry,
                biomes,
                dims,
                containers,
                container_dim: self.dim.id(),
                dropped,
                power,
                entities,
                res: self.res.clone(),
                touched: Vec::new(),
            }));
        }

        /// The redstone signal at the positions mods keep block data at in
        /// the simulated dimension, and at `extra` ones, where the chunk is
        /// loaded: the most [`MAX_POWER_READS`] a tick.
        fn mod_block_power(
            &mut self,
            extra: impl Iterator<Item = [i32; 3]>,
        ) -> std::collections::HashMap<[i32; 3], u8> {
            if !self.mods.redstone {
                return Default::default();
            }
            let dim = self.dim.id();
            let mut at: std::collections::BTreeSet<[i32; 3]> = self
                .mods
                .block_data
                .keys()
                .filter(|(_, d, _)| d == dim)
                .map(|(_, _, p)| *p)
                .collect();
            at.extend(extra);
            let mut power = std::collections::HashMap::new();
            for p in at.into_iter().take(MAX_POWER_READS) {
                if self.world.chunks.get(&(p[0] >> 4, p[2] >> 4)).is_none() {
                    continue;
                }
                let pos = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                let best =
                    self.with_level(|b, level, ticks| b.best_neighbor_signal(level, ticks, pos));
                power.insert(p, best.clamp(0, 15) as u8);
            }
            power
        }

        /// Puts back the chunks [`Game::lend_world`] lent.
        fn return_world(&mut self) {
            let Some(view) = self.mods.mods.take_world() else {
                return;
            };
            let view: Box<dyn std::any::Any> = view;
            let Ok(lent) = view.downcast::<LentWorld>() else {
                return;
            };
            let lent = *lent;
            self.world.block_entities = lent.entities;
            for chunk in lent.touched {
                self.world.modified.insert(chunk);
            }
            let mut dims = lent.dims;
            if let Some(i) = dims.iter().position(|(d, _)| *d == self.dim.id()) {
                self.world.chunks = dims.swap_remove(i).1;
            }
            for (dim, chunks) in self.levels.parked_chunks() {
                if let Some(i) = dims.iter().position(|(d, _)| *d == dim.id()) {
                    *chunks = dims.swap_remove(i).1;
                }
            }
        }

        /// Makes the blocks server mods set in the level in place, in order,
        /// as `/setblock` does. Those of other dimensions wait for their level.
        pub(in crate::game) fn apply_mod_block_writes(&mut self) {
            if self.mods.block_writes.is_empty() {
                return;
            }
            let dim = self.dim.id();
            let (now, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.mods.block_writes)
                .into_iter()
                .partition(|(d, _)| d == dim);
            self.mods.block_writes = later;
            let registry = self.pack.assets.registry;
            for (_, b) in now {
                let state = match crate::game::render_control::parse_state(registry, &b.state) {
                    Ok(state) => state,
                    Err(e) => {
                        log::warn!("Horizon: a server mod set {}: {e}", b.state);
                        continue;
                    }
                };
                if let Err(e) = self.place_state([b.x, b.y, b.z], state, false) {
                    log::debug!(
                        "Horizon: a server mod's block at {} {} {}: {e:?}",
                        b.x,
                        b.y,
                        b.z
                    );
                }
            }
        }

        /// The block entity a mod's container block gets when first opened
        /// (a chest's 27 slots), none for any other block.
        pub(in crate::game) fn mod_container_entity(
            &self,
            block: &str,
        ) -> Option<horizon_client::containers::BlockEntity> {
            self.mods
                .container_blocks
                .get(block)
                .map(|&slots| horizon_client::containers::BlockEntity::Chest(vec![None; slots]))
        }

        /// Makes the changes server mods queued for their containers:
        /// opening one for a player, writing slots. Only the dimension the
        /// window simulates is there to change; others are dropped.
        pub(in crate::game) fn apply_mod_containers(&mut self) {
            use horizon_client::{containers::BlockEntity, items::ItemStack};
            for (mod_id, op) in std::mem::take(&mut self.mods.container_ops) {
                match op {
                    horizon_mods::ContainerOp::Register { .. } => {}
                    horizon_mods::ContainerOp::Open {
                        player,
                        dimension,
                        pos,
                    } => {
                        if dimension != self.dim.id() {
                            continue;
                        }
                        let key = (pos[0], pos[1], pos[2]);
                        let block = self.world.block(pos[0], pos[1], pos[2]);
                        let name = self.res.facts().get(block).name.clone();
                        let Some(entity) = self.mod_container_entity(&name) else {
                            log::debug!(
                                "Horizon: mod {mod_id} opened {name}, which is not a container it made"
                            );
                            continue;
                        };
                        self.world.block_entities.get_or_insert_with(key, || entity);
                        if self.options.execution_role.is_client()
                            && self.remote.is_none()
                            && self.player.is_some()
                            && self.player_uuid_string() == player
                        {
                            self.open_menu(pos);
                            continue;
                        }
                        let Some(mut host) = self.lan.take() else {
                            continue;
                        };
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| g.placed && uuid_string(g.uuid) == player)
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            self.container_opener_event(key, true);
                            if !self.lan_open_container(&mut host, conn, key) {
                                self.container_openers_changed(key, false);
                            }
                        }
                        self.lan = Some(host);
                    }
                    horizon_mods::ContainerOp::SetRule {
                        dimension,
                        pos,
                        slot,
                        allowed,
                    } => {
                        if dimension != self.dim.id() {
                            continue;
                        }
                        let block = self.world.block(pos[0], pos[1], pos[2]);
                        if self
                            .mod_container_entity(&self.res.facts().get(block).name)
                            .is_none()
                        {
                            continue;
                        }
                        let rules = self
                            .world
                            .slot_rules
                            .entry((pos[0], pos[1], pos[2]))
                            .or_default();
                        if rules.len() <= slot as usize {
                            rules.resize(slot as usize + 1, None);
                        }
                        rules[slot as usize] = allowed;
                        if rules.iter().all(Option::is_none) {
                            self.world.slot_rules.remove(&(pos[0], pos[1], pos[2]));
                        }
                    }
                    horizon_mods::ContainerOp::SetName {
                        dimension,
                        pos,
                        name,
                    } => {
                        if dimension != self.dim.id() {
                            continue;
                        }
                        let key = (pos[0], pos[1], pos[2]);
                        let block = self.world.block(pos[0], pos[1], pos[2]);
                        if self
                            .mod_container_entity(&self.res.facts().get(block).name)
                            .is_none()
                        {
                            continue;
                        }
                        match name {
                            Some(n) => self.world.block_names.insert(key, n),
                            None => self.world.block_names.remove(&key),
                        };
                        self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
                    }
                    horizon_mods::ContainerOp::SetSlot {
                        dimension,
                        pos,
                        slot,
                        stack,
                    } => {
                        if dimension != self.dim.id() {
                            continue;
                        }
                        let key = (pos[0], pos[1], pos[2]);
                        let block = self.world.block(pos[0], pos[1], pos[2]);
                        let name = self.res.facts().get(block).name.clone();
                        let Some(entity) = self.mod_container_entity(&name) else {
                            continue;
                        };
                        if let BlockEntity::Chest(items) =
                            self.world.block_entities.get_or_insert_with(key, || entity)
                            && let Some(cell) = items.get_mut(slot as usize)
                        {
                            *cell = stack.map(|(item, count)| ItemStack::new(item.as_str(), count));
                            self.world.modified.insert((pos[0] >> 4, pos[2] >> 4));
                        }
                    }
                }
            }
        }

        /// Plays the sounds and level events server mods queued, through the
        /// game's own level events so LAN guests see and hear them too. Only
        /// the dimension the window simulates is there; others are dropped.
        pub(in crate::game) fn apply_mod_effects(&mut self) {
            let queued = std::mem::take(&mut self.mods.effects);
            if queued.is_empty() {
                return;
            }
            let dim = self.dim.id();
            for (dimension, p, effect) in queued {
                if dimension != dim {
                    continue;
                }
                let pos = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                self.level_events.push(match effect {
                    horizon_mods::EffectOp::Sound {
                        event,
                        volume,
                        pitch,
                    } => horizon_world::level::LevelEvent::Sound {
                        pos,
                        sound: event.into(),
                        volume,
                        pitch,
                    },
                    horizon_mods::EffectOp::LevelEvent { id, data } => {
                        horizon_world::level::LevelEvent::LevelEvent { id, pos, data }
                    }
                });
            }
            if let Err(e) = self.process_level_events() {
                log::debug!("Horizon: a mod's effects: {e:?}");
            }
        }

        /// Breaks the blocks server mods asked to as a player with the tool
        /// would: loot, break particles and sound, air; the drops into the
        /// inventory named (a chest's or a mod container's slots, like
        /// stacks first), the rest dropped where the block stood.
        pub(in crate::game) fn apply_mod_breaks(&mut self) {
            use horizon_client::{containers::BlockEntity, items::ItemStack};
            let breaks = std::mem::take(&mut self.mods.breaks);
            if breaks.is_empty() {
                return;
            }
            for (dimension, p, tool, enchantments, into) in breaks {
                if dimension != self.dim.id() {
                    continue;
                }
                let state = self.world.block(p[0], p[1], p[2]);
                let facts = self.res.facts().get(state).clone();
                if self.res.facts().is_air(state) || facts.hardness < 0.0 {
                    continue;
                }
                let tool = tool.map(|t| {
                    let mut stack = ItemStack::new(t.as_str(), 1);
                    if !enchantments.is_empty() {
                        stack.data_mut().enchantments = enchantments
                            .iter()
                            .map(|(e, l)| (std::sync::Arc::from(e.as_str()), *l))
                            .collect();
                    }
                    stack
                });
                // As a player breaks it (ServerPlayerGameMode.destroyBlock):
                // the block entity kept for what the drops copy from it, then
                // the vanilla removal, which spills a container and plays
                // the destroy event; drops only for the right tool.
                let key = (p[0], p[1], p[2]);
                if let Some(entity) = self.world.block_entities.get(&key).cloned() {
                    let name = self.world.block_names.get(&key).cloned();
                    self.pending.record(p, &entity, &facts.name, name);
                }
                let removed = self.with_level(|b, level, ticks| {
                    b.player_destroy_block(
                        level,
                        ticks,
                        horizon_core::position::BlockPos::new(p[0], p[1], p[2]),
                        false,
                    )
                });
                if !removed {
                    continue;
                }
                let harvest = !facts.requires_tool
                    || self
                        .res
                        .items()
                        .correct_for_drops(tool.as_ref(), &facts.name);
                let drops = if harvest {
                    self.block_loot_at(p, state, tool.as_ref())
                } else {
                    Vec::new()
                };
                let drops: Vec<ItemStack> = drops
                    .into_iter()
                    .filter(|d| &*d.item != "minecraft:air")
                    .collect();
                let items = self.res.items();
                let max = |s: &ItemStack| items.get(&s.item).map_or(64, |i| i.max_stack).max(1);
                let maxes: Vec<u32> = drops.iter().map(max).collect();
                let mut rest = Vec::new();
                for (mut stack, cap) in drops.into_iter().zip(maxes) {
                    if let Some(k) = into
                        && let Some(BlockEntity::Chest(slots)) =
                            self.world.block_entities.get_mut(&(k[0], k[1], k[2]))
                    {
                        for merge in [true, false] {
                            for cell in slots.iter_mut() {
                                if stack.count == 0 {
                                    break;
                                }
                                match cell {
                                    Some(s) if merge && s.same_item_same_components(&stack) => {
                                        let put = stack.count.min(cap.saturating_sub(s.count));
                                        s.count += put;
                                        stack.count -= put;
                                    }
                                    None if !merge => {
                                        let put = stack.count.min(cap);
                                        let mut part = stack.clone();
                                        part.count = put;
                                        *cell = Some(part);
                                        stack.count -= put;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        self.world.modified.insert((k[0] >> 4, k[2] >> 4));
                    }
                    if stack.count > 0 {
                        rest.push(stack);
                    }
                }
                let at = [
                    f64::from(p[0]) + 0.5,
                    f64::from(p[1]) + 0.5,
                    f64::from(p[2]) + 0.5,
                ];
                for stack in rest {
                    self.spawn_item(stack, at, [0.0, 0.1, 0.0], 10);
                }
            }
            if let Err(e) = self.process_level_events() {
                log::debug!("Horizon: a server mod's breaks: {e:?}");
            }
        }

        /// This window's own player pressed a button of a mod's screen: the
        /// mod hears it at once.
        pub(in crate::game) fn local_menu_message(
            &mut self,
            message: horizon_mods::menu_layout::MenuMessage,
        ) {
            use horizon_mods::menu_layout::MenuMessage;
            let player = self.player_uuid_string();
            let Some((mod_id, None)) = self.mods.menu_owner.get(&player).cloned() else {
                return;
            };
            if matches!(message, MenuMessage::Closed { .. }) {
                self.mods.menu_owner.remove(&player);
            }
            let players = self.server_players(None);
            let mut snapshot = self.mod_snapshot_with(players);
            self.fill_mod_inventories(&mut snapshot, None);
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            match message {
                MenuMessage::Button { id, .. } => self.mods.mods.menu_button(&mod_id, &player, id),
                MenuMessage::Text { id, value, .. } => {
                    self.mods.mods.menu_text(&mod_id, &player, id, &value)
                }
                MenuMessage::Closed { .. } => self.mods.mods.menu_closed(&mod_id, &player),
                _ => {}
            }
            self.return_world();
            self.show_mod_output();
        }

        /// Each online player's inventory for `players.inventory`: 41 slots
        /// (hotbar, main, armor feet to head, off hand) and the selected slot.
        pub(in crate::game) fn fill_mod_inventories(
            &self,
            snapshot: &mut horizon_mods::Snapshot,
            host: Option<&LanHost>,
        ) {
            let stack = |s: &Option<horizon_client::items::ItemStack>| s.as_ref().map(stack_info);
            let mut out = std::collections::HashMap::new();
            if self.options.execution_role.is_client()
                && self.remote.is_none()
                && self.player.is_some()
            {
                let mut slots: Vec<_> = self.inventory.iter().take(36).map(stack).collect();
                slots.resize(36, None);
                slots.extend(self.armor.iter().map(stack));
                slots.push(stack(&self.offhand));
                out.insert(self.player_uuid_string(), (slots, self.selected as u32));
            }
            let guests = host
                .or(self.lan.as_ref())
                .into_iter()
                .flat_map(|h| &h.hub.guests);
            for g in guests.filter(|g| g.placed) {
                let mut slots: Vec<_> = g.inventory.iter().take(36).map(stack).collect();
                slots.resize(36, None);
                slots.extend(g.armor.iter().map(stack));
                slots.push(stack(&g.offhand));
                out.insert(uuid_string(g.uuid), (slots, g.selected as u32));
            }
            snapshot.inventories = out;
        }

        /// Puts the stacks server mods asked for in players' slots (0 to 35
        /// the inventory, 36 to 39 armor feet to head, 40 the off hand); a
        /// guest is sent their inventory again.
        pub(in crate::game) fn apply_mod_player_stacks(&mut self) {
            let sets = std::mem::take(&mut self.mods.player_stacks);
            if sets.is_empty() {
                return;
            }
            type Stack = Option<horizon_client::items::ItemStack>;
            fn cell<'a>(
                inventory: &'a mut [Stack],
                armor: &'a mut [Stack; 4],
                offhand: &'a mut Stack,
                slot: usize,
            ) -> Option<&'a mut Stack> {
                match slot {
                    0..36 => inventory.get_mut(slot),
                    36..40 => Some(&mut armor[slot - 36]),
                    40 => Some(offhand),
                    _ => None,
                }
            }
            let mut host = self.lan.take();
            let mut resend = Vec::new();
            for (mod_id, player, slot, stack) in sets {
                let local = self.options.execution_role.is_client()
                    && self.remote.is_none()
                    && self.player.is_some()
                    && self.player_uuid_string() == player;
                if local {
                    if let Some(c) = cell(
                        &mut self.inventory,
                        &mut self.armor,
                        &mut self.offhand,
                        slot as usize,
                    ) {
                        *c = stack.map(|info| stack_with(c.as_ref(), info, &mod_id));
                    }
                    continue;
                }
                let Some(h) = host.as_mut() else { continue };
                let Some(i) = h
                    .hub
                    .guests
                    .iter()
                    .position(|g| g.placed && uuid_string(g.uuid) == player)
                else {
                    continue;
                };
                let g = &mut h.hub.guests[i];
                if let Some(c) = cell(
                    &mut g.inventory,
                    &mut g.armor,
                    &mut g.offhand,
                    slot as usize,
                ) {
                    *c = stack.map(|info| stack_with(c.as_ref(), info, &mod_id));
                    if !resend.contains(&i) {
                        resend.push(i);
                    }
                }
            }
            if let Some(h) = host.as_mut() {
                for i in resend {
                    h.hub.send_inventory(i);
                }
            }
            self.lan = host;
        }

        /// Runs the commands server mods asked for as the server's console,
        /// quietly; one line in the log a mod.
        pub(in crate::game) fn apply_mod_console(&mut self) {
            let lines = std::mem::take(&mut self.mods.console_lines);
            if lines.is_empty() {
                return;
            }
            let mut ran: std::collections::BTreeMap<String, (usize, usize)> = Default::default();
            for (mod_id, line) in lines {
                let ok = self.as_console(|g| g.run_command_quietly(&line));
                let e = ran.entry(mod_id).or_default();
                e.0 += 1;
                e.1 += usize::from(!ok);
            }
            for (mod_id, (n, failed)) in ran {
                log::info!("[mod {mod_id}] ran {n} console command(s): {failed} failed");
            }
        }

        /// Opens the screens server mods asked for (their container's menu,
        /// with the layout sent to the player's client) and sends the data
        /// values they set. Only the dimension the window simulates.
        pub(in crate::game) fn apply_mod_menus(&mut self) {
            use horizon_mods::{
                MenuOp,
                menu_layout::{CHANNEL, MenuMessage},
            };
            let send = |host: &mut LanHost, conn: u32, message: &MenuMessage| {
                if let Ok(data) = serde_json::to_vec(message) {
                    host.hub.send_to(
                        conn,
                        &C::CustomPayload(CustomPayload {
                            channel: CHANNEL.into(),
                            data,
                        }),
                    );
                }
            };
            for (mod_id, op) in std::mem::take(&mut self.mods.menu_ops) {
                match op {
                    MenuOp::Open {
                        player,
                        dimension,
                        pos,
                        layout,
                    } => {
                        if dimension != self.dim.id() {
                            continue;
                        }
                        let key = (pos[0], pos[1], pos[2]);
                        let block = self.world.block(pos[0], pos[1], pos[2]);
                        let name = self.res.facts().get(block).name.clone();
                        let Some(entity) = self.mod_container_entity(&name) else {
                            continue;
                        };
                        self.world.block_entities.get_or_insert_with(key, || entity);
                        if self.options.execution_role.is_client()
                            && self.remote.is_none()
                            && self.player.is_some()
                            && self.player_uuid_string() == player
                        {
                            self.open_menu(pos);
                            self.mod_menu =
                                Some(crate::game::mod_menus::ModMenu::new(None, layout, false));
                            self.mods.menu_owner.insert(player, (mod_id, None));
                            continue;
                        }
                        let Some(mut host) = self.lan.take() else {
                            continue;
                        };
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| g.placed && uuid_string(g.uuid) == player)
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            self.container_opener_event(key, true);
                            if self.lan_open_container(&mut host, conn, key) {
                                let window = host
                                    .hub
                                    .guest(conn)
                                    .and_then(|g| g.menu.as_ref().map(|m| m.window));
                                if let Some(window) = window {
                                    send(&mut host, conn, &MenuMessage::Open { window, layout });
                                    self.mods.menu_owner.insert(player, (mod_id, Some(window)));
                                }
                            } else {
                                self.container_openers_changed(key, false);
                            }
                        }
                        self.lan = Some(host);
                    }
                    MenuOp::Panel { player, layout } => {
                        if self.options.execution_role.is_client()
                            && self.remote.is_none()
                            && self.player.is_some()
                            && self.player_uuid_string() == player
                        {
                            self.open_mod_panel(None, layout);
                            self.mods.menu_owner.insert(player, (mod_id, None));
                            continue;
                        }
                        let Some(mut host) = self.lan.take() else {
                            continue;
                        };
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| g.placed && uuid_string(g.uuid) == player)
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            send(&mut host, conn, &MenuMessage::Panel { layout });
                            self.mods
                                .menu_owner
                                .insert(player, (mod_id, Some(horizon_mods::menu_layout::PANEL)));
                        }
                        self.lan = Some(host);
                    }
                    MenuOp::Close { player } => {
                        if self
                            .mods
                            .menu_owner
                            .get(&player)
                            .is_none_or(|(m, _)| *m != mod_id)
                        {
                            continue;
                        }
                        let (_, window) = self.mods.menu_owner.remove(&player).unwrap_or_default();
                        if window.is_none() {
                            if self.mod_menu.as_ref().is_some_and(|m| m.panel) {
                                self.close_mod_panel(false);
                            } else {
                                self.close_container();
                            }
                            continue;
                        }
                        let Some(mut host) = self.lan.take() else {
                            continue;
                        };
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == player)
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            send(&mut host, conn, &MenuMessage::Close);
                        }
                        self.lan = Some(host);
                    }
                    MenuOp::Data {
                        player,
                        index,
                        value,
                    } => {
                        let Some((owner, window)) = self.mods.menu_owner.get(&player).cloned()
                        else {
                            continue;
                        };
                        if owner != mod_id {
                            continue;
                        }
                        let Some(window) = window else {
                            if let Some(m) = self.mod_menu.as_mut()
                                && m.window.is_none()
                                && let Some(cell) = m.data.get_mut(index as usize)
                            {
                                *cell = value;
                            }
                            continue;
                        };
                        let Some(mut host) = self.lan.take() else {
                            continue;
                        };
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| {
                                uuid_string(g.uuid) == player
                                    && g.menu.as_ref().is_some_and(|m| m.window == window)
                            })
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            send(
                                &mut host,
                                conn,
                                &MenuMessage::Data {
                                    window,
                                    index,
                                    value,
                                },
                            );
                        }
                        self.lan = Some(host);
                    }
                }
            }
        }

        /// Uses the items server mods asked to on blocks, as a player facing
        /// the face would: the block's own answer (`use_item_on`), else a
        /// block item placed against the face where there is air.
        pub(in crate::game) fn apply_mod_item_uses(&mut self) {
            use horizon_world::level::PlayerContext;
            let uses = std::mem::take(&mut self.mods.item_uses);
            for (dimension, p, face, item) in uses {
                if dimension != self.dim.id() {
                    continue;
                }
                let (direction, normal, look) = match face {
                    0 => (
                        horizon_core::geometry::Direction::Down,
                        [0, -1, 0],
                        (0.0, -90.0),
                    ),
                    1 => (
                        horizon_core::geometry::Direction::Up,
                        [0, 1, 0],
                        (0.0, 90.0),
                    ),
                    2 => (
                        horizon_core::geometry::Direction::North,
                        [0, 0, -1],
                        (0.0, 0.0),
                    ),
                    3 => (
                        horizon_core::geometry::Direction::South,
                        [0, 0, 1],
                        (180.0, 0.0),
                    ),
                    4 => (
                        horizon_core::geometry::Direction::West,
                        [-1, 0, 0],
                        (270.0, 0.0),
                    ),
                    _ => (
                        horizon_core::geometry::Direction::East,
                        [1, 0, 0],
                        (90.0, 0.0),
                    ),
                };
                let at = horizon_core::geometry::Vec3::new(
                    f64::from(p[0]) + 0.5 + f64::from(normal[0]) * 0.5,
                    f64::from(p[1]) + 0.5 + f64::from(normal[1]) * 0.5,
                    f64::from(p[2]) + 0.5 + f64::from(normal[2]) * 0.5,
                );
                // A player facing the face from in front of it.
                let player = PlayerContext {
                    yaw: look.0,
                    pitch: look.1,
                    may_build: true,
                    can_eat: false,
                    game_master: false,
                };
                let result = self.with_level(|b, level, ticks| {
                    b.use_item_on(
                        level,
                        ticks,
                        horizon_core::position::BlockPos::new(p[0], p[1], p[2]),
                        direction,
                        at,
                        player,
                        Some(item.as_str()),
                    )
                });
                if result.consumes_action() {
                    continue;
                }
                let Some(state) = self.res.items().get(item.as_str()).and_then(|i| i.block) else {
                    continue;
                };
                let to = [p[0] + normal[0], p[1] + normal[1], p[2] + normal[2]];
                if self
                    .res
                    .facts()
                    .is_air(self.world.block(to[0], to[1], to[2]))
                    && let Err(e) = self.place_state(to, state, false)
                {
                    log::debug!("Horizon: a server mod's {item} at {to:?}: {e:?}");
                }
            }
            if let Err(e) = self.process_level_events() {
                log::debug!("Horizon: a server mod's item uses: {e:?}");
            }
        }

        /// Sets the redstone signals server mods asked for, through the
        /// table a trapped chest's signal is read from, and tells the blocks
        /// around of a change. Only the dimension the window simulates.
        pub(in crate::game) fn apply_mod_signals(&mut self) {
            for (dimension, p, level) in std::mem::take(&mut self.mods.signals) {
                if dimension != self.dim.id() {
                    continue;
                }
                let state = self.world.block(p[0], p[1], p[2]);
                if self.res.facts().get(state).name.starts_with("minecraft:") {
                    continue;
                }
                let key = (p[0], p[1], p[2]);
                let before = self.lid_openers.get(&key).copied().unwrap_or(0);
                if before == i32::from(level) {
                    continue;
                }
                if level == 0 {
                    self.lid_openers.remove(&key);
                } else {
                    self.lid_openers.insert(key, i32::from(level));
                }
                let pos = horizon_core::position::BlockPos::new(p[0], p[1], p[2]);
                self.with_level(|b, level, ticks| {
                    let block = b.block(state);
                    b.update_neighbors_at(level, ticks, pos, block);
                    b.update_neighbors_at(level, ticks, pos.offset(0, -1, 0), block);
                });
            }
        }

        /// Applies the pushes server mods asked for: to item entities, and to
        /// players (the window's own, or a guest, whose client is sent the
        /// motion).
        pub(in crate::game) fn apply_mod_pushes(&mut self) {
            use horizon_core::geometry::Vec3;
            for (target, v) in std::mem::take(&mut self.mods.pushes) {
                let delta = Vec3::new(v[0], v[1], v[2]);
                match target {
                    horizon_mods::PushTarget::Item(id) => {
                        if let Some(e) = self.dropped.iter_mut().find(|e| e.id == id) {
                            e.velocity = e.velocity.add_vector(delta);
                        }
                    }
                    horizon_mods::PushTarget::Player(uuid) => {
                        if self.options.execution_role.is_client()
                            && self.remote.is_none()
                            && self.player_uuid_string() == uuid
                        {
                            if let Some(p) = self.player.as_mut() {
                                p.velocity = p.velocity.add_vector(delta);
                            }
                            continue;
                        }
                        let Some(mut host) = self.lan.take() else {
                            continue;
                        };
                        let conn = host
                            .hub
                            .guests
                            .iter()
                            .find(|g| uuid_string(g.uuid) == uuid)
                            .map(|g| g.conn);
                        if let Some(conn) = conn {
                            self.guest_hit_by(&mut host, conn, |g| {
                                if let Some(p) = g.player.as_mut() {
                                    p.velocity = p.velocity.add_vector(delta);
                                }
                            });
                        }
                        self.lan = Some(host);
                    }
                }
            }
        }

        /// Removes the item entities server mods took.
        pub(in crate::game) fn apply_mod_takes(&mut self) {
            let takes = std::mem::take(&mut self.mods.takes);
            if !takes.is_empty() {
                self.dropped.retain(|e| !takes.contains(&e.id));
            }
        }

        /// Drops the stacks server mods queued as item entities. Only the
        /// dimension the window simulates is there; others are dropped.
        pub(in crate::game) fn apply_mod_drops(&mut self) {
            for (dimension, pos, item, count) in std::mem::take(&mut self.mods.drops) {
                if dimension != self.dim.id() {
                    continue;
                }
                let stack = horizon_client::items::ItemStack::new(item.as_str(), count);
                self.spawn_item(stack, pos, [0.0, 0.2, 0.0], 10);
            }
        }

        /// Whether this armor stand is a mod's entity (`entities`): those are
        /// never saved with the world; the mod places them again.
        pub(in crate::game) fn is_mod_entity(&self, id: u32) -> bool {
            self.mods.mod_entities.values().any(|&s| s == id)
        }

        /// Makes the changes server mods queued for their entities, as armor
        /// stands (invisible markers holding the item). Only the dimension the
        /// window simulates is there to change; others are dropped.
        pub(in crate::game) fn apply_mod_entities(&mut self) {
            use horizon_client::{entities::ArmorStand, items::ItemStack};
            use horizon_core::geometry::Vec3;
            // Solid blocks that moved: where each stood and by how much.
            let mut carried: Vec<(Vec3, Vec3)> = Vec::new();
            let doomed = std::mem::take(&mut self.mods.doomed_blocks);
            if !doomed.is_empty() {
                self.falling
                    .retain(|f| !(f.mod_owned && doomed.contains(&f.id)));
            }
            for (mod_id, op) in std::mem::take(&mut self.mods.entity_ops) {
                match op {
                    horizon_mods::EntityOp::Spawn {
                        handle,
                        dimension,
                        pos,
                        yaw,
                        item,
                    } => {
                        if dimension != self.dim.id() {
                            log::debug!(
                                "Horizon: mod {mod_id} placed an entity in {dimension}, which is not simulated"
                            );
                            continue;
                        }
                        self.next_entity += 1;
                        let mut stand = ArmorStand::new(
                            self.next_entity,
                            Vec3::new(pos[0], pos[1], pos[2]),
                            yaw,
                        );
                        stand.invisible = true;
                        stand.marker = true;
                        stand.equipment[0] = Some(ItemStack::new(item.as_str(), 1));
                        self.mods.mod_entities.insert((mod_id, handle), stand.id);
                        self.armor_stands.push(stand);
                    }
                    horizon_mods::EntityOp::SpawnBlock {
                        handle,
                        dimension,
                        pos,
                        state,
                    } => {
                        if dimension != self.dim.id() {
                            continue;
                        }
                        let registry = self.pack.assets.registry;
                        let Ok(state) = crate::game::render_control::parse_state(registry, &state)
                        else {
                            continue;
                        };
                        self.next_entity += 1;
                        let at = Vec3::new(pos[0], pos[1], pos[2]);
                        self.falling.push(crate::game::FallingBlock {
                            id: self.next_entity,
                            state,
                            start_pos: horizon_core::position::BlockPos::new(
                                pos[0].floor() as i32,
                                pos[1].floor() as i32,
                                pos[2].floor() as i32,
                            ),
                            position: at,
                            previous: at,
                            velocity: Vec3::ZERO,
                            age: 0,
                            no_gravity: true,
                            drop_item: false,
                            fall_distance: 0.0,
                            stuck: Vec3::ZERO,
                            damage: Default::default(),
                            saved: None,
                            mod_owned: true,
                            yaw: 0.0,
                            pitch: 0.0,
                            solid: false,
                            target: None,
                            target_rotation: None,
                            previous_rotation: (0.0, 0.0),
                        });
                        self.mods
                            .mod_entities
                            .insert((mod_id, handle), self.next_entity);
                    }
                    horizon_mods::EntityOp::SetSolid { handle, solid } => {
                        if let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle))
                            && let Some(f) =
                                self.falling.iter_mut().find(|f| f.mod_owned && f.id == id)
                        {
                            f.solid = solid;
                        }
                    }
                    horizon_mods::EntityOp::Rotate { handle, yaw, pitch } => {
                        let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle)) else {
                            continue;
                        };
                        if let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id) {
                            a.yaw = yaw;
                        } else if let Some(f) =
                            self.falling.iter_mut().find(|f| f.mod_owned && f.id == id)
                        {
                            // The byte clients get: the server keeps the same.
                            let snap = |a: f32| (a * 256.0 / 360.0).round() * 360.0 / 256.0;
                            (f.yaw, f.pitch) = (snap(yaw), snap(pitch));
                        }
                    }
                    horizon_mods::EntityOp::Move { handle, pos, yaw } => {
                        let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle)) else {
                            continue;
                        };
                        let to = Vec3::new(pos[0], pos[1], pos[2]);
                        if let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id) {
                            a.position = to;
                            a.yaw = yaw;
                        } else if let Some(f) =
                            self.falling.iter_mut().find(|f| f.mod_owned && f.id == id)
                        {
                            if f.solid {
                                carried.push((f.position, to.subtract(f.position)));
                            }
                            f.previous = f.position;
                            f.position = to;
                        }
                    }
                    horizon_mods::EntityOp::SetItem { handle, item } => {
                        if let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle))
                            && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id)
                        {
                            a.equipment[0] = Some(ItemStack::new(item.as_str(), 1));
                        }
                    }
                    horizon_mods::EntityOp::SetSlot { handle, slot, item } => {
                        if let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle))
                            && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id)
                            && let Some(cell) = a.equipment.get_mut(slot)
                        {
                            *cell = item.map(|i| ItemStack::new(i.as_str(), 1));
                        }
                    }
                    horizon_mods::EntityOp::SetPose {
                        handle,
                        part,
                        rotation,
                    } => {
                        if let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle))
                            && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id)
                            && let Some(cell) = a.pose.get_mut(part)
                        {
                            *cell = rotation;
                        }
                    }
                    horizon_mods::EntityOp::Remove { handle } => {
                        if let Some(id) = self.mods.mod_entities.remove(&(mod_id, handle)) {
                            self.armor_stands.retain(|a| a.id != id);
                            // A block goes next tick, after its last move is sent.
                            if self.falling.iter().any(|f| f.mod_owned && f.id == id) {
                                self.mods.doomed_blocks.push(id);
                            }
                        }
                    }
                }
            }
            self.carry_riders(&carried);
        }

        /// Moves the players standing on a server mod's solid blocks with
        /// them: a block at `at` (bottom centre) moved by the delta. The
        /// window's own player is moved; a guest is sent a teleport that keeps
        /// their motion. One block's move carries a player at most once.
        fn carry_riders(
            &mut self,
            carried: &[(horizon_core::geometry::Vec3, horizon_core::geometry::Vec3)],
        ) {
            if carried.is_empty() {
                return;
            }
            // Feet over a block's top (within a little either way), inside its
            // square widened by a player's half width.
            let on = |feet: [f64; 3]| {
                carried.iter().find(|(at, _)| {
                    (feet[0] - at.x).abs() < 0.8
                        && (feet[2] - at.z).abs() < 0.8
                        && (feet[1] - (at.y + 1.0)).abs() < 0.3
                })
            };
            if self.options.execution_role.is_client()
                && self.remote.is_none()
                && let Some(p) = self.player.as_mut()
                && let Some((_, d)) = on([p.position.x, p.position.y, p.position.z])
            {
                p.position = p.position.add_vector(*d);
            }
            // A guest's own client carries them (it moves the block too), as
            // their client moves them anyway: a teleport here would fight it.
        }

        /// A player broke (or placed) `state` at `pos`: the player acting now.
        pub(in crate::game) fn mod_block_event(
            &mut self,
            pos: [i32; 3],
            state: BlockStateId,
            placed: bool,
        ) {
            if !self.hears_players() {
                return;
            }
            let who = self.lan_acting.map_or(Who::Local, Who::Conn);
            let state = self.state_string(state);
            self.mods.events.push(ModEvent::Block {
                who,
                pos,
                state,
                placed,
            });
        }

        /// The acting player used the block at `pos` with an empty-handed
        /// click: whether a server mod took it. Only blocks of a mod's own
        /// namespace are offered, so vanilla blocks cost nothing here.
        pub(in crate::game) fn mod_block_use(&mut self, pos: [i32; 3]) -> bool {
            if !self.hears_players() {
                return false;
            }
            let players = self.server_players(None);
            let id = match self.lan_acting {
                None => self.player_uuid_string(),
                Some(conn) => {
                    let guests = self.lan.iter().flat_map(|h| &h.hub.guests);
                    match guests.into_iter().find(|g| g.conn == conn) {
                        Some(g) => uuid_string(g.uuid),
                        None => return false,
                    }
                }
            };
            self.mod_block_used_by(None, players, &id, pos)
        }

        /// A guest's empty-handed click on the block at `pos`, offered to the
        /// server mods before the guest's turn runs: the host is not in
        /// `self.lan` while a guest's packet is handled, so it is passed in.
        /// A sneaking guest uses nothing, as the main-hand use of vanilla.
        pub(in crate::game) fn mod_block_use_by_guest(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            pos: [i32; 3],
        ) -> bool {
            if !self.hears_players() {
                return false;
            }
            let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else {
                return false;
            };
            if g.sneaking {
                return false;
            }
            let id = uuid_string(g.uuid);
            let players = self.server_players(Some(host));
            self.mod_block_used_by(Some(host), players, &id, pos)
        }

        /// A guest's right click with the item in their main hand, on a block
        /// (`target`: position and face) or in the air, offered to the
        /// server mods before the guest's turn runs: true when one took it.
        pub(in crate::game) fn mod_item_use_by_guest(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            target: Option<([i32; 3], u8)>,
        ) -> bool {
            if !self.hears_players() {
                return false;
            }
            let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else {
                return false;
            };
            let Some(item) = g.inventory.get(g.selected).cloned().flatten() else {
                return false;
            };
            let (id, sneaking) = (uuid_string(g.uuid), g.sneaking);
            let players = self.server_players(Some(host));
            let Some(me) = players.iter().find(|p| p.id == id).cloned() else {
                return false;
            };
            let dimension = self.dim.id();
            let target = target.map(|(p, face)| horizon_mods::BlockTarget {
                dimension: dimension.to_owned(),
                x: p[0],
                y: p[1],
                z: p[2],
                face,
                state: self.state_string(self.world.block(p[0], p[1], p[2])),
            });
            let mut snapshot = self.mod_snapshot_with(players);
            self.fill_mod_inventories(&mut snapshot, Some(&*host));
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            let used = self
                .mods
                .mods
                .item_used(&me, &item.item, sneaking, target.as_ref());
            self.return_world();
            self.route_mod_output(Some(host));
            self.apply_mod_block_writes();
            used
        }

        /// Offers the click of the player `id` on the block at `pos` to the
        /// server mods.
        fn mod_block_used_by(
            &mut self,
            host: Option<&mut LanHost>,
            players: Vec<ServerPlayer>,
            id: &str,
            pos: [i32; 3],
        ) -> bool {
            let state = self.world.block(pos[0], pos[1], pos[2]);
            let state = self.state_string(state);
            if state.starts_with("minecraft:") {
                return false;
            }
            let Some(me) = players.iter().find(|p| p.id == id).cloned() else {
                return false;
            };
            let dimension = self.dim.id();
            let mut snapshot = self.mod_snapshot_with(players);
            self.fill_mod_inventories(&mut snapshot, host.as_deref());
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            let pos = BlockPos {
                x: pos[0],
                y: pos[1],
                z: pos[2],
            };
            let used = self.mods.mods.block_used(&me, dimension, pos, &state);
            self.return_world();
            self.route_mod_output(host);
            self.apply_mod_block_writes();
            used
        }

        /// A chat message a player sent.
        pub(in crate::game) fn mod_player_chat(&mut self, who: Who, text: &str) {
            if self.hears_players() {
                self.mods.events.push(ModEvent::Chat(who, text.to_owned()));
            }
        }

        /// `/line` typed by the window's own player in-frame: true when a
        /// server mod owns it.
        pub(in crate::game) fn local_server_mod_command(&mut self, line: &str) -> bool {
            if !self.hears_players() {
                return false;
            }
            let players = self.server_players(None);
            let Some(me) = players.first().cloned() else {
                return false;
            };
            let mut snapshot = self.mod_snapshot_with(players);
            self.fill_mod_inventories(&mut snapshot, None);
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            let owned = self.mods.mods.server_command(&me, line);
            self.return_world();
            self.show_mod_output();
            self.apply_mod_block_writes();
            owned
        }

        /// `/line` typed by a connection's player: true when a server mod
        /// owns it, whatever the player's permission level.
        pub(in crate::game) fn guest_server_mod_command(
            &mut self,
            host: &mut LanHost,
            conn: u32,
            line: &str,
        ) -> bool {
            if !self.hears_players() {
                return false;
            }
            let Some(g) = host.hub.guest(conn) else {
                return false;
            };
            let id = uuid_string(g.uuid);
            let players = self.server_players(Some(host));
            let Some(me) = players.iter().find(|p| p.id == id).cloned() else {
                return false;
            };
            let mut snapshot = self.mod_snapshot_with(players);
            self.fill_mod_inventories(&mut snapshot, Some(&*host));
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            let owned = self.mods.mods.server_command(&me, line);
            self.return_world();
            if !owned {
                return false;
            }
            self.route_mod_output(Some(host));
            self.apply_mod_block_writes();
            true
        }

        /// Sends a server mod's line to the player with this id: the
        /// window's own chat for the local player, a system message for a
        /// connection's. `host` is the hub when it is out of `self.lan`.
        pub(in crate::game) fn tell_server_player(
            &mut self,
            host: Option<&mut LanHost>,
            id: &str,
            line: Text,
        ) {
            let local = self.options.execution_role.is_client()
                && self.remote.is_none()
                && self.player.is_some();
            if local && self.player_uuid_string() == id {
                self.add_mod_chat_line(line);
                return;
            }
            let Some(host) = host.or(self.lan.as_mut()) else {
                return;
            };
            let to = host
                .hub
                .guests
                .iter()
                .find(|g| g.placed && uuid_string(g.uuid) == id);
            if let Some(conn) = to.map(|g| g.conn) {
                host.hub.send_to(
                    conn,
                    &C::SystemChat(play::SystemChat {
                        content: component_nbt(&line.component()),
                        overlay: false,
                    }),
                );
            }
        }

        /// A message a mod sent with `net.send`. A client mod's goes to the
        /// server it plays on (this window's own world included); a server
        /// mod's to one player or all, this window's own player included.
        pub(in crate::game) fn route_mod_net(
            &mut self,
            mut host: Option<&mut LanHost>,
            mod_id: String,
            server: bool,
            player: Option<String>,
            data: Vec<u8>,
        ) {
            let channel = format!("{}{mod_id}", super::NET_CHANNEL);
            if !server {
                if self.remote.is_some() {
                    self.send(play::Serverbound::CustomPayload(CustomPayload {
                        channel,
                        data,
                    }));
                } else {
                    let me = self.player_uuid_string();
                    self.mods.net_to_server.push((mod_id, me, data));
                }
                return;
            }
            let local = self.options.execution_role.is_client()
                && self.remote.is_none()
                && self.player.is_some();
            let me = local.then(|| self.player_uuid_string());
            let targets = match player {
                Some(id) => vec![id],
                None => self
                    .server_players(host.as_deref())
                    .into_iter()
                    .map(|p| p.id)
                    .collect(),
            };
            for id in targets {
                if me.as_deref() == Some(id.as_str()) {
                    self.mods.net_to_client.push((mod_id.clone(), data.clone()));
                    continue;
                }
                let hub = match host.as_deref_mut() {
                    Some(h) => Some(h),
                    None => self.lan.as_mut(),
                };
                let Some(hub) = hub else { continue };
                let to = hub
                    .hub
                    .guests
                    .iter()
                    .find(|g| g.placed && uuid_string(g.uuid) == id)
                    .map(|g| g.conn);
                if let Some(conn) = to {
                    hub.hub.send_to(
                        conn,
                        &C::CustomPayload(CustomPayload {
                            channel: channel.clone(),
                            data: data.clone(),
                        }),
                    );
                }
            }
        }

        /// A guest's custom payload on a mod's channel, for the server mod
        /// of that id: queued for the tick's end.
        pub(in crate::game) fn guest_mod_net(
            &mut self,
            host: &LanHost,
            conn: u32,
            channel: &str,
            data: &[u8],
        ) {
            // A button of a mod's screen (`menus`), heard next tick.
            if channel == horizon_mods::menu_layout::CHANNEL {
                let Some(g) = host.hub.guests.iter().find(|g| g.conn == conn) else {
                    return;
                };
                use horizon_mods::menu_layout::{MenuMessage, PANEL};
                let Ok(message) = serde_json::from_slice::<MenuMessage>(data) else {
                    return;
                };
                let player = uuid_string(g.uuid);
                let window = match &message {
                    MenuMessage::Button { window, .. }
                    | MenuMessage::Text { window, .. }
                    | MenuMessage::Closed { window } => *window,
                    _ => return,
                };
                // A panel has no menu on the server; a container screen's
                // window is the one the guest has open.
                let open = window == PANEL || g.menu.as_ref().is_some_and(|m| m.window == window);
                let Some((mod_id, Some(w))) = self.mods.menu_owner.get(&player).cloned() else {
                    return;
                };
                if w != window || !open || self.mods.menu_events.len() >= MAX_QUEUED_NET {
                    return;
                }
                let message = match message {
                    MenuMessage::Text { window, id, value } => MenuMessage::Text {
                        window,
                        id,
                        value: value.chars().take(256).collect(),
                    },
                    MenuMessage::Closed { window } => {
                        self.mods.menu_owner.remove(&player);
                        MenuMessage::Closed { window }
                    }
                    other => other,
                };
                self.mods.menu_events.push((mod_id, player, message));
                return;
            }
            let Some(mod_id) = channel.strip_prefix(super::NET_CHANNEL) else {
                return;
            };
            let guest = host.hub.guests.iter().find(|g| g.conn == conn);
            if let Some(g) = guest.filter(|g| g.placed) {
                let queue = &mut self.mods.net_to_server;
                if queue.len() < MAX_QUEUED_NET {
                    queue.push((mod_id.to_owned(), uuid_string(g.uuid), data.to_vec()));
                }
            }
        }

        /// A custom payload from the server this window plays on, for the
        /// client mod of that channel's id.
        pub(in crate::game) fn remote_mod_net(&mut self, channel: &str, data: Vec<u8>) {
            let Some(mod_id) = channel.strip_prefix(super::NET_CHANNEL) else {
                return;
            };
            let queue = &mut self.mods.net_to_client;
            if queue.len() < MAX_QUEUED_NET {
                queue.push((mod_id.to_owned(), data));
            }
        }

        /// Routes a server mod's output: `Tell` and `Reply` to their player,
        /// `Broadcast` to everyone and the log.
        pub(in crate::game) fn route_server_output(
            &mut self,
            mut host: Option<&mut LanHost>,
            output: Output,
        ) {
            let named = |name: &str, text: String| {
                let mut line = Text::literal(format!("[{name}] ")).with_color(text::GRAY);
                line.push(Text::literal(text));
                line
            };
            match output {
                Output::Tell { name, player, text } => {
                    self.tell_server_player(host, &player, named(&name, text));
                }
                Output::Reply {
                    player,
                    text,
                    error,
                } => {
                    let line = Text::literal(text);
                    let line = if error {
                        line.with_color(text::RED)
                    } else {
                        line
                    };
                    self.tell_server_player(host, &player, line);
                }
                Output::Broadcast { name, text } => {
                    let plain = format!("[{name}] {text}");
                    if self.options.dedicated.is_some() {
                        super::super::dedicated::log("INFO", &plain);
                    } else {
                        log::info!("{plain}");
                    }
                    for p in self.server_players(host.as_deref()) {
                        let line = named(&name, text.clone());
                        self.tell_server_player(host.as_deref_mut(), &p.id, line);
                    }
                }
                Output::SetBlocks {
                    name,
                    dimension,
                    blocks,
                } => {
                    let queued = &mut self.mods.block_writes;
                    queued.extend(blocks.into_iter().map(|b| (dimension.clone(), b)));
                    if queued.len() > MAX_QUEUED_WRITES {
                        let over = queued.len() - MAX_QUEUED_WRITES;
                        queued.drain(..over);
                        log::warn!(
                            "Horizon: mod {name} set more blocks than wait for their level: {over} dropped"
                        );
                    }
                }
                Output::BlockData {
                    mod_id,
                    dimension,
                    pos,
                    data,
                } => self.store_mod_block_data(mod_id, dimension, pos, data),
                Output::Container {
                    op: horizon_mods::ContainerOp::Register { block, slots },
                    ..
                } => {
                    self.mods
                        .container_blocks
                        .entry(block)
                        .or_insert(slots as usize);
                }
                Output::Container { mod_id, op } => {
                    let queue = &mut self.mods.container_ops;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((mod_id, op));
                    }
                }
                Output::Effect {
                    dimension,
                    pos,
                    effect,
                    ..
                } => {
                    let queue = &mut self.mods.effects;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((dimension, pos, effect));
                    }
                }
                Output::SetPlayerStack {
                    mod_id,
                    player,
                    slot,
                    stack,
                } => {
                    let queue = &mut self.mods.player_stacks;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((mod_id, player, slot, stack));
                    }
                }
                Output::Typed { mod_id, op } => {
                    let queue = &mut self.mods.typed_ops;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((mod_id, op));
                    }
                }
                Output::Console { mod_id, line } => {
                    let queue = &mut self.mods.console_lines;
                    if queue.len() < MAX_QUEUED_NET {
                        queue.push((mod_id, line));
                    }
                }
                Output::Menu { mod_id, op } => {
                    let queue = &mut self.mods.menu_ops;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((mod_id, op));
                    }
                }
                Output::UseItem {
                    dimension,
                    pos,
                    face,
                    item,
                    ..
                } => {
                    let queue = &mut self.mods.item_uses;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((dimension, pos, face, item));
                    }
                }
                Output::Peer { from, to, data } => {
                    let queue = &mut self.mods.peer_messages;
                    if queue.len() < MAX_QUEUED_NET {
                        queue.push((from, to, data));
                    }
                }
                Output::Break {
                    dimension,
                    pos,
                    tool,
                    enchantments,
                    into,
                    ..
                } => {
                    let queue = &mut self.mods.breaks;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((dimension, pos, tool, enchantments, into));
                    }
                }
                Output::Signal {
                    dimension,
                    pos,
                    level,
                    ..
                } => {
                    let queue = &mut self.mods.signals;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((dimension, pos, level));
                    }
                }
                Output::Push {
                    target, velocity, ..
                } => {
                    let queue = &mut self.mods.pushes;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((target, velocity));
                    }
                }
                Output::Take { id, .. } => {
                    let queue = &mut self.mods.takes;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push(id);
                    }
                }
                Output::Drop {
                    dimension,
                    pos,
                    item,
                    count,
                    ..
                } => {
                    let queue = &mut self.mods.drops;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((dimension, pos, item, count));
                    }
                }
                Output::Entity { mod_id, op } => {
                    let queue = &mut self.mods.entity_ops;
                    if queue.len() < MAX_QUEUED_ENTITY_OPS {
                        queue.push((mod_id, op));
                    }
                }
                _ => {}
            }
        }
    }
}
