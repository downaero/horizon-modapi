//! Loading and calling mods with wasmtime's component model.
use crate::manifest::{Manifest, Package, Side};
use server_bindings::{
    HorizonServerMod,
    exports::horizon::mods::server_events::Registration as ServerRegistration,
    horizon::mods::{players, world as mod_world},
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};
use wasmtime::{
    Config, Engine, Store, StoreLimits, StoreLimitsBuilder, Trap,
    component::{Component, HasSelf, Linker},
};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "horizon-mod",
    });
}
/// The server mods' world: `log` and `storage` are the client world's.
mod server_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "horizon-server-mod",
        additional_derives: [PartialEq],
        with: {
            "horizon:mods/log": super::bindings::horizon::mods::log,
            "horizon:mods/storage": super::bindings::horizon::mods::storage,
        },
    });
}
/// Capability `block-entities`' world beyond `horizon-server-mod`: a second
/// world, so server mods built before it (no `block-events` export) still
/// instantiate and simply hear nothing of blocks.
mod block_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "block-host",
    });
}
use block_bindings::{BlockHost, horizon::mods::block_entities};
/// Capability `net`'s world: a second one again, as mods built before it
/// have no `net-events` export.
mod net_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "net-host",
    });
}
/// Capability `entities`' imports.
mod entity_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "entities-host",
    });
}
use entity_bindings::{EntitiesHost, horizon::mods::entities};
/// Capability `containers`' imports.
mod container_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "containers-host",
    });
}
use container_bindings::{ContainersHost, horizon::mods::containers};
/// Capability `redstone`'s imports.
mod redstone_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "redstone-host",
    });
}
use redstone_bindings::{RedstoneHost, horizon::mods::redstone};
/// Capability `items`' imports.
mod item_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "items-host",
    });
}
use item_bindings::{ItemsHost, horizon::mods::items};
/// The package files every mod may read.
mod file_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "files-host",
    });
}
use file_bindings::{FilesHost, horizon::mods::files};
/// Data on players, dimensions and chunks.
mod attachment_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "attachments-host",
    });
}
use attachment_bindings::{AttachmentsHost, horizon::mods::attachments};
/// Capability `transfer`'s imports.
mod transfer_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "transfer-host",
    });
}
use transfer_bindings::{TransferHost, horizon::mods::transfer};
/// The mod id the game saves the mods' tanks and batteries under, among
/// their block data.
pub const TRANSFER_ID: &str = "@transfer";

/// A tank and a battery on a block, as capability `transfer` keeps them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Storage {
    /// The mod whose block it is.
    pub owner: String,
    pub fluid: Option<(String, u32)>,
    pub fluid_capacity: u32,
    pub energy: u64,
    pub energy_capacity: u64,
}
impl Storage {
    /// Owner (u16 length, bytes), tank capacity (u32), fluid (u16 length,
    /// bytes, 0 for none) and amount (u32), energy and capacity (u64 each),
    /// little-endian.
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend((self.owner.len() as u16).to_le_bytes());
        out.extend(self.owner.as_bytes());
        out.extend(self.fluid_capacity.to_le_bytes());
        let (fluid, amount) = self.fluid.clone().unwrap_or_default();
        out.extend((fluid.len() as u16).to_le_bytes());
        out.extend(fluid.as_bytes());
        out.extend(amount.to_le_bytes());
        out.extend(self.energy.to_le_bytes());
        out.extend(self.energy_capacity.to_le_bytes());
        out
    }
    pub fn decode(mut b: &[u8]) -> Option<Self> {
        fn take<'a>(b: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
            let (h, r) = b.split_at_checked(n)?;
            *b = r;
            Some(h)
        }
        let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?) as usize;
        let owner = String::from_utf8(take(&mut b, n)?.to_vec()).ok()?;
        let fluid_capacity = u32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
        let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?) as usize;
        let fluid = String::from_utf8(take(&mut b, n)?.to_vec()).ok()?;
        let amount = u32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
        let energy = u64::from_le_bytes(take(&mut b, 8)?.try_into().ok()?);
        let energy_capacity = u64::from_le_bytes(take(&mut b, 8)?.try_into().ok()?);
        Some(Self {
            owner,
            fluid: (!fluid.is_empty() && amount > 0).then_some((fluid, amount)),
            fluid_capacity,
            energy,
            energy_capacity,
        })
    }
}
/// Every mod's tanks and batteries, by dimension and position, shared.
type StorageSlot = Arc<Mutex<std::collections::HashMap<(String, [i32; 3]), Storage>>>;
/// Most tanks and batteries all mods keep.
const STORAGES: usize = 1 << 16;
/// Marks an attachment among a mod's saved block data: the game keeps both
/// in one file per mod, an attachment's target in the dimension's place.
pub const ATTACHMENT_MARK: char = '\u{1}';
/// Largest attachment value, and most values a mod keeps.
const ATTACHMENT_BYTES: usize = 64 << 10;
const ATTACHMENTS: usize = 1 << 16;
/// Capability `effects`' imports.
mod effect_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "effects-host",
    });
}
use effect_bindings::{EffectsHost, horizon::mods::effects};
mod server_net_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "server-net-host",
    });
}
mod neighbor_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "neighbor-host",
    });
}
use neighbor_bindings::NeighborHost;
mod item_event_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "item-host",
    });
}
use item_event_bindings::ItemHost;
mod peer_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "peers-host",
    });
}
use peer_bindings::{PeersHost, horizon::mods::peers};
mod menu_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "menus-host",
    });
}
use menu_bindings::{MenusHost, horizon::mods::menus};
mod console_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "console-host",
    });
}
use console_bindings::{ConsoleHost, horizon::mods::console};
mod stack_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "stacks-host",
    });
}
use stack_bindings::{StacksHost, horizon::mods::stacks};
mod recipe_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "recipe-manager-host",
    });
}
use recipe_bindings::{RecipeManagerHost, horizon::mods::recipe_manager};
/// Most bytes of recipe files one `recipe-manager.of-type` call returns.
const RECIPE_FILE_BYTES: usize = 4 << 20;

/// A crafting recipe a grid matched (`recipe-manager.craft`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crafted {
    pub recipe: String,
    pub result: (String, u32),
    pub remainders: Vec<Option<(String, u32)>>,
}

/// A cooking recipe (`recipe-manager.cook`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cooked {
    pub recipe: String,
    pub result: (String, u32),
    pub experience: f32,
    pub time: u32,
}
/// Largest bytes a mod keeps on one stack.
const STACK_DATA_BYTES: usize = 16 << 10;

/// An item stack as capability `stacks` sees it: every mod's own bytes on
/// it, by mod id (each mod is shown only its own).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StackInfo {
    pub item: String,
    pub count: u32,
    pub damage: u32,
    pub name: Option<String>,
    pub enchantments: Vec<(String, u32)>,
    pub data: Vec<(String, Vec<u8>)>,
}

pub use crate::menu_layout::{MenuLayout, MenuWidget};
/// A change to a mod's screens.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuOp {
    Open {
        player: String,
        dimension: String,
        pos: [i32; 3],
        layout: MenuLayout,
    },
    Data {
        player: String,
        index: u32,
        value: i32,
    },
    /// `menus.open-panel`: a screen with no container.
    Panel { player: String, layout: MenuLayout },
    /// `menus.close`.
    Close { player: String },
}
impl MenuLayout {
    fn from_wit(l: menus::Layout) -> Option<Self> {
        use menus::Widget;
        let clip = |t: String| t.chars().take(64).collect::<String>();
        let ok = l.width <= 512 && l.height <= 512 && l.slots.len() <= 72 && l.widgets.len() <= 64;
        ok.then(|| Self {
            width: l.width,
            height: l.height,
            slots: l.slots.into_iter().map(|s| (s.slot, s.x, s.y)).collect(),
            widgets: l
                .widgets
                .into_iter()
                .map(|w| match w {
                    Widget::Label(l) => MenuWidget::Label {
                        x: l.x,
                        y: l.y,
                        text: clip(l.text),
                        color: l.color & 0xFF_FFFF,
                    },
                    Widget::Progress(p) => MenuWidget::Progress {
                        x: p.x,
                        y: p.y,
                        width: p.width.min(512),
                        height: p.height.min(512),
                        value: p.value.min(31),
                        max: p.max.min(31),
                        vertical: p.vertical,
                        color: p.color & 0xFF_FFFF,
                        background: p.background & 0xFF_FFFF,
                    },
                    Widget::Button(b) => MenuWidget::Button {
                        id: b.id,
                        x: b.x,
                        y: b.y,
                        width: b.width.min(512),
                        height: b.height.min(512),
                        label: clip(b.label),
                    },
                    Widget::TextField(f) => MenuWidget::TextField {
                        id: f.id,
                        x: f.x,
                        y: f.y,
                        width: f.width.min(512),
                        max_length: f.max_length.min(256),
                        value: f.value.chars().take(256).collect(),
                    },
                })
                .collect(),
        })
    }
}
/// Largest message between mods, and most a call sends.
const PEER_BYTES: usize = 64 << 10;
const PEER_PER_CALL: usize = 64;
pub use item_event_bindings::exports::horizon::mods::item_events::BlockTarget;
use net_bindings::{NetHost, horizon::mods::net};
use server_net_bindings::ServerNetHost;
/// Capability `ui`'s world beyond `horizon-mod` (docs/mods.md): a second
/// world, so mods built before it (no `ui-events` export) still instantiate
/// as `horizon-mod` and simply hear nothing of their widgets.
mod ui_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "ui-host",
    });
}
pub use bindings::horizon::mods::{
    game::{Clock, Hit, Player},
    hud::{Corner, Line},
};
use bindings::{
    HorizonMod,
    exports::horizon::mods::events::Registration,
    horizon::mods::{chat, command, game, hud, log as mod_log, storage},
};
pub use server_bindings::{
    exports::horizon::mods::server_events::{
        Argument, ArgumentKind, ArgumentValue, BlockPos, ServerCommand,
    },
    horizon::mods::players::{GameMode, Player as ServerPlayer},
    horizon::mods::world::{BlockChange, Region, ScannedBlock},
};
type ClientEvents = bindings::exports::horizon::mods::events::Guest;
type ServerEvents = server_bindings::exports::horizon::mods::server_events::Guest;
/// The widget model a `ui` screen or settings page shows (WIT `interface
/// ui`), for the game to draw with its menus' own widgets.
pub use ui_bindings::horizon::mods::ui::{
    ButtonWidget, ItemWidget, ListWidget, Screen as UiScreen, TextWidget, Widget as UiWidget,
};
use ui_bindings::{UiHost, horizon::mods::ui};
/// Capability `render`'s imports (docs/mods.md): the game draws the shapes.
mod render_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "render-host",
    });
}
use render_bindings::horizon::mods::input;
/// Capability `input`'s records (WIT `interface input`).
pub use render_bindings::horizon::mods::input::{Event as InputEvent, HeldItem, ItemUse};
/// What capability `render` asks the game to draw (WIT `interface render`).
pub use render_bindings::horizon::mods::render::{
    Corner as RenderCorner, FarTerrain as ModFarTerrain, FillShape, FrameRate, ItemShape,
    Minimap as ModMinimap, Point as RenderPoint, QuadShape, Rect as RenderRect, Shape, SpriteShape,
    TextShape, Zoom as ModZoom,
};
use render_bindings::{RenderHost, horizon::mods::render};
/// Capability `recipes`'s imports (docs/mods.md): the game's recipes and items.
mod recipes_bindings {
    wasmtime::component::bindgen!({
        path: "../horizon-mod-sdk/wit",
        world: "recipes-host",
    });
}
/// Capability `ui`'s recipe viewer panel (WIT `interface browser`).
pub use recipes_bindings::horizon::mods::browser::{
    Click as BrowserClick, Event as BrowserEvent, ItemKey as BrowserItemKey,
    Layout as BrowserLayout, RecipeView as BrowserRecipe, SlotClick as BrowserSlotClick,
    Stack as BrowserStack,
};
/// What capability `recipes` reads (WIT `interface recipes`).
pub use recipes_bindings::horizon::mods::recipes::{
    Item as RecipeItem, Kind as RecipeKind, Recipe, Stack as RecipeStack,
};
use recipes_bindings::{RecipesHost, horizon::mods::browser, horizon::mods::recipes};

/// A mod's linear memory cap.
const MEMORY: usize = 64 << 20;
/// Fuel (about one unit per WebAssembly instruction) per call: a bound on
/// runaway loops, far above the hooks' time budgets.
const FUEL_INIT: u64 = 2_000_000_000;
const FUEL_TICK: u64 = 50_000_000;
const FUEL_FRAME: u64 = 10_000_000;
const FUEL_EVENT: u64 = 100_000_000;
/// Blocks a mod may set per `world.set-blocks` call.
pub const BLOCKS_PER_CALL: usize = 4096;
/// Positions in the box of one `world.scan` call.
const SCAN_POSITIONS: i64 = 32768;
/// Bytes a mod may keep at one block position (`block-entities.set`), and
/// positions it may keep them at.
pub const BLOCK_DATA_BYTES: usize = 64 << 10;
pub const BLOCK_DATA_POSITIONS: usize = 1 << 16;
/// Positions [`Mods::tick_blocks`] ticks per mod per server tick.
pub const BLOCK_TICKS_PER_TICK: usize = 4096;
/// Bytes of one `net.send`, and messages a mod may send per hook call.
pub const NET_BYTES: usize = 32 << 10;
const NET_PER_CALL: usize = 64;
/// Entities a mod may hold at once (`entities.spawn`), and the operations
/// it may queue per hook call.
pub const ENTITIES_PER_MOD: usize = 1024;
/// Blocks a mod may register as containers, slots in one, and the changes
/// it may queue per hook call.
const CONTAINER_BLOCKS: usize = 64;
pub const CONTAINER_SLOTS: u32 = 27;
const CONTAINER_OPS_PER_CALL: usize = 4096;
/// Item drops (capability `items`) a mod may queue in one call.
const DROPS_PER_CALL: usize = 64;
/// Item entities `items.nearby` lists, and `items.take`s a call.
const NEARBY_ITEMS: usize = 256;
/// `neighbor-changed` calls to one mod in a server tick.
const NEIGHBOR_CALLS_PER_TICK: usize = 1024;
/// Sounds and level events (capability `effects`) a mod may queue in one call.
const EFFECTS_PER_CALL: usize = 64;
/// The vanilla level events capability `effects` allows.
const EFFECT_EVENTS: [i32; 4] = [2000, 2004, 2005, 2009];
const ENTITY_OPS_PER_CALL: usize = 4096;
/// Chat lines a mod may show per call, HUD lines and their length.
const CHAT_PER_CALL: usize = 8;
/// `command.run-quiet` lines one call of a mod may queue (a quiet payload's
/// cap, `horizon_protocol::quiet::MAX_LINES`).
const QUIET_PER_CALL: usize = 256;
/// How far `game.raycast` sees, in blocks.
pub const RAYCAST_MAX: f32 = 1024.0;
const HUD_LINES: usize = 16;
const LINE_CHARS: usize = 256;
/// Total bytes a mod's `storage` may hold on disk, across all its keys.
const STORAGE_QUOTA: u64 = 4 << 20;
/// Distinct keys a mod's `storage` may hold: `STORAGE_QUOTA` alone doesn't
/// bound a flood of zero- or near-zero-byte files (each still a file name,
/// a directory entry and an inode).
const STORAGE_MAX_KEYS: usize = 64;
/// Bytes `storage::set` may write across one hook call: fuel bounds a
/// mod's CPU time per call, not the time a synchronous disk write takes, so
/// a cheap-in-fuel loop replacing one key with large payloads needs its own
/// cap, separate from the on-disk quota it would otherwise stay under.
const STORAGE_WRITE_BUDGET: u64 = 1 << 20;

/// What mods may read of the game, copied once per tick by the game.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub player: Option<Player>,
    pub clock: Option<Clock>,
    /// The screen in GUI pixels (capability `render`'s `gui-size`).
    pub gui_size: (f32, f32),
    /// The main hand's stack (capability `input`'s `held`).
    pub held: Option<HeldItem>,
    /// The game's frame counter: fps, min, 1% low (`render.frame-stats`).
    pub frame: (u32, u32, u32),
    /// The movement and click mappings `input.mapping` answers for:
    /// (mapping, the bound key's label, held).
    pub mappings: Vec<(&'static str, String, bool)>,
    /// Attack and use clicks in the last second.
    pub cps: (u32, u32),
    /// The HUD font's advance per character from U+0000
    /// (`render.text-width`); empty: 6 each.
    pub font_widths: Arc<[f32]>,
    /// The players on the server, for server mods.
    pub players: Vec<ServerPlayer>,
    /// Each online player's 41 inventory slots and selected hotbar slot, by
    /// UUID (`players.inventory`).
    pub inventories: std::collections::HashMap<String, (Vec<Option<StackInfo>>, u32)>,
    /// The first block along the player's view within [`RAYCAST_MAX`] and
    /// its distance from the eye, for `game.raycast`; the game works it out
    /// only while a mod calls it ([`Mods::wants_raycast`]).
    pub view_hit: Option<(f32, Hit)>,
    /// Every item and recipe, for capability `recipes`; the game makes them
    /// once, and only while a mod reads them ([`Mods::wants_recipes`]).
    pub recipes: Option<Arc<Recipes>>,
    /// The `book-id`s of the recipes the player's book holds.
    pub known_recipes: Arc<[i32]>,
    /// The open screen takes recipes (`recipes.place`).
    pub placing: bool,
    /// The player is in creative mode (`recipes.give`).
    pub creative: bool,
}

/// Capability `recipes`'s tables: `items` in the registry's order, which
/// each [`RecipeStack::item`] indexes.
#[derive(Clone, Debug, Default)]
pub struct Recipes {
    pub items: Vec<RecipeItem>,
    pub recipes: Vec<Recipe>,
}

/// Which mods a game starts: the window's game the client mods, the game
/// simulating the world the server mods, single player in the frame both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Runs {
    Client,
    Server,
    Both,
}
impl Runs {
    fn starts(self, side: Side) -> bool {
        match self {
            Runs::Both => true,
            Runs::Client => side != Side::Server,
            Runs::Server => side == Side::Server,
        }
    }
}

