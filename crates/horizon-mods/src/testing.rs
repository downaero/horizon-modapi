//! A mod run with no game: `cargo horizon-mod test` (docs/mods.md, "Testing a mod").
//!
//! [`TestGame`] loads one `.hmod` from a scratch copy (its `storage` lives and
//! dies with the run) into the same sandbox the game uses, and plays the game's
//! part: the snapshot, ticks, frames, chat lines and commands, and for a
//! server mod the players joining, leaving, chatting and breaking or placing
//! blocks. [`Script`] is a
//! test written in TOML, `tests/<name>.toml` in the mod's folder: steps, each
//! saying what happens and what the mod must have shown by then.
use crate::{
    BlockPos, BlockTarget, Clock, Corner, GameMode, Mods, Output, Panel, Player, ServerPlayer,
    Snapshot, StackInfo, Status,
};
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

/// One mod in the sandbox, driven by a test.
pub struct TestGame {
    mods: Mods,
    dir: PathBuf,
    hmod: PathBuf,
    /// What the mod reads with `game-read`; ticks move the clock as the game does.
    pub player: Option<Player>,
    pub clock: Option<Clock>,
    /// The players on the server, for a server mod, in join order.
    pub players: Vec<ServerPlayer>,
    /// Every name that joined, for their id.
    names: Vec<String>,
    tick: u64,
    /// Capability `world`'s Overworld: the blocks a script set or the mod
    /// changed; every other block, and the other dimensions, are air. Every
    /// biome is the dimension's plainest: plains, nether wastes, the end.
    pub world: std::collections::BTreeMap<(i32, i32, i32), String>,
    /// The redstone signal the last step set, by position.
    pub power: std::collections::BTreeMap<(i32, i32, i32), u8>,
    /// The item entities lying in the Overworld; `take` removes them.
    pub items: Vec<crate::DroppedStack>,
    /// The Overworld's `transfer.forward`s: a block's storage is another's.
    pub forwards: std::collections::BTreeMap<(i32, i32, i32), [i32; 3]>,
    /// The signals the mod's blocks give (`redstone.set-output`): weak and
    /// strong by face, while the block is the same.
    pub outputs: std::collections::BTreeMap<(i32, i32, i32), [[u8; 2]; 6]>,
    /// What the mod showed during the last ticks, already applied to the
    /// world: [`TestGame::drain`] hands it out.
    shown: Vec<Output>,
    /// The mobs in the Overworld (`entities.living-nearby`): a step's
    /// `living`, then what the mod's `hurt` and `discard` leave of them.
    pub living: Vec<crate::Living>,
    /// The parts the mod draws at its blocks of the Overworld (`visuals`),
    /// gone with the block's data as in the game.
    pub visuals: std::collections::BTreeMap<(i32, i32, i32), Vec<crate::visual::VisualPart>>,
    /// The slots of the containers in the Overworld, by position: a step's
    /// `slots`, then what the mod changes with `containers.set-slot` and
    /// `transfer` (slot rules are not kept to here).
    pub containers: Containers,
    /// Each player's 41 slots, by id (`players.inventory`): a step's
    /// `inventory`, then the mod's `set-player-stack`.
    pub inventories: std::collections::HashMap<String, Vec<Option<StackInfo>>>,
    /// The package's own recipe files (`data/<ns>/recipe/`): id, type, JSON.
    /// `recipe-manager` sees these and no vanilla recipe.
    recipes: Vec<(String, String, String)>,
    /// Item tags by id, each its values as the files list them (`#` for a
    /// tag in a tag): the package's own over vanilla's.
    tags: std::sync::Arc<Tags>,
    /// Block tags by id, as `tags`.
    block_tags: std::sync::Arc<Tags>,
    /// Every item id (`recipe-manager.items`): vanilla's, when the prepared
    /// resources are found, then the package's own.
    item_ids: std::sync::Arc<Vec<String>>,
}

/// The world a script's mod reads: [`TestGame::world`], the signals and item
/// entities a step set, copied for the step.
struct ScriptWorld {
    blocks: std::collections::BTreeMap<(i32, i32, i32), String>,
    power: std::collections::BTreeMap<(i32, i32, i32), u8>,
    dropped: Vec<crate::DroppedStack>,
    living: Vec<crate::Living>,
    forwards: std::collections::BTreeMap<(i32, i32, i32), [i32; 3]>,
    recipes: Vec<(String, String, String)>,
    /// Item tags by id, each its values as the files list them (`#` for a
    /// tag in a tag): the package's own over vanilla's.
    tags: std::sync::Arc<Tags>,
    block_tags: std::sync::Arc<Tags>,
    item_ids: std::sync::Arc<Vec<String>>,
    containers: Containers,
}

/// Containers' slots by position, shared by a test game and the world it
/// lends a step, so what `transfer` changes stays.
pub type Containers = std::sync::Arc<
    std::sync::Mutex<std::collections::BTreeMap<(i32, i32, i32), Vec<Option<crate::SlotStack>>>>,
>;

/// The most items of one stack in a test container.
const STACK: u32 = 64;

/// The recipe files a package carries: id, type, JSON.
/// Item tags by id, with the values their files list.
type Tags = std::collections::BTreeMap<String, Vec<String>>;

/// Adds a tag file's values to `tags` (or replaces them, `"replace": true`).
fn add_tag(tags: &mut Tags, id: String, json: &str) {
    let Ok(file) = serde_json::from_str::<serde_json::Value>(json) else {
        return;
    };
    let values = file["values"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().or_else(|| v["id"].as_str()).map(str::to_owned));
    let list = tags.entry(id).or_default();
    if file["replace"].as_bool() == Some(true) {
        list.clear();
    }
    list.extend(values);
}

/// The prepared resources, when they are found: `HORIZON_RESOURCES`, or
/// `.local/horizon-client/26.3` in the working folder or one above it.
fn prepared_resources() -> Option<PathBuf> {
    std::env::var_os("HORIZON_RESOURCES")
        .map(PathBuf::from)
        .or_else(|| {
            let here = std::env::current_dir().ok()?;
            here.ancestors()
                .map(|d| d.join(".local/horizon-client/26.3"))
                .find(|d| d.join("data").is_dir())
        })
}

/// The item ids a test game knows: vanilla's in registry order, from the
/// prepared resources when they are found, then the package's own blocks'
/// items and items.
fn item_ids(hmod: &Path) -> Vec<String> {
    let mut ids: Vec<String> = prepared_resources()
        .and_then(|root| std::fs::read(root.join("command-registries.json")).ok())
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| serde_json::from_value(v["items"].clone()).ok())
        .unwrap_or_default();
    let content = crate::Package::read(hmod)
        .ok()
        .and_then(|p| crate::Content::read(hmod, &p.manifest).ok());
    if let Some(content) = content {
        let blocks = content.blocks.iter().filter(|b| b.item).map(|b| &b.id);
        ids.extend(blocks.chain(content.items.iter().map(|i| &i.id)).cloned());
    }
    ids
}

