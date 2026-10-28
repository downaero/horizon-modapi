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
    }
    impl WorldView for LentWorld {
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
                return;
            }
            let players = self.server_players(None);
            let snapshot = self.mod_snapshot_with(players.clone());
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
            self.mods.mods.server_tick(self.ticks);
            self.mods.mods.tick_blocks();
            self.return_world();
            self.mods.players = players;
            self.show_mod_output();
            self.apply_mod_block_writes();
            self.apply_mod_entities();
            if self.mods.block_data_dirty && self.ticks % 200 == 0 {
                self.flush_mod_block_data();
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
            }));
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
                    horizon_mods::EntityOp::Move { handle, pos, yaw } => {
                        if let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle))
                            && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id)
                        {
                            a.position = Vec3::new(pos[0], pos[1], pos[2]);
                            a.yaw = yaw;
                        }
                    }
                    horizon_mods::EntityOp::SetItem { handle, item } => {
                        if let Some(&id) = self.mods.mod_entities.get(&(mod_id, handle))
                            && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == id)
                        {
                            a.equipment[0] = Some(ItemStack::new(item.as_str(), 1));
                        }
                    }
                    horizon_mods::EntityOp::Remove { handle } => {
                        if let Some(id) = self.mods.mod_entities.remove(&(mod_id, handle)) {
                            self.armor_stands.retain(|a| a.id != id);
                        }
                    }
                }
            }
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
            let state = self.world.block(pos[0], pos[1], pos[2]);
            let state = self.state_string(state);
            if state.starts_with("minecraft:") {
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
            let Some(me) = players.iter().find(|p| p.id == id).cloned() else {
                return false;
            };
            let dimension = self.dim.id();
            let snapshot = self.mod_snapshot_with(players);
            self.mods.mods.set_snapshot(snapshot);
            self.lend_world();
            let pos = BlockPos {
                x: pos[0],
                y: pos[1],
                z: pos[2],
            };
            let used = self.mods.mods.block_used(&me, dimension, pos, &state);
            self.return_world();
            self.show_mod_output();
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
            let snapshot = self.mod_snapshot_with(players);
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
            let snapshot = self.mod_snapshot_with(players);
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