/// One change to a mod's entities, by the mod's own handle.
#[derive(Clone, Debug, PartialEq)]
pub enum EntityOp {
    Spawn {
        handle: u32,
        dimension: String,
        pos: [f64; 3],
        yaw: f32,
        item: String,
    },
    /// `entities.spawn-block`: a block state as an entity.
    SpawnBlock {
        handle: u32,
        dimension: String,
        pos: [f64; 3],
        state: String,
    },
    /// `entities.set-solid`.
    SetSolid {
        handle: u32,
        solid: bool,
    },
    /// `entities.rotate`: yaw and pitch, in degrees.
    Rotate {
        handle: u32,
        yaw: f32,
        pitch: f32,
    },
    Move {
        handle: u32,
        pos: [f64; 3],
        yaw: f32,
    },
    SetItem {
        handle: u32,
        item: String,
    },
    /// `entities.set-slot`: an equipment slot (0 mainhand to 5 head).
    SetSlot {
        handle: u32,
        slot: usize,
        item: Option<String>,
    },
    /// `entities.set-pose`: a part (0 head to 5 right leg), in degrees.
    SetPose {
        handle: u32,
        part: usize,
        rotation: [f32; 3],
    },
    Remove {
        handle: u32,
    },
}

/// A stack in a mod container slot: the item and its count.
pub type SlotStack = (String, u32);

/// What a push moves.
#[derive(Clone, Debug, PartialEq)]
pub enum PushTarget {
    /// The player with this UUID.
    Player(String),
    /// The item entity with this id.
    Item(u32),
}

/// An item entity as capability `items` sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct DroppedStack {
    pub id: u32,
    pub item: String,
    pub count: u32,
    pub pos: [f64; 3],
}

/// What an `effects` call asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectOp {
    Sound {
        event: String,
        volume: f32,
        pitch: f32,
    },
    LevelEvent {
        id: i32,
        data: i32,
    },
}

/// One change to a mod's containers.
#[derive(Clone, Debug, PartialEq)]
pub enum ContainerOp {
    /// `containers.register`: the block now holds items.
    Register { block: String, slots: u32 },
    /// `containers.open`: for this player, at a position.
    Open {
        player: String,
        dimension: String,
        pos: [i32; 3],
    },
    /// `containers.set-slot`.
    SetSlot {
        dimension: String,
        pos: [i32; 3],
        slot: u32,
        stack: Option<SlotStack>,
    },
    /// `containers.set-slot-rule`: the items a slot takes (none: any).
    SetRule {
        dimension: String,
        pos: [i32; 3],
        slot: u32,
        allowed: Option<Vec<String>>,
    },
    /// `containers.set-name`.
    SetName {
        dimension: String,
        pos: [i32; 3],
        name: Option<String>,
    },
}

/// What mods asked the game to show since the last [`Mods::drain`].
#[derive(Clone, Debug, PartialEq)]
pub enum Output {
    /// `chat.show`: a line after the mod's name.
    Chat { name: String, text: String },
    /// A mod command's feedback, red when `error`.
    Feedback { text: String, error: bool },
    /// `command.run`: a vanilla command line, without its slash, to run as
    /// the player.
    Command { line: String },
    /// `command.run-quiet`: vanilla command lines to run as the player with
    /// nothing in the chat (`horizon_protocol::quiet` on a server).
    QuietCommands { mod_id: String, lines: Vec<String> },
    /// The mod was turned off: it trapped, overran or failed to start.
    Stopped { name: String, reason: String },
    /// `players.tell`: a line after the mod's name, to the player with this id.
    Tell {
        name: String,
        player: String,
        text: String,
    },
    /// `players.broadcast`: a line after the mod's name, to every player.
    Broadcast { name: String, text: String },
    /// A server mod command's feedback, to the player who typed it.
    Reply {
        player: String,
        text: String,
        error: bool,
    },
    /// `world.set-blocks`: changes to make in `dimension`, in order, each
    /// state checked by the [`WorldView`] lent for the call.
    SetBlocks {
        name: String,
        dimension: String,
        blocks: Vec<BlockChange>,
    },
    /// `ui.open-screen`: the mod's entry in `entries()`; what it shows is
    /// [`Mods::screen`].
    OpenScreen { owner: usize },
    /// `ui.close-screen`: the mod closed its own screen (no `closed` back).
    CloseScreen { owner: usize },
    /// `render.shake`: strength 0 to 1, fading out over `seconds` (≤ 10).
    Shake { strength: f32, seconds: f32 },
    /// `recipes.place`: the recipe book's click on this display id, all of
    /// it when `all`.
    PlaceRecipe { book_id: i32, all: bool },
    /// `block-entities.set` (`data` some) or `remove` (none): the game saves
    /// the bytes with the chunk holding `pos`, for the mod `mod_id`.
    BlockData {
        mod_id: String,
        dimension: String,
        pos: [i32; 3],
        data: Option<Vec<u8>>,
    },
    /// `net.send`: a message on the mod's channel. `player` is the target
    /// of a server mod's message (none: every player); a client mod's goes to
    /// its server.
    Net {
        mod_id: String,
        /// Sent by a server mod.
        server: bool,
        player: Option<String>,
        data: Vec<u8>,
    },
    /// `containers.*`: a change to the mod's containers, applied in order.
    Container { mod_id: String, op: ContainerOp },
    /// `entities.*`: a change to the mod's entities, applied in order.
    Entity { mod_id: String, op: EntityOp },
    /// `effects.*`: a sound or a level event at a block.
    Effect {
        mod_id: String,
        dimension: String,
        pos: [i32; 3],
        effect: EffectOp,
    },
    /// `players.push` and `items.push`: add a velocity.
    Push {
        mod_id: String,
        target: PushTarget,
        velocity: [f64; 3],
    },
    /// `redstone.set-signal`: the signal a mod block gives.
    Signal {
        mod_id: String,
        dimension: String,
        pos: [i32; 3],
        level: u8,
    },
    /// `world.break-block`: break as a player with `tool` would; drops into
    /// the inventory at `into`, or where the block stood.
    Break {
        mod_id: String,
        dimension: String,
        pos: [i32; 3],
        tool: Option<String>,
        enchantments: Vec<(String, u32)>,
        into: Option<[i32; 3]>,
    },
    /// `world.use-item`: an item used on a block as a player would.
    UseItem {
        mod_id: String,
        dimension: String,
        pos: [i32; 3],
        face: u8,
        item: String,
    },
    /// `stacks.set-player-stack`: a stack (with the mod's own bytes only)
    /// for a player's slot.
    SetPlayerStack {
        mod_id: String,
        player: String,
        slot: u32,
        stack: Option<StackInfo>,
    },
    /// `console.run`: a command as the server's console.
    Console { mod_id: String, line: String },
    /// `menus.*`: a screen of the mod's own opened, or its data.
    Menu { mod_id: String, op: MenuOp },
    /// `peers.send`: a message from one server mod to another.
    Peer {
        from: String,
        to: String,
        data: Vec<u8>,
    },
    /// `items.take`: remove this item entity.
    Take { mod_id: String, id: u32 },
    /// `items.drop`: a stack to drop as an item entity.
    Drop {
        mod_id: String,
        dimension: String,
        pos: [f64; 3],
        item: String,
        count: u32,
    },
    /// `recipes.give`: a stack for a creative player.
    Give {
        item: String,
        count: u32,
        components: String,
    },
}