/// The tags of `registry` (`item`, `block`) a test game knows: vanilla's,
/// from the prepared resources when they are found, then the package's own
/// `data/<ns>/tags/<registry>/` (vanilla's and the convention tags it adds
/// to too).
fn package_tags(hmod: &Path, registry: &str) -> Tags {
    let mut tags = Tags::new();
    let folder = format!("/tags/{registry}/");
    if let Some(root) = prepared_resources()
        && let Ok(namespaces) = std::fs::read_dir(root.join("data"))
    {
        for ns in namespaces.flatten() {
            let namespace = ns.file_name().to_string_lossy().into_owned();
            let base = ns.path().join("tags").join(registry);
            let mut stack = vec![base.clone()];
            while let Some(dir) = stack.pop() {
                for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
                    let path = e.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if let (Some(rel), Ok(json)) = (
                        path.strip_prefix(&base).ok().and_then(|r| r.to_str()),
                        std::fs::read_to_string(&path),
                    ) {
                        let rel = rel.trim_end_matches(".json").replace('\\', "/");
                        add_tag(&mut tags, format!("{namespace}:{rel}"), &json);
                    }
                }
            }
        }
    }
    let Ok(file) = std::fs::File::open(hmod) else {
        return tags;
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return tags;
    };
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let name = entry.name().to_owned();
        let Some((ns, path)) = name
            .strip_prefix("data/")
            .and_then(|n| n.split_once(folder.as_str()))
        else {
            continue;
        };
        let Some(path) = path.strip_suffix(".json") else {
            continue;
        };
        let mut json = String::new();
        if std::io::Read::read_to_string(&mut entry, &mut json).is_ok() {
            add_tag(&mut tags, format!("{ns}:{path}"), &json);
        }
    }
    tags
}

/// Whether `item` is in tag `id`, through the tags it names, 16 deep.
fn in_tag(tags: &Tags, id: &str, item: &str, depth: u32) -> bool {
    depth < 16
        && tags.get(id).is_some_and(|values| {
            values.iter().any(|v| match v.strip_prefix('#') {
                Some(inner) => in_tag(tags, inner, item, depth + 1),
                None => v == item,
            })
        })
}

