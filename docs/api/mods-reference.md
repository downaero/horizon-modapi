<!-- Generated from crates/horizon-mod-sdk/wit/horizon.wit and the manifest's capabilities by crates/horizon-mods/tests/api_reference.rs; a test fails when this file is out of date. Update it with: HORIZON_WRITE_DOCS=1 cargo test -p horizon-mods --features pack --test api_reference -->
# Mod API reference

The contract between Horizon and its mods: the WIT package `horizon:mods@0.1.0`, every interface, type, function and hook in it, and the capabilities a manifest may list. The guide is [mods.md](../mods.md); stability and versions: [stability.md](stability.md). Manifest `api` lines this game serves: `0.1`, `0.2`, `0.3`.

## Capabilities

| In `capabilities` | Grants | The player reads "May …" |
| --- | --- | --- |
| `game-read` | [`game`](#game) | see your position, health and the time |
| `chat` | [`chat`](#chat) | show you chat messages |
| `hud` | [`hud`](#hud) | draw text on your screen |
| `storage` | [`storage`](#storage) | save its own settings to disk |
| `command` | [`command`](#command) | run commands as you, with your permissions |
| `registries` | data files (`assets/`, `data/`), no interface | add blocks and items to your worlds |
| `ui` | [`ui`](#ui) | show its own screens and a settings page |
| `render` | [`render`](#render) | draw on your screen, shake and zoom the camera |
| `input` | [`input`](#input) | see the keys you hold and your clicks, take over the use button on its own items and the mouse wheel, and add keys |
| `players` | [`players`](#players) | see who is on the server and message them |
| `world` | [`world`](#world) | read and change the world's blocks |
| `recipes` | [`recipes`](#recipes) | read the game's recipes and items and which recipes you know, place recipes in your crafting grid and, in creative mode, give you items |
| `block-entities` | [`block-entities`](#block-entities) | keep data at the positions of its own blocks and tick them |
| `net` | [`net`](#net) | send messages between the game and the server it plays on |
| `entities` | [`entities`](#entities) | place and move small entities in your worlds |
| `containers` | [`containers`](#containers) | keep items in its own blocks and open them for players |
| `redstone` | [`redstone`](#redstone) | see the redstone signal reaching its own blocks |
| `items` | [`items`](#items) | drop items into your worlds |
| `effects` | [`effects`](#effects) | play sounds and particles in your worlds |
| `transfer` | [`transfer`](#transfer) | move items, fluids and energy in and out of blocks |
| `menus` | [`menus`](#menus) | show you screens of its own for its blocks |
| `console` | [`console`](#console) | run any command as the server, with an operator's rights |
| `stacks` | [`stacks`](#stacks) | read and change the names, enchantments and data on items |

## World `horizon-mod`

A client mod: runs in the player's game.

- Imports (what the game offers): `log`, `game`, `chat`, `hud`, `storage`, `command`
- Exports (what the mod provides): `events`

## World `horizon-server-mod`

A server mod (manifest `side = "server"`, `api = "0.3"`, experimental): runs where the world is simulated, on the dedicated server, a LAN host and single player's own server.

- Imports (what the game offers): `log`, `storage`, `players`, `world`
- Exports (what the mod provides): `server-events`

## World `horizon-block-mod`

A server mod with blocks (capability `block-entities`): the SDK builds every server mod against this world. Mods built for `horizon-server-mod` alone (no `block-events` export) still load and hear nothing of blocks.

- Imports (what the game offers): `block-entities`, `visuals`, `structures`, `net`, `entities`, `containers`, `redstone`, `items`, `effects`, `files`, `attachments`, `transfer`, `peers`, `menus`, `console`, `stacks`, `recipe-manager`, `entity-data`, `entity-types`, `log`, `storage`, `players`, `world`
- Exports (what the mod provides): `block-events`, `server-net-events`, `neighbor-events`, `item-events`, `peer-events`, `menu-events`, `entity-events`, `server-events`

## World `entity-types-host`

The game's side of `entity-types` and `entity-events`.

- Imports (what the game offers): `entity-data`, `entity-types`
- Exports (what the mod provides): `entity-events`

## World `recipe-manager-host`

The game's side of `recipe-manager` (imports only).

- Imports (what the game offers): `recipe-manager`
- Exports (what the mod provides): 

## World `stacks-host`

The game's side of capability `stacks` (imports only).

- Imports (what the game offers): `stacks`
- Exports (what the mod provides): 

## World `console-host`

The game's side of capability `console` (imports only).

- Imports (what the game offers): `console`
- Exports (what the mod provides): 

## World `menus-host`

The game's side of capability `menus`.

- Imports (what the game offers): `menus`
- Exports (what the mod provides): `menu-events`

## World `peers-host`

The game's side of `peers` and `peer-events`.

- Imports (what the game offers): `peers`
- Exports (what the mod provides): `peer-events`

## World `item-host`

The game's side of `item-events`.

- Imports (what the game offers): 
- Exports (what the mod provides): `item-events`

## World `transfer-host`

The game's side of capability `transfer` (imports only).

- Imports (what the game offers): `transfer`
- Exports (what the mod provides): 

## World `attachments-host`

The game's side of `attachments` (imports only).

- Imports (what the game offers): `attachments`
- Exports (what the mod provides): 

## World `files-host`

The game's side of `files` (imports only).

- Imports (what the game offers): `files`
- Exports (what the mod provides): 

## World `neighbor-host`

The game's side of `neighbor-events`.

- Imports (what the game offers): 
- Exports (what the mod provides): `neighbor-events`

## World `effects-host`

The game's side of capability `effects` (imports only).

- Imports (what the game offers): `effects`
- Exports (what the mod provides): 

## World `items-host`

The game's side of capability `items` (imports only).

- Imports (what the game offers): `items`
- Exports (what the mod provides): 

## World `redstone-host`

The game's side of capability `redstone` (imports only).

- Imports (what the game offers): `redstone`
- Exports (what the mod provides): 

## World `block-host`

The game's side of `horizon-block-mod` beyond `horizon-server-mod`.

- Imports (what the game offers): `block-entities`
- Exports (what the mod provides): `block-events`

## World `visuals-host`

The game's side of `visuals` (imports only).

- Imports (what the game offers): `visuals`
- Exports (what the mod provides): 

## World `structures-host`

The game's side of `structures` (imports only).

- Imports (what the game offers): `structures`
- Exports (what the mod provides): 

## World `net-host`

The game's side of capability `net` for client mods (it adds the `net` import for server mods too).

- Imports (what the game offers): `net`
- Exports (what the mod provides): `net-events`

## World `containers-host`

The game's side of capability `containers` (imports only).

- Imports (what the game offers): `containers`
- Exports (what the mod provides): 

## World `entities-host`

The game's side of capability `entities` (imports only).

- Imports (what the game offers): `entities`
- Exports (what the mod provides): 

## World `server-net-host`

The game's side of `server-net-events`.

- Imports (what the game offers): 
- Exports (what the mod provides): `server-net-events`

## World `horizon-ui-mod`

A mod with widgets (capability `ui`): the SDK builds every mod against this world. Mods built for `horizon-mod` alone (no `ui-events` export) still load and simply hear nothing of their widgets.

- Imports (what the game offers): `ui`, `render`, `input`, `recipes`, `browser`, `net`, `block-data`, `log`, `game`, `chat`, `hud`, `storage`, `command`
- Exports (what the mod provides): `ui-events`, `net-events`, `events`

## World `block-data-host`

The game's side of `block-data` (imports only).

- Imports (what the game offers): `block-data`
- Exports (what the mod provides): 

## World `ui-host`

The game's side of `horizon-ui-mod` beyond `horizon-mod`.

- Imports (what the game offers): `ui`
- Exports (what the mod provides): `ui-events`

## World `render-host`

The game's side of capability `render` (imports only).

- Imports (what the game offers): `render`, `input`
- Exports (what the mod provides): 

## World `recipes-host`

The game's side of capability `recipes` (imports only).

- Imports (what the game offers): `recipes`, `browser`
- Exports (what the mod provides): 

## `log`

Access: always granted.

The mod's log, in the game's log under the mod's id. Always granted.

### enum `level`

| Case | |
| --- | --- |
| `debug` |  |
| `info` |  |
| `warn` |  |
| `error` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `write` | `func(level: level, message: string)` | One line in the game's log as `[mod <id>] message`, at that level. |

## `game`

Access: capability `game-read`.

Capability `game-read`: the local player, the world clock and the block the player looks at.

### record `player`

| Field | Type | |
| --- | --- | --- |
| `x` | `f64` | Feet position, in blocks. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `yaw` | `f32` | Degrees, as F3 shows them. |
| `pitch` | `f32` | Degrees: -90 looks straight up, 90 straight down. |
| `health` | `f32` | Hit points as vanilla counts them: 20 is full health without effects. |
| `food` | `u32` | 0 to 20, as the hunger bar. |
| `dimension` | `string` | `minecraft:overworld`, `minecraft:the_nether`, ... |

### record `clock`

| Field | Type | |
| --- | --- | --- |
| `game-time` | `u64` | Ticks since the world was created. |
| `day-time` | `u64` | 24000 ticks per day; 0 is 6:00 on the first day. |

### record `hit`

A block along the player's view.

| Field | Type | |
| --- | --- | --- |
| `block-x` | `s32` | The block hit. |
| `block-y` | `s32` |  |
| `block-z` | `s32` |  |
| `x` | `f64` | The point on its face where the view meets it. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `block` | `string` | `minecraft:stone`, ... |

| Function | Signature | What it does |
| --- | --- | --- |
| `get-player` | `func() -> option<player>` | None on the title screen. |
| `get-clock` | `func() -> option<clock>` | None on the title screen. |
| `raycast` | `func(distance: f32) -> option<hit>` | The first block along the player's view (fluids pass), up to `distance` blocks (at most 1024; past the loaded chunks, across the distant terrain when LOD Distance is on), as of this tick. None when nothing is hit, and on the title screen. |

## `chat`

Access: capability `chat`.

Capability `chat`: messages only the local player sees.

| Function | Signature | What it does |
| --- | --- | --- |
| `show` | `func(text: string)` | Shown in chat after the mod's name, like a system message. |

## `hud`

Access: capability `hud`.

Capability `hud`: one panel of text lines per mod, drawn by the game every frame until the mod changes or clears it (no call per frame).

### enum `corner`

| Case | |
| --- | --- |
| `top-left` |  |
| `top-right` |  |
| `bottom-left` |  |
| `bottom-right` |  |

### record `line`

| Field | Type | |
| --- | --- | --- |
| `text` | `string` | Plain text, up to 256 characters. |
| `color` | `u32` | 0xRRGGBB. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-panel` | `func(corner: corner, lines: list<line>)` | Replaces the mod's panel: up to 16 lines in that corner of the screen. |
| `clear` | `func()` | Removes the mod's panel. |

## `storage`

Access: capability `storage`.

Capability `storage`: a per-mod folder of small key/value files, quota'd (docs/mods.md). Keys are `[a-z0-9_.-]{1,64}`, one path segment.

| Function | Signature | What it does |
| --- | --- | --- |
| `get` | `func(key: string) -> option<list<u8>>` | None if `key` has never been `set`, or the key is not well-formed. |
| `set` | `func(key: string, value: list<u8>) -> result<_, string>` | Errs on a malformed key or over quota (bytes for the whole mod). |
| `delete` | `func(key: string)` | No error if `key` was already absent. |

## `command`

Access: capability `command`.

Capability `command`: vanilla commands run as the player, with the player's permissions, after the hook call returns (8 per call). A mod's own commands are not run this way.

| Function | Signature | What it does |
| --- | --- | --- |
| `run` | `func(line: string)` | `line` without its slash; its feedback goes to the chat as if typed. |
| `run-quiet` | `func(lines: list<string>)` | `lines` (each without its slash, at most 256 a call) as `run` does, but with nothing in the chat: no feedback and no failure. In single player and on Horizon's servers, each runs exactly as the player typing it would (the same rights), in order, 64 a tick, and the server logs one line a batch; one that carries a chat message (`say`, `msg`) is refused. On any other server they go as chat commands, and there their feedback shows: a mod running many should say so. |

## `ui`

Access: capability `ui`.

Capability `ui`: widgets the game draws with its own menus' widgets, for a screen the mod owns and a settings page in Options > Mods. Layout and drawing are the game's: a mod says what its widgets hold, never where they sit or how they look, and the game clips everything it is handed.

### record `text-widget`

A line of text.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget; `ui-events` calls echo it. |
| `text` | `string` | Up to 256 characters. |
| `color` | `u32` | 0xRRGGBB; 0 is the menus' white. |

### record `item-widget`

An item's icon and a name beside it, as recipe viewers show them.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget. |
| `item` | `string` | `minecraft:diamond`, `<mod>:<path>`; an unknown id draws a "?". |
| `name` | `string` | The name shown beside the icon (empty: the item's own). |

### record `list-widget`

Rows the player picks one of, as an options page lists choices: a pick is `ui-events::select`, `selected` is the row to mark now.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget; `ui-events::select` echoes it. |
| `rows` | `list<string>` | The rows' texts, at most 512, each up to 256 characters. |
| `selected` | `option<u32>` | The row to mark as the current choice, if any. |

### record `button-widget`

A thing on a mod's screen or settings page. A button as the options pages have them (200 × 20): a click is `ui-events::select(id, 0)`. Mods cycle its label for a choice ("Corner: Top Left").

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The mod's own name for the widget. |
| `label` | `string` | Up to 256 characters. |

### variant `widget`

| Case | Payload | |
| --- | --- | --- |
| `text` | `text-widget` | Text the player reads. |
| `button` | `button-widget` | A button. |
| `item` | `item-widget` | An item's icon and name. |
| `list` | `list-widget` | Rows the player picks from. |

### record `screen`

A screen the mod owns: a title over widgets stacked down the middle, the Done button the game adds, and Esc to leave it.

| Field | Type | |
| --- | --- | --- |
| `title` | `string` | Its title, up to 256 characters. |
| `widgets` | `list<widget>` | Its widgets, in order, at most 64. |

| Function | Signature | What it does |
| --- | --- | --- |
| `open-screen` | `func(screen: screen)` | Show `screen` over the game; a second call while it is open replaces it. Only the last call of one hook counts. |
| `close-screen` | `func()` | Close the mod's screen when it is open (the mod hears no `closed` for this: it was its own doing). |
| `set-settings` | `func(widgets: list<widget>)` | The mod's page in Options > Mods, reached from a settings button on its row: the widgets it shows, an empty list for no page. The player opening the page is `ui-events::settings-opened`: push fresh widgets then; later calls update the page while it shows. |

## `ui-events`

Access: exported by the mod: the game calls it.

What the game calls on a mod that shows widgets (capability `ui`): what the player did with them.

| Function | Signature | What it does |
| --- | --- | --- |
| `select` | `func(id: u32, row: u32)` | A row of one of the mod's `list` widgets was picked: the widget's `id` and the row, on the screen or the settings page that showed it. |
| `closed` | `func()` | The player left the mod's screen (Esc or Done); not for a screen the mod closed itself or one that went away with the mod. |
| `settings-opened` | `func()` | The player opened the mod's settings page in Options > Mods: call `ui::set-settings` now to fill it with fresh widgets. |

## `input`

Access: capability `input`.

Capability `input` (experimental: the manifest needs `experimental = ["input"]`): the use button on items the mod claims, and the mod's own keys. Events wait for `poll`, at most 256.

### record `held-item`

The main hand's stack.

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `minecraft:spyglass`, `<mod>:<path>`. |
| `count` | `u32` |  |
| `slot` | `u32` | The hotbar slot, 0 to 8. |

### record `item-use`

| Field | Type | |
| --- | --- | --- |
| `item` | `held-item` |  |
| `ticks` | `u32` | Game ticks the button was held (0 on `use-start`). |

### variant `event`

| Case | Payload | |
| --- | --- | --- |
| `use-start` | `item-use` | The use button went down with a claimed item in the main hand. The game does nothing else with the click: no placing, no eating, no swing. |
| `use-stop` | `item-use` | It came up again, or the item left the hand, or a screen opened. |
| `key-down` | `string` | A key `register-key` named went down or up while playing (no screen open). The game's own mapping on that key still runs. A screen opening lets every held key up. |
| `key-up` | `string` |  |
| `wheel` | `f32` | Mouse wheel steps while the mod holds the wheel (`claim-wheel`): positive away from the player, as the hotbar reads them. |

### record `mapping-state`

A mapping as the player has bound it.

| Field | Type | |
| --- | --- | --- |
| `label` | `string` | The bound key's name as Controls shows it: `W`, `Space`, `LMB`. |
| `down` | `bool` | Held down now (as of this frame for a mod with a `frame` hook, else this tick). |

| Function | Signature | What it does |
| --- | --- | --- |
| `claim-items` | `func(ids: list<string>)` | The items whose use the mod handles instead of the game, by id (at most 64). Replaces the previous list. |
| `register-key` | `func(name: string, label: string, key: string) -> result<_, string>` | A key of the mod's: `name` comes back in `key-down` and `key-up`, `label` says what it does, `key` is the key as options.txt writes it (`key.keyboard.r`), the default: the player can bind another one on the mod's page in Options > Mods, and that one wins. At most 16; an unknown key is an error. |
| `held` | `func() -> option<held-item>` | The main hand's stack, as of this tick; none when it is empty. |
| `poll` | `func() -> list<event>` | The events since the last call, oldest first. |
| `claim-wheel` | `func(on: bool)` | Takes the mouse wheel while playing (true) or gives it back: while taken, its steps are the mod's `wheel` events and the hotbar doesn't move. The game gives it back when the mod stops. |
| `mapping` | `func(name: string) -> option<mapping-state>` | A movement or click mapping as the player has bound it: `key.forward`, `key.left`, `key.back`, `key.right`, `key.jump`, `key.sneak`, `key.sprint`, `key.attack` or `key.use`. None for another name. |
| `clicks-per-second` | `func() -> tuple<u32, u32>` | Clicks of the attack and use buttons in the last second. |

## `render`

Access: capability `render`.

Capability `render` (experimental: the manifest needs `experimental = ["render"]`): shapes the game draws for the mod with its own renderer, kept until the mod replaces them (no call per frame). A mod never supplies shader code.

### enum `corner`

A corner of the screen, as `hud`'s panels use them.

| Case | |
| --- | --- |
| `top-left` |  |
| `top-right` |  |
| `bottom-left` |  |
| `bottom-right` |  |

### record `rect`

A rectangle in GUI pixels: the top left corner and the size.

| Field | Type | |
| --- | --- | --- |
| `x` | `f32` |  |
| `y` | `f32` |  |
| `w` | `f32` |  |
| `h` | `f32` |  |

### record `point`

A point in GUI pixels from the screen's top left.

| Field | Type | |
| --- | --- | --- |
| `x` | `f32` |  |
| `y` | `f32` |  |

### record `fill-shape`

| Field | Type | |
| --- | --- | --- |
| `rect` | `rect` |  |
| `color` | `u32` | 0xAARRGGBB. |

### record `quad-shape`

Any four-cornered shape (a wheel's segment, an arrow): the corners in order round it.

| Field | Type | |
| --- | --- | --- |
| `corners` | `tuple<point, point, point, point>` |  |
| `color` | `u32` | 0xAARRGGBB. |

### record `sprite-shape`

| Field | Type | |
| --- | --- | --- |
| `sprite` | `string` | One of the game's GUI sprites, as `gui/sprites/` names them (`hud/heart/full`, `widget/button`), the resource packs' versions included. An unknown one draws nothing. |
| `rect` | `rect` |  |
| `color` | `u32` | A tint, 0xAARRGGBB: 0xFFFFFFFF draws it as it is. |

### record `item-shape`

| Field | Type | |
| --- | --- | --- |
| `item` | `string` | `minecraft:diamond`, `<mod>:<path>`; an unknown one draws nothing. |
| `at` | `point` | The icon's top left. |
| `scale` | `f32` | 1.0 is the hotbar's 16 × 16. |

### record `text-shape`

| Field | Type | |
| --- | --- | --- |
| `text` | `string` | Up to 256 characters, one line. |
| `at` | `point` |  |
| `color` | `u32` | 0xRRGGBB, as `hud`'s lines. |
| `centered` | `bool` | Centred on `at.x` instead of starting there. |
| `shadow` | `bool` |  |

### variant `shape`

| Case | Payload | |
| --- | --- | --- |
| `fill` | `fill-shape` |  |
| `quad` | `quad-shape` |  |
| `sprite` | `sprite-shape` |  |
| `item` | `item-shape` |  |
| `text` | `text-shape` |  |

### record `frame-rate`

| Field | Type | |
| --- | --- | --- |
| `fps` | `u32` | Frames drawn in the last second. |
| `min` | `u32` | The slowest frame's rate, and the 1% low (the 99th percentile frame time's rate), over the last 5 seconds. |
| `low` | `u32` |  |

### record `zoom`

| Field | Type | |
| --- | --- | --- |
| `factor` | `f32` | The field of view is divided by it: 1 is none, at most 10. |
| `smooth` | `bool` | Ease in and out through the game's field-of-view easing, as the spyglass does, instead of at once. |
| `turn` | `f32` | Mouse turning is multiplied by it while zoomed: 1 keeps the player's sensitivity, at least 0.05. |

### record `minimap`

| Field | Type | |
| --- | --- | --- |
| `corner` | `corner` |  |
| `size` | `f32` | Its side in GUI pixels, 16 to 256. |
| `blocks-per-pixel` | `f32` | World blocks per map pixel, 0.5 to 4; the map is 128 × 128 pixels. |
| `rotate` | `bool` | The map turns with the player, who faces up, and an N on its rim marks north. Otherwise north is up and the player's arrow turns. |
| `round` | `bool` | Round, the corners left clear; otherwise square. |

### record `far-terrain`

| Field | Type | |
| --- | --- | --- |
| `distance` | `u32` | How far the distant terrain reaches, in chunks: 64 to 1024. On integrated graphics other than Apple's the game caps it lower. |
| `reduced` | `bool` | Coarser far tiles, as the Far Detail setting's Reduced: 2 to 4 ms less GPU time per frame on integrated graphics. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-hud` | `func(shapes: list<shape>)` | Replaces the mod's HUD shapes: up to 1024, drawn every frame in order over the HUD (after the hotbar, under chat and screens), hidden with the HUD (F1). An empty list removes them. |
| `gui-size` | `func() -> tuple<f32, f32>` | The screen's size in GUI pixels (width, height), as of this tick. |
| `shake` | `func(strength: f32, seconds: f32)` | Shakes the camera: a jolt of `strength` (0 to 1, 1 the hardest) that fades out over `seconds` (at most 10). Jolts add up to 1. The view turns, the player doesn't, and the player's Distortion Effects option scales it (0 turns it off). |
| `set-corner` | `func(corner: corner, width: f32, height: f32, shapes: list<shape>)` | Replaces the mod's corner panel: shapes in GUI pixels from the top left of a `width` × `height` box that the game places in `corner`, stacked with the game's overlays and the other mods' panels (under the status effects at the top right), as `hud`'s panels are. |
| `clear-corner` | `func()` | Removes the mod's corner panel. |
| `text-width` | `func(text: string) -> f32` | The width of one line of text in GUI pixels, in the HUD's font: exact for Latin letters (U+0000 to U+024F), 6 per other character. |
| `frame-stats` | `func() -> frame-rate` | The game's frame counter, recomputed twice a second. |
| `set-zoom` | `func(zoom: zoom)` | Zooms the camera until the next call (`factor` 1 ends it), only while playing: a screen opening or the spyglass's own view (which wins) leave the camera vanilla's. With several mods, the strongest zoom counts. |
| `set-minimap` | `func(minimap: option<minimap>)` | Shows a map of the world around the player in `corner`, stacked with the game's overlays and the mods' panels, until the next call (none hides it). It looks like a map item: vanilla's map colours with their height shading and water depth, and the player's arrow. Underground, under a roof and in the Nether it maps the floor around the player's height. The game keeps the map's tiles and refreshes them within its own frame budget; hidden, they are dropped. One map shows at a time: the first loaded mod's. |
| `set-far-terrain` | `func(far-terrain: option<far-terrain>)` | Draws the world beyond the render distance as distant terrain, out to `distance` chunks, until the next call (none ends it). The game builds it on its own workers, from the chunks already loaded or saved and, where nobody has been, from the world's generator; the simulation distance doesn't change. Only worlds whose generator the game has (single player and a LAN world you host) show it. The player's own LOD Distance setting, when on, wins. One far terrain shows at a time: the first loaded mod's. |

## `events`

Access: exported by the mod: the game calls it.

What the game calls. Only the hooks `init` asks for are ever called.

### record `registration`

| Field | Type | |
| --- | --- | --- |
| `commands` | `list<string>` | Chat commands, without the slash. A name vanilla already uses is refused (vanilla's command runs). |
| `tick` | `bool` | Call `tick` every game tick (20 per second while playing). |
| `frame` | `bool` | Call `frame` every rendered frame. Budget: 0.1 ms. |
| `chat` | `bool` | Call `chat` for each chat line the player receives. |

| Function | Signature | What it does |
| --- | --- | --- |
| `init` | `func() -> result<registration, string>` | Once, after loading. An error keeps the mod off and is shown. |
| `tick` | `func(tick: u64)` | Each game tick while in a world, with the game's tick counter. |
| `frame` | `func(seconds: f32)` | Each rendered frame, with the seconds since the last one. |
| `command` | `func(name: string, args: string) -> result<option<string>, string>` | `/name args`: ok(some(text)) is shown as feedback, err(text) in red. |
| `chat` | `func(text: string)` | Each chat line the player receives, as plain text. |

## `recipes`

Access: capability `recipes`.

Capability `recipes` (experimental): the recipes and items the game knows, for recipe viewers. In single player and on Horizon's servers they are the world's; on any other server they are vanilla 26.3's (a client is sent no others), while `known` follows that server's book.

### record `stack`

An item of `items`, by its index there, with a count and the components it has beyond the item's own, in vanilla's item syntax (`[potion_contents={potion:"minecraft:swiftness"}]`), or "".

| Field | Type | |
| --- | --- | --- |
| `item` | `u32` |  |
| `count` | `u32` |  |
| `components` | `string` |  |

### enum `kind`

The station a recipe is made at.

| Case | |
| --- | --- |
| `crafting-shaped` |  |
| `crafting-shapeless` |  |
| `smelting` |  |
| `blasting` |  |
| `smoking` |  |
| `campfire` |  |
| `stonecutting` |  |
| `smithing` |  |
| `brewing` |  |

### record `recipe`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `minecraft:oak_planks`. |
| `kind` | `kind` |  |
| `category` | `string` | The recipe book's tab (`crafting_building_blocks`, `furnace_food`), or "". |
| `group` | `string` | Recipes the book shows on one button share a group, or "". |
| `width` | `u32` | A shaped recipe's grid: `slots` holds its cells row by row. 0 for the other kinds. |
| `height` | `u32` |  |
| `slots` | `list<list<stack>>` | Each slot's stacks, any one of which fits; an empty list is an empty cell. Cooking and stonecutting: the input. Smithing: the template, the base, the addition. Brewing: the bottle, the reagent. |
| `results` | `list<stack>` | What it makes; several when that varies (a trim on any armour). |
| `cooking-time` | `u32` | Cooking: game ticks and experience; 0 for the other kinds. |
| `experience` | `f32` |  |
| `book-id` | `s32` | The recipe book's id for it (one per way the book shows it), as `known` lists them; -1 for the kinds the book doesn't show (campfire, stonecutting, smithing, brewing). |

### record `item`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `minecraft:diamond`, `<mod>:<path>`. |
| `name` | `string` | In the player's language. |
| `tags` | `list<string>` | Its item tags, without the `#`. |

| Function | Signature | What it does |
| --- | --- | --- |
| `items` | `func() -> list<item>` | Every item, in the registry's order: `stack.item` indexes this list. |
| `all` | `func() -> list<recipe>` | Every recipe: the book's in its order, then campfire, stonecutting, smithing and brewing. They don't change while a world is open. |
| `known` | `func() -> list<s32>` | The `book-id`s of the recipes the player's recipe book holds now. |
| `place` | `func(book-id: s32, all: bool) -> result<_, string>` | Places a recipe in the open screen's grid as a click in the recipe book does (`ServerboundPlaceRecipePacket`): the inventory's grid or a crafting table. `all` places as many as the items allow, as with Shift. Refused while no such screen is open, for a recipe the player's book doesn't hold and for one that can't be placed; one missing items shows as a ghost in the grid, as in the book. |
| `give` | `func(stack: stack) -> result<_, string>` | Creative mode only: gives the player the stack, as taking it from the creative inventory does (`ServerboundSetCreativeModeSlot`): the selected slot first, then the hotbar, then the inventory. The count is cut to one stack of the item (64, 16 or 1). |

## `browser`

Access: not granted by any capability.

Capability `ui` (experimental): a recipe viewer's panel beside the game's inventory and container screens (not the creative inventory), on the right of the screen. The game draws it with its own widgets and slots, pages it and keeps its search box; the mod says what it holds. The first loaded mod with a panel shows.

### record `stack`

An item to show: its id, a count (1 draws none) and its components beyond the item's own, in vanilla's item syntax, or "".

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |
| `components` | `string` |  |

### record `click`

A click on an item of the panel, by its index in `set-items`'s list.

| Field | Type | |
| --- | --- | --- |
| `index` | `u32` |  |
| `right` | `bool` |  |

### record `item-key`

A key the mod registered (`input.register-key`) went down while the pointer was on an item: a slot of the screen's, or the panel's.

| Field | Type | |
| --- | --- | --- |
| `key` | `string` |  |
| `stack` | `stack` |  |

### record `slot-click`

A click on a slot of a recipe the panel shows (`set-recipes`).

| Field | Type | |
| --- | --- | --- |
| `recipe` | `u32` | The recipe's index in `set-recipes`'s list and the slot's in it; the results come after the slots. |
| `slot` | `u32` |  |
| `stack` | `stack` | The stack the slot showed at that moment. |
| `right` | `bool` |  |

### variant `event`

| Case | Payload | |
| --- | --- | --- |
| `clicked` | `click` |  |
| `search` | `string` | The search box's text, each time the player changes it. |
| `key` | `item-key` |  |
| `slot-clicked` | `slot-click` |  |
| `action` | `u32` | The action button of the recipe at this index in `set-recipes`'s list. |
| `page` | `u32` | The player turned the recipes' page (`set-recipes`): the index of the first recipe now shown. |
| `back` |  | The back button of `set-recipes`. |
| `tab` | `u32` | The player picked the tab at this index of `set-recipes`' tabs. |

### enum `layout`

How the game lays out a recipe of `set-recipes`.

| Case | |
| --- | --- |
| `grid` | A crafting grid of `width` x `height` slots, an arrow and the result. |
| `cooking` | One input over a flame, an arrow and the result (furnaces, smokers, campfires). |
| `single` | One input, an arrow and the result (the stonecutter). |
| `smithing` | Template, base and addition, an arrow and the result. |
| `brewing` | A potion and an ingredient, an arrow and the result. |

### record `recipe-view`

A recipe to show. Each slot, and the result, cycles through its stacks, one a second; an empty slot shows nothing.

| Field | Type | |
| --- | --- | --- |
| `layout` | `layout` |  |
| `width` | `u32` | The grid's size (`grid` only), at most 3 x 3. |
| `height` | `u32` |  |
| `slots` | `list<list<stack>>` |  |
| `results` | `list<stack>` |  |
| `note` | `string` | A short line under the recipe (a cooking time), or "". |
| `action` | `option<bool>` | The action button beside it: none, enabled, or shown greyed. |

| Function | Signature | What it does |
| --- | --- | --- |
| `set-items` | `func(items: list<stack>)` | The panel's items, at most 8192 (an empty list hides the panel). |
| `set-recipes` | `func(title: string, tabs: list<stack>, tab: u32, recipes: list<recipe-view>)` | Shows these recipes in the panel instead of its items, as many as fit a page, under `title` and a back button, with a row of `tabs` (each an icon; `tab` is the selected one) between them; an empty list of recipes shows the items again. At most 256 recipes and 16 tabs. |
| `poll` | `func() -> list<event>` | What the player did on the panel since the last poll, at most 256 kept. |

## `players`

Access: capability `players`.

Capability `players`, server mods only (manifest `api = "0.3"`, experimental): the players on the server, and messages to them.

### enum `game-mode`

| Case | |
| --- | --- |
| `survival` |  |
| `creative` |  |
| `adventure` |  |
| `spectator` |  |

### record `player`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | The player's UUID, `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`. |
| `name` | `string` |  |
| `x` | `f64` | Feet position, in blocks. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `dimension` | `string` | `minecraft:overworld`, `minecraft:the_nether`, ... |
| `game-mode` | `game-mode` |  |
| `health` | `f32` | Hit points as vanilla counts them: 20 is full health without effects. |

### record `slot-stack`

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `online` | `func() -> list<player>` | The players online, in the order they joined. |
| `rotation` | `func(player: string) -> option<tuple<f32, f32>>` | Where a player looks, as of the start of this tick: yaw and pitch in degrees as F3 shows them (-90 pitch looks straight up), for the direction a block is placed facing (NeoForge's `BlockPlaceContext`); none when they are not online. |
| `tell` | `func(player: string, text: string) -> bool` | A system message to one player, after the mod's name; false when no player with that id is online. Up to 8 messages per call. |
| `broadcast` | `func(text: string)` | A system message to every player and the server's log, after the mod's name. Counts toward the same 8 per call. |
| `push` | `func(player: string, x: f64, y: f64, z: f64) -> bool` | Adds a velocity, in blocks per tick, to a player after the call (a conveyor, a fan): false when no player with that id is online. Each component is clamped to -4 to 4. At most 256 pushes a call. |
| `inventory` | `func(player: string) -> option<list<option<slot-stack>>>` | A player's inventory as of the start of this tick: 41 slots, 0 to 8 the hotbar, 9 to 35 the rest, 36 to 39 the armor from feet to head, 40 the off hand (item components not shown); none when they are not online. |
| `selected-slot` | `func(player: string) -> option<u32>` | The hotbar slot (0 to 8) a player holds, as of the start of this tick. |

## `world`

Access: capability `world`.

Capability `world`, server mods only (manifest `api = "0.3"`, experimental): the blocks of the server's loaded chunks, and changes to them through the server's own block updates.

### record `block-change`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |
| `state` | `string` | `minecraft:oak_log[axis=y]`; properties left out take the block's defaults, `minecraft:stone` alone is the default state. |

### record `scanned-block`

One block of a scanned region, its state an index into the palette.

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |
| `state` | `u32` |  |

### record `region`

The solid part of a region: every non-air block in loaded chunks, and the states they use once each.

| Field | Type | |
| --- | --- | --- |
| `palette` | `list<string>` |  |
| `blocks` | `list<scanned-block>` |  |

### record `state-info`

What a block state is made of, as vanilla's block properties say.

| Field | Type | |
| --- | --- | --- |
| `hardness` | `f32` | `destroyTime`: seconds of a bare hand's mining, roughly; -1 for a block nothing breaks (bedrock). |
| `requires-tool` | `bool` | Only the right tool gets its drops (stone and a pickaxe). |
| `explosion-resistance` | `f32` |  |
| `break-sound` | `string` | Its sound type's events: breaking, stepping, placing, hitting. |
| `step-sound` | `string` |  |
| `place-sound` | `string` |  |
| `hit-sound` | `string` |  |
| `liquid` | `bool` | It holds a fluid (water, lava, a waterlogged state). |
| `push-reaction` | `string` | How a piston or a moving structure treats it: `normal`, `destroy`, `block`, `ignore` or `push_only`, as vanilla's `PushReaction`. |

| Function | Signature | What it does |
| --- | --- | --- |
| `get-block` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<string>` | The block state at a position in a dimension (`minecraft:overworld`, ...), as `minecraft:oak_log[axis=y]`, as of this call: none where the server has no chunk loaded there. |
| `get-biome` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<string>` | The biome at a position (`minecraft:plains`, ...), as the server stores it (one per 4 x 4 x 4 cell): none where no chunk is loaded. |
| `set-blocks` | `func(dimension: string, blocks: list<block-change>) -> result<_, string>` | Sets blocks as `/setblock` does: the old block entity removed, neighbours and shapes updated, light, and every player told. Every state is checked now (an error queues nothing); the changes are made in order after this call returns, before that dimension's next tick; one in a chunk not loaded then is skipped. At most 4096 per call. |
| `use-item` | `func(dimension: string, x: s32, y: s32, z: s32, face: u8, item: string) -> result<_, string>` | Uses an item on a block as a player would, after the call (a deployer): the block's own answer to it first (a lever pulled, a door opened, a note block tuned), and when it has none and the item is a block's, that block placed against `face` (0 down, 1 up, 2 north, 3 south, 4 west, 5 east) where there is air. Items' own uses (bone meal, flint and steel, buckets) are not run yet. At most 256 a call. |
| `break-block` | `func(dimension: string, x: s32, y: s32, z: s32, tool: option<string>, enchantments: list<tuple<string, u32>>, into: option<tuple<s32, s32, s32>>) -> result<_, string>` | Breaks a block as a player holding `tool` (an item id; none for a bare hand) enchanted with `enchantments` (`minecraft:fortune`, 3 / `minecraft:silk_touch`, 1) would, after the call: its break particles and sound, a container's contents spilled, air in its place, and its loot table's drops when the tool is right for it (a pickaxe for stone). The drops go into the inventory at `into` (as `transfer` puts items in), what does not fit and all of them without one dropping where the block stood. A drill or a saw. At most 256 a call; unbreakable blocks (bedrock) are left. |
| `scan` | `func(dimension: string, min-x: s32, min-y: s32, min-z: s32, max-x: s32, max-y: s32, max-z: s32) -> result<region, string>` | Reads every block from (min-x, min-y, min-z) to (max-x, max-y, max-z), both ends included, as of this call; air and blocks in chunks not loaded are left out. At most 32768 positions in the box. |
| `block-info` | `func(state: string) -> option<state-info>` | The properties of a block state (`minecraft:stone`, a mod's own), none for one that names no state: a drill timing its breaking by its hardness, a contraption leaving what it cannot move. |
| `game-time` | `func() -> u64` | The world's game time, the ticks since it was made, as of this tick: what `visuals.set-in-phase` turns parts by. |

## `server-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod (manifest `api = "0.3"`, experimental). Only the hooks `init` asks for are ever called. The player in each event is as the server saw them when it happened.

### type `player`

`type player = player`

### record `block-pos`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |

### variant `argument-kind`

What an argument of a command takes. The server sends each command and its arguments to players' games, whose chat box completes and colours them as vanilla's; the server parses them before the mod's `command` hook gets their values.

| Case | Payload | |
| --- | --- | --- |
| `word` |  | One word: letters, digits and `_ - . +`. |
| `text` |  | The rest of the line, spaces included: only last. |
| `integer` | `tuple<option<s32>, option<s32>>` | A whole number, within the bounds given. |
| `number` |  | A number with or without decimals. |
| `boolean` |  | `true` or `false`. |
| `player` |  | One online player: a name, or `@s` or `@p` (whoever typed it, as vanilla picks for a player). |
| `block-pos` |  | A block position: three whole numbers, each one also `~` or `~n`, relative to whoever typed it. |

### record `argument`

| Field | Type | |
| --- | --- | --- |
| `name` | `string` | Shown in the chat box's hint (`<height>`) and the key of its value. |
| `kind` | `argument-kind` |  |
| `optional` | `bool` | Optional arguments come after every required one. |

### record `server-command`

| Field | Type | |
| --- | --- | --- |
| `name` | `string` | Without the slash. A name vanilla already uses is refused (vanilla's command runs). |
| `arguments` | `list<argument>` | In order; none: the command takes nothing after its name. Free text is one optional `text` argument. |

### variant `argument-value`

An argument's value, parsed as its kind says.

| Case | Payload | |
| --- | --- | --- |
| `word` | `string` |  |
| `text` | `string` |  |
| `integer` | `s32` |  |
| `number` | `f64` |  |
| `boolean` | `bool` |  |
| `player` | `player` |  |
| `block-pos` | `block-pos` |  |

### record `registration`

| Field | Type | |
| --- | --- | --- |
| `commands` | `list<server-command>` | Commands players may type. |
| `tick` | `bool` | Call `tick` every server tick (20 per second). |
| `joins` | `bool` | Call `player-joined` and `player-left`. |
| `chat` | `bool` | Call `chat` for each chat message a player sends. |
| `blocks` | `bool` | Call `block-broken` and `block-placed`. |

| Function | Signature | What it does |
| --- | --- | --- |
| `init` | `func() -> result<registration, string>` | Once, after loading. An error keeps the mod off and is logged. |
| `tick` | `func(tick: u64)` | Each server tick, with the server's tick counter. |
| `player-joined` | `func(player: player)` | A player joined the server; they are in `players.online` already. |
| `player-left` | `func(player: player)` | A player left the server; they are no longer in `players.online`. |
| `chat` | `func(player: player, text: string)` | A chat message a player sent, as plain text. |
| `command` | `func(player: player, name: string, args: string, values: list<tuple<string, argument-value>>) -> result<option<string>, string>` | `/name args` typed by `player`: ok(some(text)) is their feedback, err(text) their error in red. `values` holds each argument given, by name, in order (an optional one left out isn't there); a line that doesn't parse never reaches the mod: the player gets vanilla's error. |
| `block-broken` | `func(player: player, pos: block-pos, state: string)` | A player broke the block at `pos` in their dimension: `state` is what was there, as `minecraft:oak_log[axis=y]`. |
| `block-placed` | `func(player: player, pos: block-pos, state: string)` | A player placed `state` at `pos` in their dimension. |

## `block-entities`

Access: capability `block-entities`.

Capability `block-entities`, server mods only (manifest `api = "0.3"`, experimental): bytes a mod keeps at block positions, for blocks it owns (a gear's speed, a machine's progress). The game saves them with the world's chunks and hands them back after a restart.

| Function | Signature | What it does |
| --- | --- | --- |
| `get` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<list<u8>>` | The bytes last stored at a position, none when there are none. |
| `set` | `func(dimension: string, x: s32, y: s32, z: s32, data: list<u8>) -> result<_, string>` | Stores bytes at a position (at most 64 KiB; 65,536 positions per mod), replacing any there. The game forgets them when the block is broken or replaced by something that is not one of the mod's. |
| `remove` | `func(dimension: string, x: s32, y: s32, z: s32)` | Forgets the bytes at a position. |
| `clicked-face` | `func() -> option<u8>` | During `block-events.block-used`: the face the player clicked (0 down, 1 up, 2 north, 3 south, 4 west, 5 east), as `item-events` gives it; none at any other time. |
| `set-synced` | `func(dimension: string, x: s32, y: s32, z: s32, data: list<u8>) -> result<_, string>` | `set`, and the bytes also go to every player whose game has the chunk, now, whenever they change and when the chunk reaches a player later: what the mod's client half reads with `block-data` (NeoForge's update tag). A later `set` at the position keeps them on the server only; `remove` takes them from the players' games too. Saved with the world as `set`'s are. |

## `structures`

Access: not granted by any capability.

Capability `entities`, server mods (`experimental = ["entities"]`): structures, blocks taken out of the grid as one object that moves and turns as a whole, as Create's contraptions and Aeronautics' ships. Players' games draw each as one model, between its poses of the last two ticks. The game saves them with the world; one still there when the world loads is put back into the grid where it stood, so no block is lost.

### record `structure-block`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` | Its place in the structure (0, 0, 0 is the structure's corner). |
| `y` | `s32` |  |
| `z` | `s32` |  |
| `state` | `string` | `minecraft:oak_planks`, `mymod:shaft[axis=x]`. |

### record `pose`

Where a structure is: the world position of its (0, 0, 0) corner, and its turn about that corner as a quaternion (x, y, z, w), made unit by the game.

| Field | Type | |
| --- | --- | --- |
| `x` | `f64` |  |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `rotation` | `tuple<f32, f32, f32, f32>` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `create` | `func(dimension: string, blocks: list<structure-block>, at: pose) -> result<u32, string>` | Makes a structure of these blocks (at most 4096) at `pose` in a dimension, after the call: its id, the mod's own (at most 64 at once). The blocks are only drawn; taking them out of the world is `world.set-blocks`, as the SDK's `Structure::assemble` does. |
| `set-pose` | `func(id: u32, at: pose) -> bool` | Moves and turns one, after the call; players' games see it go there over the next tick. False for an id the mod does not hold. |
| `remove` | `func(id: u32) -> bool` | Takes one away, after the call (its blocks with it: putting them back is `world.set-blocks`). |

## `visuals`

Access: not granted by any capability.

Capability `block-entities`, server mods: model parts drawn at the mod's own blocks, each turning at a constant speed, as Flywheel's instances draw Create's shafts and cogs. The server sends a position's parts to the players whose games have its chunk once, and again only when they change; their games turn them every frame. Saved with the world, and gone when the block is broken.

### record `part`

| Field | Type | |
| --- | --- | --- |
| `state` | `string` | The block state whose model is drawn: `mymod:shaft[axis=x]`, a vanilla one too. |
| `axis` | `u8` | The axis it turns about: 0 x, 1 y, 2 z. |
| `pivot` | `tuple<f32, f32, f32>` | The point it turns about, in blocks from the position's corner (0.5, 0.5, 0.5 is the block's centre); at most 16 away. |
| `offset` | `tuple<f32, f32, f32>` | Where its model's corner is, in blocks from the position's corner: (0, 1, 0) draws it on top of the block. |
| `speed` | `f32` | Degrees a tick (360 at most), counterclockwise looking from the axis's positive end; negative turns the other way, 0 not at all. |
| `angle` | `f32` | Its angle in degrees when set: parts set in one tick at the same speed turn together. |
| `velocity` | `tuple<f32, f32, f32>` | Blocks a tick the whole part moves, from the tick it was set (0 not at all): a press's head going down. It keeps moving until the mod sets the position's parts again, so a mod animating something sets them each tick, with this tick's offset and the step to the next. |
| `item` | `bool` | `state` names an item (`minecraft:iron_ingot`) instead: drawn as an item frame shows it, a block across, its centre half a block from `offset` (an item lying on a depot, a belt, in a basin). |
| `scale` | `f32` | How large it is drawn, about its model's centre: 1 as it is, at most 4 (0.5: an item as a frame or a depot shows it). |

| Function | Signature | What it does |
| --- | --- | --- |
| `set` | `func(dimension: string, x: s32, y: s32, z: s32, parts: list<part>) -> result<_, string>` | Draws `parts` (at most 16) at a block of the mod's own, replacing what it drew there, after the call; none draws nothing there any more. |
| `set-in-phase` | `func(dimension: string, x: s32, y: s32, z: s32, parts: list<part>) -> result<_, string>` | `set`, with each part's `angle` its angle at game time 0 instead of at the tick it was set: parts at one speed turn in step however far apart they were set, as Flywheel turns Create's cogs (angle = time x speed + offset), so their teeth mesh. |

## `block-data`

Access: not granted by any capability.

Capability `net`, client mods: the bytes the mod's server half keeps with `block-entities.set-synced`, at the blocks of the chunks the player's game has, in the dimension the player is in, as of this tick.

### record `synced-block`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |
| `data` | `list<u8>` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `get` | `func(x: s32, y: s32, z: s32) -> option<list<u8>>` | The bytes at a position, none where the server sent none. |
| `in-box` | `func(min-x: s32, min-y: s32, min-z: s32, max-x: s32, max-y: s32, max-z: s32) -> list<synced-block>` | Every position from (min-x, min-y, min-z) to (max-x, max-y, max-z), both ends included, holding bytes: at most 256, nearest the box's minimum corner first. |

## `block-events`

Access: exported by the mod: the game calls it.

What the server calls in a mod's blocks, beside `server-events` (a mod built without this export hears nothing of them).

### record `pos`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `ticking-blocks` | `func() -> list<string>` | The blocks (`mymod:gear`, ids without properties) whose instances the game ticks, asked once after `init`. |
| `block-tick` | `func(dimension: string, pos: pos, state: string)` | A position where the mod keeps data (`block-entities.set`) and a ticking block stands in a loaded chunk, once per server tick (at most 4096 positions per tick, the rest next tick), in `dimension`; `state` is the block's state as `minecraft:oak_log[axis=y]`. Data under any other block is forgotten. |
| `block-used` | `func(player: string, dimension: string, pos: pos, state: string) -> bool` | The player with this id (`players.online`) used (right-clicked) a block of the mod's with an empty hand, in `dimension`; true when the mod handled it (the game then does nothing else). |

## `net`

Access: capability `net`.

Capability `net`: messages between a mod's client half and its server half, as custom payloads on the channel `horizon:mod/<mod id>`. A client mod sends to the server it plays on; a server mod to one player or to all.

| Function | Signature | What it does |
| --- | --- | --- |
| `send` | `func(player: option<string>, data: list<u8>) -> result<_, string>` | Sends up to 32 KiB. A client mod passes `none` (the server it plays on, which must run a mod with the same id). A server mod passes the id of a player (`players.online`), or `none` for every player. |

## `net-events`

Access: exported by the mod: the game calls it.

What a client mod hears on its channel, beside its other exports (a mod built without this export hears nothing).

| Function | Signature | What it does |
| --- | --- | --- |
| `message` | `func(data: list<u8>)` | A message from the mod's server half. |

## `server-net-events`

Access: exported by the mod: the game calls it.

The same for a server mod (a separate interface: the SDK builds client and server mods in one crate, whose exports can't share a name).

| Function | Signature | What it does |
| --- | --- | --- |
| `message` | `func(player: string, data: list<u8>)` | A message from the mod's client half on `player` (their id). |

## `entities`

Access: capability `entities`.

Capability `entities`, server mods only (manifest `api = "0.3"`, experimental): small marker entities a mod places and moves, drawn as an item (a mod's own item included) held where the entity stands. A first step: no hitbox, no AI, no saving; a mod keeps what it needs in `block-entities` or `storage` and places them again after a restart. They exist in the dimension the world is simulating (the overworld at first).

| Function | Signature | What it does |
| --- | --- | --- |
| `spawn` | `func(dimension: string, x: f64, y: f64, z: f64, yaw: f32, item: string) -> result<u32, string>` | Places one at a position, facing `yaw` degrees, drawn as `item` (`mymod:gear`). Returns the mod's own handle for it (at most 1024 live at once). |
| `move-to` | `func(handle: u32, x: f64, y: f64, z: f64, yaw: f32) -> bool` | Moves one of the mod's entities; false for a handle it does not hold. |
| `set-item` | `func(handle: u32, item: string) -> bool` | Changes the item an entity is drawn as. |
| `remove` | `func(handle: u32) -> bool` | Removes one of the mod's entities. |
| `set-slot` | `func(handle: u32, slot: string, item: option<string>) -> bool` | Puts an item in an equipment slot of the entity (`mainhand`, `offhand`, `head`, `chest`, `legs`, `feet`); none empties it. A block's item on the head is drawn as the block, so a mod's own block (`mymod:gear`) can stand in the world as a model that `set-pose` then turns. |
| `set-pose` | `func(handle: u32, part: string, x: f32, y: f32, z: f32) -> bool` | Turns a body part of the entity, in degrees about x, y and z as an armor stand's `Pose` does: `head`, `body`, `left-arm`, `right-arm`, `left-leg` or `right-leg`. Values are wrapped to -360 to 360. |
| `spawn-block` | `func(dimension: string, x: f64, y: f64, z: f64, state: string) -> result<u32, string>` | Places a block state (`mymod:gear[axis=x]`, a vanilla one too) as an entity: drawn full size like a falling block, with no gravity, never landing; the mod moves it with `move-to` (x and z are its centre, y its bottom) and takes it away with `remove`. Players do not collide with it. Counts toward the same 1024 entities. A moving structure is its blocks taken out of the world (`world.set-blocks`) and put back. |
| `rotate` | `func(handle: u32, yaw: f32, pitch: f32) -> bool` | Turns an entity, in degrees: a block entity (`spawn-block`) about its centre, first `pitch` about x then `yaw` about y, so a block with `axis=x` spins about any axis (yaw 90 puts its x along z); a marker (`spawn`) by its yaw only. Sent to every client as the entity's rotation, a byte: angles go to the nearest 360/256 degrees, so a turn of a whole number of those a tick looks even. |
| `set-solid` | `func(handle: u32, solid: bool) -> bool` | Makes a block entity (`spawn-block`) solid for players, as a boat is: they stand on it and walk into it as on its full cube, and a player standing on it rides along as it moves; one it moves into from the side is not pushed. Sent to clients as the entity being silent. |

## `containers`

Access: capability `containers`.

Capability `containers`, server mods only (manifest `api = "0.3"`, experimental): the mod's own blocks that hold items and open as a chest (27 slots) for a player. The game keeps the items in the block's chest entity, saved with the chunk.

### record `item-stack`

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |

### record `hopper-rule`

What hoppers, droppers and other containers' moves may do with one slot of a registered container, as a WorldlyContainer's faces allow (players are held to `set-slot-rule` instead).

| Field | Type | |
| --- | --- | --- |
| `insert` | `bool` | Items may be put in. |
| `extract` | `bool` | Items may be taken out. |
| `max` | `u32` | The most items a move fills the slot to, 1 to 99: 1 takes an item only while the slot is empty. |

| Function | Signature | What it does |
| --- | --- | --- |
| `register` | `func(block: string) -> result<_, string>` | Makes a block of the mod's (`mymod:crate`, no properties) a container: the game gives its positions a chest's slots as the block is placed (by a player or the mod), so hoppers and comparators find them, or when opened or written. At most 64 blocks per mod. |
| `register-sized` | `func(block: string, slots: u32) -> result<_, string>` | `register` with a size: 27 slots opens as a chest, 9 as a dispenser's 3 x 3 and 5 as a hopper's row. A block registered twice keeps its first size. |
| `open` | `func(player: string, dimension: string, x: s32, y: s32, z: s32) -> bool` | Opens the container at a position for the player with this id, after the call; false when the player is not online. |
| `slots` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<list<option<item-stack>>>` | The 27 slots of a container in a loaded chunk, as of the start of this tick; none where there is no container with items yet or none loaded. (Item components are not shown.) |
| `set-slot` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32, stack: option<item-stack>) -> result<_, string>` | Puts a stack in slot 0 to 26 (none empties it), after the call, in a registered container; count 1 to 99. Components of what was there go. |
| `set-name` | `func(dimension: string, x: s32, y: s32, z: s32, name: option<string>) -> result<_, string>` | Names the container at a position (the title its window shows, saved with the chunk as a renamed chest's is); none clears it. After the call, in a registered container; at most 64 characters. |
| `set-slot-rule` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32, allowed: option<list<string>>) -> result<_, string>` | Limits what a player may put in one slot of a registered container, as vanilla's slots do (a furnace's fuel slot): `some(items)` allows only those item ids (`minecraft:coal`), an empty list nothing (an output or display slot players can only take from); none lifts the rule. Hoppers and the mod itself are not held to it. After the call; not saved, so the mod sets its rules again after a load. |
| `set-hopper-rule` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32, rule: option<hopper-rule>) -> result<_, string>` | Sets one slot's rule for hoppers (none: vanilla's), after the call. Not saved, as `set-slot-rule` is not; gone when the block changes. |

## `redstone`

Access: capability `redstone`.

Capability `redstone`, server mods only (manifest `api = "0.3"`, experimental): the redstone signal reaching the mod's own blocks.

| Function | Signature | What it does |
| --- | --- | --- |
| `power` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<u8>` | The strongest signal (0 to 15) reaching the block at a position from its neighbours (vanilla's best neighbour signal), as of the start of this tick. Known only for positions where the mod keeps block data (`block-entities.set`) or has a container, in the simulated dimension; none elsewhere. |
| `set-signal` | `func(dimension: string, x: s32, y: s32, z: s32, level: u8) -> result<_, string>` | Makes a block of the mod's give a redstone signal of 0 to 15 to its neighbours, after the call, until the mod sets another: the signal a trapped chest gives (analog, to every side). Only a block declared with the template `minecraft:trapped_chest` in `data/<id>/horizon/block/` gives one; any other block is left as it is. The signal is not saved: the mod sets it again when the world loads. At most 1024 a call. |
| `set-comparator-signal` | `func(dimension: string, x: s32, y: s32, z: s32, level: option<u8>) -> result<_, string>` | The signal (0 to 15) a comparator reads from a block of the mod's (`getAnalogOutputSignal`: how full a depot or a tank is), after the call, until the mod sets another; none gives back vanilla's (a container's fullness). Comparators read a block whose template has an analog output (a chest, barrel, hopper or composter). Not saved; gone when the block changes. At most 1024 a call, with `set-signal`'s. |

## `items`

Access: capability `items`.

Capability `items`, server mods only (manifest `api = "0.3"`, experimental): items a mod's blocks put into the world.

### record `dropped-item`

An item entity lying in the world.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The entity's id, for `take`; it lasts while the entity does. |
| `item` | `string` |  |
| `count` | `u32` |  |
| `x` | `f64` |  |
| `y` | `f64` |  |
| `z` | `f64` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `drop` | `func(dimension: string, x: f64, y: f64, z: f64, item: string, count: u32) -> result<_, string>` | Drops a stack as an item entity at a position (blocks, so x + 0.5 is a block's middle), after the call, in the dimension the world simulates; count 1 to 99. At most 64 drops a call. The item is any item id, a mod's own too. |
| `nearby` | `func(dimension: string, x: f64, y: f64, z: f64, radius: f32) -> list<dropped-item>` | The item entities within `radius` blocks (at most 16) of a point, as of the start of this tick, nearest first, at most 256; only in the dimension the world is simulating. |
| `take` | `func(id: u32) -> bool` | Removes an item entity (one `nearby` showed) after the call, to carry it into a machine; false when over the 256 takes a call. |
| `push` | `func(id: u32, x: f64, y: f64, z: f64) -> bool` | Adds a velocity, in blocks per tick, to an item entity (one `nearby` showed) after the call: a belt moving what lies on it. Clamped as `players.push` is; counts toward the same 256 a call as `take`. |

## `effects`

Access: capability `effects`.

Capability `effects`, server mods only (manifest `api = "0.3"`, experimental): sounds and vanilla's level events (particles) that every player near the block sees and hears, LAN guests included.

| Function | Signature | What it does |
| --- | --- | --- |
| `play-sound` | `func(dimension: string, x: s32, y: s32, z: s32, event: string, volume: f32, pitch: f32) -> result<_, string>` | Plays a sound event (`minecraft:block.anvil.use`, or one a mod's pack adds) at a block's position, after the call; volume and pitch are clamped to 0 to 4 and 0.5 to 2. At most 64 effects a call. |
| `level-event` | `func(dimension: string, x: s32, y: s32, z: s32, id: s32, data: s32) -> result<_, string>` | Vanilla's `levelEvent` at a block: 2000 smoke (data 0 to 8, the direction), 2004 spawner flames, 2005 bone meal particles (data 0 to 15, how many) and 2009 water evaporating. Others are refused. |
| `particles` | `func(dimension: string, x: f64, y: f64, z: f64, particle: string, delta: tuple<f32, f32, f32>, speed: f32, count: u32) -> result<_, string>` | Particles as `/particle` sends them (`ServerLevel.sendParticles`), after the call, to the players within 32 blocks: `particle` as the command writes it, with no spaces (`minecraft:flame`, `minecraft:item{item:"minecraft:iron_ingot"}`, `minecraft:block{block_state:"minecraft:stone"}`); `count` of them (0: one moving by `speed` times `delta`) spread by `delta`. At most 1000 a call, counting toward the same 64 effects. |
| `destroy-progress` | `func(dimension: string, x: s32, y: s32, z: s32, breaker: u32, stage: s8) -> result<_, string>` | The cracks of a block being broken, as a player's mining shows them (`Level.destroyBlockProgress`), to the players within 64 blocks: `stage` 0 to 9, or -1 to take them away. `breaker` is the mod's own number for what breaks it (a drill's), one crack each. |

## `neighbor-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod when a block next to one of its blocks changes (capability `block-entities`). The blocks asked about are those where the mod keeps data (`block-entities.set`).

| Function | Signature | What it does |
| --- | --- | --- |
| `neighbor-changed` | `func(dimension: string, x: s32, y: s32, z: s32, state: string, from-x: s32, from-y: s32, from-z: s32, from-state: string)` | The block at (from-x, from-y, from-z) became `from-state`; the mod's block next to it, at (x, y, z) in `state`, is told, in the tick the change was noticed (the start of the next server tick). At most 1024 calls a tick; the rest are dropped. |

## `files`

Access: not granted by any capability.

The files a mod carries in its own package (`data/` and `assets/`), read only: every mod has them, with no capability. A server mod keeps its own recipe types there as JSON, a machine reads them at its start.

| Function | Signature | What it does |
| --- | --- | --- |
| `read` | `func(path: string) -> option<list<u8>>` | A file of the mod's package by its path in it (`data/mymod/press.json`), at most 1 MiB; none where there is no such file. Only `data/` and `assets/` are readable. |
| `paths` | `func(prefix: string) -> list<string>` | The paths in the package that start with `prefix` (`data/mymod/recipes/`), in order, at most 1024. |

## `attachments`

Access: not granted by any capability.

Data a server mod attaches to a player, a dimension or a chunk, saved with the world beside its block data (as NeoForge's data attachments): a player's mana, a dimension's weather machine, a chunk's pollution. Every server mod has it, with no capability; each mod sees only its own.

### record `chunk`

| Field | Type | |
| --- | --- | --- |
| `dimension` | `string` |  |
| `x` | `s32` | Chunk coordinates: a block's x and z shifted right by 4. |
| `z` | `s32` |  |

### variant `target`

| Case | Payload | |
| --- | --- | --- |
| `player` | `string` | A player, by UUID, whether online or not. |
| `dimension` | `string` | A dimension, `minecraft:overworld`. |
| `chunk` | `chunk` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `get` | `func(target: target, key: string) -> option<list<u8>>` | The bytes under `key` (at most 64 characters) on `target`. |
| `set` | `func(target: target, key: string, data: list<u8>) -> result<_, string>` | Keeps `data` (at most 64 KiB) under `key` on `target`; at most 65536 values a mod. |
| `remove` | `func(target: target, key: string)` | Forgets the value under `key` on `target`. |
| `keys` | `func(target: target) -> list<string>` | The keys the mod keeps on `target`. |

## `transfer`

Access: capability `transfer`.

Capability `transfer`, server mods only (experimental): moving items, fluids and energy between blocks, as NeoForge's capabilities (`IItemHandler`, `IFluidHandler`, `IEnergyStorage`) do between mods. Items go in and out of any inventory there is: chests, barrels, shulker boxes, hoppers, dispensers, droppers, mod containers, and furnaces by face as a hopper reaches them. Fluid tanks and batteries are the mods' own: a mod puts one on its block and every mod fills and drains it. What is moved is moved at once, during the call; `simulate` only asks.

### record `item-stack`

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |

### enum `face`

The face of the block reached through, as a hopper or pipe touches it; none for the block as a whole.

| Case | |
| --- | --- |
| `down` |  |
| `up` |  |
| `north` |  |
| `south` |  |
| `west` |  |
| `east` |  |

### record `fluid-stack`

| Field | Type | |
| --- | --- | --- |
| `fluid` | `string` | `minecraft:water`, `minecraft:lava` or a mod's own fluid id. |
| `amount` | `u32` | In millibuckets: 1000 is a bucket. |

### record `tank`

| Field | Type | |
| --- | --- | --- |
| `content` | `option<fluid-stack>` |  |
| `capacity` | `u32` |  |

### record `battery`

| Field | Type | |
| --- | --- | --- |
| `stored` | `u64` |  |
| `capacity` | `u64` |  |

| Function | Signature | What it does |
| --- | --- | --- |
| `item-slots` | `func(dimension: string, x: s32, y: s32, z: s32, face: option<face>) -> option<list<option<item-stack>>>` | The slots reachable through `face` of the inventory at a position in the simulated dimension; none where there is none. |
| `insert-item` | `func(dimension: string, x: s32, y: s32, z: s32, face: option<face>, stack: item-stack, simulate: bool) -> u32` | Puts a stack in (merged with like stacks first, then into empty slots): how many did not fit. |
| `extract-item` | `func(dimension: string, x: s32, y: s32, z: s32, face: option<face>, count: u32, simulate: bool) -> option<item-stack>` | Takes up to `count` of the first stack reachable: what was taken. |
| `add-tank` | `func(dimension: string, x: s32, y: s32, z: s32, capacity: u32) -> result<_, string>` | Gives one of the mod's own blocks a tank of `capacity` millibuckets (keeping what it holds when it had one). Saved with the world. |
| `get-tank` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<tank>` | The tank on a block: what it holds and how much it can. |
| `fill` | `func(dimension: string, x: s32, y: s32, z: s32, stack: fluid-stack, simulate: bool) -> u32` | Pours in (one fluid a tank): how much went in. |
| `drain` | `func(dimension: string, x: s32, y: s32, z: s32, amount: u32, simulate: bool) -> option<fluid-stack>` | Takes out up to `amount`: what came out. |
| `add-battery` | `func(dimension: string, x: s32, y: s32, z: s32, capacity: u64) -> result<_, string>` | Gives one of the mod's own blocks a battery of `capacity` units (any unit the mods agree on, as Forge Energy). Saved with the world. |
| `get-battery` | `func(dimension: string, x: s32, y: s32, z: s32) -> option<battery>` | The battery on a block: what it stores and how much it can. |
| `receive-energy` | `func(dimension: string, x: s32, y: s32, z: s32, amount: u64, simulate: bool) -> u64` | Charges: how much it took. |
| `extract-energy` | `func(dimension: string, x: s32, y: s32, z: s32, amount: u64, simulate: bool) -> u64` | Discharges: how much came out. |
| `remove-storage` | `func(dimension: string, x: s32, y: s32, z: s32)` | Takes the tank and battery off one of the mod's own blocks. |

## `item-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod when a player uses an item (capability `block-entities`, as `block-used`).

### record `block-target`

The block a player clicked with the item.

| Field | Type | |
| --- | --- | --- |
| `dimension` | `string` |  |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `z` | `s32` |  |
| `face` | `u8` | The face clicked: 0 down, 1 up, 2 north, 3 south, 4 west, 5 east. |
| `state` | `string` | `mymod:gear[axis=y]`. |

| Function | Signature | What it does |
| --- | --- | --- |
| `item-used` | `func(player: string, item: string, sneaking: bool, target: option<block-target>) -> bool` | A player right-clicked with `item` in the main hand, on a block or in the air, sneaking or not, before anything vanilla does with it: offered to every server mod, those whose namespace the item or the block is in first, until one takes it: true takes the click (a wrench turning a block, a filter set on a machine), and nothing else happens. |

## `peers`

Access: not granted by any capability.

Messages between server mods: one mod's API for others (an addon asking a tech mod for a machine's speed, a mod telling another its block changed). Every server mod has it, with no capability.

| Function | Signature | What it does |
| --- | --- | --- |
| `send` | `func(mod: string, data: list<u8>) -> result<_, string>` | Sends `data` (at most 64 KiB) to the running server mod with this id; it hears it as `peer-events.message` at the start of the next server tick. At most 64 messages a call. |
| `running` | `func(mod: string) -> bool` | Whether a server mod with this id runs now. |

## `peer-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod a peer wrote to.

| Function | Signature | What it does |
| --- | --- | --- |
| `message` | `func(from: string, data: list<u8>)` | A message from the server mod `from` (its id). |

## `menus`

Access: capability `menus`.

Capability `menus`, server mods only (experimental): a container of the mod's own (`containers.register`) opened with a screen of its own, as NeoForge's menus and screens are: where each slot is drawn, labels, progress bars fed by the menu's data values (`ContainerData`) and buttons the mod hears (`clickMenuButton`). Horizon's clients draw it; a vanilla client sees the container as a chest. Positions are in GUI pixels from the screen's top left corner, as a container screen's are.

### record `slot-pos`

A slot of the menu and where it is drawn: slots 0 up to the container's size are the container's, then 27 of the player's main inventory and 9 of the hotbar.

| Field | Type | |
| --- | --- | --- |
| `slot` | `u32` |  |
| `x` | `s32` |  |
| `y` | `s32` |  |

### record `label`

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `text` | `string` |  |
| `color` | `u32` | 0xRRGGBB. |

### record `progress`

A bar filled as data value `value` is to data value `max` (a furnace's arrow, a machine's progress, a tank's level).

| Field | Type | |
| --- | --- | --- |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `width` | `u32` |  |
| `height` | `u32` |  |
| `value` | `u32` |  |
| `max` | `u32` |  |
| `vertical` | `bool` | Fills upward instead of rightward. |
| `color` | `u32` | 0xRRGGBB of the filled part and of the rest. |
| `background` | `u32` |  |

### record `button`

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | What `menu-events.button` hears. |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `width` | `u32` |  |
| `height` | `u32` |  |
| `label` | `string` |  |

### record `text-field`

A box the player types into: what they typed reaches `menu-events.text` when they press Enter and when the screen closes.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` |  |
| `x` | `s32` |  |
| `y` | `s32` |  |
| `width` | `u32` |  |
| `value` | `string` | What it holds when the screen opens. |
| `max-length` | `u32` | At most 256. |

### variant `widget`

| Case | Payload | |
| --- | --- | --- |
| `label` | `label` |  |
| `progress` | `progress` |  |
| `button` | `button` |  |
| `text-field` | `text-field` |  |

### record `layout`

| Field | Type | |
| --- | --- | --- |
| `width` | `u32` | The screen's size, at most 512 x 512. |
| `height` | `u32` |  |
| `slots` | `list<slot-pos>` | Where the menu's slots are drawn (at most 72); a slot left out is not drawn and cannot be clicked. |
| `widgets` | `list<widget>` | At most 64. |

| Function | Signature | What it does |
| --- | --- | --- |
| `open` | `func(player: string, dimension: string, x: s32, y: s32, z: s32, layout: layout) -> bool` | Opens the container at a position (one of the mod's registered containers) for a player, after the call, with this screen. False when the player is not online or the layout is too large. |
| `set-data` | `func(player: string, index: u32, value: s32) -> bool` | Sets data value `index` (0 to 31) of the screen the player has open from this mod, sent to their client; the bars read it. |
| `open-panel` | `func(player: string, layout: layout) -> bool` | Opens a screen of the mod's own with no block behind it (a toolbox, a book, a schedule), after the call: the layout's widgets on a panel over the dimmed world; its slots are not drawn. Esc closes it (`menu-events.closed`). False when the player is not online. |
| `close` | `func(player: string)` | Closes the screen the mod opened for a player, after the call. |

## `menu-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod about its screens.

| Function | Signature | What it does |
| --- | --- | --- |
| `button` | `func(player: string, id: u32)` | The player pressed button `id` on the mod's screen they have open. |
| `text` | `func(player: string, id: u32, value: string)` | What the player typed into text field `id` (on Enter, and for every field as the screen closes). |
| `closed` | `func(player: string)` | The player closed the mod's screen (Esc, or leaving its container). |

## `console`

Access: capability `console`.

Capability `console`, server mods only (experimental): any command run as the server's console, with an operator's rights, after the call, as a command block or a datapack function would: giving and clearing items, effects, experience, teleports, summons. Players see no feedback; the server's log has a line per batch. The SDK's `player_ops` builds the usual ones.

| Function | Signature | What it does |
| --- | --- | --- |
| `run` | `func(line: string) -> result<_, string>` | Runs `line` (no leading slash needed) at the end of this tick. At most 256 commands a call; a line that does not parse fails then, not now. |

## `stacks`

Access: capability `stacks`.

Capability `stacks`, server mods only (experimental): what an item stack carries beyond its item and count, as NeoForge's data components: damage, a custom name, enchantments, and the mod's own bytes, kept in vanilla's `minecraft:custom_data` under `horizon:<mod id>` so they stay with the stack wherever it goes (a filter's setting, a tool's charge). Each mod sees and writes only its own bytes.

### record `stack`

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |
| `damage` | `u32` | Durability used up. |
| `name` | `option<string>` | `minecraft:custom_name` as plain text. |
| `enchantments` | `list<tuple<string, u32>>` | `minecraft:fortune`, 3. |
| `data` | `option<list<u8>>` | The mod's own bytes on it, at most 16 KiB. |

| Function | Signature | What it does |
| --- | --- | --- |
| `player-stack` | `func(player: string, slot: u32) -> option<stack>` | A slot of a player's inventory (0 to 40, as `players.inventory`) as of the start of this tick. |
| `set-player-stack` | `func(player: string, slot: u32, stack: option<stack>) -> bool` | Puts a stack in a player's slot (none empties it), after the call. What other mods keep on the stack there stays when the item does. |
| `container-stack` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32) -> option<stack>` | A slot of an inventory (a chest, a mod container, a furnace) in the simulated dimension, as it is now. |
| `set-container-stack` | `func(dimension: string, x: s32, y: s32, z: s32, slot: u32, stack: option<stack>) -> bool` | Puts a stack in an inventory's slot at once (none empties it); false when there is no such slot. |

## `recipe-manager`

Access: not granted by any capability.

The game's recipes, as vanilla's data and content mods' `data/` give them (NeoForge's `RecipeManager`): a machine of a mod's crafts, cooks or cuts as vanilla's stations do (a fan smelting what blows past it, a crafter of the mod's), and recipe types of the mod's own are read from every content mod's `data/<namespace>/recipe/` files, so other mods add recipes to its machines as data. Every server mod has it, with no capability.

### record `item-count`

An item and how many of it.

| Field | Type | |
| --- | --- | --- |
| `item` | `string` |  |
| `count` | `u32` |  |

### record `crafted`

A crafting recipe a grid matched.

| Field | Type | |
| --- | --- | --- |
| `recipe` | `string` | The recipe's id, `minecraft:oak_planks`. |
| `output` | `item-count` |  |
| `remainders` | `list<option<item-count>>` | What stays in each cell of the grid once it is crafted (a bucket from a milk bucket), row by row as the grid. |

### record `cooked`

A cooking recipe.

| Field | Type | |
| --- | --- | --- |
| `recipe` | `string` |  |
| `output` | `item-count` |  |
| `experience` | `f32` |  |
| `time` | `u32` | Ticks it takes. |

### record `recipe-file`

A recipe file of a type the game does not run itself.

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `gears:pressing/iron`: the namespace, then the path under `recipe/` without `.json`. |
| `json` | `string` | The file's JSON as written. |

| Function | Signature | What it does |
| --- | --- | --- |
| `craft` | `func(grid: list<option<item-count>>, width: u32) -> option<crafted>` | The crafting table's recipe for a `width` by `width` grid (1 to 3, cells row by row), matched as vanilla matches it (a shaped recipe anywhere in the grid, mirrored too). |
| `cook` | `func(kind: string, item: string) -> option<cooked>` | The cooking recipe of `kind` (`minecraft:smelting`, `minecraft:blasting`, `minecraft:smoking` or `minecraft:campfire_cooking`) taking `item`. |
| `stonecut` | `func(item: string) -> list<tuple<string, item-count>>` | The stonecutter's recipes taking `item`, in its order: each recipe's id and result. |
| `of-type` | `func(kind: string) -> list<recipe-file>` | Every recipe file of type `kind` (`gears:pressing`) the content mods carry, by id; empty for vanilla's own types (`minecraft:`). |
| `test-ingredient` | `func(ingredient: string, item: string) -> bool` | Whether an ingredient, as recipe files write one (`minecraft:iron_ingot`, `#minecraft:logs`, or a JSON list of them), takes `item`. |

## `entity-data`

Access: not granted by any capability.

One of a server mod's entities of its own types (`entity-types`), as its events and reads give it. Types only: every server mod has it.

### record `entity`

One of the mod's entities, as of the start of this tick.

| Field | Type | |
| --- | --- | --- |
| `id` | `u32` | The game's id for it, the same after a restart. |
| `kind` | `string` | Its type, `mymod:roller`. |
| `dimension` | `string` |  |
| `x` | `f64` | Its bottom centre. |
| `y` | `f64` |  |
| `z` | `f64` |  |
| `yaw` | `f32` |  |
| `vx` | `f64` | Blocks a tick. |
| `vy` | `f64` |  |
| `vz` | `f64` |  |
| `on-ground` | `bool` |  |
| `health` | `f32` |  |
| `data` | `list<u8>` | The mod's own bytes on it (`set-data`). |


## `entity-types`

Access: not granted by any capability.

Capability `entities`, server mods only (experimental): entity types of the mod's own, as NeoForge's `EntityType`s. The game keeps each one: it falls and slides as the type says, takes hits, and is saved with the world with the mod's own bytes; the mod gives it its behaviour each tick (`entity-events`). Every client sees it as an invisible armor stand wearing the type's model on its head (a block's item is drawn as the block), with the armor stand's hitbox.

### type `entity`

`type entity = entity`

### record `entity-type`

| Field | Type | |
| --- | --- | --- |
| `id` | `string` | `mymod:roller`, in one of the mod's namespaces. |
| `model` | `string` | The item its model is, worn on its head: `mymod:roller`. |
| `small` | `bool` | The small armor stand's size (0.5 by 0.9875) instead of the full one (0.5 by 1.975). |
| `gravity` | `bool` | Falls as a mob does (0.08 a tick); false: it floats and moves only by its velocity. |
| `health` | `f32` | The health it starts with, at most 1024; 0: no hit hurts it. |

| Function | Signature | What it does |
| --- | --- | --- |
| `register` | `func(def: entity-type) -> result<_, string>` | Registers a type, at init or later; again replaces it for entities spawned from then on. At most 64 types. |
| `spawn` | `func(kind: string, dimension: string, x: f64, y: f64, z: f64, yaw: f32) -> result<u32, string>` | Spawns an entity of a registered type, after the call; its id now. At most 256 of the mod's at once. |
| `get` | `func(id: u32) -> option<entity>` | One of the mod's entities as of this tick's start; none for one spawned this tick or gone. |
| `all` | `func() -> list<entity>` | Every entity of the mod's, as of this tick's start. |
| `set-velocity` | `func(id: u32, vx: f64, vy: f64, vz: f64) -> bool` | Sets its velocity, blocks a tick (each at most 10), after the call. On the ground it slows as a mob does (0.546 a tick), in the air by 0.91. |
| `move-to` | `func(id: u32, x: f64, y: f64, z: f64, yaw: f32) -> bool` | Puts it at a position, facing `yaw`, after the call. |
| `set-data` | `func(id: u32, data: list<u8>) -> bool` | Keeps bytes on it (at most 4 KiB), saved with it. |
| `hurt` | `func(id: u32, amount: f32) -> bool` | Takes `amount` off its health, after the call; at 0 it dies (`entity-events.died`). |
| `set-model` | `func(id: u32, item: string) -> bool` | Changes the item its model is. |
| `set-pose` | `func(id: u32, part: string, x: f32, y: f32, z: f32) -> bool` | Turns a part of its model, as `entities.set-pose` does: `head` turns the model. |
| `remove` | `func(id: u32) -> bool` | Removes it, after the call, with no `died`. |

## `entity-events`

Access: exported by the mod: the game calls it.

What the server calls in a server mod for its entity types.

### type `entity`

`type entity = entity`

| Function | Signature | What it does |
| --- | --- | --- |
| `tick` | `func(entities: list<entity>)` | Each server tick, the mod's entities in the dimension the server simulates, after the game moved them: their behaviour. |
| `attacked` | `func(id: u32, player: string, damage: f32) -> bool` | A player hit one with `damage` (their attack's). True takes the hit (no damage); false lets the game take it off its health. |
| `interacted` | `func(id: u32, player: string, item: string)` | A player right-clicked one, `item` in their main hand ("" for none). |
| `died` | `func(id: u32, kind: string)` | Its health reached 0; it is gone after the call. |