/// The game's world as capability `world` reads it during a call: the
/// game lends one with [`Mods::lend_world`] and takes it back after.
pub trait WorldView: std::any::Any + Send {
    /// The block state at a position, `minecraft:oak_log[axis=y]`; `None`
    /// where no chunk of that dimension is loaded.
    fn block(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String>;
    /// The biome at a position, `minecraft:plains`; `None` as for blocks.
    fn biome(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String>;
    /// Whether `dimension` is one the server has.
    fn has_dimension(&self, dimension: &str) -> bool;
    /// `Err` (why) for a state string that names no block state.
    fn check_state(&self, state: &str) -> Result<(), String>;
    /// The 27 slots of a mod container at a position, as of the tick's
    /// start (capability `containers`); `None` where there is none.
    fn container(
        &self,
        _dimension: &str,
        _x: i32,
        _y: i32,
        _z: i32,
    ) -> Option<Vec<Option<SlotStack>>> {
        None
    }
    /// The item entities in the simulated dimension, as of the tick's start
    /// (capability `items`).
    fn dropped(&self, _dimension: &str) -> Vec<DroppedStack> {
        Vec::new()
    }
    /// The reachable slots of the inventory at a position, through `face`
    /// (0 down, 1 up, 2 north, 3 south, 4 west, 5 east; none: all of them).
    fn item_slots(
        &self,
        _dimension: &str,
        _pos: [i32; 3],
        _face: Option<u8>,
    ) -> Option<Vec<Option<SlotStack>>> {
        None
    }
    /// Puts `count` of `item` in: how many did not fit.
    fn insert_item(
        &mut self,
        _dimension: &str,
        _pos: [i32; 3],
        _face: Option<u8>,
        _item: &str,
        count: u32,
        _simulate: bool,
    ) -> u32 {
        count
    }
    /// Takes up to `count` of the first reachable stack.
    fn extract_item(
        &mut self,
        _dimension: &str,
        _pos: [i32; 3],
        _face: Option<u8>,
        _count: u32,
        _simulate: bool,
    ) -> Option<SlotStack> {
        None
    }
    /// The stack in slot `slot` of the inventory at a position, as it is now.
    fn stack_at(&self, _dimension: &str, _pos: [i32; 3], _slot: u32) -> Option<StackInfo> {
        None
    }
    /// Puts a stack in slot `slot` of the inventory at a position (none
    /// empties it); mod `mod_id`'s bytes are the stack's, other mods' stay
    /// when the item does. False when there is no such slot.
    fn set_stack_at(
        &mut self,
        _dimension: &str,
        _pos: [i32; 3],
        _slot: u32,
        _stack: Option<StackInfo>,
        _mod_id: &str,
    ) -> bool {
        false
    }
    /// The crafting recipe a `width` by `width` grid matches.
    fn craft(&self, _grid: &[Option<(String, u32)>], _width: u32) -> Option<Crafted> {
        None
    }
    /// The cooking recipe of `kind` taking `item`.
    fn cook(&self, _kind: &str, _item: &str) -> Option<Cooked> {
        None
    }
    /// The stonecutter's recipes taking `item`: id, result.
    fn stonecut(&self, _item: &str) -> Vec<(String, (String, u32))> {
        Vec::new()
    }
    /// The content mods' recipe files of a type the game does not run: id,
    /// JSON.
    fn recipes_of_type(&self, _kind: &str) -> Vec<(String, String)> {
        Vec::new()
    }
    /// Whether a recipe file's ingredient takes `item`.
    fn test_ingredient(&self, _ingredient: &str, _item: &str) -> bool {
        false
    }
    /// The redstone signal reaching a block of the mod's, as of the tick's
    /// start (capability `redstone`); `None` where it is not known.
    fn power(&self, _dimension: &str, _x: i32, _y: i32, _z: i32) -> Option<u8> {
        None
    }
}

/// The view lent for the current calls, shared by every mod's store.
type WorldSlot = Arc<Mutex<Option<Box<dyn WorldView>>>>;

/// A mod's text panel, drawn every frame until it changes.
#[derive(Clone, Debug)]
pub struct Panel {
    pub corner: Corner,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Installed and turned off by the player.
    Off,
    Running,
    /// Turned on, and run by another game: a server mod in the window while
    /// the world runs on its server thread.
    Elsewhere,
    /// Could not load or start, or stopped while running.
    Failed(String),
}

/// One installed mod, for the Mods page.
pub struct Entry {
    pub path: PathBuf,
    /// None when the manifest could not be read (`status` says why).
    pub manifest: Option<Manifest>,
    pub status: Status,
    /// The mod's average time per call of its busiest hook.
    pub cost: Duration,
    run: Option<Running>,
    modified: Option<SystemTime>,
}
impl Entry {
    /// The running mod has a settings page (capability `ui` set widgets
    /// with `ui.set-settings`, or `input` registered keys the player may
    /// bind there): its row in Options > Mods gains a button.
    pub fn has_ui_settings(&self) -> bool {
        self.run.as_ref().is_some_and(|r| {
            let d = r.store.data();
            !d.ui.settings.is_empty() || !d.input.keys.is_empty()
        })
    }
}

struct Running {
    store: Store<State>,
    /// The mod's `ui-events` export; none for a mod built before `ui`, or a
    /// server mod.
    ui: Option<UiHost>,
    hooks: Hooks,
    /// It imports `game.raycast`.
    raycasts: bool,
    /// It imports capability `recipes`.
    reads_recipes: bool,
    /// A server mod's `block-events` export, none for one built before
    /// capability `block-entities`.
    blocks: Option<BlockHost>,
    /// The blocks (ids without properties) it asked the game to tick.
    ticking: Vec<String>,
    /// A client mod's `net-events` export, none for one built before `net`.
    net: Option<NetHost>,
    /// A server mod's `server-net-events` export.
    server_net: Option<ServerNetHost>,
    /// A server mod's `neighbor-events` export, none for one built before it.
    neighbor: Option<NeighborHost>,
    /// A server mod's `item-events` export.
    item: Option<ItemHost>,
    /// A server mod's `peer-events` export.
    peer: Option<PeersHost>,
    /// A server mod's `menu-events` export.
    menu: Option<MenusHost>,
}

/// A running mod's exports and the hooks its `init` asked for.
enum Hooks {
    Client(HorizonMod, Registration),
    Server(HorizonServerMod, ServerRegistration),
}
impl Hooks {
    fn client(&self) -> Option<&Registration> {
        match self {
            Hooks::Client(_, r) => Some(r),
            Hooks::Server(..) => None,
        }
    }
    fn server(&self) -> Option<&ServerRegistration> {
        match self {
            Hooks::Server(_, r) => Some(r),
            Hooks::Client(..) => None,
        }
    }
    fn commands(&self) -> Vec<&str> {
        match self {
            Hooks::Client(_, r) => r.commands.iter().map(String::as_str).collect(),
            Hooks::Server(_, r) => r.commands.iter().map(|c| c.name.as_str()).collect(),
        }
    }
}

/// What capability `ui` pushed since the hooks last ran.
#[derive(Default)]
struct UiState {
    /// The screen `ui.open-screen` last pushed, until `ui.close-screen`.
    screen: Option<UiScreen>,
    /// The widgets `ui.set-settings` last pushed (the mod's page in
    /// Options > Mods).
    settings: Vec<UiWidget>,
    /// The calls since the last hook ended, for [`Output`].
    open: bool,
    close: bool,
}

/// Capability `ui`'s panel beside container screens (`browser`).
#[derive(Default)]
struct BrowserState {
    items: Vec<BrowserStack>,
    /// Which `set-items` or `set-recipes` call the panel is from, unique
    /// across mods and reloads, so the game converts them once.
    version: u64,
    /// `set-recipes`' title and recipes, shown instead of the items.
    recipes: Option<RecipePage>,
    events: std::collections::VecDeque<BrowserEvent>,
}
/// What `browser.set-recipes` shows: a title, tabs (`tab` the selected
/// one) and the recipes.
#[derive(Clone, Debug)]
pub struct RecipePage {
    pub title: String,
    pub tabs: Vec<BrowserStack>,
    pub tab: u32,
    pub recipes: Vec<BrowserRecipe>,
}
static BROWSER_VERSION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
/// Items a panel may hold, and events kept for `browser.poll`.
const BROWSER_ITEMS: usize = 8192;
/// `recipes.place` and `give` calls a hook call may make.
const RECIPE_ACTIONS: usize = 36;
/// Recipes `set-recipes` may show, slots in one, and stacks a slot cycles.
const BROWSER_RECIPES: usize = 256;
const RECIPE_SLOTS: usize = 9;
const RECIPE_TABS: usize = 16;
const SLOT_STACKS: usize = 64;
const BROWSER_EVENTS: usize = 256;
impl BrowserState {
    fn push(&mut self, event: BrowserEvent) {
        if self.events.len() == BROWSER_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

struct State {
    name: String,
    id: String,
    snapshot: Arc<Snapshot>,
    out: Vec<Output>,
    panel: Option<Panel>,
    ui: UiState,
    /// Capability `render`'s HUD shapes, drawn until replaced.
    shapes: Vec<Shape>,
    /// Capability `render`'s corner panel.
    corner: Option<CornerPanel>,
    /// Capability `render`'s zoom, when the mod zooms.
    zoom: Option<ModZoom>,
    /// Capability `render`'s minimap, when the mod shows one.
    minimap: Option<ModMinimap>,
    /// Capability `render`'s far terrain, when the mod draws one.
    far_terrain: Option<ModFarTerrain>,
    input: InputState,
    browser: BrowserState,
    /// Capability `storage`'s folder; made on first `set`.
    data_dir: PathBuf,
    /// Capability `transfer`'s tanks and batteries, shared by every mod.
    storage: StorageSlot,
    /// The ids of the running server mods, for `peers.running`.
    peers: Arc<Mutex<Vec<String>>>,
    /// Capability-free `attachments`: (target and key, chunk position) to bytes.
    attachments: std::collections::HashMap<(String, [i32; 3]), Vec<u8>>,
    /// The mod's own `.hmod` or folder, for `files`.
    package_path: PathBuf,
    /// Bytes `storage::set` may still write before this hook call ends
    /// (reset per call in [`Mods::call`]): fuel bounds a mod's CPU time but
    /// not disk I/O time, so a loop of small-fuel, large-payload `set`s
    /// needs its own budget.
    storage_budget: u64,
    limits: StoreLimits,
    world: WorldSlot,
    /// Capability `block-entities`: the bytes the mod keeps at block
    /// positions, by dimension and position; the game loads them back with
    /// [`Mods::load_block_data`].
    block_data: std::collections::HashMap<(String, [i32; 3]), Vec<u8>>,
    /// The namespaces whose blocks are the mod's: its id and manifest
    /// `namespaces`.
    block_namespaces: Vec<String>,
    /// Where [`Mods::tick_blocks`] resumes in `block_data`.
    block_cursor: usize,
    /// A server mod: `net.send` may name a player.
    server: bool,
    /// Capability `entities`: the handles of the mod's live entities, and
    /// the next one to hand out.
    entity_handles: std::collections::HashSet<u32>,
    next_handle: u32,
    /// Capability `containers`: the blocks the mod registered.
    container_blocks: Vec<(String, u32)>,
}

fn block_pos(p: BlockPos) -> block_bindings::exports::horizon::mods::block_events::Pos {
    block_bindings::exports::horizon::mods::block_events::Pos {
        x: p.x,
        y: p.y,
        z: p.z,
    }
}

/// Whether block `id` (`mymod:gear`) is in a namespace the mod owns.
fn owns_block(m: &Manifest, id: &str) -> bool {
    id.split_once(':')
        .is_some_and(|(ns, _)| ns == m.id || m.namespaces.iter().any(|n| n == ns))
}

/// The installed mods and the engine running them. `Mods::default()` is
/// the state with none: no engine, and every hook a no-op.
#[derive(Default)]
pub struct Mods {
    engine: Option<(Engine, Linker<State>)>,
    entries: Vec<Entry>,
    snapshot: Arc<Snapshot>,
    out: Vec<Output>,
    world: WorldSlot,
    /// Capability `transfer`'s tanks and batteries, every mod's.
    storage: StorageSlot,
    /// The ids of the running server mods, shared with every store.
    peers: Arc<Mutex<Vec<String>>>,
    /// Moves on whenever a mod starts or stops: the game resends the server
    /// mods' commands to players when it changes.
    generation: u64,
}

impl Mods {
    /// Changes whenever the running mods (so their commands) change.
    pub fn generation(&self) -> u64 {
        self.generation
    }
    /// Whether a running mod may use capability `world`: only then is a
    /// view worth lending.
    pub fn wants_world(&self) -> bool {
        self.entries.iter().any(|e| {
            e.status == Status::Running
                && e.manifest.as_ref().is_some_and(|m| {
                    m.capabilities.contains(&crate::Capability::World)
                        || m.capabilities.contains(&crate::Capability::BlockEntities)
                        || m.capabilities.contains(&crate::Capability::Containers)
                        || m.capabilities.contains(&crate::Capability::Redstone)
                        || m.capabilities.contains(&crate::Capability::Transfer)
                })
        })
    }
    /// Whether a running mod may use capability `redstone`: only then does
    /// the game read signals for the lent view.
    pub fn wants_redstone(&self) -> bool {
        self.entries.iter().any(|e| {
            e.status == Status::Running
                && e.manifest
                    .as_ref()
                    .is_some_and(|m| m.capabilities.contains(&crate::Capability::Redstone))
        })
    }
    /// Lends the game's world to capability `world` for the calls until
    /// [`Mods::take_world`]; with none lent, `get-block` finds nothing and
    /// `set-blocks` checks only its count.
    pub fn lend_world(&mut self, view: Box<dyn WorldView>) {
        if let Ok(mut slot) = self.world.lock() {
            *slot = Some(view);
        }
    }
    /// The view [`Mods::lend_world`] lent, back to the game.
    pub fn take_world(&mut self) -> Option<Box<dyn WorldView>> {
        self.world.lock().ok()?.take()
    }

    /// Reads every mod at `paths` and starts those `enabled` says are on,
    /// client and server mods alike. A command that `reserved` (vanilla's
    /// commands) or an earlier mod already uses is refused.
    pub fn load(
        paths: &[PathBuf],
        enabled: impl Fn(&str) -> bool,
        reserved: impl Fn(&str) -> bool,
    ) -> Mods {
        Mods::load_for(Runs::Both, paths, enabled, reserved)
    }
    /// [`Mods::load`], starting only the mods `runs` says this game runs; the
    /// others that are on are [`Status::Elsewhere`].
    pub fn load_for(
        runs: Runs,
        paths: &[PathBuf],
        enabled: impl Fn(&str) -> bool,
        reserved: impl Fn(&str) -> bool,
    ) -> Mods {
        let mut mods = Mods::default();
        for path in paths {
            let modified = modified(path);
            let mut entry = Entry {
                path: path.clone(),
                manifest: None,
                status: Status::Off,
                cost: Duration::ZERO,
                run: None,
                modified,
            };
            match Package::read(path) {
                Err(e) => entry.status = Status::Failed(e),
                Ok(package) => {
                    let on = enabled(&package.manifest.id);
                    entry.manifest = Some(package.manifest.clone());
                    if on && !runs.starts(package.manifest.side) {
                        entry.status = Status::Elsewhere;
                    } else if on {
                        // Content is read before the world loads; a mod whose
                        // content is refused stays off, with the reason.
                        let content = crate::Content::read(path, &package.manifest);
                        entry.status =
                            match content.and_then(|_| mods.start(path, &package, &reserved)) {
                                Ok(run) => {
                                    entry.run = run;
                                    Status::Running
                                }
                                Err(e) => Status::Failed(e),
                            };
                    }
                }
            }
            if let (Status::Failed(reason), Some(m)) = (&entry.status, &entry.manifest) {
                log::warn!("Horizon: mod {} did not start: {reason}", m.id);
            }
            mods.entries.push(entry);
        }
        mods
    }

    fn engine(&mut self) -> Result<&(Engine, Linker<State>), String> {
        if self.engine.is_none() {
            let mut config = Config::new();
            config.wasm_component_model(true).consume_fuel(true);
            let engine = Engine::new(&config).map_err(|e| e.to_string())?;
            let mut linker = Linker::new(&engine);
            HorizonMod::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            players::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            mod_world::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            BlockHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            NetHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            EntitiesHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            ContainersHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            RedstoneHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            ItemsHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            FilesHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            AttachmentsHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            TransferHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            PeersHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            MenusHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            ConsoleHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            StacksHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            RecipeManagerHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            EffectsHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            UiHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            RenderHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            RecipesHost::add_to_linker::<State, HasSelf<State>>(&mut linker, |s| s)
                .map_err(|e| e.to_string())?;
            self.engine = Some((engine, linker));
        }
        Ok(self.engine.as_ref().expect("made above"))
    }

    /// Starts a mod's code; `Ok(None)` for a content-only mod, which has
    /// nothing to run (the game reads its content before the world loads).
    /// `path` is the mod's own `.hmod` or folder, for capability `storage`'s
    /// per-mod directory.
    fn start(
        &mut self,
        path: &Path,
        package: &Package,
        reserved: &dyn Fn(&str) -> bool,
    ) -> Result<Option<Running>, String> {
        let m = &package.manifest;
        let Some(wasm) = &package.wasm else {
            return Ok(None);
        };
        // Two running mods with the same id would share one `data_dir`
        // (capability `storage`): the first loaded keeps the id.
        if self
            .entries
            .iter()
            .any(|e| e.run.is_some() && e.manifest.as_ref().is_some_and(|other| other.id == m.id))
        {
            return Err(format!("another running mod already uses id {}", m.id));
        }
        let server = m.side == Side::Server;
        let taken: Vec<String> = self
            .entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .filter(|r| r.hooks.server().is_some() == server)
            .flat_map(|r| r.hooks.commands())
            .map(str::to_owned)
            .collect();
        let snapshot = self.snapshot.clone();
        let world = self.world.clone();
        let storage = self.storage.clone();
        let peers = self.peers.clone();
        let (engine, linker) = self.engine()?;
        let component =
            Component::new(engine, wasm).map_err(|e| format!("not a component: {e}"))?;
        let ty = component.component_type();
        let imports: Vec<String> = ty
            .imports(engine)
            .map(|(name, _)| name.to_string())
            .collect();
        m.check_imports(imports.iter().map(String::as_str))?;
        // A function this Horizon doesn't have: say so, not wasmtime's linking error.
        let functions = crate::api_level::imports(wasm)?;
        let (level, function) = crate::api_level::needed(functions.iter().map(String::as_str));
        if let Some(why) = crate::api_level::too_new(level, function) {
            return Err(why);
        }
        let data_dir = path
            .parent()
            .unwrap_or(Path::new("."))
            .join(crate::manifest::STORAGE_DIR)
            .join(&m.id);
        let state = State {
            name: m.name.clone(),
            id: m.id.clone(),
            snapshot,
            out: Vec::new(),
            panel: None,
            ui: UiState::default(),
            shapes: Vec::new(),
            corner: None,
            zoom: None,
            minimap: None,
            far_terrain: None,
            input: InputState::default(),
            browser: BrowserState::default(),
            data_dir,
            package_path: path.to_owned(),
            attachments: Default::default(),
            storage,
            peers,
            storage_budget: STORAGE_WRITE_BUDGET,
            limits: StoreLimitsBuilder::new()
                .memory_size(MEMORY)
                .instances(16)
                .build(),
            world,
            block_data: Default::default(),
            block_cursor: 0,
            block_namespaces: std::iter::once(m.id.clone())
                .chain(m.namespaces.iter().cloned())
                .collect(),
            server,
            entity_handles: Default::default(),
            next_handle: 1,
            container_blocks: Vec::new(),
        };
        let mut store = Store::new(engine, state);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(FUEL_INIT).map_err(|e| e.to_string())?;
        let instance = linker
            .instantiate(&mut store, &component)
            .map_err(|e| reason(&e))?;
        let (net, server_net) = if server {
            (None, ServerNetHost::new(&mut store, &instance).ok())
        } else {
            (NetHost::new(&mut store, &instance).ok(), None)
        };
        let neighbor = if server {
            NeighborHost::new(&mut store, &instance).ok()
        } else {
            None
        };
        let item = if server {
            ItemHost::new(&mut store, &instance).ok()
        } else {
            None
        };
        let peer = if server {
            PeersHost::new(&mut store, &instance).ok()
        } else {
            None
        };
        let menu = if server {
            MenusHost::new(&mut store, &instance).ok()
        } else {
            None
        };
        let mut blocks = None;
        let mut ticking = Vec::new();
        let (mut hooks, ui) = if server {
            let exports = HorizonServerMod::new(&mut store, &instance).map_err(|e| reason(&e))?;
            let r = exports
                .horizon_mods_server_events()
                .call_init(&mut store)
                .map_err(|e| reason(&e))??;
            // `block-events` is a mod built since `block-entities`; older
            // server mods have no export and hear nothing of blocks.
            if let Ok(b) = BlockHost::new(&mut store, &instance) {
                store.set_fuel(FUEL_INIT).map_err(|e| e.to_string())?;
                ticking = b
                    .horizon_mods_block_events()
                    .call_ticking_blocks(&mut store)
                    .map_err(|e| reason(&e))?;
                ticking.retain(|id| {
                    let owned = owns_block(m, id);
                    if !owned {
                        log::warn!("Horizon: mod {} cannot tick {id}: not its block", m.id);
                    }
                    owned
                });
                blocks = Some(b);
            }
            (Hooks::Server(exports, r), None)
        } else {
            let exports = HorizonMod::new(&mut store, &instance).map_err(|e| reason(&e))?;
            // `ui-events` is a mod built since `ui`'s; older mods have no export
            // and stay callable through `events` alone.
            let ui = UiHost::new(&mut store, &instance).ok();
            let r = exports
                .horizon_mods_events()
                .call_init(&mut store)
                .map_err(|e| reason(&e))??;
            (Hooks::Client(exports, r), ui)
        };
        let free = |c: &str| {
            let free = !reserved(c) && !taken.iter().any(|t| t == c);
            if !free {
                log::warn!("Horizon: mod {} cannot take /{c}: already a command", m.id);
            }
            free
        };
        match &mut hooks {
            Hooks::Client(_, r) => r.commands.retain(|c| free(c)),
            Hooks::Server(_, r) => {
                r.commands.retain(|c| free(&c.name));
                for c in &mut r.commands {
                    if let Err(e) = check_arguments(&c.arguments) {
                        log::warn!(
                            "Horizon: mod {}'s /{}: {e}; it takes free text",
                            m.id,
                            c.name
                        );
                        c.arguments.clear();
                    }
                }
            }
        }
        self.generation += 1;
        self.out.append(&mut store.data_mut().out);
        Ok(Some(Running {
            store,
            ui,
            hooks,
            raycasts: functions.iter().any(|f| f == "game.raycast"),
            reads_recipes: functions.iter().any(|f| f.starts_with("recipes.")),
            blocks,
            ticking,
            net,
            server_net,
            neighbor,
            item,
            peer,
            menu,
        }))
    }

    /// True when at least one mod runs: the game skips its mod work otherwise.
    pub fn is_running(&self) -> bool {
        self.entries.iter().any(|e| e.run.is_some())
    }
    /// True when a server mod runs: the server skips its mod work otherwise.
    pub fn runs_server(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.run.as_ref().is_some_and(|r| r.hooks.server().is_some()))
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// What mods read until the next snapshot.
    pub fn set_snapshot(&mut self, snapshot: Snapshot) {
        self.snapshot = Arc::new(snapshot);
    }

    /// Calls `f` on every running mod whose hooks `wants`, with `fuel`.
    fn each(
        &mut self,
        fuel: u64,
        wants: impl Fn(&Hooks) -> bool,
        mut f: impl FnMut(&mut Running) -> wasmtime::Result<()>,
    ) {
        for i in 0..self.entries.len() {
            if self.entries[i]
                .run
                .as_ref()
                .is_some_and(|r| wants(&r.hooks))
            {
                self.call(i, fuel, &mut f);
            }
        }
    }
    /// [`Mods::each`] over the client mods.
    fn each_client(
        &mut self,
        fuel: u64,
        wants: impl Fn(&Registration) -> bool,
        mut f: impl FnMut(&ClientEvents, &mut Store<State>) -> wasmtime::Result<()>,
    ) {
        self.each(
            fuel,
            |h| h.client().is_some_and(&wants),
            |run| match &run.hooks {
                Hooks::Client(m, _) => f(m.horizon_mods_events(), &mut run.store),
                Hooks::Server(..) => Ok(()),
            },
        );
    }
    /// [`Mods::each`] over the server mods.
    fn each_server(
        &mut self,
        fuel: u64,
        wants: impl Fn(&ServerRegistration) -> bool,
        mut f: impl FnMut(&ServerEvents, &mut Store<State>) -> wasmtime::Result<()>,
    ) {
        self.each(
            fuel,
            |h| h.server().is_some_and(&wants),
            |run| match &run.hooks {
                Hooks::Server(m, _) => f(m.horizon_mods_server_events(), &mut run.store),
                Hooks::Client(..) => Ok(()),
            },
        );
    }

    /// One call into mod `i`; a trap or an overrun turns the mod off.
    fn call<R>(
        &mut self,
        i: usize,
        fuel: u64,
        f: impl FnOnce(&mut Running) -> wasmtime::Result<R>,
    ) -> Option<R> {
        let entry = &mut self.entries[i];
        let run = entry.run.as_mut()?;
        run.store.data_mut().snapshot = self.snapshot.clone();
        run.store.data_mut().storage_budget = STORAGE_WRITE_BUDGET;
        let started = Instant::now();
        let result = run.store.set_fuel(fuel).and_then(|()| f(run));
        // An average over about the last 64 calls.
        entry.cost = (entry.cost * 63 + started.elapsed()) / 64;
        let data = run.store.data_mut();
        let (open, close) = (
            std::mem::take(&mut data.ui.open),
            std::mem::take(&mut data.ui.close),
        );
        self.out.append(&mut data.out);
        if open {
            self.out.push(Output::OpenScreen { owner: i });
        }
        if close {
            self.out.push(Output::CloseScreen { owner: i });
        }
        match result {
            Ok(r) => Some(r),
            Err(e) => {
                let reason = reason(&e);
                log::warn!("Horizon: mod {} stopped: {reason}", run.store.data().id);
                let name = run.store.data().name.clone();
                self.out.push(Output::Stopped {
                    name,
                    reason: reason.clone(),
                });
                entry.status = Status::Failed(reason);
                entry.run = None;
                self.generation += 1;
                None
            }
        }
    }

    /// A game tick.
    pub fn tick(&mut self, tick: u64) {
        self.each_client(FUEL_TICK, |h| h.tick, |m, s| m.call_tick(s, tick));
    }
    /// A rendered frame, `seconds` after the last.
    pub fn frame(&mut self, seconds: f32) {
        self.each_client(FUEL_FRAME, |h| h.frame, |m, s| m.call_frame(s, seconds));
    }
    /// A chat line the player received, as plain text.
    pub fn chat(&mut self, text: &str) {
        self.each_client(FUEL_EVENT, |h| h.chat, |m, s| m.call_chat(s, text));
    }
    /// Whether a running client mod owns `/name`.
    pub fn has_command(&self, name: &str) -> bool {
        self.command_owner(false, name).is_some()
    }
    fn command_owner(&self, server: bool, name: &str) -> Option<usize> {
        self.entries.iter().position(|e| {
            e.run.as_ref().is_some_and(|r| {
                r.hooks.server().is_some() == server && r.hooks.commands().contains(&name)
            })
        })
    }
    /// Runs `line` (without its slash) if a client mod owns its command;
    /// false when none does.
    pub fn command(&mut self, line: &str) -> bool {
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        let Some(i) = self.command_owner(false, name) else {
            return false;
        };
        let result = self.call(i, FUEL_EVENT, |run| match &run.hooks {
            Hooks::Client(m, _) => m
                .horizon_mods_events()
                .call_command(&mut run.store, name, args),
            Hooks::Server(..) => Ok(Ok(None)),
        });
        match result {
            Some(Ok(Some(text))) => self.out.push(Output::Feedback { text, error: false }),
            Some(Err(text)) => self.out.push(Output::Feedback { text, error: true }),
            Some(Ok(None)) | None => {}
        }
        true
    }
    /// The commands running client mods own, for suggestions.
    pub fn commands(&self) -> impl Iterator<Item = &str> {
        self.owned_commands(false)
    }
    fn owned_commands(&self, server: bool) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .filter(move |r| r.hooks.server().is_some() == server)
            .flat_map(|r| r.hooks.commands())
    }

    /// A server tick.
    pub fn server_tick(&mut self, tick: u64) {
        // The running server mods, for `peers.running`.
        let ids = self.running_server_ids();
        if let Ok(mut peers) = self.peers.lock() {
            *peers = ids;
        }
        self.each_server(FUEL_TICK, |h| h.tick, |m, s| m.call_tick(s, tick));
    }
    /// The blocks (ids without properties) running server mods ask the
    /// game to tick: the game calls [`Mods::block_tick`] for loaded
    /// instances of them.
    pub fn ticking_blocks(&self) -> impl Iterator<Item = &str> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| r.ticking.iter().map(String::as_str))
    }
    /// Ticks the positions where a mod keeps data (`block-entities.set`)
    /// whose chunk is loaded in the lent world: `block-tick` for each that
    /// is still a block the mod ticks, at most [`BLOCK_TICKS_PER_TICK`] per
    /// mod (the rest next tick). Data under any other block is forgotten.
    pub fn tick_blocks(&mut self) {
        let world = self.world.clone();
        for i in self.block_owners(|r| !r.ticking.is_empty()) {
            let mut due = Vec::new();
            let mut gone = Vec::new();
            {
                let Ok(view) = world.lock() else { return };
                let Some(view) = view.as_ref() else { return };
                let run = self.entries[i].run.as_mut().expect("owner is running");
                let n = run.store.data().block_data.len();
                if n == 0 {
                    continue;
                }
                let start = run.store.data().block_cursor % n;
                let keys: Vec<_> = {
                    let data = &run.store.data().block_data;
                    data.keys()
                        .skip(start)
                        .chain(data.keys())
                        .take(n.min(BLOCK_TICKS_PER_TICK))
                        .cloned()
                        .collect()
                };
                run.store.data_mut().block_cursor = start + keys.len();
                for (dimension, [x, y, z]) in keys {
                    let Some(state) = view.block(&dimension, x, y, z) else {
                        continue;
                    };
                    let id = state.split('[').next().unwrap_or(&state);
                    if run.ticking.iter().any(|t| t == id) {
                        due.push((dimension, BlockPos { x, y, z }, state));
                    } else {
                        gone.push((dimension, [x, y, z]));
                    }
                }
            }
            if !gone.is_empty() {
                let data = self.entries[i]
                    .run
                    .as_mut()
                    .expect("owner is running")
                    .store
                    .data_mut();
                for (dimension, pos) in gone {
                    if data.block_data.remove(&(dimension.clone(), pos)).is_some() {
                        data.out.push(Output::BlockData {
                            mod_id: data.id.clone(),
                            dimension,
                            pos,
                            data: None,
                        });
                    }
                }
                self.out.append(&mut data.out);
            }
            for (dimension, pos, state) in due {
                self.call(i, FUEL_EVENT, |run| match &run.blocks {
                    Some(b) => b.horizon_mods_block_events().call_block_tick(
                        &mut run.store,
                        &dimension,
                        block_pos(pos),
                        &state,
                    ),
                    None => Ok(()),
                });
                if self.entries[i].run.is_none() {
                    break;
                }
            }
        }
    }
    /// Blocks changed since the last tick in `dimension`: each mod that
    /// keeps data at a neighbour of one hears `neighbor-changed`, at most
    /// [`NEIGHBOR_CALLS_PER_TICK`] calls to a mod.
    pub fn neighbors_changed(&mut self, dimension: &str, changed: &[[i32; 3]]) {
        const AROUND: [[i32; 3]; 6] = [
            [1, 0, 0],
            [-1, 0, 0],
            [0, 1, 0],
            [0, -1, 0],
            [0, 0, 1],
            [0, 0, -1],
        ];
        let world = self.world.clone();
        for i in self.block_owners(|r| r.neighbor.is_some()) {
            let mut calls = Vec::new();
            {
                let Ok(view) = world.lock() else { return };
                let Some(view) = view.as_ref() else { return };
                let run = self.entries[i].run.as_ref().expect("owner is running");
                let data = &run.store.data().block_data;
                'changes: for p in changed {
                    let Some(from_state) = view.block(dimension, p[0], p[1], p[2]) else {
                        continue;
                    };
                    for d in AROUND {
                        let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                        if !data.contains_key(&(dimension.to_owned(), n)) {
                            continue;
                        }
                        let Some(state) = view.block(dimension, n[0], n[1], n[2]) else {
                            continue;
                        };
                        calls.push((n, state, *p, from_state.clone()));
                        if calls.len() >= NEIGHBOR_CALLS_PER_TICK {
                            break 'changes;
                        }
                    }
                }
            }
            for (n, state, p, from_state) in calls {
                self.call(i, FUEL_EVENT, |run| match &run.neighbor {
                    Some(h) => h.horizon_mods_neighbor_events().call_neighbor_changed(
                        &mut run.store,
                        dimension,
                        n[0],
                        n[1],
                        n[2],
                        &state,
                        p[0],
                        p[1],
                        p[2],
                        &from_state,
                    ),
                    None => Ok(()),
                });
                if self.entries[i].run.is_none() {
                    break;
                }
            }
        }
    }
    /// A loaded instance of a block some mod ticks, at `pos` in
    /// `dimension`, in `state` (`mymod:gear[axis=y]`).
    pub fn block_tick(&mut self, dimension: &str, pos: BlockPos, state: &str) {
        let id = state.split('[').next().unwrap_or(state);
        for i in self.block_owners(|r| r.ticking.iter().any(|t| t == id)) {
            self.call(i, FUEL_EVENT, |run| match &run.blocks {
                Some(b) => b.horizon_mods_block_events().call_block_tick(
                    &mut run.store,
                    dimension,
                    block_pos(pos),
                    state,
                ),
                None => Ok(()),
            });
        }
    }
    /// A message from server mod `from` to server mod `to`: true when one by
    /// that id runs and heard it.
    pub fn peer_message(&mut self, to: &str, from: &str, data: &[u8]) -> bool {
        let Some(i) = (0..self.entries.len()).find(|&i| {
            self.entries[i]
                .run
                .as_ref()
                .is_some_and(|r| r.peer.is_some() && r.store.data().id == to)
        }) else {
            return false;
        };
        self.call(i, FUEL_EVENT, |run| match &run.peer {
            Some(h) => h
                .horizon_mods_peer_events()
                .call_message(&mut run.store, from, data),
            None => Ok(()),
        });
        true
    }
    /// What `player` typed into text field `id` of mod `mod_id`'s screen.
    pub fn menu_text(&mut self, mod_id: &str, player: &str, id: u32, value: &str) {
        let Some(i) = self.menu_owner_index(mod_id) else {
            return;
        };
        self.call(i, FUEL_EVENT, |run| match &run.menu {
            Some(h) => h
                .horizon_mods_menu_events()
                .call_text(&mut run.store, player, id, value),
            None => Ok(()),
        });
    }
    /// `player` closed the screen mod `mod_id` opened for them.
    pub fn menu_closed(&mut self, mod_id: &str, player: &str) {
        let Some(i) = self.menu_owner_index(mod_id) else {
            return;
        };
        self.call(i, FUEL_EVENT, |run| match &run.menu {
            Some(h) => h
                .horizon_mods_menu_events()
                .call_closed(&mut run.store, player),
            None => Ok(()),
        });
    }
    fn menu_owner_index(&self, mod_id: &str) -> Option<usize> {
        (0..self.entries.len()).find(|&i| {
            self.entries[i]
                .run
                .as_ref()
                .is_some_and(|r| r.menu.is_some() && r.store.data().id == mod_id)
        })
    }
    /// `player` pressed button `id` on the screen mod `mod_id` opened for them.
    pub fn menu_button(&mut self, mod_id: &str, player: &str, id: u32) {
        let Some(i) = (0..self.entries.len()).find(|&i| {
            self.entries[i]
                .run
                .as_ref()
                .is_some_and(|r| r.menu.is_some() && r.store.data().id == mod_id)
        }) else {
            return;
        };
        self.call(i, FUEL_EVENT, |run| match &run.menu {
            Some(h) => h
                .horizon_mods_menu_events()
                .call_button(&mut run.store, player, id),
            None => Ok(()),
        });
    }
    /// The ids of the server mods running now, for `peers.running`.
    fn running_server_ids(&self) -> Vec<String> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .filter(|r| r.store.data().server)
            .map(|r| r.store.data().id.clone())
            .collect()
    }
    /// `player` used `item` on `target` (or in the air): true when a mod whose
    /// namespace the item or the block is in took the click.
    pub fn item_used(
        &mut self,
        player: &ServerPlayer,
        item: &str,
        sneaking: bool,
        target: Option<&BlockTarget>,
    ) -> bool {
        let ns = |id: &str| {
            id.split('[')
                .next()
                .unwrap_or(id)
                .split_once(':')
                .map(|(n, _)| n.to_owned())
        };
        let item_ns = ns(item);
        let block_ns = target.and_then(|t| ns(&t.state));
        // Every mod that hears items, those whose namespace the item or the
        // block is in first: the first to take it ends it.
        let owns = |i: usize| {
            self.entries[i].run.as_ref().is_some_and(|r| {
                r.store.data().block_namespaces.iter().any(|n| {
                    item_ns.as_deref() == Some(n.as_str())
                        || block_ns.as_deref() == Some(n.as_str())
                })
            })
        };
        let mut owners: Vec<usize> = (0..self.entries.len())
            .filter(|&i| {
                self.entries[i]
                    .run
                    .as_ref()
                    .is_some_and(|r| r.item.is_some())
            })
            .collect();
        owners.sort_by_key(|&i| !owns(i));
        let mut handled = false;
        for i in owners {
            handled |= self
                .call(i, FUEL_EVENT, |run| match &run.item {
                    Some(h) => h.horizon_mods_item_events().call_item_used(
                        &mut run.store,
                        &player.id,
                        item,
                        sneaking,
                        target,
                    ),
                    None => Ok(false),
                })
                .unwrap_or(false);
            if handled {
                break;
            }
        }
        handled
    }
    /// `player` used the block at `pos` with an empty hand: true when the
    /// mod that owns it handled that.
    pub fn block_used(
        &mut self,
        player: &ServerPlayer,
        dimension: &str,
        pos: BlockPos,
        state: &str,
    ) -> bool {
        let id = state.split('[').next().unwrap_or(state);
        let owners = self.block_owners(|r| {
            r.store.data().block_namespaces.iter().any(|n| {
                id.strip_prefix(n.as_str())
                    .is_some_and(|r| r.starts_with(':'))
            })
        });
        let mut handled = false;
        for i in owners {
            handled |= self
                .call(i, FUEL_EVENT, |run| match &run.blocks {
                    Some(b) => b.horizon_mods_block_events().call_block_used(
                        &mut run.store,
                        &player.id,
                        dimension,
                        block_pos(pos),
                        state,
                    ),
                    None => Ok(false),
                })
                .unwrap_or(false);
        }
        handled
    }
    /// Gives a mod back the bytes the game saved at a block position
    /// (`Output::BlockData` when they were set), as a chunk loads.
    pub fn load_block_data(&mut self, mod_id: &str, dimension: &str, pos: [i32; 3], data: Vec<u8>) {
        // The mods' tanks and batteries, saved under one id.
        if mod_id == TRANSFER_ID {
            if let (Ok(mut map), Some(storage)) = (self.storage.lock(), Storage::decode(&data))
                && map.len() < STORAGES
            {
                map.insert((dimension.to_owned(), pos), storage);
            }
            return;
        }
        if data.len() > BLOCK_DATA_BYTES {
            return;
        }
        if let Some(run) = self
            .entries
            .iter_mut()
            .filter_map(|e| e.run.as_mut())
            .find(|r| r.store.data().id == mod_id)
        {
            // An attachment saved among the block data.
            if let Some(target) = dimension.strip_prefix(ATTACHMENT_MARK) {
                let map = &mut run.store.data_mut().attachments;
                if map.len() < ATTACHMENTS {
                    map.insert((target.to_owned(), pos), data);
                }
                return;
            }
            let map = &mut run.store.data_mut().block_data;
            if map.len() < BLOCK_DATA_POSITIONS {
                map.insert((dimension.to_owned(), pos), data);
            }
        }
    }
    /// A message on mod `mod_id`'s channel: for a client mod `player` is none,
    /// for a server mod the id of the player who sent it. True when a running
    /// mod of that id took it.
    pub fn net_message(&mut self, mod_id: &str, player: Option<&str>, data: &[u8]) -> bool {
        if data.len() > NET_BYTES {
            return false;
        }
        let Some(i) = (0..self.entries.len()).find(|&i| {
            self.entries[i].run.as_ref().is_some_and(|r| {
                (r.net.is_some() || r.server_net.is_some()) && r.store.data().id == mod_id
            })
        }) else {
            return false;
        };
        self.call(i, FUEL_EVENT, |run| match (&run.net, &run.server_net) {
            (Some(n), _) => n
                .horizon_mods_net_events()
                .call_message(&mut run.store, data),
            (_, Some(n)) => n.horizon_mods_server_net_events().call_message(
                &mut run.store,
                player.unwrap_or_default(),
                data,
            ),
            _ => Ok(()),
        })
        .is_some()
    }
    /// The running server mods with a `block-events` export that `wants`.
    fn block_owners(&self, wants: impl Fn(&Running) -> bool) -> Vec<usize> {
        (0..self.entries.len())
            .filter(|&i| {
                self.entries[i]
                    .run
                    .as_ref()
                    .is_some_and(|r| r.blocks.is_some() && wants(r))
            })
            .collect()
    }
    /// `player` joined the server (they are in the snapshot's players).
    pub fn player_joined(&mut self, player: &ServerPlayer) {
        self.each_server(
            FUEL_EVENT,
            |h| h.joins,
            |m, s| m.call_player_joined(s, player),
        );
    }
    /// `player` left the server (they are no longer in the snapshot's players).
    pub fn player_left(&mut self, player: &ServerPlayer) {
        self.each_server(
            FUEL_EVENT,
            |h| h.joins,
            |m, s| m.call_player_left(s, player),
        );
    }
    /// A chat message `player` sent, as plain text.
    pub fn player_chat(&mut self, player: &ServerPlayer, text: &str) {
        self.each_server(FUEL_EVENT, |h| h.chat, |m, s| m.call_chat(s, player, text));
    }
    /// `player` broke `state` at `pos`.
    pub fn block_broken(&mut self, player: &ServerPlayer, pos: BlockPos, state: &str) {
        self.each_server(
            FUEL_EVENT,
            |h| h.blocks,
            |m, s| m.call_block_broken(s, player, pos, state),
        );
    }
    /// `player` placed `state` at `pos`.
    pub fn block_placed(&mut self, player: &ServerPlayer, pos: BlockPos, state: &str) {
        self.each_server(
            FUEL_EVENT,
            |h| h.blocks,
            |m, s| m.call_block_placed(s, player, pos, state),
        );
    }
    /// The commands running server mods own and their arguments, for the
    /// server's command tree.
    pub fn server_commands(&self) -> impl Iterator<Item = &ServerCommand> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.hooks.server())
            .flat_map(|r| r.commands.iter())
    }
    /// Runs `line` (without its slash), typed by `player`, if a server mod
    /// owns its command; its feedback is an [`Output::Reply`]. False when
    /// none does.
    pub fn server_command(&mut self, player: &ServerPlayer, line: &str) -> bool {
        let (name, args) = line.split_once(' ').unwrap_or((line, ""));
        let Some(i) = self.command_owner(true, name) else {
            return false;
        };
        let reply = |text, error| Output::Reply {
            player: player.id.clone(),
            text,
            error,
        };
        let arguments = self.entries[i]
            .run
            .as_ref()
            .and_then(|r| r.hooks.server())
            .and_then(|r| r.commands.iter().find(|c| c.name == name))
            .map(|c| c.arguments.as_slice())
            .unwrap_or_default();
        let values = match crate::commands::parse(arguments, args, player, &self.snapshot.players) {
            Ok(values) => values,
            Err(e) => {
                self.out.push(reply(e, true));
                return true;
            }
        };
        let result = self.call(i, FUEL_EVENT, |run| match &run.hooks {
            Hooks::Server(m, _) => m.horizon_mods_server_events().call_command(
                &mut run.store,
                player,
                name,
                args,
                &values,
            ),
            Hooks::Client(..) => Ok(Ok(None)),
        });
        match result {
            Some(Ok(Some(text))) => self.out.push(reply(text, false)),
            Some(Err(text)) => self.out.push(reply(text, true)),
            Some(Ok(None)) | None => {}
        }
        true
    }
    /// The running mods' HUD panels.
    pub fn panels(&self) -> impl Iterator<Item = &Panel> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.store.data().panel.as_ref())
    }
    /// The running mods' corner panels (capability `render`), in load order.
    pub fn corner_panels(&self) -> impl Iterator<Item = &CornerPanel> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.store.data().corner.as_ref())
    }
    /// Some running mod calls `game.raycast`: the snapshot needs a
    /// [`Snapshot::view_hit`].
    pub fn wants_raycast(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.run.as_ref().is_some_and(|r| r.raycasts))
    }
    /// The panel beside container screens (`browser.set-items`): the first
    /// running mod with items or recipes, its index, the panel's version and
    /// the items.
    pub fn browser_items(&self) -> Option<(usize, u64, &[BrowserStack])> {
        self.entries.iter().enumerate().find_map(|(i, e)| {
            let b = &e.run.as_ref()?.store.data().browser;
            (!b.items.is_empty() || b.recipes.is_some()).then_some((
                i,
                b.version,
                b.items.as_slice(),
            ))
        })
    }
    /// What mod `owner`'s panel shows instead of its items
    /// (`browser.set-recipes`).
    pub fn browser_recipes(&self, owner: usize) -> Option<&RecipePage> {
        let b = &self.entries.get(owner)?.run.as_ref()?.store.data().browser;
        b.recipes.as_ref()
    }
    /// The player did `event` on mod `owner`'s panel.
    pub fn browser_event(&mut self, owner: usize, event: BrowserEvent) {
        if let Some(run) = self.entries.get_mut(owner).and_then(|e| e.run.as_mut()) {
            run.store.data_mut().browser.push(event);
        }
    }
    /// The key `key` went down with the pointer on `stack` in a container
    /// screen: each mod with a key on it hears its own name for it.
    pub fn browser_key(&mut self, key: &str, stack: &BrowserStack) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let data = run.store.data_mut();
            let names: Vec<String> = data
                .input
                .keys
                .iter()
                .filter(|k| k.key == key)
                .map(|k| k.name.clone())
                .collect();
            for key in names {
                data.browser.push(BrowserEvent::Key(BrowserItemKey {
                    key,
                    stack: stack.clone(),
                }));
            }
        }
    }
    /// Some running mod reads capability `recipes`: the snapshot needs
    /// [`Snapshot::recipes`] and [`Snapshot::known_recipes`].
    pub fn wants_recipes(&self) -> bool {
        self.entries
            .iter()
            .any(|e| e.run.as_ref().is_some_and(|r| r.reads_recipes))
    }
    /// Some running mod has a `frame` hook: the game refreshes the snapshot
    /// every frame for it.
    pub fn wants_frames(&self) -> bool {
        self.entries.iter().any(|e| {
            e.run
                .as_ref()
                .is_some_and(|r| r.hooks.client().is_some_and(|h| h.frame))
        })
    }
    /// Some running mod claimed items (capability `input`).
    pub fn claims_any(&self) -> bool {
        self.inputs().any(|i| !i.claimed.is_empty())
    }
    /// Some running mod claimed the item `id`.
    pub fn claims(&self, id: &str) -> bool {
        self.inputs().any(|i| i.claimed.iter().any(|c| c == id))
    }
    /// Some running mod registered a key.
    pub fn has_keys(&self) -> bool {
        self.inputs().any(|i| !i.keys.is_empty())
    }
    /// The mods' keys: (mod name, key name, label, key), for Controls.
    pub fn mod_keys(&self) -> impl Iterator<Item = (&str, &ModKey)> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| {
                let d = r.store.data();
                d.input.keys.iter().map(move |k| (d.name.as_str(), k))
            })
    }
    /// Mod `i`'s keys, as it registered them (with the player's choices).
    pub fn keys_of(&self, i: usize) -> &[ModKey] {
        self.entries
            .get(i)
            .and_then(|e| e.run.as_ref())
            .map_or(&[], |r| r.store.data().input.keys.as_slice())
    }
    /// The keys the player chose (`ui`'s key widget), by mod id: each
    /// running mod's `name → key`. They replace the defaults registered
    /// so far and later ones. Keys that aren't keys are skipped.
    pub fn set_bindings<'a>(&mut self, chosen: impl Fn(&str) -> Vec<(&'a str, &'a str)>) {
        for e in &mut self.entries {
            let (Some(m), Some(run)) = (&e.manifest, e.run.as_mut()) else {
                continue;
            };
            let input = &mut run.store.data_mut().input;
            input.bindings = chosen(&m.id)
                .into_iter()
                .filter(|(_, key)| key_ok(key))
                .map(|(n, k)| (n.to_owned(), k.to_owned()))
                .collect();
            for (name, key) in input.bindings.clone() {
                input.rebind(&name, key);
            }
        }
    }
    /// The player binds mod `i`'s key `name` to `key` (`key.keyboard.x`,
    /// `key.mouse.4`, `key.keyboard.unknown` for none). False when the mod
    /// has no such key or `key` isn't one.
    pub fn rebind(&mut self, i: usize, name: &str, key: &str) -> bool {
        let Some(run) = self.entries.get_mut(i).and_then(|e| e.run.as_mut()) else {
            return false;
        };
        let input = &mut run.store.data_mut().input;
        if !key_ok(key) || !input.keys.iter().any(|k| k.name == name) {
            return false;
        }
        input.bindings.retain(|(n, _)| n != name);
        input.bindings.push((name.to_owned(), key.to_owned()));
        input.rebind(name, key.to_owned());
        true
    }
    /// The use button went down with `item`: `use-start` to each mod that
    /// claimed it.
    pub fn use_start(&mut self, item: &HeldItem) {
        let event = InputEvent::UseStart(ItemUse {
            item: item.clone(),
            ticks: 0,
        });
        self.push_input(|i| i.claimed.contains(&item.id), &event);
    }
    /// The claimed use ended after `ticks`.
    pub fn use_stop(&mut self, item: &HeldItem, ticks: u32) {
        let event = InputEvent::UseStop(ItemUse {
            item: item.clone(),
            ticks,
        });
        self.push_input(|i| i.claimed.contains(&item.id), &event);
    }
    /// The key `key` (`key.keyboard.r`) went down or up: each mod with a
    /// key on it hears its own name for it.
    pub fn key(&mut self, key: &str, down: bool) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let input = &mut run.store.data_mut().input;
            let names: Vec<String> = input
                .keys
                .iter()
                .filter(|k| k.key == key)
                .map(|k| k.name.clone())
                .collect();
            for name in names {
                let held = input.down.iter().position(|n| *n == name);
                match (down, held) {
                    (true, None) => input.down.push(name.clone()),
                    (false, Some(i)) => {
                        input.down.swap_remove(i);
                    }
                    // Already let go when a screen opened.
                    (false, None) => continue,
                    (true, Some(_)) => {}
                }
                input.push(if down {
                    InputEvent::KeyDown(name)
                } else {
                    InputEvent::KeyUp(name)
                });
            }
        }
    }
    /// A screen opened, or the game stopped playing: every mod key held
    /// lets go (`key-up`), so a key needs a new press afterwards.
    pub fn release_keys(&mut self) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let input = &mut run.store.data_mut().input;
            for name in std::mem::take(&mut input.down) {
                input.push(InputEvent::KeyUp(name));
            }
        }
    }
    /// Some running mod holds the mouse wheel.
    pub fn wheel_claimed(&self) -> bool {
        self.inputs().any(|i| i.wheel)
    }
    /// Wheel steps while playing: `wheel` to the mods holding it; false
    /// when none does (the hotbar takes them).
    pub fn wheel(&mut self, steps: f32) -> bool {
        let event = InputEvent::Wheel(steps);
        let claimed = self.wheel_claimed();
        self.push_input(|i| i.wheel, &event);
        claimed
    }
    /// The strongest zoom a running mod asks for (capability `render`).
    pub fn zoom(&self) -> Option<ModZoom> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref()?.store.data().zoom)
            .max_by(|a, b| a.factor.total_cmp(&b.factor))
    }
    /// The minimap the first running mod shows (capability `render`).
    pub fn minimap(&self) -> Option<ModMinimap> {
        self.entries
            .iter()
            .find_map(|e| e.run.as_ref()?.store.data().minimap)
    }
    /// The far terrain the first running mod draws (capability `render`).
    pub fn far_terrain(&self) -> Option<ModFarTerrain> {
        self.entries
            .iter()
            .find_map(|e| e.run.as_ref()?.store.data().far_terrain)
    }
    fn inputs(&self) -> impl Iterator<Item = &InputState> {
        self.entries
            .iter()
            .filter_map(|e| Some(&e.run.as_ref()?.store.data().input))
    }
    fn push_input(&mut self, to: impl Fn(&InputState) -> bool, event: &InputEvent) {
        for run in self.entries.iter_mut().filter_map(|e| e.run.as_mut()) {
            let input = &mut run.store.data_mut().input;
            if to(input) {
                input.push(event.clone());
            }
        }
    }
    /// The running mods' HUD shapes (capability `render`), each mod's in
    /// its own order, mods in load order.
    pub fn hud_shapes(&self) -> impl Iterator<Item = &Shape> {
        self.entries
            .iter()
            .filter_map(|e| e.run.as_ref())
            .flat_map(|r| &r.store.data().shapes)
    }
    /// The screen mod `i` last pushed with `ui.open-screen`, while it is up.
    pub fn screen(&self, i: usize) -> Option<&UiScreen> {
        self.entries
            .get(i)?
            .run
            .as_ref()?
            .store
            .data()
            .ui
            .screen
            .as_ref()
    }
    /// The widgets `ui.set-settings` pushed: mod `i`'s page in
    /// Options > Mods. Empty while it has none.
    pub fn settings_widgets(&self, i: usize) -> &[UiWidget] {
        self.entries
            .get(i)
            .and_then(|e| e.run.as_ref())
            .map_or(&[], |r| &r.store.data().ui.settings)
    }
    /// A `list` widget's row was picked on a surface of mod `i`.
    pub fn ui_select(&mut self, i: usize, id: u32, row: u32) {
        self.ui_event(i, |ui, s| {
            ui.horizon_mods_ui_events().call_select(s, id, row)
        });
    }
    /// The player left mod `i`'s screen (Esc or Done).
    pub fn ui_closed(&mut self, i: usize) {
        self.ui_event(i, |ui, s| ui.horizon_mods_ui_events().call_closed(s));
    }
    /// The player opened mod `i`'s settings page.
    pub fn ui_settings_opened(&mut self, i: usize) {
        self.ui_event(i, |ui, s| {
            ui.horizon_mods_ui_events().call_settings_opened(s)
        });
    }
    /// A `ui-events` call on mod `i`; no export, no call (mods built
    /// before `ui` have none).
    fn ui_event(
        &mut self,
        i: usize,
        f: impl FnOnce(&UiHost, &mut Store<State>) -> wasmtime::Result<()>,
    ) {
        let has_ui = self
            .entries
            .get(i)
            .and_then(|e| e.run.as_ref())
            .is_some_and(|r| r.ui.is_some());
        if has_ui {
            self.call(i, FUEL_EVENT, |run| {
                f(run.ui.as_ref().expect("checked above"), &mut run.store)
            });
        }
    }
    /// What mods asked to show since the last call.
    pub fn drain(&mut self) -> Vec<Output> {
        std::mem::take(&mut self.out)
    }

    /// Restarts the mods whose file changed since they loaded (mod
    /// development: rebuild, and the game picks it up). True when any did.
    pub fn reload_changed(&mut self, reserved: impl Fn(&str) -> bool) -> bool {
        let mut any = false;
        for i in 0..self.entries.len() {
            let entry = &self.entries[i];
            let now = modified(&entry.path);
            let was_on = !matches!(entry.status, Status::Off | Status::Elsewhere);
            if now == entry.modified || !was_on {
                continue;
            }
            any = true;
            self.generation += 1;
            self.entries[i].modified = now;
            self.entries[i].run = None;
            let path = self.entries[i].path.clone();
            let started = Package::read(&path).and_then(|p| {
                self.entries[i].manifest = Some(p.manifest.clone());
                self.start(&path, &p, &reserved)
            });
            let entry = &mut self.entries[i];
            let id = entry.manifest.as_ref().map_or("?", |m| m.id.as_str());
            match started {
                Ok(run) => {
                    log::info!("Horizon: mod {id} reloaded");
                    entry.run = run;
                    entry.status = Status::Running;
                }
                Err(e) => {
                    log::warn!("Horizon: mod {id} did not reload: {e}");
                    entry.status = Status::Failed(e);
                }
            }
        }
        any
    }
}