fn package_recipes(hmod: &Path) -> Vec<(String, String, String)> {
    let Ok(file) = std::fs::File::open(hmod) else {
        return Vec::new();
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..zip.len() {
        let Ok(mut entry) = zip.by_index(i) else {
            continue;
        };
        let name = entry.name().to_owned();
        let Some((ns, path)) = name
            .strip_prefix("data/")
            .and_then(|n| n.split_once("/recipe/"))
        else {
            continue;
        };
        let Some(path) = path.strip_suffix(".json") else {
            continue;
        };
        let mut json = String::new();
        if std::io::Read::read_to_string(&mut entry, &mut json).is_err() {
            continue;
        }
        let kind = serde_json::from_str::<serde_json::Value>(&json)
            .ok()
            .and_then(|v| v["type"].as_str().map(str::to_owned))
            .unwrap_or_default();
        out.push((format!("{ns}:{path}"), kind, json));
    }
    out
}

impl crate::WorldView for ScriptWorld {
    fn container(
        &self,
        dimension: &str,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<Vec<Option<crate::SlotStack>>> {
        let all = self.containers.lock().ok()?;
        (dimension == "minecraft:overworld").then(|| all.get(&(x, y, z)).cloned())?
    }
    fn item_slots(
        &self,
        dimension: &str,
        pos: [i32; 3],
        _face: Option<u8>,
    ) -> Option<Vec<Option<crate::SlotStack>>> {
        self.container(dimension, pos[0], pos[1], pos[2])
    }
    fn insert_item(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        _face: Option<u8>,
        item: &str,
        count: u32,
        simulate: bool,
    ) -> u32 {
        let Ok(mut all) = self.containers.lock() else {
            return count;
        };
        let Some(slots) = all
            .get_mut(&(pos[0], pos[1], pos[2]))
            .filter(|_| dimension == "minecraft:overworld")
        else {
            return count;
        };
        let mut left = count;
        // Like stacks first, then empty slots, as a hopper fills them.
        for merge in [true, false] {
            for slot in slots.iter_mut() {
                let put = match slot {
                    Some((i, n)) if merge && i == item => left.min(STACK.saturating_sub(*n)),
                    None if !merge => left.min(STACK),
                    _ => 0,
                };
                if put > 0 && !simulate {
                    match slot {
                        Some((_, n)) => *n += put,
                        None => *slot = Some((item.to_owned(), put)),
                    }
                }
                left -= put;
            }
        }
        left
    }
    fn extract_item(
        &mut self,
        dimension: &str,
        pos: [i32; 3],
        _face: Option<u8>,
        count: u32,
        simulate: bool,
    ) -> Option<crate::SlotStack> {
        let mut all = self.containers.lock().ok()?;
        let slots = all
            .get_mut(&(pos[0], pos[1], pos[2]))
            .filter(|_| dimension == "minecraft:overworld")?;
        let slot = slots.iter_mut().find(|s| s.is_some())?;
        let (item, n) = slot.clone()?;
        let take = count.min(n);
        if !simulate {
            *slot = (n > take).then(|| (item.clone(), n - take));
        }
        Some((item, take))
    }
    fn items(&self) -> Vec<String> {
        self.item_ids.to_vec()
    }
    fn forward(&self, dimension: &str, pos: [i32; 3]) -> Option<[i32; 3]> {
        let to = self.forwards.get(&(pos[0], pos[1], pos[2]))?;
        (dimension == "minecraft:overworld").then_some(*to)
    }
    fn living(&self, dimension: &str) -> Vec<crate::Living> {
        match dimension {
            "minecraft:overworld" => self.living.clone(),
            _ => Vec::new(),
        }
    }
    fn block_tags(&self, state: &str) -> Vec<String> {
        let block = state.split('[').next().unwrap_or(state);
        // A Script's world names vanilla's states bare too (`stone`).
        let block = match block.contains(':') {
            true => block.to_owned(),
            false => format!("minecraft:{block}"),
        };
        let tags = &self.block_tags;
        tags.keys()
            .filter(|t| in_tag(tags, t, &block, 0))
            .cloned()
            .collect()
    }
    fn recipes_of_type(&self, kind: &str) -> Vec<(String, String)> {
        if kind.starts_with("minecraft:") {
            return Vec::new();
        }
        self.recipes
            .iter()
            .filter(|(_, k, _)| k == kind)
            .map(|(id, _, json)| (id.clone(), json.clone()))
            .collect()
    }
    fn test_ingredient(&self, ingredient: &str, item: &str) -> bool {
        // An item's id, `#` and a tag, `{"item"}` or `{"tag"}`, or a list of them.
        let value = serde_json::from_str(ingredient)
            .unwrap_or_else(|_| serde_json::Value::String(ingredient.to_owned()));
        let is = |v: &serde_json::Value| match (v.as_str(), v["tag"].as_str()) {
            (Some(s), _) => match s.strip_prefix('#') {
                Some(tag) => in_tag(&self.tags, tag, item, 0),
                None => s == item,
            },
            (None, Some(tag)) => in_tag(&self.tags, tag, item, 0),
            (None, None) => v["item"].as_str() == Some(item),
        };
        match &value {
            serde_json::Value::Array(list) => list.iter().any(is),
            v => is(v),
        }
    }
    fn power(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<u8> {
        (dimension == "minecraft:overworld")
            .then(|| self.power.get(&(x, y, z)).copied())
            .flatten()
    }
    fn dropped(&self, dimension: &str) -> Vec<crate::DroppedStack> {
        if dimension == "minecraft:overworld" {
            self.dropped.clone()
        } else {
            Vec::new()
        }
    }
    fn block(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let here = (dimension == "minecraft:overworld")
            .then(|| self.blocks.get(&(x, y, z)).cloned())
            .flatten();
        Some(here.unwrap_or_else(|| "minecraft:air".into()))
    }
    fn biome(&self, dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<String> {
        let biome = match dimension {
            "minecraft:overworld" => "minecraft:plains",
            "minecraft:the_nether" => "minecraft:nether_wastes",
            "minecraft:the_end" => "minecraft:the_end",
            _ => return None,
        };
        Some(biome.into())
    }
    fn has_dimension(&self, dimension: &str) -> bool {
        matches!(
            dimension,
            "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
        )
    }
    fn check_state(&self, state: &str) -> Result<(), String> {
        // No registry here: the shape only (the game checks the names).
        let name = state.split('[').next().unwrap_or_default();
        let plain = |t: &str| {
            !t.is_empty()
                && t.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-/".contains(&b))
        };
        let ok = match name.split_once(':') {
            Some((ns, path)) => plain(ns) && plain(path),
            None => plain(name),
        };
        let closed = !state.contains('[') || state.ends_with(']');
        (ok && closed)
            .then_some(())
            .ok_or_else(|| format!("{state} is not a block state"))
    }
}

/// Where a script starts: standing at 0.5 64 0.5 in the Overworld, facing
/// south, full health and food, at the first tick of a new world's first morning.
pub fn default_player() -> Player {
    Player {
        x: 0.5,
        y: 64.0,
        z: 0.5,
        yaw: 0.0,
        pitch: 0.0,
        health: 20.0,
        food: 20,
        dimension: "minecraft:overworld".into(),
    }
}

impl TestGame {
    /// Starts the mod at `hmod`, as the game would with it turned on. Errs
    /// when it doesn't start, with the game's reason.
    pub fn load(hmod: &Path) -> Result<TestGame, String> {
        static RUN: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "horizon-mod-test-{}-{}",
            std::process::id(),
            RUN.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let copy = dir.join(hmod.file_name().unwrap_or("mod.hmod".as_ref()));
        let mut game = TestGame {
            mods: Mods::default(),
            hmod: copy.clone(),
            dir,
            player: Some(default_player()),
            clock: Some(Clock {
                game_time: 0,
                day_time: 0,
            }),
            players: Vec::new(),
            names: Vec::new(),
            tick: 0,
            world: Default::default(),
            power: Default::default(),
            items: Vec::new(),
            living: Vec::new(),
            forwards: Default::default(),
            outputs: Default::default(),
            shown: Vec::new(),
            visuals: Default::default(),
            containers: Default::default(),
            inventories: Default::default(),
            recipes: package_recipes(hmod),
            tags: std::sync::Arc::new(package_tags(hmod, "item")),
            block_tags: std::sync::Arc::new(package_tags(hmod, "block")),
            item_ids: std::sync::Arc::new(item_ids(hmod)),
        };
        copy_mod(hmod, &copy)?;
        game.start()?;
        Ok(game)
    }

    fn start(&mut self) -> Result<(), String> {
        self.mods = Mods::load(std::slice::from_ref(&self.hmod), |_| true, |_| false);
        match &self.mods.entries()[0].status {
            Status::Failed(reason) => Err(format!("the mod didn't start: {reason}")),
            _ if !self.mods.is_running() => Err("the mod has no code to run".into()),
            _ => Ok(()),
        }
    }

    /// Stops the mod and starts it again, as a rebuild does in the game: its
    /// memory starts over, its `storage` stays.
    pub fn reload(&mut self) -> Result<(), String> {
        self.mods = Mods::default();
        self.start()
    }

    fn snapshot(&mut self) {
        self.mods.set_snapshot(Snapshot {
            player: self.player.clone(),
            clock: self.clock,
            game_time: self.tick,
            day_time: self.clock.map_or(self.tick, |c| c.day_time),
            players: self.players.clone(),
            // Players look south, level.
            rotations: self
                .players
                .iter()
                .map(|p| (p.id.clone(), (0.0, 0.0)))
                .collect(),
            inventories: self
                .players
                .iter()
                .map(|p| {
                    let slots = self.inventories.get(&p.id).cloned();
                    (p.id.clone(), (slots.unwrap_or_else(|| vec![None; 41]), 0))
                })
                .collect(),
            // The GUI of a 1280 x 720 window at scale 3, as the trial run's.
            gui_size: (427.0, 240.0),
            ..Snapshot::default()
        });
    }
    /// Whether the mod is a server mod.
    pub fn server(&self) -> bool {
        self.mods.runs_server()
    }

    /// `n` game ticks; each moves the clock one tick on, as the game does.
    /// What the mod changed on a tick is in the world for the next, as the
    /// game applies it before the dimension's next tick.
    pub fn tick(&mut self, n: u64) {
        for _ in 0..n {
            self.tick += 1;
            if let Some(clock) = &mut self.clock {
                clock.game_time += 1;
                clock.day_time += 1;
            }
            self.snapshot();
            self.mods.tick(self.tick);
            self.mods.server_tick(self.tick);
            self.mods.tick_blocks();
            let lent = self.mods.take_world().is_some();
            let out = self.mods.drain();
            self.apply(&out);
            self.shown.extend(out);
            if lent {
                self.lend();
            }
        }
    }
    /// Lends the mod the world as it is now: blocks, signals, item entities,
    /// mobs, forwards and containers.
    fn lend(&mut self) {
        self.mods.lend_world(Box::new(ScriptWorld {
            blocks: self.world.clone(),
            power: self.power.clone(),
            dropped: self.items.clone(),
            living: self.living.clone(),
            forwards: self.forwards.clone(),
            recipes: self.recipes.clone(),
            tags: self.tags.clone(),
            block_tags: self.block_tags.clone(),
            item_ids: self.item_ids.clone(),
            containers: self.containers.clone(),
        }));
    }
    /// `n` frames, 1/60 s apart.
    pub fn frames(&mut self, n: u64) {
        self.snapshot();
        for _ in 0..n {
            self.mods.frame(1.0 / 60.0);
        }
    }
    /// A chat line the player receives.
    pub fn chat(&mut self, line: &str) {
        self.snapshot();
        self.mods.chat(line);
    }
    /// `/line` typed by the player (without the slash); false when the mod
    /// owns no such command.
    pub fn command(&mut self, line: &str) -> bool {
        self.snapshot();
        self.mods.command(line)
    }

    /// A player named `name` joins the server, standing where a script starts
    /// in survival. Their id is `00000000-0000-4000-8000-000000000001` for
    /// the first name that joined, `...0002` for the second, and so on.
    pub fn join(&mut self, name: &str) -> ServerPlayer {
        let n = match self.names.iter().position(|n| n == name) {
            Some(i) => i + 1,
            None => {
                self.names.push(name.to_owned());
                self.names.len()
            }
        };
        let p = default_player();
        let player = ServerPlayer {
            id: format!("00000000-0000-4000-8000-{n:012x}"),
            name: name.to_owned(),
            x: p.x,
            y: p.y,
            z: p.z,
            dimension: p.dimension,
            game_mode: GameMode::Survival,
            health: p.health,
        };
        self.players.push(player.clone());
        self.snapshot();
        self.mods.player_joined(&player);
        player
    }
    /// The player named `name` leaves; false when no such player is on.
    pub fn leave(&mut self, name: &str) -> bool {
        let Some(i) = self.players.iter().position(|p| p.name == name) else {
            return false;
        };
        let player = self.players.remove(i);
        self.snapshot();
        self.mods.player_left(&player);
        true
    }
    /// A chat message `player` sends, for a server mod.
    pub fn player_chat(&mut self, player: &ServerPlayer, line: &str) {
        self.snapshot();
        self.mods.player_chat(player, line);
    }
    /// `/line` typed by `player`, for a server mod; false when the mod owns
    /// no such command.
    pub fn player_command(&mut self, player: &ServerPlayer, line: &str) -> bool {
        self.snapshot();
        self.mods.server_command(player, line)
    }
    /// `player` breaks (or places) `state` at `pos`.
    pub fn block(&mut self, player: &ServerPlayer, pos: BlockPos, state: &str, placed: bool) {
        self.snapshot();
        if placed {
            self.mods.block_placed(player, pos, state);
        } else {
            self.mods.block_broken(player, pos, state);
        }
    }

    /// `player` right-clicks `state` at `pos` with an empty hand; whether
    /// a mod took the click.
    /// `face` is the face clicked (0 down, 1 up, 2 north ... 5 east), `hit`
    /// where the click hit, in blocks in the world.
    pub fn use_block(
        &mut self,
        player: &ServerPlayer,
        pos: BlockPos,
        face: Option<u8>,
        hit: [f64; 3],
        state: &str,
    ) -> bool {
        self.snapshot();
        self.mods
            .block_used(player, "minecraft:overworld", pos, face, Some(hit), state)
    }

    /// `player` right-clicks `state` at `pos` on `face`, hitting `hit`, with
    /// `item` in their main hand (`item-events`); whether a mod took the
    /// click.
    #[allow(clippy::too_many_arguments)]
    pub fn use_item_on(
        &mut self,
        player: &ServerPlayer,
        item: &str,
        sneaking: bool,
        pos: BlockPos,
        face: u8,
        hit: [f64; 3],
        state: &str,
    ) -> bool {
        self.snapshot();
        let target = BlockTarget {
            dimension: "minecraft:overworld".into(),
            x: pos.x,
            y: pos.y,
            z: pos.z,
            face,
            state: state.to_owned(),
        };
        self.mods
            .item_used(player, item, sneaking, Some(&target), Some(hit))
    }

    /// `player` right-clicks the mob `id` (one of [`TestGame::living`]) with
    /// `item` ("" for none) in their main hand (`living-events`); whether a
    /// mod took the click.
    pub fn use_item_on_entity(
        &mut self,
        player: &ServerPlayer,
        item: &str,
        sneaking: bool,
        id: u32,
    ) -> Result<bool, String> {
        self.snapshot();
        let target = self.living.iter().find(|l| l.id == id).cloned();
        let target = target.ok_or_else(|| format!("no mob {id}: add it with `living`"))?;
        Ok(self
            .mods
            .item_used_on_entity(player, item, sneaking, &target))
    }

    /// The scratch copy the mod runs from; its `storage` is beside it.
    pub fn hmod(&self) -> &Path {
        &self.hmod
    }
    /// The commands the mod registered.
    pub fn commands(&self) -> Vec<String> {
        let server = self.mods.server_commands().map(|c| c.name.as_str());
        self.mods
            .commands()
            .chain(server)
            .map(str::to_owned)
            .collect()
    }
    pub fn panel(&self) -> Option<&Panel> {
        self.mods.panels().next()
    }
    pub fn status(&self) -> &Status {
        &self.mods.entries()[0].status
    }
    /// What the mod showed since the last call, applied to the world.
    pub fn drain(&mut self) -> Vec<Output> {
        let fresh = self.mods.drain();
        self.apply(&fresh);
        let mut out = std::mem::take(&mut self.shown);
        out.extend(fresh);
        out
    }
    /// What `out` changes, as the game applies it: blocks set and broken,
    /// slots, player stacks, visuals, forwards, mobs, item entities taken.
    fn apply(&mut self, out: &[Output]) {
        let mut set = Vec::new();
        let overworld = |d: &str| d == "minecraft:overworld";
        for o in out {
            match o {
                Output::SetBlocks {
                    dimension, blocks, ..
                } if overworld(dimension) => {
                    for b in blocks {
                        set.push([b.x, b.y, b.z]);
                        let at = (b.x, b.y, b.z);
                        let block = |s: &str| s.split('[').next().unwrap_or_default().to_owned();
                        let was = self.world.get(&at).map(|s| block(s));
                        if was.as_deref() != Some(block(&b.state).as_str()) {
                            self.outputs.remove(&at);
                        }
                        self.world.insert(at, b.state.clone());
                    }
                }
                Output::Container {
                    op:
                        crate::ContainerOp::SetSlot {
                            dimension,
                            pos,
                            slot,
                            stack,
                        },
                    ..
                } if overworld(dimension) => {
                    if let Ok(mut all) = self.containers.lock() {
                        let slots = all
                            .entry((pos[0], pos[1], pos[2]))
                            .or_insert_with(|| vec![None; 27]);
                        if let Some(s) = slots.get_mut(*slot as usize) {
                            *s = stack.clone();
                        }
                    }
                }
                Output::SetPlayerStack {
                    player,
                    slot,
                    stack,
                    ..
                } => {
                    let slots = self
                        .inventories
                        .entry(player.clone())
                        .or_insert_with(|| vec![None; 41]);
                    if let Some(s) = slots.get_mut(*slot as usize) {
                        *s = stack.clone();
                    }
                }
                // A block the mod broke is air; its drops are the game's.
                Output::Break { dimension, pos, .. } if overworld(dimension) => {
                    self.world.remove(&(pos[0], pos[1], pos[2]));
                    self.outputs.remove(&(pos[0], pos[1], pos[2]));
                }
                Output::RedstoneOutput {
                    dimension,
                    pos,
                    face,
                    weak,
                    strong,
                    ..
                } if overworld(dimension) => {
                    let at = (pos[0], pos[1], pos[2]);
                    let faces = self.outputs.entry(at).or_default();
                    for (f, out) in faces.iter_mut().enumerate() {
                        if face.is_none_or(|face| usize::from(face) == f) {
                            *out = [*weak, *strong];
                        }
                    }
                    if *faces == [[0; 2]; 6] {
                        self.outputs.remove(&at);
                    }
                }
                Output::Visual {
                    dimension,
                    pos,
                    parts,
                    ..
                } if overworld(dimension) => {
                    let at = (pos[0], pos[1], pos[2]);
                    if parts.is_empty() {
                        self.visuals.remove(&at);
                    } else {
                        // Scrolls and orientations stay on the parts by index,
                        // and fluid boxes after them, as in the self.
                        let mut parts = parts.clone();
                        if let Some(old) = self.visuals.get(&at) {
                            let own = old.iter().filter(|o| o.fluid.is_none());
                            for (p, o) in parts.iter_mut().zip(own) {
                                p.scroll = o.scroll.clone();
                                p.orientation = o.orientation;
                            }
                            parts.extend(old.iter().filter(|o| o.fluid.is_some()).cloned());
                        }
                        self.visuals.insert(at, parts);
                    }
                }
                Output::Living { op, .. } => match op {
                    crate::LivingOp::Hurt { id, amount, .. } => {
                        if let Some(l) = self.living.iter_mut().find(|l| l.id == *id) {
                            l.health -= amount;
                        }
                        self.living.retain(|l| l.health > 0.0);
                    }
                    crate::LivingOp::Discard { id } => self.living.retain(|l| l.id != *id),
                },
                Output::VisualFluids {
                    dimension,
                    pos,
                    boxes,
                    ..
                } if overworld(dimension) => {
                    let at = (pos[0], pos[1], pos[2]);
                    let mut parts: Vec<_> = self
                        .visuals
                        .get(&at)
                        .into_iter()
                        .flatten()
                        .filter(|p| p.fluid.is_none())
                        .cloned()
                        .collect();
                    parts.extend(boxes.iter().cloned());
                    if parts.is_empty() {
                        self.visuals.remove(&at);
                    } else {
                        self.visuals.insert(at, parts);
                    }
                }
                Output::Forward { dimension, pos, to } if overworld(dimension) => {
                    let at = (pos[0], pos[1], pos[2]);
                    match to {
                        Some(t) => self.forwards.insert(at, *t),
                        None => self.forwards.remove(&at),
                    };
                }
                Output::VisualOrientation {
                    dimension,
                    pos,
                    orientations,
                    ..
                } if overworld(dimension) => {
                    if let Some(parts) = self.visuals.get_mut(&(pos[0], pos[1], pos[2])) {
                        for (i, p) in parts.iter_mut().enumerate() {
                            p.orientation = orientations
                                .iter()
                                .rev()
                                .find(|(at, _)| *at as usize == i)
                                .map(|(_, q)| *q);
                        }
                    }
                }
                Output::VisualScroll {
                    dimension,
                    pos,
                    scrolls,
                    ..
                } if overworld(dimension) => {
                    if let Some(parts) = self.visuals.get_mut(&(pos[0], pos[1], pos[2])) {
                        for (i, p) in parts.iter_mut().enumerate() {
                            p.scroll = scrolls
                                .iter()
                                .find(|(at, _)| *at as usize == i)
                                .map(|(_, s)| s.clone());
                        }
                    }
                }
                Output::BlockData {
                    dimension,
                    pos,
                    data: None,
                    ..
                } if overworld(dimension) => {
                    self.visuals.remove(&(pos[0], pos[1], pos[2]));
                }
                _ => {}
            }
        }
        let taken: Vec<u32> = out
            .iter()
            .filter_map(|o| match o {
                Output::Take { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        self.items.retain(|i| !taken.contains(&i.id));
        // The mod's ticking blocks among those its `set-blocks` put get their
        // data, as the game gives them: they tick from the next tick on.
        if !set.is_empty() {
            let lent = self.mods.take_world().is_some();
            self.lend();
            self.mods.adopt_blocks("minecraft:overworld", &set);
            if !lent {
                self.mods.take_world();
            }
        }
    }
}

impl Drop for TestGame {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A `.hmod` file, or a mod folder (`horizon-mod.toml` + `mod.wasm`).
fn copy_mod(from: &Path, to: &Path) -> Result<(), String> {
    let err = |e: std::io::Error| format!("{}: {e}", from.display());
    if !from.is_dir() {
        return std::fs::copy(from, to).map(drop).map_err(err);
    }
    std::fs::create_dir_all(to).map_err(err)?;
    for entry in std::fs::read_dir(from).map_err(err)? {
        let entry = entry.map_err(err)?;
        if entry.file_type().map_err(err)?.is_file() {
            std::fs::copy(entry.path(), to.join(entry.file_name())).map_err(err)?;
        }
    }
    Ok(())
}

/// A test in TOML: where a client mod's player starts, then steps run in order.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Script {
    #[serde(default)]
    pub player: PlayerSet,
    #[serde(default)]
    pub clock: ClockSet,
    #[serde(default, rename = "step")]
    pub steps: Vec<Step>,
}

/// Fields of the player to change; the others keep their values. A server
/// mod's players have no `yaw`, `pitch` or `food`, and a client mod's player
/// no `game-mode`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct PlayerSet {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    yaw: Option<f32>,
    pitch: Option<f32>,
    health: Option<f32>,
    food: Option<u32>,
    dimension: Option<String>,
    game_mode: Option<Mode>,
}

/// `players.game-mode` as a script spells it.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}
impl From<Mode> for GameMode {
    fn from(m: Mode) -> GameMode {
        match m {
            Mode::Survival => GameMode::Survival,
            Mode::Creative => GameMode::Creative,
            Mode::Adventure => GameMode::Adventure,
            Mode::Spectator => GameMode::Spectator,
        }
    }
}

/// A redstone signal reaching a block (capability `redstone`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerSet {
    x: i32,
    y: i32,
    z: i32,
    level: u8,
}

/// The block a player right-clicks with an empty hand, and the face they
/// click (`down`, `up`, `north`, `south`, `west`, `east`; none: unknown).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseAt {
    x: i32,
    y: i32,
    z: i32,
    face: Option<String>,
    /// Where the click hits, in blocks in the world
    /// (`block-entities.clicked-at`); none: the middle of the face (of the
    /// block, with no face).
    at: Option<[f64; 3]>,
    /// The item in their main hand (`item-events`); none: an empty hand.
    item: Option<String>,
    /// They sneak as they click (with an item).
    #[serde(default)]
    sneaking: bool,
}

/// A mob in the Overworld.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivingSet {
    id: u32,
    /// `minecraft:blaze`.
    kind: String,
    x: f64,
    y: f64,
    z: f64,
    #[serde(default = "full_health")]
    health: f32,
}

fn full_health() -> f32 {
    20.0
}

/// A right click on a mob, with the item in the main hand.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UseEntity {
    id: u32,
    /// None: an empty hand.
    item: Option<String>,
    #[serde(default)]
    sneaking: bool,
}

/// An item entity lying in the Overworld (capability `items`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemSet {
    id: u32,
    item: String,
    #[serde(default = "one")]
    count: u32,
    x: f64,
    y: f64,
    z: f64,
}

fn one() -> u32 {
    1
}

/// A block a player breaks or places.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockSet {
    x: i32,
    y: i32,
    z: i32,
    state: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct ClockSet {
    game_time: Option<u64>,
    day_time: Option<u64>,
}

/// One step. Its parts happen in this order: `reload`, `title-screen`,
/// `blocks`, `join`, `player`, `clock`, `ticks`, `frames`, `chat`, `command`,
/// `break`, `place`, `leave`; then the blocks the mod set are made, and
/// `expect` is checked against what the mod showed and changed during the
/// step. For a server mod, `player`, `chat`, `command`, `break`
/// and `place` are the doing of the player named by `as`, or of the first
/// player on.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Step {
    #[serde(default)]
    reload: bool,
    /// Leave the world: the mod reads no player and no clock until a step sets them.
    #[serde(default)]
    title_screen: bool,
    player: Option<PlayerSet>,
    clock: Option<ClockSet>,
    #[serde(default)]
    ticks: u64,
    #[serde(default)]
    frames: u64,
    chat: Option<String>,
    command: Option<String>,
    /// Server mods: a player joins, leaves, breaks or places a block.
    join: Option<String>,
    leave: Option<String>,
    #[serde(rename = "as")]
    actor: Option<String>,
    #[serde(rename = "break")]
    broken: Option<BlockSet>,
    place: Option<BlockSet>,
    /// Server mods: the player right-clicks the block there with an empty
    /// hand (`block-events.block-used`).
    #[serde(rename = "use")]
    used: Option<UseAt>,
    /// Blocks of the Overworld before the rest of the step (capability
    /// `world`); every other block is air.
    #[serde(default)]
    blocks: Vec<BlockSet>,
    /// The redstone signal at those positions from now on (capability
    /// `redstone`); a position left out has none.
    power: Option<Vec<PowerSet>>,
    /// The item entities lying about from now on (capability `items`).
    items: Option<Vec<ItemSet>>,
    /// The mobs about from now on (`entities.living-nearby`).
    living: Option<Vec<LivingSet>>,
    /// Server mods: the player right-clicks one of them with an item
    /// (`living-events.item-used-on-entity`).
    use_entity: Option<UseEntity>,
    /// Stacks put in containers' slots of the Overworld (a container with
    /// none before has 27 empty slots); count 0 empties the slot.
    #[serde(default)]
    slots: Vec<SlotSet>,
    /// Stacks put in the acting player's slots (0 to 40, as
    /// `players.inventory` counts them); count 0 empties one.
    #[serde(default)]
    inventory: Vec<InventorySet>,
    #[serde(default)]
    expect: Expect,
}