/// A command's arguments are well formed: names given and distinct,
/// optional ones last, free text only last.
fn check_arguments(arguments: &[Argument]) -> Result<(), String> {
    for (i, a) in arguments.iter().enumerate() {
        if a.name.is_empty()
            || !a
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            return Err(format!("argument name '{}' isn't [A-Za-z0-9_]+", a.name));
        }
        if arguments[..i].iter().any(|b| b.name == a.name) {
            return Err(format!("two arguments are named {}", a.name));
        }
        if i > 0 && arguments[i - 1].optional && !a.optional {
            return Err(format!("{} is required after an optional argument", a.name));
        }
        if a.kind == ArgumentKind::Text && i + 1 < arguments.len() {
            return Err(format!(
                "{} takes the rest of the line, so it must be last",
                a.name
            ));
        }
    }
    Ok(())
}

fn modified(path: &Path) -> Option<SystemTime> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.is_dir() {
        // A folder mod changes when its component does.
        return std::fs::metadata(path.join(crate::manifest::WASM))
            .ok()?
            .modified()
            .ok();
    }
    meta.modified().ok()
}

/// A trap or error, as the player reads it.
fn reason(e: &wasmtime::Error) -> String {
    match e.downcast_ref::<Trap>() {
        Some(Trap::OutOfFuel) => "it took too long (an endless loop?)".into(),
        Some(Trap::UnreachableCodeReached) => "it crashed (a panic)".into(),
        Some(trap) => format!("it crashed: {trap}"),
        None => format!("{e:#}"),
    }
}

fn clip(text: String) -> String {
    clip_to(text, LINE_CHARS)
}
fn clip_to(text: String, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => text[..at].to_string(),
        None => text,
    }
}

/// Widgets on a `ui` screen or settings page, and rows on one `list`.
const WIDGETS: usize = 64;
const LIST_ROWS: usize = 512;

fn clip_widgets(widgets: Vec<UiWidget>) -> Vec<UiWidget> {
    widgets
        .into_iter()
        .take(WIDGETS)
        .map(|w| match w {
            UiWidget::Text(mut t) => {
                t.text = clip(t.text);
                UiWidget::Text(t)
            }
            UiWidget::Button(mut b) => {
                b.label = clip(b.label);
                UiWidget::Button(b)
            }
            UiWidget::Item(mut i) => {
                i.item = clip(i.item);
                i.name = clip(i.name);
                UiWidget::Item(i)
            }
            UiWidget::List(mut l) => {
                l.rows = l.rows.into_iter().take(LIST_ROWS).map(clip).collect();
                l.selected = l.selected.filter(|&s| (s as usize) < l.rows.len());
                UiWidget::List(l)
            }
        })
        .collect()
}

impl mod_log::Host for State {
    fn write(&mut self, level: mod_log::Level, message: String) {
        let level = match level {
            mod_log::Level::Debug => log::Level::Debug,
            mod_log::Level::Info => log::Level::Info,
            mod_log::Level::Warn => log::Level::Warn,
            mod_log::Level::Error => log::Level::Error,
        };
        log::log!(level, "[mod {}] {}", self.id, clip(message));
    }
}
impl game::Host for State {
    fn get_player(&mut self) -> Option<Player> {
        self.snapshot.player.clone()
    }
    fn get_clock(&mut self) -> Option<Clock> {
        self.snapshot.clock
    }
    fn raycast(&mut self, distance: f32) -> Option<Hit> {
        let (at, hit) = self.snapshot.view_hit.as_ref()?;
        // NaN compares false: no hit.
        (*at <= distance.min(RAYCAST_MAX) && !distance.is_nan()).then(|| hit.clone())
    }
}
impl browser::Host for State {
    fn set_items(&mut self, items: Vec<BrowserStack>) {
        self.browser.items = items
            .into_iter()
            .take(BROWSER_ITEMS)
            .map(|s| BrowserStack {
                item: clip(s.item),
                components: clip_to(s.components, 4096),
                ..s
            })
            .collect();
        self.browser.version = BROWSER_VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn set_recipes(
        &mut self,
        title: String,
        tabs: Vec<BrowserStack>,
        tab: u32,
        recipes: Vec<BrowserRecipe>,
    ) {
        let stack = |s: BrowserStack| BrowserStack {
            item: clip(s.item),
            components: clip_to(s.components, 4096),
            ..s
        };
        let cycle = |list: Vec<BrowserStack>| -> Vec<BrowserStack> {
            list.into_iter().take(SLOT_STACKS).map(stack).collect()
        };
        self.browser.recipes = (!recipes.is_empty()).then(|| {
            let recipes = recipes
                .into_iter()
                .take(BROWSER_RECIPES)
                .map(|r| BrowserRecipe {
                    width: r.width.clamp(1, 3),
                    height: r.height.clamp(1, 3),
                    slots: r.slots.into_iter().take(RECIPE_SLOTS).map(cycle).collect(),
                    results: cycle(r.results),
                    note: clip(r.note),
                    ..r
                })
                .collect();
            RecipePage {
                title: clip(title),
                tabs: tabs.into_iter().take(RECIPE_TABS).map(stack).collect(),
                tab,
                recipes,
            }
        });
        self.browser.version = BROWSER_VERSION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn poll(&mut self) -> Vec<BrowserEvent> {
        self.browser.events.drain(..).collect()
    }
}
impl recipes::Host for State {
    fn items(&mut self) -> Vec<RecipeItem> {
        self.snapshot
            .recipes
            .as_ref()
            .map_or_else(Vec::new, |r| r.items.clone())
    }
    fn all(&mut self) -> Vec<Recipe> {
        self.snapshot
            .recipes
            .as_ref()
            .map_or_else(Vec::new, |r| r.recipes.clone())
    }
    fn known(&mut self) -> Vec<i32> {
        self.snapshot.known_recipes.to_vec()
    }
    fn place(&mut self, book_id: i32, all: bool) -> Result<(), String> {
        self.recipe_action()?;
        if !self.snapshot.placing {
            return Err("no screen that takes recipes is open".into());
        }
        if !self.snapshot.known_recipes.contains(&book_id) {
            return Err(format!(
                "recipe {book_id} is not in the player's recipe book"
            ));
        }
        self.out.push(Output::PlaceRecipe { book_id, all });
        Ok(())
    }
    fn give(&mut self, stack: RecipeStack) -> Result<(), String> {
        self.recipe_action()?;
        if !self.snapshot.creative {
            return Err("only in creative mode".into());
        }
        let item = self
            .snapshot
            .recipes
            .as_ref()
            .and_then(|r| r.items.get(stack.item as usize))
            .ok_or_else(|| format!("no item {}", stack.item))?
            .id
            .clone();
        self.out.push(Output::Give {
            item,
            count: stack.count.clamp(1, 99),
            components: clip_to(stack.components, 4096),
        });
        Ok(())
    }
}
impl State {
    /// `recipes.place` and `give` take at most [`RECIPE_ACTIONS`] a call.
    fn recipe_action(&self) -> Result<(), String> {
        let done = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::PlaceRecipe { .. } | Output::Give { .. }))
            .count();
        if done < RECIPE_ACTIONS {
            Ok(())
        } else {
            Err(format!("at most {RECIPE_ACTIONS} places and gives a call"))
        }
    }
}
impl State {
    /// Queues a chat line unless the call already sent [`CHAT_PER_CALL`].
    fn say(&mut self, line: impl FnOnce(String) -> Output) -> bool {
        let shown = self
            .out
            .iter()
            .filter(|o| {
                matches!(
                    o,
                    Output::Chat { .. } | Output::Tell { .. } | Output::Broadcast { .. }
                )
            })
            .count();
        if shown < CHAT_PER_CALL {
            let name = self.name.clone();
            self.out.push(line(name));
        }
        shown < CHAT_PER_CALL
    }
}
impl chat::Host for State {
    fn show(&mut self, text: String) {
        self.say(|name| Output::Chat {
            name,
            text: clip(text),
        });
    }
}
impl mod_world::Host for State {
    fn get_block(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<String> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.block(&dimension, x, y, z)
    }
    fn get_biome(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<String> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.biome(&dimension, x, y, z)
    }
    fn scan(
        &mut self,
        dimension: String,
        min_x: i32,
        min_y: i32,
        min_z: i32,
        max_x: i32,
        max_y: i32,
        max_z: i32,
    ) -> Result<Region, String> {
        let side = |lo: i32, hi: i32| (i64::from(hi) - i64::from(lo) + 1).max(0);
        let volume = side(min_x, max_x) * side(min_y, max_y) * side(min_z, max_z);
        if volume == 0 {
            return Err("the box is empty: a maximum below a minimum".into());
        }
        if volume > SCAN_POSITIONS {
            return Err(format!("{volume} positions: at most {SCAN_POSITIONS}"));
        }
        let mut palette: Vec<String> = Vec::new();
        let mut blocks = Vec::new();
        let guard = self.world.lock().map_err(|_| "no world".to_string())?;
        let Some(view) = guard.as_ref() else {
            return Ok(Region { palette, blocks });
        };
        if !view.has_dimension(&dimension) {
            return Err(format!("no dimension {dimension}"));
        }
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    let Some(state) = view.block(&dimension, x, y, z) else {
                        continue;
                    };
                    let id = state.split('[').next().unwrap_or(&state);
                    if matches!(
                        id,
                        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
                    ) {
                        continue;
                    }
                    let state = match palette.iter().position(|p| *p == state) {
                        Some(i) => i,
                        None => {
                            palette.push(state);
                            palette.len() - 1
                        }
                    };
                    blocks.push(ScannedBlock {
                        x,
                        y,
                        z,
                        state: state as u32,
                    });
                }
            }
        }
        Ok(Region { palette, blocks })
    }
    fn use_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        face: u8,
        item: String,
    ) -> Result<(), String> {
        if face > 5 {
            return Err(format!("face {face}: 0 to 5"));
        }
        if item.is_empty() || item.len() > 128 {
            return Err("an item id of 1 to 128 characters".into());
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::UseItem { .. }))
            .count();
        if queued >= 256 {
            return Err("at most 256 uses a call".into());
        }
        self.out.push(Output::UseItem {
            mod_id: self.id.clone(),
            dimension,
            pos: [x, y, z],
            face,
            item,
        });
        Ok(())
    }
    fn break_block(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        tool: Option<String>,
        enchantments: Vec<(String, u32)>,
        into: Option<(i32, i32, i32)>,
    ) -> Result<(), String> {
        if tool.as_ref().is_some_and(|t| t.is_empty() || t.len() > 128) {
            return Err("a tool id of at most 128 characters".into());
        }
        if enchantments.len() > 32 || enchantments.iter().any(|(e, l)| e.len() > 128 || *l > 255) {
            return Err("at most 32 enchantments, levels up to 255".into());
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Break { .. }))
            .count();
        if queued >= 256 {
            return Err("at most 256 blocks broken a call".into());
        }
        if let Some(view) = self.world.lock().ok().as_ref().and_then(|v| v.as_ref())
            && !view.has_dimension(&dimension)
        {
            return Err(format!("no dimension {dimension}"));
        }
        self.out.push(Output::Break {
            mod_id: self.id.clone(),
            dimension,
            pos: [x, y, z],
            tool,
            enchantments,
            into: into.map(|(a, b, c)| [a, b, c]),
        });
        Ok(())
    }
    fn set_blocks(&mut self, dimension: String, blocks: Vec<BlockChange>) -> Result<(), String> {
        if blocks.len() > BLOCKS_PER_CALL {
            return Err(format!(
                "{} blocks: at most {BLOCKS_PER_CALL} per call",
                blocks.len()
            ));
        }
        if let Some(view) = self.world.lock().ok().as_ref().and_then(|v| v.as_ref()) {
            if !view.has_dimension(&dimension) {
                return Err(format!("no dimension {dimension}"));
            }
            for b in &blocks {
                view.check_state(&b.state)?;
            }
        }
        if !blocks.is_empty() {
            self.out.push(Output::SetBlocks {
                name: self.name.clone(),
                dimension,
                blocks,
            });
        }
        Ok(())
    }
}
impl block_entities::Host for State {
    fn get(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<Vec<u8>> {
        self.block_data.get(&(dimension, [x, y, z])).cloned()
    }
    fn set(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        data: Vec<u8>,
    ) -> Result<(), String> {
        if data.len() > BLOCK_DATA_BYTES {
            return Err(format!(
                "{} bytes: at most {BLOCK_DATA_BYTES} per position",
                data.len()
            ));
        }
        let key = (dimension.clone(), [x, y, z]);
        if self.block_data.len() >= BLOCK_DATA_POSITIONS && !self.block_data.contains_key(&key) {
            return Err(format!("at most {BLOCK_DATA_POSITIONS} positions"));
        }
        if let Some(view) = self.world.lock().ok().as_ref().and_then(|v| v.as_ref()) {
            if !view.has_dimension(&dimension) {
                return Err(format!("no dimension {dimension}"));
            }
            // Only where one of the mod's own blocks stands.
            let owned = view.block(&dimension, x, y, z).is_some_and(|s| {
                let id = s.split('[').next().unwrap_or(&s);
                id.split_once(':')
                    .is_some_and(|(ns, _)| self.block_namespaces.iter().any(|n| n == ns))
            });
            if !owned {
                return Err(format!("({x}, {y}, {z}) is not a block of this mod"));
            }
        }
        self.block_data.insert(key, data.clone());
        self.out.push(Output::BlockData {
            mod_id: self.id.clone(),
            dimension,
            pos: [x, y, z],
            data: Some(data),
        });
        Ok(())
    }
    fn remove(&mut self, dimension: String, x: i32, y: i32, z: i32) {
        if self
            .block_data
            .remove(&(dimension.clone(), [x, y, z]))
            .is_some()
        {
            self.out.push(Output::BlockData {
                mod_id: self.id.clone(),
                dimension,
                pos: [x, y, z],
                data: None,
            });
        }
    }
}
impl State {
    /// Queues an entity change, within the per-call cap.
    fn entity_op(&mut self, op: EntityOp) -> bool {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Entity { .. }))
            .count();
        if queued >= ENTITY_OPS_PER_CALL {
            return false;
        }
        self.out.push(Output::Entity {
            mod_id: self.id.clone(),
            op,
        });
        true
    }
}
impl State {
    /// Queues a container change, within the per-call cap.
    fn container_op(&mut self, op: ContainerOp) -> bool {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Container { .. }))
            .count();
        if queued >= CONTAINER_OPS_PER_CALL {
            return false;
        }
        self.out.push(Output::Container {
            mod_id: self.id.clone(),
            op,
        });
        true
    }
    fn owns_container_block(&self, id: &str) -> bool {
        id.split_once(':')
            .is_some_and(|(ns, _)| self.block_namespaces.iter().any(|n| n == ns))
    }
}
impl State {
    /// Queues a sound or level event, within the per-call cap.
    fn effect(&mut self, dimension: String, pos: [i32; 3], effect: EffectOp) -> Result<(), String> {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Effect { .. }))
            .count();
        if queued >= EFFECTS_PER_CALL {
            return Err(format!("at most {EFFECTS_PER_CALL} effects a call"));
        }
        self.out.push(Output::Effect {
            mod_id: self.id.clone(),
            dimension,
            pos,
            effect,
        });
        Ok(())
    }
}
impl effects::Host for State {
    fn play_sound(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        event: String,
        volume: f32,
        pitch: f32,
    ) -> Result<(), String> {
        if event.is_empty() || event.len() > 128 || event.contains(char::is_whitespace) {
            return Err(format!("{event:?} is not a sound event"));
        }
        let clamp = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        let effect = EffectOp::Sound {
            event,
            volume: clamp(volume, 0.0, 4.0),
            pitch: clamp(pitch, 0.5, 2.0),
        };
        self.effect(dimension, [x, y, z], effect)
    }
    fn level_event(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        id: i32,
        data: i32,
    ) -> Result<(), String> {
        if !EFFECT_EVENTS.contains(&id) {
            return Err(format!("level event {id} is not offered"));
        }
        let effect = EffectOp::LevelEvent {
            id,
            data: data.clamp(0, 15),
        };
        self.effect(dimension, [x, y, z], effect)
    }
}
/// The package path `files` may read: under `data/` or `assets/`, no way out.
fn package_file_ok(path: &str) -> bool {
    (path.starts_with("data/") || path.starts_with("assets/"))
        && !path.contains("..")
        && !path.contains('\\')
        && !path.starts_with('/')
}
/// Largest file `files.read` gives, and most paths `files.paths` gives.
const PACKAGE_FILE_BYTES: u64 = 1 << 20;
const PACKAGE_LIST: usize = 1024;
/// An attachment's target as the key it is kept under: kind, name and key
/// apart by NULs, a chunk's coordinates in the position.
fn attachment_key(target: &attachments::Target, key: &str) -> (String, [i32; 3]) {
    use attachments::Target;
    match target {
        Target::Player(id) => (format!("player\0{id}\0{key}"), [0; 3]),
        Target::Dimension(d) => (format!("dimension\0{d}\0{key}"), [0; 3]),
        Target::Chunk(c) => (format!("chunk\0{}\0{key}", c.dimension), [c.x, 0, c.z]),
    }
}
fn face_index(face: Option<transfer::Face>) -> Option<u8> {
    use transfer::Face;
    face.map(|f| match f {
        Face::Down => 0,
        Face::Up => 1,
        Face::North => 2,
        Face::South => 3,
        Face::West => 4,
        Face::East => 5,
    })
}
impl State {
    /// The storage at a position, while the block there is still its
    /// owner's: one whose block went is forgotten (and its save with it).
    fn storage_at(&mut self, dimension: &str, pos: [i32; 3]) -> Option<Storage> {
        let key = (dimension.to_owned(), pos);
        let found = self.storage.lock().ok()?.get(&key).cloned()?;
        let standing = self
            .world
            .lock()
            .ok()
            .and_then(|v| {
                v.as_ref()
                    .map(|v| v.block(dimension, pos[0], pos[1], pos[2]))
            })
            .flatten();
        if let Some(state) = standing {
            let id = state.split('[').next().unwrap_or(&state);
            let ns = id.split_once(':').map_or("", |(ns, _)| ns);
            if ns != found.owner && !self.block_namespaces.iter().any(|n| n == ns) {
                // Not this mod's view of its own: keep it only while some
                // block of its owner's namespace stands there.
                if !id.starts_with(&format!("{}:", found.owner)) {
                    if let Ok(mut map) = self.storage.lock() {
                        map.remove(&key);
                    }
                    self.save_storage(dimension, pos, None);
                    return None;
                }
            }
        }
        Some(found)
    }
    fn put_storage(&mut self, dimension: &str, pos: [i32; 3], storage: Storage) {
        self.save_storage(dimension, pos, Some(&storage));
        if let Ok(mut map) = self.storage.lock() {
            map.insert((dimension.to_owned(), pos), storage);
        }
    }
    fn save_storage(&mut self, dimension: &str, pos: [i32; 3], storage: Option<&Storage>) {
        self.out.push(Output::BlockData {
            mod_id: TRANSFER_ID.to_owned(),
            dimension: dimension.to_owned(),
            pos,
            data: storage.map(Storage::encode),
        });
    }
    /// Whether one of this mod's blocks stands at a position (true with no
    /// world to ask).
    fn owns_block_at(&self, dimension: &str, pos: [i32; 3]) -> bool {
        let Some(state) = self.world.lock().ok().and_then(|v| {
            v.as_ref()
                .map(|v| v.block(dimension, pos[0], pos[1], pos[2]))
        }) else {
            return true;
        };
        state.is_some_and(|s| {
            let id = s.split('[').next().unwrap_or(&s);
            id.split_once(':')
                .is_some_and(|(ns, _)| self.block_namespaces.iter().any(|n| n == ns))
        })
    }
    fn new_storage(&mut self, dimension: &str, pos: [i32; 3]) -> Result<Storage, String> {
        if !self.owns_block_at(dimension, pos) {
            return Err(format!(
                "({}, {}, {}) is not a block of this mod",
                pos[0], pos[1], pos[2]
            ));
        }
        let existing = self
            .storage
            .lock()
            .ok()
            .and_then(|m| m.get(&(dimension.to_owned(), pos)).cloned());
        if existing.is_none() && self.storage.lock().map_or(0, |m| m.len()) >= STORAGES {
            return Err(format!("at most {STORAGES} tanks and batteries"));
        }
        Ok(existing.unwrap_or_else(|| Storage {
            owner: self.id.clone(),
            ..Storage::default()
        }))
    }
}
impl State {
    /// A stack as this mod sees it: its own bytes only.
    fn stack_view(&self, s: &StackInfo) -> stacks::Stack {
        stacks::Stack {
            item: s.item.clone(),
            count: s.count,
            damage: s.damage,
            name: s.name.clone(),
            enchantments: s.enchantments.clone(),
            data: s
                .data
                .iter()
                .find(|(m, _)| *m == self.id)
                .map(|(_, d)| d.clone()),
        }
    }
    /// A stack this mod wrote, its bytes under its own id; `None` when it
    /// is out of bounds.
    fn stack_from(&self, s: stacks::Stack) -> Option<StackInfo> {
        let ok = !s.item.is_empty()
            && s.item.len() <= 128
            && (1..=99).contains(&s.count)
            && s.name.as_ref().is_none_or(|n| n.len() <= 256)
            && s.enchantments.len() <= 32
            && s.data.as_ref().is_none_or(|d| d.len() <= STACK_DATA_BYTES);
        ok.then(|| StackInfo {
            item: s.item,
            count: s.count,
            damage: s.damage,
            name: s.name,
            enchantments: s.enchantments,
            data: s
                .data
                .map(|d| vec![(self.id.clone(), d)])
                .unwrap_or_default(),
        })
    }
}
impl stacks::Host for State {
    fn player_stack(&mut self, player: String, slot: u32) -> Option<stacks::Stack> {
        let (slots, _) = self.snapshot.inventories.get(&player)?;
        let s = slots.get(slot as usize)?.as_ref()?;
        Some(self.stack_view(s))
    }
    fn set_player_stack(
        &mut self,
        player: String,
        slot: u32,
        stack: Option<stacks::Stack>,
    ) -> bool {
        if slot > 40 || !self.snapshot.players.iter().any(|p| p.id == player) {
            return false;
        }
        let stack = match stack {
            Some(s) => match self.stack_from(s) {
                Some(s) => Some(s),
                None => return false,
            },
            None => None,
        };
        self.out.push(Output::SetPlayerStack {
            mod_id: self.id.clone(),
            player,
            slot,
            stack,
        });
        true
    }
    fn container_stack(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        slot: u32,
    ) -> Option<stacks::Stack> {
        let s = self
            .world
            .lock()
            .ok()?
            .as_ref()?
            .stack_at(&dimension, [x, y, z], slot)?;
        Some(self.stack_view(&s))
    }
    fn set_container_stack(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        slot: u32,
        stack: Option<stacks::Stack>,
    ) -> bool {
        let stack = match stack {
            Some(s) => match self.stack_from(s) {
                Some(s) => Some(s),
                None => return false,
            },
            None => None,
        };
        let id = self.id.clone();
        let Ok(mut view) = self.world.lock() else {
            return false;
        };
        view.as_mut()
            .is_some_and(|v| v.set_stack_at(&dimension, [x, y, z], slot, stack, &id))
    }
}
impl recipe_manager::Host for State {
    fn craft(
        &mut self,
        grid: Vec<Option<recipe_manager::ItemCount>>,
        width: u32,
    ) -> Option<recipe_manager::Crafted> {
        if !(1..=3).contains(&width) || grid.len() != (width * width) as usize {
            return None;
        }
        let grid: Vec<Option<(String, u32)>> = grid
            .into_iter()
            .map(|c| c.filter(|c| c.count > 0).map(|c| (c.item, c.count)))
            .collect();
        let c = self.world.lock().ok()?.as_ref()?.craft(&grid, width)?;
        let item = |(item, count): (String, u32)| recipe_manager::ItemCount { item, count };
        Some(recipe_manager::Crafted {
            recipe: c.recipe,
            output: item(c.result),
            remainders: c.remainders.into_iter().map(|r| r.map(item)).collect(),
        })
    }
    fn cook(&mut self, kind: String, item: String) -> Option<recipe_manager::Cooked> {
        let c = self.world.lock().ok()?.as_ref()?.cook(&kind, &item)?;
        Some(recipe_manager::Cooked {
            recipe: c.recipe,
            output: recipe_manager::ItemCount {
                item: c.result.0,
                count: c.result.1,
            },
            experience: c.experience,
            time: c.time,
        })
    }
    fn stonecut(&mut self, item: String) -> Vec<(String, recipe_manager::ItemCount)> {
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        view.as_ref()
            .map(|v| v.stonecut(&item))
            .unwrap_or_default()
            .into_iter()
            .map(|(id, (item, count))| (id, recipe_manager::ItemCount { item, count }))
            .collect()
    }
    fn of_type(&mut self, kind: String) -> Vec<recipe_manager::RecipeFile> {
        let Ok(view) = self.world.lock() else {
            return Vec::new();
        };
        let mut budget = RECIPE_FILE_BYTES;
        view.as_ref()
            .map(|v| v.recipes_of_type(&kind))
            .unwrap_or_default()
            .into_iter()
            .take_while(|(_, json)| {
                let fits = json.len() <= budget;
                budget = budget.saturating_sub(json.len());
                fits
            })
            .map(|(id, json)| recipe_manager::RecipeFile { id, json })
            .collect()
    }
    fn test_ingredient(&mut self, ingredient: String, item: String) -> bool {
        if ingredient.len() > 64 << 10 {
            return false;
        }
        let Ok(view) = self.world.lock() else {
            return false;
        };
        view.as_ref()
            .is_some_and(|v| v.test_ingredient(&ingredient, &item))
    }
}
impl console::Host for State {
    fn run(&mut self, line: String) -> Result<(), String> {
        let line = line.trim().trim_start_matches('/').to_owned();
        if line.is_empty() || line.len() > 32_000 || line.contains(['\n', '\r']) {
            return Err("one command line of at most 32000 characters".into());
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Console { .. }))
            .count();
        if queued >= 256 {
            return Err("at most 256 commands a call".into());
        }
        self.out.push(Output::Console {
            mod_id: self.id.clone(),
            line,
        });
        Ok(())
    }
}
impl menus::Host for State {
    fn open(
        &mut self,
        player: String,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        layout: menus::Layout,
    ) -> bool {
        let Some(layout) = MenuLayout::from_wit(layout) else {
            return false;
        };
        if !self.snapshot.players.iter().any(|p| p.id == player) {
            return false;
        }
        self.out.push(Output::Menu {
            mod_id: self.id.clone(),
            op: MenuOp::Open {
                player,
                dimension,
                pos: [x, y, z],
                layout,
            },
        });
        true
    }
    fn open_panel(&mut self, player: String, layout: menus::Layout) -> bool {
        let Some(layout) = MenuLayout::from_wit(layout) else {
            return false;
        };
        if !self.snapshot.players.iter().any(|p| p.id == player) {
            return false;
        }
        self.out.push(Output::Menu {
            mod_id: self.id.clone(),
            op: MenuOp::Panel { player, layout },
        });
        true
    }
    fn close(&mut self, player: String) {
        self.out.push(Output::Menu {
            mod_id: self.id.clone(),
            op: MenuOp::Close { player },
        });
    }
    fn set_data(&mut self, player: String, index: u32, value: i32) -> bool {
        if index > 31 {
            return false;
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Menu { .. }))
            .count();
        if queued >= 4096 {
            return false;
        }
        self.out.push(Output::Menu {
            mod_id: self.id.clone(),
            op: MenuOp::Data {
                player,
                index,
                value,
            },
        });
        true
    }
}
impl peers::Host for State {
    fn send(&mut self, to: String, data: Vec<u8>) -> Result<(), String> {
        if data.len() > PEER_BYTES {
            return Err(format!("{} bytes: at most {PEER_BYTES}", data.len()));
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Peer { .. }))
            .count();
        if queued >= PEER_PER_CALL {
            return Err(format!("at most {PEER_PER_CALL} messages a call"));
        }
        if to.is_empty() || to.len() > 64 {
            return Err("a mod id of 1 to 64 characters".into());
        }
        self.out.push(Output::Peer {
            from: self.id.clone(),
            to,
            data,
        });
        Ok(())
    }
    fn running(&mut self, id: String) -> bool {
        self.peers.lock().is_ok_and(|p| p.contains(&id))
    }
}
impl transfer::Host for State {
    fn item_slots(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        face: Option<transfer::Face>,
    ) -> Option<Vec<Option<transfer::ItemStack>>> {
        let view = self.world.lock().ok()?;
        let slots = view
            .as_ref()?
            .item_slots(&dimension, [x, y, z], face_index(face))?;
        Some(
            slots
                .into_iter()
                .map(|s| s.map(|(item, count)| transfer::ItemStack { item, count }))
                .collect(),
        )
    }
    fn insert_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        face: Option<transfer::Face>,
        stack: transfer::ItemStack,
        simulate: bool,
    ) -> u32 {
        if stack.count == 0 || stack.item.is_empty() || stack.item.len() > 128 {
            return stack.count;
        }
        let Ok(mut view) = self.world.lock() else {
            return stack.count;
        };
        let Some(view) = view.as_mut() else {
            return stack.count;
        };
        view.insert_item(
            &dimension,
            [x, y, z],
            face_index(face),
            &stack.item,
            stack.count.min(99 * 64),
            simulate,
        ) + stack.count.saturating_sub(99 * 64)
    }
    fn extract_item(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        face: Option<transfer::Face>,
        count: u32,
        simulate: bool,
    ) -> Option<transfer::ItemStack> {
        if count == 0 {
            return None;
        }
        let mut view = self.world.lock().ok()?;
        let (item, count) = view.as_mut()?.extract_item(
            &dimension,
            [x, y, z],
            face_index(face),
            count,
            simulate,
        )?;
        Some(transfer::ItemStack { item, count })
    }
    fn add_tank(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        capacity: u32,
    ) -> Result<(), String> {
        let mut s = self.new_storage(&dimension, [x, y, z])?;
        s.fluid_capacity = capacity;
        if let Some((_, amount)) = &mut s.fluid {
            *amount = (*amount).min(capacity);
        }
        if s.fluid.as_ref().is_some_and(|(_, a)| *a == 0) {
            s.fluid = None;
        }
        self.put_storage(&dimension, [x, y, z], s);
        Ok(())
    }
    fn get_tank(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<transfer::Tank> {
        let s = self.storage_at(&dimension, [x, y, z])?;
        (s.fluid_capacity > 0).then(|| transfer::Tank {
            content: s
                .fluid
                .map(|(fluid, amount)| transfer::FluidStack { fluid, amount }),
            capacity: s.fluid_capacity,
        })
    }
    fn fill(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        stack: transfer::FluidStack,
        simulate: bool,
    ) -> u32 {
        let Some(mut s) = self.storage_at(&dimension, [x, y, z]) else {
            return 0;
        };
        if s.fluid_capacity == 0
            || stack.amount == 0
            || stack.fluid.is_empty()
            || stack.fluid.len() > 128
        {
            return 0;
        }
        let held = match &s.fluid {
            Some((f, a)) if *f == stack.fluid => *a,
            Some(_) => return 0,
            None => 0,
        };
        let took = stack.amount.min(s.fluid_capacity.saturating_sub(held));
        if took > 0 && !simulate {
            s.fluid = Some((stack.fluid, held + took));
            self.put_storage(&dimension, [x, y, z], s);
        }
        took
    }
    fn drain(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        amount: u32,
        simulate: bool,
    ) -> Option<transfer::FluidStack> {
        let mut s = self.storage_at(&dimension, [x, y, z])?;
        let (fluid, held) = s.fluid.clone()?;
        let out = amount.min(held);
        if out == 0 {
            return None;
        }
        if !simulate {
            s.fluid = (held > out).then(|| (fluid.clone(), held - out));
            self.put_storage(&dimension, [x, y, z], s);
        }
        Some(transfer::FluidStack { fluid, amount: out })
    }
    fn add_battery(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        capacity: u64,
    ) -> Result<(), String> {
        let mut s = self.new_storage(&dimension, [x, y, z])?;
        s.energy_capacity = capacity;
        s.energy = s.energy.min(capacity);
        self.put_storage(&dimension, [x, y, z], s);
        Ok(())
    }
    fn get_battery(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<transfer::Battery> {
        let s = self.storage_at(&dimension, [x, y, z])?;
        (s.energy_capacity > 0).then_some(transfer::Battery {
            stored: s.energy,
            capacity: s.energy_capacity,
        })
    }
    fn receive_energy(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        amount: u64,
        simulate: bool,
    ) -> u64 {
        let Some(mut s) = self.storage_at(&dimension, [x, y, z]) else {
            return 0;
        };
        let took = amount.min(s.energy_capacity.saturating_sub(s.energy));
        if took > 0 && !simulate {
            s.energy += took;
            self.put_storage(&dimension, [x, y, z], s);
        }
        took
    }
    fn extract_energy(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        amount: u64,
        simulate: bool,
    ) -> u64 {
        let Some(mut s) = self.storage_at(&dimension, [x, y, z]) else {
            return 0;
        };
        let out = amount.min(s.energy);
        if out > 0 && !simulate {
            s.energy -= out;
            self.put_storage(&dimension, [x, y, z], s);
        }
        out
    }
    fn remove_storage(&mut self, dimension: String, x: i32, y: i32, z: i32) {
        let key = (dimension.clone(), [x, y, z]);
        let owned = self
            .storage
            .lock()
            .ok()
            .and_then(|m| m.get(&key).map(|s| s.owner == self.id))
            .unwrap_or(false);
        if owned {
            if let Ok(mut map) = self.storage.lock() {
                map.remove(&key);
            }
            self.save_storage(&dimension, [x, y, z], None);
        }
    }
}
impl attachments::Host for State {
    fn get(&mut self, target: attachments::Target, key: String) -> Option<Vec<u8>> {
        self.attachments
            .get(&attachment_key(&target, &key))
            .cloned()
    }
    fn set(
        &mut self,
        target: attachments::Target,
        key: String,
        data: Vec<u8>,
    ) -> Result<(), String> {
        if key.is_empty() || key.len() > 64 || key.contains('\0') {
            return Err("a key of 1 to 64 characters".into());
        }
        if data.len() > ATTACHMENT_BYTES {
            return Err(format!("{} bytes: at most {ATTACHMENT_BYTES}", data.len()));
        }
        let k = attachment_key(&target, &key);
        if self.attachments.len() >= ATTACHMENTS && !self.attachments.contains_key(&k) {
            return Err(format!("at most {ATTACHMENTS} values"));
        }
        if self.attachments.get(&k) == Some(&data) {
            return Ok(());
        }
        self.attachments.insert(k.clone(), data.clone());
        self.out.push(Output::BlockData {
            mod_id: self.id.clone(),
            dimension: format!("{ATTACHMENT_MARK}{}", k.0),
            pos: k.1,
            data: Some(data),
        });
        Ok(())
    }
    fn remove(&mut self, target: attachments::Target, key: String) {
        let k = attachment_key(&target, &key);
        if self.attachments.remove(&k).is_some() {
            self.out.push(Output::BlockData {
                mod_id: self.id.clone(),
                dimension: format!("{ATTACHMENT_MARK}{}", k.0),
                pos: k.1,
                data: None,
            });
        }
    }
    fn keys(&mut self, target: attachments::Target) -> Vec<String> {
        let (prefix, pos) = attachment_key(&target, "");
        let mut keys: Vec<String> = self
            .attachments
            .keys()
            .filter(|(t, p)| *p == pos && t.starts_with(&prefix))
            .map(|(t, _)| t[prefix.len()..].to_owned())
            .collect();
        keys.sort();
        keys
    }
}
impl files::Host for State {
    fn read(&mut self, path: String) -> Option<Vec<u8>> {
        if !package_file_ok(&path) {
            return None;
        }
        if self.package_path.is_dir() {
            let file = self.package_path.join(&path);
            let meta = std::fs::metadata(&file).ok()?;
            return (meta.is_file() && meta.len() <= PACKAGE_FILE_BYTES)
                .then(|| std::fs::read(file).ok())
                .flatten();
        }
        let file = std::fs::File::open(&self.package_path).ok()?;
        let mut zip = zip::ZipArchive::new(file).ok()?;
        let entry = zip.by_name(&path).ok()?;
        if entry.size() > PACKAGE_FILE_BYTES {
            return None;
        }
        let mut out = Vec::with_capacity(entry.size() as usize);
        std::io::Read::read_to_end(
            &mut std::io::Read::take(entry, PACKAGE_FILE_BYTES),
            &mut out,
        )
        .ok()?;
        Some(out)
    }
    fn paths(&mut self, prefix: String) -> Vec<String> {
        if !package_file_ok(&prefix) {
            return Vec::new();
        }
        let mut found = Vec::new();
        if self.package_path.is_dir() {
            fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
                let Ok(read) = std::fs::read_dir(dir) else {
                    return;
                };
                for e in read.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, root, out);
                    } else if let Ok(rel) = p.strip_prefix(root) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
            walk(&self.package_path, &self.package_path, &mut found);
        } else if let Some(zip) = std::fs::File::open(&self.package_path)
            .ok()
            .and_then(|f| zip::ZipArchive::new(f).ok())
        {
            found = zip.file_names().map(str::to_owned).collect();
        }
        found.retain(|p| p.starts_with(&prefix) && !p.ends_with('/'));
        found.sort();
        found.truncate(PACKAGE_LIST);
        found
    }
}
impl items::Host for State {
    fn nearby(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        radius: f32,
    ) -> Vec<items::DroppedItem> {
        let Ok(guard) = self.world.lock() else {
            return Vec::new();
        };
        let Some(view) = guard.as_ref() else {
            return Vec::new();
        };
        let r = f64::from(if radius.is_finite() {
            radius.clamp(0.0, 16.0)
        } else {
            0.0
        });
        let dist = |s: &DroppedStack| {
            (s.pos[0] - x).powi(2) + (s.pos[1] - y).powi(2) + (s.pos[2] - z).powi(2)
        };
        let mut found: Vec<_> = view
            .dropped(&dimension)
            .into_iter()
            .filter(|s| dist(s) <= r * r)
            .collect();
        found.sort_by(|a, b| dist(a).total_cmp(&dist(b)));
        found.truncate(NEARBY_ITEMS);
        found
            .into_iter()
            .map(|s| items::DroppedItem {
                id: s.id,
                item: s.item,
                count: s.count,
                x: s.pos[0],
                y: s.pos[1],
                z: s.pos[2],
            })
            .collect()
    }
    fn push(&mut self, id: u32, x: f64, y: f64, z: f64) -> bool {
        State::push(self, PushTarget::Item(id), [x, y, z])
    }
    fn take(&mut self, id: u32) -> bool {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Take { .. }))
            .count();
        if queued >= NEARBY_ITEMS {
            return false;
        }
        self.out.push(Output::Take {
            mod_id: self.id.clone(),
            id,
        });
        true
    }
    fn drop(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        item: String,
        count: u32,
    ) -> Result<(), String> {
        if !(1..=99).contains(&count) || item.is_empty() || item.len() > 128 {
            return Err("a stack of 1 to 99".into());
        }
        if ![x, y, z].iter().all(|v| v.is_finite() && v.abs() < 3.0e7) {
            return Err("position out of range".into());
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Drop { .. }))
            .count();
        if queued >= DROPS_PER_CALL {
            return Err(format!("at most {DROPS_PER_CALL} drops a call"));
        }
        self.out.push(Output::Drop {
            mod_id: self.id.clone(),
            dimension,
            pos: [x, y, z],
            item,
            count,
        });
        Ok(())
    }
}
impl redstone::Host for State {
    fn set_signal(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        level: u8,
    ) -> Result<(), String> {
        if level > 15 {
            return Err(format!("signal {level}: 0 to 15"));
        }
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Signal { .. }))
            .count();
        if queued >= 1024 {
            return Err("at most 1024 signals a call".into());
        }
        self.out.push(Output::Signal {
            mod_id: self.id.clone(),
            dimension,
            pos: [x, y, z],
            level,
        });
        Ok(())
    }
    fn power(&mut self, dimension: String, x: i32, y: i32, z: i32) -> Option<u8> {
        let view = self.world.lock().ok()?;
        view.as_ref()?.power(&dimension, x, y, z)
    }
}
impl containers::Host for State {
    fn register(&mut self, block: String) -> Result<(), String> {
        self.register_sized(block, CONTAINER_SLOTS)
    }
    fn register_sized(&mut self, block: String, slots: u32) -> Result<(), String> {
        if !matches!(slots, 5 | 9 | 27) {
            return Err(format!("{slots} slots: 5, 9 or 27"));
        }
        if block.contains('[') || !self.owns_container_block(&block) {
            return Err(format!("{block} is not a block of this mod"));
        }
        if self.container_blocks.iter().any(|(b, _)| *b == block) {
            return Ok(());
        }
        if self.container_blocks.len() >= CONTAINER_BLOCKS {
            return Err(format!("at most {CONTAINER_BLOCKS} container blocks"));
        }
        if !self.container_op(ContainerOp::Register {
            block: block.clone(),
            slots,
        }) {
            return Err(format!("at most {CONTAINER_OPS_PER_CALL} changes a call"));
        }
        self.container_blocks.push((block, slots));
        Ok(())
    }
    fn open(&mut self, player: String, dimension: String, x: i32, y: i32, z: i32) -> bool {
        self.snapshot.players.iter().any(|p| p.id == player)
            && self.container_op(ContainerOp::Open {
                player,
                dimension,
                pos: [x, y, z],
            })
    }
    fn slots(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<Vec<Option<containers::ItemStack>>> {
        let view = self.world.lock().ok()?;
        let slots = view.as_ref()?.container(&dimension, x, y, z)?;
        Some(
            slots
                .into_iter()
                .map(|s| s.map(|(item, count)| containers::ItemStack { item, count }))
                .collect(),
        )
    }
    fn set_slot_rule(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        slot: u32,
        allowed: Option<Vec<String>>,
    ) -> Result<(), String> {
        if slot >= CONTAINER_SLOTS {
            return Err(format!("slot {slot}: 0 to {}", CONTAINER_SLOTS - 1));
        }
        if allowed
            .as_ref()
            .is_some_and(|l| l.len() > 64 || l.iter().any(|i| i.is_empty() || i.len() > 128))
        {
            return Err("at most 64 item ids of at most 128 characters".into());
        }
        let op = ContainerOp::SetRule {
            dimension,
            pos: [x, y, z],
            slot,
            allowed,
        };
        if self.container_op(op) {
            Ok(())
        } else {
            Err(format!("at most {CONTAINER_OPS_PER_CALL} changes a call"))
        }
    }
    fn set_name(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        name: Option<String>,
    ) -> Result<(), String> {
        if name.as_ref().is_some_and(|n| n.chars().count() > 64) {
            return Err("a name of at most 64 characters".into());
        }
        let op = ContainerOp::SetName {
            dimension,
            pos: [x, y, z],
            name,
        };
        if self.container_op(op) {
            Ok(())
        } else {
            Err(format!("at most {CONTAINER_OPS_PER_CALL} changes a call"))
        }
    }
    fn set_slot(
        &mut self,
        dimension: String,
        x: i32,
        y: i32,
        z: i32,
        slot: u32,
        stack: Option<containers::ItemStack>,
    ) -> Result<(), String> {
        if slot >= CONTAINER_SLOTS {
            return Err(format!("slot {slot}: 0 to {}", CONTAINER_SLOTS - 1));
        }
        if let Some(s) = &stack
            && (!(1..=99).contains(&s.count) || s.item.len() > 128)
        {
            return Err("a stack of 1 to 99".into());
        }
        let op = ContainerOp::SetSlot {
            dimension,
            pos: [x, y, z],
            slot,
            stack: stack.map(|s| (s.item, s.count)),
        };
        if self.container_op(op) {
            Ok(())
        } else {
            Err(format!("at most {CONTAINER_OPS_PER_CALL} changes a call"))
        }
    }
}
impl entities::Host for State {
    fn spawn(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        item: String,
    ) -> Result<u32, String> {
        if self.entity_handles.len() >= ENTITIES_PER_MOD {
            return Err(format!("at most {ENTITIES_PER_MOD} entities"));
        }
        if ![x, y, z, f64::from(yaw)].iter().all(|v| v.is_finite()) {
            return Err("a position that is not a number".into());
        }
        if item.len() > 128 {
            return Err("an item id too long".into());
        }
        if let Some(view) = self.world.lock().ok().as_ref().and_then(|v| v.as_ref())
            && !view.has_dimension(&dimension)
        {
            return Err(format!("no dimension {dimension}"));
        }
        let handle = self.next_handle;
        let op = EntityOp::Spawn {
            handle,
            dimension,
            pos: [x, y, z],
            yaw,
            item,
        };
        if !self.entity_op(op) {
            return Err(format!("at most {ENTITY_OPS_PER_CALL} changes a call"));
        }
        self.next_handle += 1;
        self.entity_handles.insert(handle);
        Ok(handle)
    }
    fn spawn_block(
        &mut self,
        dimension: String,
        x: f64,
        y: f64,
        z: f64,
        state: String,
    ) -> Result<u32, String> {
        if self.entity_handles.len() >= ENTITIES_PER_MOD {
            return Err(format!("at most {ENTITIES_PER_MOD} entities"));
        }
        if ![x, y, z].iter().all(|v| v.is_finite()) {
            return Err("a position that is not a number".into());
        }
        if let Some(view) = self.world.lock().ok().as_ref().and_then(|v| v.as_ref()) {
            if !view.has_dimension(&dimension) {
                return Err(format!("no dimension {dimension}"));
            }
            view.check_state(&state)?;
        }
        let handle = self.next_handle;
        let op = EntityOp::SpawnBlock {
            handle,
            dimension,
            pos: [x, y, z],
            state,
        };
        if !self.entity_op(op) {
            return Err(format!("at most {ENTITY_OPS_PER_CALL} changes a call"));
        }
        self.next_handle += 1;
        self.entity_handles.insert(handle);
        Ok(handle)
    }
    fn set_solid(&mut self, handle: u32, solid: bool) -> bool {
        self.entity_handles.contains(&handle)
            && self.entity_op(EntityOp::SetSolid { handle, solid })
    }
    fn rotate(&mut self, handle: u32, yaw: f32, pitch: f32) -> bool {
        let wrap = |v: f32| if v.is_finite() { v % 360.0 } else { 0.0 };
        self.entity_handles.contains(&handle)
            && self.entity_op(EntityOp::Rotate {
                handle,
                yaw: wrap(yaw),
                pitch: wrap(pitch),
            })
    }
    fn move_to(&mut self, handle: u32, x: f64, y: f64, z: f64, yaw: f32) -> bool {
        self.entity_handles.contains(&handle)
            && [x, y, z, f64::from(yaw)].iter().all(|v| v.is_finite())
            && self.entity_op(EntityOp::Move {
                handle,
                pos: [x, y, z],
                yaw,
            })
    }
    fn set_item(&mut self, handle: u32, item: String) -> bool {
        self.entity_handles.contains(&handle)
            && item.len() <= 128
            && self.entity_op(EntityOp::SetItem { handle, item })
    }
    fn remove(&mut self, handle: u32) -> bool {
        self.entity_handles.remove(&handle) && self.entity_op(EntityOp::Remove { handle })
    }
    fn set_slot(&mut self, handle: u32, slot: String, item: Option<String>) -> bool {
        const SLOTS: [&str; 6] = ["mainhand", "offhand", "feet", "legs", "chest", "head"];
        let Some(slot) = SLOTS.iter().position(|s| *s == slot) else {
            return false;
        };
        self.entity_handles.contains(&handle)
            && item.as_ref().is_none_or(|i| i.len() <= 128)
            && self.entity_op(EntityOp::SetSlot { handle, slot, item })
    }
    fn set_pose(&mut self, handle: u32, part: String, x: f32, y: f32, z: f32) -> bool {
        const PARTS: [&str; 6] = [
            "head",
            "body",
            "left-arm",
            "right-arm",
            "left-leg",
            "right-leg",
        ];
        let Some(part) = PARTS.iter().position(|p| *p == part) else {
            return false;
        };
        let wrap = |v: f32| if v.is_finite() { v % 360.0 } else { 0.0 };
        self.entity_handles.contains(&handle)
            && self.entity_op(EntityOp::SetPose {
                handle,
                part,
                rotation: [wrap(x), wrap(y), wrap(z)],
            })
    }
}
impl net::Host for State {
    fn send(&mut self, player: Option<String>, data: Vec<u8>) -> Result<(), String> {
        if data.len() > NET_BYTES {
            return Err(format!(
                "{} bytes: at most {NET_BYTES} a message",
                data.len()
            ));
        }
        match &player {
            Some(_) if !self.server => {
                return Err("a client mod sends to its server only".into());
            }
            Some(id) if !self.snapshot.players.iter().any(|p| &p.id == id) => {
                return Err(format!("no player {id}"));
            }
            _ => {}
        }
        let sent = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Net { .. }))
            .count();
        if sent >= NET_PER_CALL {
            return Err(format!("at most {NET_PER_CALL} messages a call"));
        }
        self.out.push(Output::Net {
            mod_id: self.id.clone(),
            server: self.server,
            player,
            data,
        });
        Ok(())
    }
}
impl State {
    /// Queues a push, within the per-call cap and with each component clamped.
    fn push(&mut self, target: PushTarget, v: [f64; 3]) -> bool {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Push { .. } | Output::Take { .. }))
            .count();
        if queued >= NEARBY_ITEMS {
            return false;
        }
        let clamp = |c: f64| {
            if c.is_finite() {
                c.clamp(-4.0, 4.0)
            } else {
                0.0
            }
        };
        self.out.push(Output::Push {
            mod_id: self.id.clone(),
            target,
            velocity: [clamp(v[0]), clamp(v[1]), clamp(v[2])],
        });
        true
    }
}
impl players::Host for State {
    fn inventory(&mut self, player: String) -> Option<Vec<Option<players::SlotStack>>> {
        let (slots, _) = self.snapshot.inventories.get(&player)?;
        Some(
            slots
                .iter()
                .map(|s| {
                    s.as_ref().map(|s| players::SlotStack {
                        item: s.item.clone(),
                        count: s.count,
                    })
                })
                .collect(),
        )
    }
    fn selected_slot(&mut self, player: String) -> Option<u32> {
        self.snapshot.inventories.get(&player).map(|(_, s)| *s)
    }
    fn push(&mut self, player: String, x: f64, y: f64, z: f64) -> bool {
        self.snapshot.players.iter().any(|p| p.id == player)
            && State::push(self, PushTarget::Player(player), [x, y, z])
    }
    fn online(&mut self) -> Vec<ServerPlayer> {
        self.snapshot.players.clone()
    }
    fn tell(&mut self, player: String, text: String) -> bool {
        self.snapshot.players.iter().any(|p| p.id == player)
            && self.say(|name| Output::Tell {
                name,
                player,
                text: clip(text),
            })
    }
    fn broadcast(&mut self, text: String) {
        self.say(|name| Output::Broadcast {
            name,
            text: clip(text),
        });
    }
}
/// A key a mod registered (capability `input`).
#[derive(Clone, Debug, PartialEq)]
pub struct ModKey {
    /// The mod's own name for it, echoed in `key-down`.
    pub name: String,
    /// What it does, for Controls.
    pub label: String,
    /// `key.keyboard.r`.
    pub key: String,
}