/// A stack in a container's slot.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SlotSet {
    x: i32,
    y: i32,
    z: i32,
    slot: u32,
    item: String,
    count: u32,
}

/// A stack in a player's slot.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InventorySet {
    slot: u32,
    item: String,
    count: u32,
}

/// What a step checks; a part left out is not checked.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Expect {
    /// The commands the mod owns.
    commands: Option<Vec<String>>,
    /// Every chat line shown during the step, in order.
    chat: Option<Vec<String>>,
    /// The command's feedback, and its red error.
    feedback: Option<String>,
    error: Option<String>,
    /// Server mods: every `players.tell` as `"Name: text"`, and every
    /// `players.broadcast`, in order.
    told: Option<Vec<String>>,
    broadcast: Option<Vec<String>>,
    /// The panel's lines (`[]`: no panel) and its corner.
    panel: Option<Vec<String>>,
    corner: Option<String>,
    /// Blocks of the Overworld after the step, the mod's changes made
    /// (capability `world`).
    blocks: Option<Vec<BlockSet>>,
    /// The mobs left after the step, as `"id kind health"`: what the mod's
    /// `hurt` took off and `discard` took away.
    living: Option<Vec<String>>,
    /// Capability `items`: every `drop` as `"item xN at x y z"` and the ids
    /// of the entities `take` removed, in order.
    drops: Option<Vec<String>>,
    taken: Option<Vec<u32>>,
    /// Capability `effects`: every sound as `"sound event at x y z"` and
    /// level event as `"event id data at x y z"`, in order.
    effects: Option<Vec<String>>,
    /// Capability `containers`: every `set-name` as `"name at x y z"` (`-` for
    /// none), in order.
    names: Option<Vec<String>>,
    /// Every filled slot of every container after the step, by position then
    /// slot, as `"x y z slot item xN"`.
    slots: Option<Vec<String>>,
    /// Every filled slot of the acting player's after the step, as
    /// `"slot item xN"`.
    inventory: Option<Vec<String>>,
    /// Capability `block-entities`: the parts drawn at the mod's blocks after
    /// the step, by position, each as `"x y z state speed"` (degrees a tick),
    /// then `" offset [x, y, z] velocity [x, y, z]"` when it is off its block's
    /// corner or moving.
    visuals: Option<Vec<String>>,
    /// Capability `redstone`: the signals the mod's blocks give after the
    /// step (`set-output`), a face to a line as `"x y z face weak strong"`,
    /// faces giving none left out.
    outputs: Option<Vec<String>>,
    /// The mod is turned off during this step, for a reason holding this text.
    /// Without it, a mod that stops fails the test.
    stopped: Option<String>,
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }

    /// Runs every step on `game`; the error names the first step that failed.
    pub fn run(&self, game: &mut TestGame) -> Result<(), String> {
        if let Some(p) = &mut game.player {
            self.player.apply(p);
        }
        if let Some(c) = &mut game.clock {
            self.clock.apply(c);
        }
        for (i, step) in self.steps.iter().enumerate() {
            step.run(game)
                .map_err(|e| format!("step {} ({}): {e}", i + 1, step.describe()))?;
        }
        Ok(())
    }
}