/// What capability `input` holds for one mod.
#[derive(Default)]
struct InputState {
    claimed: Vec<String>,
    keys: Vec<ModKey>,
    events: std::collections::VecDeque<InputEvent>,
    /// The mod's keys held now, by name.
    down: Vec<String>,
    /// It holds the mouse wheel.
    wheel: bool,
    /// The keys the player chose, `name → key`: they win over
    /// `register-key`'s.
    bindings: Vec<(String, String)>,
}
/// Events kept for `poll`, items claimed and keys, per mod.
const INPUT_EVENTS: usize = 256;
const CLAIMS: usize = 64;
const KEYS: usize = 16;

/// `key.keyboard.<name>` or `key.mouse.<name>`, as options.txt writes keys;
/// the game knows which names exist and ignores a key it has none for.
fn key_ok(key: &str) -> bool {
    let rest = key
        .strip_prefix("key.keyboard.")
        .or_else(|| key.strip_prefix("key.mouse."));
    rest.is_some_and(|r| {
        (1..=32).contains(&r.len())
            && r.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.')
    })
}

impl InputState {
    /// Key `name` is `key` now; held, it lets go first.
    fn rebind(&mut self, name: &str, key: String) {
        let Some(k) = self.keys.iter_mut().find(|k| k.name == name) else {
            return;
        };
        k.key = key;
        if let Some(i) = self.down.iter().position(|n| n == name) {
            let name = self.down.swap_remove(i);
            self.push(InputEvent::KeyUp(name));
        }
    }
    fn push(&mut self, event: InputEvent) {
        if self.events.len() == INPUT_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
    }
}

impl input::Host for State {
    fn claim_items(&mut self, ids: Vec<String>) {
        self.input.claimed = ids.into_iter().take(CLAIMS).map(clip).collect();
    }
    fn register_key(&mut self, name: String, label: String, key: String) -> Result<(), String> {
        if !key_ok(&key) {
            return Err(format!(
                "{key:?} is not a key as options.txt writes them (key.keyboard.r)"
            ));
        }
        let (name, label) = (clip(name), clip(label));
        let key = match self.input.bindings.iter().find(|(n, _)| *n == name) {
            Some((_, chosen)) => chosen.clone(),
            None => key,
        };
        if let Some(k) = self.input.keys.iter_mut().find(|k| k.name == name) {
            (k.label, k.key) = (label, key);
            return Ok(());
        }
        if self.input.keys.len() == KEYS {
            return Err(format!("a mod may register {KEYS} keys"));
        }
        self.input.keys.push(ModKey { name, label, key });
        Ok(())
    }
    fn held(&mut self) -> Option<HeldItem> {
        self.snapshot.held.clone()
    }
    fn poll(&mut self) -> Vec<InputEvent> {
        self.input.events.drain(..).collect()
    }
    fn mapping(&mut self, name: String) -> Option<input::MappingState> {
        let (_, label, down) = self.snapshot.mappings.iter().find(|m| m.0 == name)?;
        Some(input::MappingState {
            label: label.clone(),
            down: *down,
        })
    }
    fn clicks_per_second(&mut self) -> (u32, u32) {
        self.snapshot.cps
    }
    fn claim_wheel(&mut self, on: bool) {
        self.input.wheel = on;
    }
}