impl PlayerSet {
    fn apply(&self, p: &mut Player) {
        let set = |to: &mut f64, v: Option<f64>| *to = v.unwrap_or(*to);
        set(&mut p.x, self.x);
        set(&mut p.y, self.y);
        set(&mut p.z, self.z);
        p.yaw = self.yaw.unwrap_or(p.yaw);
        p.pitch = self.pitch.unwrap_or(p.pitch);
        p.health = self.health.unwrap_or(p.health);
        p.food = self.food.unwrap_or(p.food);
        if let Some(d) = &self.dimension {
            p.dimension.clone_from(d);
        }
    }
    fn apply_server(&self, p: &mut ServerPlayer) {
        let set = |to: &mut f64, v: Option<f64>| *to = v.unwrap_or(*to);
        set(&mut p.x, self.x);
        set(&mut p.y, self.y);
        set(&mut p.z, self.z);
        p.health = self.health.unwrap_or(p.health);
        p.game_mode = self.game_mode.map_or(p.game_mode, GameMode::from);
        if let Some(d) = &self.dimension {
            p.dimension.clone_from(d);
        }
    }
}

impl ClockSet {
    fn apply(&self, c: &mut Clock) {
        c.game_time = self.game_time.unwrap_or(c.game_time);
        c.day_time = self.day_time.unwrap_or(c.day_time);
    }
}