/// HUD shapes one mod may keep (capability `render`).
const SHAPES: usize = 1024;
/// The longest jolt `render.shake` makes, in seconds.
const SHAKE_SECONDS: f32 = 10.0;
/// `render.set-zoom`'s limits: `Camera.tickFov` keeps the modifier at 0.1
/// or more, and turning never stops.
const ZOOM_MAX: f32 = 10.0;
const TURN_MIN: f32 = 0.05;
/// `set-minimap`'s side in GUI pixels and its blocks per map pixel.
const MINIMAP_SIZE: (f32, f32) = (16.0, 256.0);
const MINIMAP_SCALE: (f32, f32) = (0.5, 4.0);
/// `set-far-terrain`'s distance in chunks, as the LOD Distance setting's.
const FAR_TERRAIN_DISTANCE: (u32, u32) = (64, 1024);

/// A mod's corner panel: shapes in a `width` × `height` box the game
/// places in `corner`, stacked with the other overlays.
#[derive(Clone, Debug)]
pub struct CornerPanel {
    pub corner: RenderCorner,
    pub width: f32,
    pub height: f32,
    pub shapes: Vec<Shape>,
}

fn clip_shapes(shapes: Vec<Shape>) -> Vec<Shape> {
    shapes
        .into_iter()
        .take(SHAPES)
        .map(|shape| match shape {
            Shape::Sprite(mut s) => {
                s.sprite = clip(s.sprite);
                Shape::Sprite(s)
            }
            Shape::Item(mut i) => {
                i.item = clip(i.item);
                i.scale = if i.scale.is_finite() {
                    i.scale.clamp(0.0, 16.0)
                } else {
                    1.0
                };
                Shape::Item(i)
            }
            Shape::Text(mut t) => {
                t.text = clip(t.text);
                t.color &= 0xFF_FFFF;
                Shape::Text(t)
            }
            other => other,
        })
        .collect()
}

impl render::Host for State {
    fn set_hud(&mut self, shapes: Vec<Shape>) {
        self.shapes = clip_shapes(shapes);
    }
    fn set_corner(&mut self, corner: RenderCorner, width: f32, height: f32, shapes: Vec<Shape>) {
        let size = |v: f32| {
            if v.is_finite() {
                v.clamp(0.0, 1024.0)
            } else {
                0.0
            }
        };
        self.corner = Some(CornerPanel {
            corner,
            width: size(width),
            height: size(height),
            shapes: clip_shapes(shapes),
        });
    }
    fn clear_corner(&mut self) {
        self.corner = None;
    }
    fn text_width(&mut self, text: String) -> f32 {
        let widths = &self.snapshot.font_widths;
        text.chars()
            .map(|c| widths.get(c as usize).copied().unwrap_or(6.0))
            .sum::<f32>()
            .ceil()
    }
    fn set_zoom(&mut self, zoom: ModZoom) {
        let finite = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        let factor = finite(zoom.factor, 1.0, ZOOM_MAX);
        self.zoom = (factor > 1.0).then(|| ModZoom {
            factor,
            smooth: zoom.smooth,
            turn: if zoom.turn.is_finite() {
                zoom.turn.clamp(TURN_MIN, 1.0)
            } else {
                1.0
            },
        });
    }
    fn set_minimap(&mut self, minimap: Option<ModMinimap>) {
        let finite = |v: f32, lo: f32, hi: f32| if v.is_finite() { v.clamp(lo, hi) } else { lo };
        self.minimap = minimap.map(|m| ModMinimap {
            size: finite(m.size, MINIMAP_SIZE.0, MINIMAP_SIZE.1),
            blocks_per_pixel: finite(m.blocks_per_pixel, MINIMAP_SCALE.0, MINIMAP_SCALE.1),
            ..m
        });
    }
    fn set_far_terrain(&mut self, far_terrain: Option<ModFarTerrain>) {
        self.far_terrain = far_terrain.map(|f| ModFarTerrain {
            distance: f
                .distance
                .clamp(FAR_TERRAIN_DISTANCE.0, FAR_TERRAIN_DISTANCE.1),
            ..f
        });
    }
    fn frame_stats(&mut self) -> FrameRate {
        let (fps, min, low) = self.snapshot.frame;
        FrameRate { fps, min, low }
    }
    fn gui_size(&mut self) -> (f32, f32) {
        self.snapshot.gui_size
    }
    fn shake(&mut self, strength: f32, seconds: f32) {
        let ok = |v: f32, max: f32| {
            if v.is_finite() {
                v.clamp(0.0, max)
            } else {
                0.0
            }
        };
        let (strength, seconds) = (ok(strength, 1.0), ok(seconds, SHAKE_SECONDS));
        if strength > 0.0 && seconds > 0.0 {
            self.out.push(Output::Shake { strength, seconds });
        }
    }
}

impl command::Host for State {
    fn run(&mut self, line: String) {
        let queued = self
            .out
            .iter()
            .filter(|o| matches!(o, Output::Command { .. }))
            .count();
        if queued < CHAT_PER_CALL {
            self.out.push(Output::Command { line: clip(line) });
        }
    }
    fn run_quiet(&mut self, lines: Vec<String>) {
        let queued: usize = self
            .out
            .iter()
            .map(|o| match o {
                Output::QuietCommands { lines, .. } => lines.len(),
                _ => 0,
            })
            .sum();
        let room = QUIET_PER_CALL.saturating_sub(queued);
        if room > 0 && !lines.is_empty() {
            self.out.push(Output::QuietCommands {
                mod_id: self.id.clone(),
                lines: lines.into_iter().take(room).map(clip).collect(),
            });
        }
    }
}
impl hud::Host for State {
    fn set_panel(&mut self, corner: Corner, lines: Vec<Line>) {
        let lines = lines
            .into_iter()
            .take(HUD_LINES)
            .map(|l| Line {
                text: clip(l.text),
                color: l.color & 0xFF_FFFF,
            })
            .collect();
        self.panel = Some(Panel { corner, lines });
    }
    fn clear(&mut self) {
        self.panel = None;
    }
}
impl ui::Host for State {
    fn open_screen(&mut self, mut screen: ui::Screen) {
        screen.title = clip(screen.title);
        screen.widgets = clip_widgets(screen.widgets);
        self.ui.screen = Some(screen);
        self.ui.open = true;
    }
    fn close_screen(&mut self) {
        self.ui.screen = None;
        self.ui.close = true;
    }
    fn set_settings(&mut self, widgets: Vec<ui::Widget>) {
        self.ui.settings = clip_widgets(widgets);
    }
}
impl storage::Host for State {
    fn get(&mut self, key: String) -> Option<Vec<u8>> {
        storage_key_ok(&key)
            .then(|| std::fs::read(self.data_dir.join(&key)).ok())
            .flatten()
    }
    fn set(&mut self, key: String, value: Vec<u8>) -> Result<(), String> {
        if !storage_key_ok(&key) {
            return Err(format!("{key:?} is not [a-z0-9_.-]{{1,64}}"));
        }
        let len = value.len() as u64;
        if len > self.storage_budget {
            return Err("too much storage traffic in one call".into());
        }
        // Everything already on disk but the file this call replaces.
        let existing: Vec<std::fs::DirEntry> = std::fs::read_dir(&self.data_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy() != key)
            .collect();
        if existing.len() >= STORAGE_MAX_KEYS {
            return Err(format!("would hold over {STORAGE_MAX_KEYS} keys"));
        }
        let used: u64 = existing
            .iter()
            .filter_map(|e| e.metadata().ok())
            .map(|m| m.len())
            .sum();
        if used + len > STORAGE_QUOTA {
            return Err(format!(
                "would use over {} MiB of storage",
                STORAGE_QUOTA >> 20
            ));
        }
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        std::fs::write(self.data_dir.join(&key), value).map_err(|e| e.to_string())?;
        self.storage_budget -= len;
        Ok(())
    }
    fn delete(&mut self, key: String) {
        if storage_key_ok(&key) {
            let _ = std::fs::remove_file(self.data_dir.join(&key));
        }
    }
}

/// A storage key: one path segment of `[a-z0-9_.-]{1,64}`, not `.` or `..`.
/// No `/`, so a key can never leave the mod's own directory.
fn storage_key_ok(key: &str) -> bool {
    let chars_ok = (1..=64).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b));
    chars_ok && key != "." && key != ".."
}