impl Step {
    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.reload {
            parts.push("reload".to_string());
        }
        let count = |n: u64, what: &str| format!("{n} {what}{}", if n == 1 { "" } else { "s" });
        if self.ticks > 0 {
            parts.push(count(self.ticks, "tick"));
        }
        if self.frames > 0 {
            parts.push(count(self.frames, "frame"));
        }
        if let Some(line) = &self.chat {
            parts.push(format!("chat {line:?}"));
        }
        if let Some(line) = &self.command {
            parts.push(format!("/{line}"));
        }
        if let Some(name) = &self.join {
            parts.insert(0, format!("{name} joins"));
        }
        for (what, b) in [("breaks", &self.broken), ("places", &self.place)] {
            if let Some(b) = b {
                parts.push(format!("{what} {} at {} {} {}", b.state, b.x, b.y, b.z));
            }
        }
        if let Some(u) = &self.used {
            parts.push(format!("uses {} {} {}", u.x, u.y, u.z));
        }
        if let Some(u) = &self.use_entity {
            parts.push(format!("uses mob {}", u.id));
        }
        if let Some(name) = &self.leave {
            parts.push(format!("{name} leaves"));
        }
        if parts.is_empty() {
            parts.push("checks".into());
        }
        parts.join(", ")
    }

    fn run(&self, game: &mut TestGame) -> Result<(), String> {
        if self.reload {
            game.reload()?;
        }
        if self.title_screen {
            game.player = None;
            game.clock = None;
        }
        for b in &self.blocks {
            game.world.insert((b.x, b.y, b.z), b.state.clone());
        }
        if let Some(power) = &self.power {
            game.power = power.iter().map(|p| ((p.x, p.y, p.z), p.level)).collect();
        }
        if let Some(items) = &self.items {
            game.items = items
                .iter()
                .map(|i| crate::DroppedStack {
                    id: i.id,
                    item: i.item.clone(),
                    count: i.count,
                    pos: [i.x, i.y, i.z],
                })
                .collect();
        }
        if let Some(living) = &self.living {
            game.living = living
                .iter()
                .map(|l| crate::Living {
                    id: l.id,
                    kind: l.kind.clone(),
                    pos: [l.x, l.y, l.z],
                    health: l.health,
                    player: None,
                })
                .collect();
        }
        if let Ok(mut all) = game.containers.lock() {
            for s in &self.slots {
                let slots = all.entry((s.x, s.y, s.z)).or_insert_with(|| vec![None; 27]);
                if slots.len() <= s.slot as usize {
                    slots.resize(s.slot as usize + 1, None);
                }
                slots[s.slot as usize] = (s.count > 0).then(|| (s.item.clone(), s.count));
            }
        }
        game.lend();
        // Blocks a step set, as /setblock sets them: a mod's ticking blocks
        // among them get their data as in the game.
        let set: Vec<[i32; 3]> = self.blocks.iter().map(|b| [b.x, b.y, b.z]).collect();
        if !set.is_empty() {
            game.mods.adopt_blocks("minecraft:overworld", &set);
        }
        if let Some(name) = &self.join {
            game.join(name);
        }
        let server = game.server();
        let actor = |game: &TestGame| match &self.actor {
            Some(name) => game
                .players
                .iter()
                .position(|p| p.name == *name)
                .ok_or_else(|| format!("{name} is not on the server")),
            None => (!game.players.is_empty())
                .then_some(0)
                .ok_or_else(|| "no player is on the server: add join = \"Name\"".to_string()),
        };
        let acts = self.player.is_some()
            || self.chat.is_some()
            || self.command.is_some()
            || self.broken.is_some()
            || self.place.is_some()
            || self.used.is_some()
            || self.use_entity.is_some()
            || !self.inventory.is_empty()
            || self.expect.inventory.is_some();
        let actor = if server && acts {
            Some(actor(game)?)
        } else {
            None
        };
        if let Some(set) = &self.player {
            match actor {
                Some(i) => set.apply_server(&mut game.players[i]),
                None => set.apply(game.player.get_or_insert_with(default_player)),
            }
        }
        if let Some(set) = &self.clock {
            set.apply(game.clock.get_or_insert(Clock {
                game_time: 0,
                day_time: 0,
            }));
        }
        if let Some(i) = actor {
            let id = game.players[i].id.clone();
            let slots = game.inventories.entry(id).or_insert_with(|| vec![None; 41]);
            for s in &self.inventory {
                if let Some(slot) = slots.get_mut(s.slot as usize) {
                    *slot = (s.count > 0).then(|| StackInfo {
                        item: s.item.clone(),
                        count: s.count,
                        ..StackInfo::default()
                    });
                }
            }
        }
        game.tick(self.ticks);
        game.frames(self.frames);
        let who = actor.map(|i| game.players[i].clone());
        if let Some(line) = &self.chat {
            match &who {
                Some(p) => game.player_chat(p, line),
                None => game.chat(line),
            }
        }
        let owned = self.command.as_ref().map(|line| match &who {
            Some(p) => game.player_command(p, line),
            None => game.command(line),
        });
        for (b, placed) in [(&self.broken, false), (&self.place, true)] {
            if let (Some(b), Some(p)) = (b, &who) {
                let pos = BlockPos {
                    x: b.x,
                    y: b.y,
                    z: b.z,
                };
                game.block(p, pos, &b.state, placed);
            }
        }
        if let (Some(u), Some(p)) = (&self.used, &who) {
            let state = game.world.get(&(u.x, u.y, u.z)).cloned();
            let state = state.unwrap_or_else(|| "minecraft:air".into());
            let pos = BlockPos {
                x: u.x,
                y: u.y,
                z: u.z,
            };
            let faces = ["down", "up", "north", "south", "west", "east"];
            let face = match &u.face {
                Some(f) => Some(
                    faces
                        .iter()
                        .position(|n| n == f)
                        .ok_or_else(|| format!("face {f:?}: one of {}", faces.join(", ")))?
                        as u8,
                ),
                None => None,
            };
            // Where it hits: the middle of the face, unless the script says.
            let middle = |face: Option<u8>| {
                let n = match face {
                    Some(0) => [0, -1, 0],
                    Some(1) => [0, 1, 0],
                    Some(2) => [0, 0, -1],
                    Some(3) => [0, 0, 1],
                    Some(4) => [-1, 0, 0],
                    Some(5) => [1, 0, 0],
                    _ => [0; 3],
                };
                let at = [u.x, u.y, u.z];
                [0, 1, 2].map(|k| f64::from(at[k]) + 0.5 + 0.5 * f64::from(n[k]))
            };
            match &u.item {
                // A click with an item reaches `item-used` with the face
                // clicked (up when the script names none).
                Some(item) => {
                    let face = face.unwrap_or(1);
                    let hit = u.at.unwrap_or_else(|| middle(Some(face)));
                    game.use_item_on(p, item, u.sneaking, pos, face, hit, &state);
                }
                None => {
                    let hit = u.at.unwrap_or_else(|| middle(face));
                    game.use_block(p, pos, face, hit, &state);
                }
            }
        }
        if let (Some(u), Some(p)) = (&self.use_entity, &who) {
            let item = u.item.as_deref().unwrap_or_default();
            game.use_item_on_entity(p, item, u.sneaking, u.id)?;
        }
        if let Some(name) = &self.leave
            && !game.leave(name)
        {
            return Err(format!("{name} is not on the server"));
        }
        game.mods.take_world();
        let out = game.drain();
        let taken: Vec<u32> = out
            .iter()
            .filter_map(|o| match o {
                Output::Take { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        let stopped = out.iter().find_map(|o| match o {
            Output::Stopped { reason, .. } => Some(reason.as_str()),
            _ => None,
        });
        let e = &self.expect;
        match (stopped, &e.stopped) {
            (Some(reason), Some(want)) if !reason.contains(want.as_str()) => {
                return Err(format!("the mod stopped ({reason}), expected {want:?}"));
            }
            (Some(_), Some(_)) => return Ok(()),
            (Some(reason), None) => return Err(format!("the mod stopped: {reason}")),
            (None, Some(want)) => {
                return Err(format!(
                    "the mod kept running, expected it to stop ({want:?})"
                ));
            }
            (None, None) => {}
        }
        if owned == Some(false) {
            let line = self.command.as_deref().unwrap_or_default();
            let name = line.split(' ').next().unwrap_or_default();
            return Err(format!("the mod owns no command /{name}"));
        }
        if let Some(want) = &e.commands {
            same("commands", want, &game.commands())?;
        }
        if let Some(want) = &e.chat {
            let got: Vec<String> = out
                .iter()
                .filter_map(|o| match o {
                    Output::Chat { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            same("chat", want, &got)?;
        }
        let feedback = |error: bool| {
            out.iter().rev().find_map(|o| match o {
                Output::Feedback { text, error: e } | Output::Reply { text, error: e, .. }
                    if *e == error =>
                {
                    Some(text.clone())
                }
                _ => None,
            })
        };
        if let Some(want) = &e.told {
            let name = |id: &str| {
                let p = game.players.iter().find(|p| p.id == id);
                p.map_or(id.to_owned(), |p| p.name.clone())
            };
            let got: Vec<String> = out
                .iter()
                .filter_map(|o| match o {
                    Output::Tell { player, text, .. } => Some(format!("{}: {text}", name(player))),
                    _ => None,
                })
                .collect();
            same("told", want, &got)?;
        }
        if let Some(want) = &e.broadcast {
            let got: Vec<String> = out
                .iter()
                .filter_map(|o| match o {
                    Output::Broadcast { text, .. } => Some(text.clone()),
                    _ => None,
                })
                .collect();
            same("broadcast", want, &got)?;
        }
        if let Some(want) = &e.feedback {
            same("feedback", &Some(want.clone()), &feedback(false))?;
        }
        if let Some(want) = &e.error {
            same("error", &Some(want.clone()), &feedback(true))?;
        }
        if let Some(want) = &e.panel {
            let got: Vec<String> = game
                .panel()
                .map(|p| p.lines.iter().map(|l| l.text.clone()).collect())
                .unwrap_or_default();
            same("panel", want, &got)?;
        }
        if let Some(want) = &e.blocks {
            for b in want {
                let got = game.world.get(&(b.x, b.y, b.z));
                let got = got.map_or("minecraft:air", String::as_str);
                if got != b.state {
                    return Err(format!(
                        "block at {} {} {}: expected {}, got {got}",
                        b.x, b.y, b.z, b.state
                    ));
                }
            }
        }
        if let Some(want) = &e.drops {
            let got: Vec<String> = out
                .iter()
                .filter_map(|o| match o {
                    Output::Drop {
                        pos, item, count, ..
                    } => Some(format!(
                        "{item} x{count} at {} {} {}",
                        pos[0], pos[1], pos[2]
                    )),
                    _ => None,
                })
                .collect();
            same("drops", want, &got)?;
        }
        if let Some(want) = &e.taken {
            same("taken", want, &taken)?;
        }
        if let Some(want) = &e.effects {
            let got: Vec<String> = out
                .iter()
                .filter_map(|o| match o {
                    Output::Effect { pos, effect, .. } => Some(match effect {
                        crate::EffectOp::Sound { event, .. } => {
                            format!("sound {event} at {} {} {}", pos[0], pos[1], pos[2])
                        }
                        crate::EffectOp::LevelEvent { id, data } => {
                            format!("event {id} {data} at {} {} {}", pos[0], pos[1], pos[2])
                        }
                        crate::EffectOp::DestroyProgress { breaker, stage } => {
                            format!(
                                "crack {breaker} {stage} at {} {} {}",
                                pos[0], pos[1], pos[2]
                            )
                        }
                        crate::EffectOp::Particles {
                            particle,
                            at,
                            count,
                            ..
                        } => format!(
                            "particles {particle} x{count} at {} {} {}",
                            at[0], at[1], at[2]
                        ),
                    }),
                    _ => None,
                })
                .collect();
            same("effects", want, &got)?;
        }
        if let Some(want) = &e.names {
            let got: Vec<String> = out
                .iter()
                .filter_map(|o| match o {
                    Output::Container {
                        op: crate::ContainerOp::SetName { pos, name, .. },
                        ..
                    } => Some(format!(
                        "{} at {} {} {}",
                        name.as_deref().unwrap_or("-"),
                        pos[0],
                        pos[1],
                        pos[2]
                    )),
                    _ => None,
                })
                .collect();
            same("names", want, &got)?;
        }
        if let Some(want) = &e.living {
            let got: Vec<String> = game
                .living
                .iter()
                .map(|l| format!("{} {} {}", l.id, l.kind, l.health))
                .collect();
            same("living", want, &got)?;
        }
        if let Some(want) = &e.slots {
            let got: Vec<String> = game.containers.lock().map_or_else(
                |_| Vec::new(),
                |all| {
                    all.iter()
                        .flat_map(|((x, y, z), slots)| {
                            slots.iter().enumerate().filter_map(move |(i, s)| {
                                let (item, n) = s.as_ref()?;
                                Some(format!("{x} {y} {z} {i} {item} x{n}"))
                            })
                        })
                        .collect()
                },
            );
            same("slots", want, &got)?;
        }
        if let Some(want) = &e.inventory {
            let who = who.as_ref().map(|p| p.id.clone()).unwrap_or_default();
            let got: Vec<String> = game
                .inventories
                .get(&who)
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(|(i, s)| {
                    let s = s.as_ref()?;
                    Some(format!("{i} {} x{}", s.item, s.count))
                })
                .collect();
            same("inventory", want, &got)?;
        }
        if let Some(want) = &e.outputs {
            let faces = ["down", "up", "north", "south", "west", "east"];
            let got: Vec<String> = game
                .outputs
                .iter()
                .flat_map(|((x, y, z), out)| {
                    out.iter().zip(faces).filter(|(o, _)| **o != [0; 2]).map(
                        move |([weak, strong], face)| format!("{x} {y} {z} {face} {weak} {strong}"),
                    )
                })
                .collect();
            same("outputs", want, &got)?;
        }
        if let Some(want) = &e.visuals {
            let got: Vec<String> = game
                .visuals
                .iter()
                .flat_map(|((x, y, z), parts)| {
                    parts.iter().map(move |p| {
                        if let Some(b) = &p.fluid {
                            return format!(
                                "{x} {y} {z} fluid {} {:?} {:?}",
                                p.state, b.min, b.max
                            );
                        }
                        let mut line = format!("{x} {y} {z} {} {}", p.state, p.speed);
                        // What moves is shown by where it stands and where it goes.
                        if p.offset != [0.0; 3] || p.velocity != [0.0; 3] {
                            line += &format!(" offset {:?} velocity {:?}", p.offset, p.velocity);
                        }
                        if let Some(s) = &p.scroll {
                            line += &format!(" scroll {} {} {}", s.to, s.speed, s.span);
                        }
                        if let Some(q) = p.orientation {
                            let q = q.map(|c| (c * 1000.0).round() / 1000.0);
                            line += &format!(" turned {q:?}");
                        }
                        line
                    })
                })
                .collect();
            same("visuals", want, &got)?;
        }
        if let Some(want) = &e.corner {
            let got = game.panel().map(|p| corner_name(p.corner).to_string());
            same("corner", &Some(want.clone()), &got)?;
        }
        Ok(())
    }
}

fn corner_name(c: Corner) -> &'static str {
    match c {
        Corner::TopLeft => "top-left",
        Corner::TopRight => "top-right",
        Corner::BottomLeft => "bottom-left",
        Corner::BottomRight => "bottom-right",
    }
}

fn same<T: PartialEq + std::fmt::Debug>(what: &str, want: &T, got: &T) -> Result<(), String> {
    if want == got {
        return Ok(());
    }
    Err(format!(
        "{what}\n    expected: {want:?}\n    got:      {got:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingredients_match_items_and_tags_through_tags() {
        let mut tags = Tags::new();
        add_tag(
            &mut tags,
            "c:stripped_logs".into(),
            r##"{"values": ["#minecraft:stripped_oak", {"id": "mymod:log", "required": false}]}"##,
        );
        add_tag(
            &mut tags,
            "minecraft:stripped_oak".into(),
            r#"{"values": ["minecraft:stripped_oak_log"]}"#,
        );
        add_tag(&mut tags, "mymod:gone".into(), r#"{"values": ["a:b"]}"#);
        add_tag(
            &mut tags,
            "mymod:gone".into(),
            r#"{"replace": true, "values": ["c:d"]}"#,
        );
        assert!(in_tag(
            &tags,
            "c:stripped_logs",
            "minecraft:stripped_oak_log",
            0
        ));
        assert!(in_tag(&tags, "c:stripped_logs", "mymod:log", 0));
        assert!(!in_tag(&tags, "c:stripped_logs", "minecraft:oak_log", 0));
        assert_eq!(tags["mymod:gone"], ["c:d"]);
        let mut block_tags = Tags::new();
        add_tag(
            &mut block_tags,
            "minecraft:logs".into(),
            r##"{"values": ["#minecraft:oak_logs", "mymod:log"]}"##,
        );
        add_tag(
            &mut block_tags,
            "minecraft:oak_logs".into(),
            r#"{"values": ["minecraft:oak_log"]}"#,
        );
        add_tag(
            &mut block_tags,
            "minecraft:leaves".into(),
            r#"{"values": ["minecraft:oak_leaves"]}"#,
        );
        let world = ScriptWorld {
            blocks: Default::default(),
            power: Default::default(),
            dropped: Vec::new(),
            living: Vec::new(),
            forwards: Default::default(),
            recipes: Vec::new(),
            tags: std::sync::Arc::new(tags),
            block_tags: std::sync::Arc::new(block_tags),
            item_ids: std::sync::Arc::new(vec!["minecraft:stick".into()]),
            containers: Default::default(),
        };
        use crate::WorldView;
        let log = "minecraft:stripped_oak_log";
        assert!(world.test_ingredient("#c:stripped_logs", log));
        assert!(world.test_ingredient(r#"{"tag": "c:stripped_logs"}"#, log));
        assert!(world.test_ingredient(r##"["minecraft:stick", "#c:stripped_logs"]"##, log));
        assert!(world.test_ingredient(r#"{"item": "minecraft:stick"}"#, "minecraft:stick"));
        assert!(!world.test_ingredient("#c:stripped_logs", "minecraft:stick"));
        // A state's block tags, through the tags a tag names; a bare id is vanilla's.
        assert_eq!(
            world.block_tags("minecraft:oak_log[axis=y]"),
            ["minecraft:logs", "minecraft:oak_logs"]
        );
        assert_eq!(world.block_tags("oak_leaves"), ["minecraft:leaves"]);
        assert_eq!(world.block_tags("mymod:log"), ["minecraft:logs"]);
        assert!(world.block_tags("minecraft:stone").is_empty());
        assert_eq!(world.items(), ["minecraft:stick"]);
    }
}
