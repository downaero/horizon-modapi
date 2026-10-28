# Horizon mods

Horizon gets its own mod system, written in Rust and run in a sandbox, plus
two ways to bring over what Java players use: native Horizon ports of popular
mods and the data-driven parts of packs and mods. Fabric and NeoForge jars do
**not** run as they are: their code calls and patches Minecraft's Java classes, and
Horizon has none. What carries over is the data and assets inside a jar
([modpacks.md](modpacks.md)).

Mods are **off and absent by default**. A fresh install has no `horizon-mods`
folder, loads no runtime and plays exactly like vanilla 26.3.

| Part | Where | State |
|---|---|---|
| The contract (WIT) | `crates/horizon-mod-sdk/wit/horizon.wit` | 0.1: log, game-read, chat, hud, storage, command, ui; tick, frame, chat and command hooks |
| SDK for mod authors | `crates/horizon-mod-sdk` | 0.1 |
| Example mods | `crates/horizon-mod-sdk/examples/coords-hud`, `examples/predator-missile` | coordinates, facing and time HUD, `/where`, `/coords`; `/missile`, a guided missile from vanilla commands |
| Horizon's official mods | `mods/official/` | the extras moving to the registry: FPS Counter (`fps-overlay`), Keystrokes, Zoom, Minimap; Far View (`far-view`), distant terrain; and Recipes, a recipe viewer |
| Runtime | `crates/horizon-mods` | wasmtime 48 LTS, component model |
| Authoring tool | `crates/cargo-horizon-mod` | `cargo horizon-mod new [--template code\|content] / build / run / test / check`, and the registry's CI (`registry build / review / list`) |
| In the game | `crates/horizon-client/src/game/mods.rs`, Options > Mods | client mods |
| Launcher mod browser (Modrinth, CurseForge with a key) | [mod-browser.md](mod-browser.md) | resource packs, modpacks |
| Registry index (`index.json`, `.hpack`), signatures | `horizon_mods::index`, [api/mod-index.md](api/mod-index.md) | schema 1; launcher and website next |
| Server mods, registries | this page, [Roadmap](#roadmap) | planned |

## Design goals

1. **Safe to install.** A mod is untrusted code. It runs in a WebAssembly
   sandbox with no file, network, process or clock access, a memory cap and a
   time budget per call. It can only do what its capabilities grant, and the
   player sees those before turning it on.
2. **One file for every platform.** The same `.hmod` runs on Windows, macOS
   and Linux, x86-64 and ARM: authors build once, players never meet "wrong
   build for my OS".
3. **Free when unused.** No mod on: no engine, no thread, no compile, one
   branch per hook. Measured below.
4. **Vanilla stays vanilla.** Mods add; they cannot patch the game's code the
   way Mixins do. What they may change is a list of hooks the game calls at
   fixed points, so the parity harnesses keep testing the same game.
5. **Stable for authors.** The contract is a versioned WIT package; a mod
   built for API 0.1 keeps loading on later 0.1.x games.

## How it works

```
 mod crate (Rust)                         Horizon
 ───────────────                          ───────
 impl Mod for MyMod { ... }               horizon-mods::Mods
 export_mod!(MyMod)                         ├─ Engine (cranelift), made on first mod
        │ cargo horizon-mod build           ├─ one Store per mod: 64 MiB, fuel per call
        ▼                                   ├─ Linker: log, game, chat, hud (WIT imports)
 my_mod.hmod = zip {                        └─ calls the events export: init, tick,
   horizon-mod.toml,                             frame, command, chat
   mod.wasm  (a WebAssembly component)
 }
```

- **WebAssembly components.** Mods compile to `wasm32-unknown-unknown`;
  `cargo horizon-mod build` wraps the module into a
  [component](https://component-model.bytecodealliance.org/) for the
  `horizon:mods/horizon-mod` world. The WIT file is the whole API: host
  functions the mod imports (`log`, `game`, `chat`, `hud`) and the `events`
  interface it exports. No WASI: the component cannot even name a file or a
  socket.
- **Capabilities are imports.** Every imported interface except `log` is a
  capability. The manifest declares them; the game reads the component's
  actual imports and refuses a mod whose code imports one its manifest does not
  declare (`uses capability chat, missing from horizon-mod.toml`). So the list
  the player reads is the truth, not a promise.
- **Hooks on request.** `init` returns a `registration`: the commands the mod
  owns and which of `tick`, `frame` and `chat` it wants. The game calls only
  those. A mod that wants nothing per frame costs nothing per frame.
- **Retained HUD.** A mod sets its panel (corner and coloured lines) when it
  changes; the game draws it every frame with the extras' overlay code. Most
  HUD mods need no frame hook at all.
- **Snapshots, not callbacks into the world.** Once per tick (and before a
  command) the game copies what mods may read (player position, look, health,
  food, dimension, world clock) into a snapshot shared by all mods. Mods never
  hold references into the game, so they cannot stall or corrupt it.

### What a mod can do (API 0.1)

| Interface | Capability | Functions |
|---|---|---|
| `log` | always | `write(level, message)`, in the game log as `[mod <id>]` |
| `game` | `game-read` | `get-player()`: x y z, yaw, pitch, health, food, dimension; `get-clock()`: game time, day time; `raycast(distance)`: the first block along the view (fluids pass), up to 1024 blocks and across the distant terrain, with the point hit and the block's id |
| `chat` | `chat` | `show(text)`: a line only the local player sees, after the mod's name (8 per call) |
| `hud` | `hud` | `set-panel(corner, lines)`, `clear()`: up to 16 lines of 256 characters |
| `storage` | `storage` | `get(key)`, `set(key, value)`, `delete(key)`: a per-mod folder of up to 64 files, 4 MiB total, at most 1 MiB written per call |
| `command` | `command` | `run(line)`: a vanilla command run as the player, with the player's permissions, after the hook returns (8 per call); mods' own commands are not run. `run-quiet(lines)`: up to 256 a call, the same rights, nothing in the chat (single player and Horizon's servers run them 64 a tick and log one line a batch; elsewhere they go as chat commands and their feedback shows) |
| `containers` | `containers` | server mods: `register`, `open`, `slots`, `set-slot` for the mod's own blocks that hold items and open as a chest ([below](#capability-containers)). **Experimental:** `experimental = ["containers"]` |
| `console` | `console` | server mods: any command as the server's console, with an operator's rights ([below](#players-inventories-and-the-console)). **Experimental:** `experimental = ["console"]` |
| `stacks` | `stacks` | server mods: damage, names, enchantments and the mod's own bytes on item stacks ([below](#capability-stacks-data-on-items)). **Experimental:** `experimental = ["stacks"]` |
| `menus` | `menus` | server mods: screens of the mod's own for its containers: slots where it wants them, labels, progress bars, buttons ([below](#capability-menus-screens-of-the-mods-own)). **Experimental:** `experimental = ["menus"]` |
| `transfer` | `transfer` | server mods: items into and out of any inventory, fluid tanks and batteries on the mod's blocks ([below](#capability-transfer-items-fluids-and-energy-between-blocks)). **Experimental:** `experimental = ["transfer"]` |
| `effects` | `effects` | server mods: `play-sound` and `level-event` at the mod's blocks ([below](#capability-effects)). **Experimental:** `experimental = ["effects"]` |
| `items` | `items` | server mods: `drop` puts item entities into the world ([below](#capability-items)). **Experimental:** `experimental = ["items"]` |
| `redstone` | `redstone` | server mods: `power` reads the redstone signal reaching the mod's own blocks ([below](#capability-redstone)). **Experimental:** `experimental = ["redstone"]` |
| `entities` | `entities` | server mods: `spawn`, `move-to`, `set-item`, `remove` of small marker entities drawn as an item ([below](#capability-entities)). **Experimental:** `experimental = ["entities"]` |
| `net` | `net` | `send(player, data)`: messages between a mod's client half and its server half ([below](#capability-net)). **Experimental:** `experimental = ["net"]` |
| `block-entities` | `block-entities` | server mods: `get`, `set`, `remove` of bytes at the positions of the mod's own blocks, and the `block-tick` and `block-used` hooks ([below](#server-mods-api-03-experimental)). **Experimental:** `experimental = ["block-entities"]` |
| `ui` | `ui` | `open-screen`, `close-screen`, `set-settings`: a screen the mod owns and a page in Options > Mods, of text, button, item and list widgets the game lays out and draws with its own menus' code; `browser.set-items`, `browser.poll`: a recipe viewer's panel of items right of the inventory and container screens (not the creative one), which the game pages and draws with its own slots and search box, and the mod hears its clicks, its searches and the mod's own keys (`input.register-key`) pressed over any item on the screen; `browser.set-recipes`: recipes in the panel's place (crafting grids, cooking, stonecutting, smithing and brewing layouts, their slots cycling through their stacks, a note and an action button each), paged by the game, with clicks on their slots, action buttons, page turns and a back button. **Experimental:** the manifest needs `experimental = ["ui"]` |
| `render` | `render` | `set-hud(shapes)`: up to 1024 fills, four-cornered shapes, GUI sprites, items and text, kept on the HUD (over the hotbar, under chat and screens, hidden with F1) until replaced; `set-corner(corner, w, h, shapes)`: a box the game places in a corner, stacked with its overlays and other panels, as `hud`'s panels are; `text-width(text)`; `frame-stats()`: the game's FPS, min and 1% low; `gui-size()`; `shake(strength, seconds)`: the view turns, the player doesn't, scaled by Distortion Effects; `set-zoom(zoom)`: the field of view divided by up to 10, eased or at once, with slower turning, while playing and not through a spyglass (the strongest of several mods' zooms counts); `set-minimap(minimap)`: a map of the world around the player in a corner, as a map item colours it (the floor underground and under roofs), square or round, turning with the player or north up, with the player's arrow; the game keeps its tiles within its own frame budget (one map at a time, the first loaded mod's); `set-far-terrain(far-terrain)`: the world past the render distance as distant terrain, out to 64 to 1024 chunks, built by the game's own workers (the player's LOD Distance, when on, wins; the first loaded mod's). The game's own HUD renderer draws them: a mod never supplies shader code. **Experimental:** `experimental = ["render"]` |
| `input` | `input` | `claim-items(ids)`: the use button on those items is the mod's (`use-start`, `use-stop` with the ticks held; the game doesn't place, eat or swing); `register-key(name, label, key)`: up to 16 keys (`key.keyboard.r`), heard while playing as `key-down` / `key-up` (the game's own mapping on that key still runs; a screen opening lets held keys up), mouse buttons as `key.mouse.4`. The player rebinds them on the mod's page in Options > Mods, where the game lists them first as Key Binds rows (click, then press a key or mouse button; Esc binds none), even for a mod without `ui`; the choice is kept in `horizon-mods.txt` (`key.<mod>.<name>:key.keyboard.z`) and wins over `register-key`'s default; `claim-wheel(on)`: the wheel's steps become `wheel` events and the hotbar doesn't move (the one input a mod takes from the game; a mod's keys never do); `held()`; `mapping(name)`: a movement or click mapping's key label and whether it is held; `clicks-per-second()`; `poll()` returns the events, at most 256 kept. **Experimental:** `experimental = ["input"]` |
| `recipes` | `recipes` | `items()`: every item in the registry's order, with its name in the player's language and its tags; `all()`: every recipe (crafting, smelting, blasting, smoking, campfire, stonecutting, smithing, brewing), its slots as the stacks that fit each, its results, cooking time and experience, and the recipe book's id for it; `known()`: the book ids the player's recipe book holds; `place(book-id, all)`: the recipe book's click on a recipe, in the open inventory's or crafting table's grid (refused for a recipe the player's book doesn't hold, as vanilla's server refuses it); `give(stack)`: in creative mode, the stack into the player's inventory, as from the creative inventory. The recipes are the world's in single player and on Horizon's servers, vanilla 26.3's on any other server (a client is sent no others), while `known` follows that server's book. The game makes the tables once, when a mod first reads them. **Experimental:** `experimental = ["recipes"]` |

| Hook (export) | When |
|---|---|
| `init() -> result<registration, string>` | once, after loading; an error keeps the mod off and is shown |
| `tick(tick)` | each game tick while in a world (20 per second) |
| `frame(seconds)` | each rendered frame |
| `command(name, args) -> result<option<string>, string>` | `/name args` typed in chat, for a command the mod registered |
| `chat(text)` | each chat line the player receives, as plain text |

Commands: vanilla's command names win. A mod asking for `/give` does not get
it (logged), and two mods cannot share a name (the first one loaded keeps it).
The chat box colours and Tab-completes a mod's commands like vanilla's, for
every player whatever their permission level, and takes any text after the
name (the mod reads it as `args`; API 0.1 has no way to declare argument types).

### Planned interfaces

Each is a new capability, added in a 0.x minor version so older mods keep
loading. Ordered by what the first native ports need:

| Interface | For | Notes |
|---|---|---|
| `keys` | keybinds | registered binds appear in Controls; press/release events |
| `target` | the crosshair's block or entity, with its state and NBT summary | Jade/WTHIT-style tooltips |
| `inventory-read` | the player's inventory, held item, open container | AppleSkin, recipe viewers |
| `world-read` | blocks and biomes around the player, within the render distance | minimaps, schematics |
| `render` (world) | world-space lines, boxes and billboards, beside the HUD shapes `render` has | Litematica-style placement, waypoints |
| `registries` | new blocks, items, entities and recipes, mostly data: JSON models, textures, loot and recipes in the `.hmod`'s `assets/` and `data/` | server side; see [Multiplayer](#multiplayer) |
| `server` | world events (entity spawn/death), game rules, world read/write, typed server commands | after [server mods 0.3](#server-mods-api-03-experimental)'s players, chat, blocks and commands |
| `net` | a mod's own client↔server channel (custom payloads) | only between the two halves of the same mod |

There is no file, socket, process or raw-input interface, on purpose.

## Choosing the runtime

| Option | Sandbox | One file for all OSes | Typed API | Cost | Verdict |
|---|---|---|---|---|---|
| **wasmtime + component model** | yes: memory, fuel, no imports but ours | yes | WIT → Rust on both sides | 85 ns per empty hook call; 50 ms to compile a 64 KB mod on load | **chosen** |
| wasmtime, core modules with a hand ABI | yes | yes | no: pointers and lengths by hand | a few ns less per call | not worth the fragile ABI |
| wasmi (interpreter) | yes | yes | no components | no compile time, execution ~10-50x slower than cranelift code | kept in mind for a platform without JIT |
| Lua (mlua) or another script | partial (strip the stdlib, hope) | yes | no | fast to write | not Rust; weak sandbox |
| native `cdylib` plugins | none | no: per OS and arch, unstable Rust ABI | yes | none | unsafe for players |
| an embedded JVM | none per mod | yes | Java | 100+ MB, seconds of startup | see the research |

wasmtime 48 is a long-term-support line (security fixes until August 2028)
and needs Rust 1.95. It reads WebAssembly with the same wasm-tools release
(0.254) as the `wit-component` that packs mods and the SDK's `wit-bindgen`, so
the build carries one copy of them. Only its `runtime`, `cranelift`,
`component-model` and `std` features are on: no WASI, no async, no GC
proposal, no profiling. Its cranelift backend generates native code for x86-64
and ARM64 on Windows, macOS and Linux, which is why one `.hmod` serves them
all.

The game links it only with its `mods` feature: every release build turns it
on (`release.yml`), and a build from source needs
`cargo build --release -p horizon-client --features mods` to run code mods.
Without it (the tests and probes) wasmtime and cranelift aren't built, a
quarter of a clean build; content mods still load, and the game logs why the
code mods don't start.

### Performance

Measured on the Mac mini (M4 Pro), release build, `cargo test -p
horizon-mods --features runtime,pack --release --test sandbox -- --ignored --nocapture`:

| | per call |
|---|---|
| tick with no mod loaded | 1.2 ns (one check) |
| empty `tick` hook (call into the sandbox and back, fuel on) | 85.5 ns |
| empty `frame` hook | 87.5 ns |
| coords-hud's `tick` (reads the player and clock, formats 3 lines, compares) | 2.3 µs |
| loading coords-hud (read, compile with cranelift, instantiate, `init`) | 50 ms, once |

In the game, same world and view, `horizon-ctl bench 10` three times each
(4 other sessions shared the machine, so the tails are noisy; the median is
the stable figure):

| | median frame | mean FPS |
|---|---|---|
| no `horizon-mods` folder | 2.62-2.71 ms | 149-163 |
| coords-hud on | 2.72-2.78 ms | 151-254 |

At 20 ticks per second coords-hud costs 46 µs per second of game time: well
under 0.01 ms per frame. The frame-time medians differ by less than the
run-to-run spread.

**Budgets.** Each call gets fuel (about one unit per WebAssembly instruction):
2 billion for `init`, 50 million for `tick`, 10 million for `frame`, 100
million for a command or chat line. A runaway loop ends in a fraction of a
second (the sandbox test's endless `/spin` is cut well within its 5 s limit)
instead of hanging the game. The target budgets a mod should stay under are
tighter: **1 ms per tick and 0.1 ms per frame**. The runtime keeps each mod's
average call time (`Entry::cost`); the Mods page will show it, and a mod over
budget for long will be flagged, then paused.

### Crash isolation

A trap (panic, out-of-bounds access, `unreachable`), running out of fuel or
hitting the 64 MiB memory cap stops **that mod only**. The game says so in
chat (`Mod Test Guest stopped: it took too long (an endless loop?)`), marks it
on the Mods page and carries on; the other mods keep running. Host functions
clip what a mod hands back (chat lines per call, HUD lines and lengths). Tests:
`crates/horizon-mods/tests/sandbox.rs` (an endless loop, a panic and a 1 GiB
allocation, next to a healthy mod; `cargo test -p horizon-mods --features
runtime,pack`).

## Playing with mods

1. Put `.hmod` files (or mod folders holding `horizon-mod.toml` and
   `mod.wasm`) in `horizon-mods/` beside `options.txt` (in the launcher's game
   folder).
2. Options > **Mods** lists them with what each may do ("May see your
   position, health and the time, draw text on your screen, ...") and a
   switch. They start off; the choice is kept in `horizon-mods.txt`
   (`mod.<id>:true`).
3. A mod that failed to load or stopped shows why, in red, on its row.

Two turned-on mods never share an `id` while running (it is also capability
`storage`'s directory): the one `find` lists first starts, the other stays
off with the reason on its row.

Benchmarks (`--benchmark`) never load mods.

## Making a mod

```sh
rustup target add wasm32-unknown-unknown
cargo install --git https://github.com/horizon-rust/horizon cargo-horizon-mod
cargo horizon-mod new my-mod        # Cargo.toml, horizon-mod.toml, src/lib.rs, tests/hello.toml
                                    # (--template content: a content mod, no code)
cd my-mod
cargo horizon-mod run               # build, then start the game with it
cargo horizon-mod build             # again after an edit: the running game reloads it
cargo horizon-mod test              # build, then run tests/*.toml with no game
```

`run` starts `horizon-client` (on `PATH`, or `--game PATH`, or
`HORIZON_CLIENT`) with `--dev-mod target/horizon-mod/<id>.hmod`. A dev mod is
always on, and the game checks its file once a second: rebuild and it restarts
with "Mods reloaded" in chat. Its in-memory state starts over; whatever it
saved with capability `storage` survives the reload.

Before you submit it to the registry, run what its CI runs:

```sh
cargo horizon-mod check target/horizon-mod/my_mod.hmod   # --json for the report as JSON
```

It reads the manifest and content, refuses imports the capabilities don't
cover, then plays the mod in the sandbox for 40 ticks, 120 frames, a chat line
and each of its commands: it must start and never trap or overrun. It checks a
`.hpack` and a resource pack `.zip` too, and a signature with `--key <public
key> --signature <file>.minisig`.

### Testing a mod

`cargo horizon-mod test` builds the mod, then runs each `tests/<name>.toml` in
the sandbox the game uses, with no game and no window: the test plays the
game's part (the player and the clock the mod reads, ticks, frames, chat lines,
commands; for a server mod, players joining, chatting, typing commands,
breaking and placing blocks and leaving) and checks what the mod showed. A script fails at its first wrong
step, with what it expected and what it got; the command fails if any script
does. `--debug` builds without optimizing, `--hmod <file>` tests a `.hmod`
already built, and a name runs only the scripts whose path holds it. Each
script gets a scratch copy of the mod, so its `storage` starts empty and is
removed afterwards.

```toml
# tests/panel.toml (examples/coords-hud)
[player]                 # where the script starts; unset fields keep the defaults
x = 12.5                 # (0.5 64 0.5, yaw 0, 20 health and food, the Overworld)
z = -3.25
yaw = 180.0

[clock]                  # day-time 0 is 6:00 on day 1; each tick adds one
day-time = 29999

[[step]]
ticks = 1
expect.panel = ["XYZ 12.5 / 64.0 / -3.2", "Facing North (-Z)", "Day 2, 12:00"]
expect.corner = "top-left"

[[step]]
command = "coords"
expect.feedback = "Coordinates panel hidden"
```

A step's parts happen in this order:

| Key | Does |
| --- | --- |
| `reload = true` | stops the mod and starts it again, as a rebuild does in the game: its memory starts over, its `storage` stays |
| `title-screen = true` | no player and no clock until a step sets them |
| `blocks = [{ x = 0, y = 63, z = 0, state = "minecraft:stone" }]` | server mods with `world`: those blocks of the Overworld, before the rest of the step (every other block is air, every biome the dimension's plainest: plains, nether wastes, the end) |
| `player = { x = 3.0, ... }`, `clock = { day-time = 0 }` | changes those fields (`x y z yaw pitch health food dimension`, `game-time day-time`) |
| `ticks = n`, `frames = n` | that many ticks (the game's tick counter and the clock move on), then frames 1/60 s apart |
| `chat = "<Steve> hi"` | a chat line the player receives |
| `command = "where north"` | the player types `/where north`; a command the mod doesn't own fails the step |
| `break = { x = 1, y = 63, z = 0, state = "minecraft:stone" }`, `place = { ... }` | server mods: the player breaks or places that block |
| `leave = "Alex"` | server mods: that player leaves |

A server mod starts with nobody on: `join = "Alex"` (after `reload` and `title-screen`) brings a
player in at the defaults, in survival, with the id
`00000000-0000-4000-8000-000000000001` for the first name that joined, `...2`
for the second, the same id when a name joins again. `player`, `chat`,
`command`, `break` and `place` are then that player's doing: the one named by
`as = "Steve"`, or the first one on. `player` takes `game-mode` there, and the
script's own `[player]` and `[clock]` are a client mod's.

Then `expect` checks what the mod did during the step; a key left out is not
checked:

| `expect.` | Checks |
| --- | --- |
| `commands = ["where"]` | the commands the mod owns |
| `chat = ["..."]` | every chat line it showed, in order (`[]`: none) |
| `feedback = "..."`, `error = "..."` | the command's answer, shown plain or in red |
| `panel = ["..."]`, `corner = "top-left"` | its HUD panel's lines (`[]`: no panel) and corner |
| `told = ["Alex: Welcome!"]`, `broadcast = ["..."]` | server mods: every `players.tell` (as `name: text`) and `players.broadcast`, in order |
| `blocks = [{ x = 2, y = 64, z = 0, state = "minecraft:glowstone" }]` | server mods with `world`: those blocks of the Overworld after the step, the mod's `set-blocks` made (the script checks a state's shape; the game checks its name) |
| `drops = ["minecraft:iron_nugget x9 at 5.5 65 5.5"]`, `taken = [7]` | server mods with `items`: every `items.drop` and the ids `items.take` removed, in order |
| `effects = ["sound minecraft:block.anvil.use at 5 64 5", "event 2000 1 at 5 65 5"]` | server mods with `effects`: every sound and level event, in order |
| `names = ["Gear Crate at 5 64 5"]` | server mods with `containers`: every `set-name` (`-` for none) |
| `stopped = "panic"` | the mod was turned off, for a reason holding this text; without it, a mod that traps, panics or overruns fails the test |

A step may also set what the mod reads: `power = [{ x, y, z, level }]` (the redstone signal at those positions from then on, capability `redstone`) and `items = [{ id, item, count, x, y, z }]` (item entities lying in the Overworld, capability `items`; the ones `take` removes are gone for later steps).

Unknown keys are errors, so a typo never passes silently. The same runner is
`horizon_mods::testing` for Rust code that drives a mod (`TestGame`,
`Script`): Horizon's own tests run the examples' scripts with it. Scripts test
any mod, whatever it was written in.

### Template layout

```
my-mod/
├── Cargo.toml           # cdylib, horizon-mod-sdk, release: opt-level "s", LTO
├── horizon-mod.toml     # the manifest
├── src/lib.rs           # impl Mod, export_mod!
└── tests/hello.toml     # cargo horizon-mod test
```

A mod is a type implementing `horizon_mod_sdk::Mod`; its state lives in the
type:

```rust
use horizon_mod_sdk::{Mod, Registration, chat};

struct Hello { greeted: u32 }

impl Mod for Hello {
    fn init() -> Result<(Self, Registration), String> {
        Ok((Hello { greeted: 0 }, Registration::new().command("hello")))
    }
    fn command(&mut self, _name: &str, args: &str) -> Result<Option<String>, String> {
        self.greeted += 1;
        chat::show(&format!("Hello {args}! ({} so far)", self.greeted));
        Ok(None)
    }
}
horizon_mod_sdk::export_mod!(Hello);
```

Hooks not asked for keep their default bodies. The full example is
[`coords-hud`](../crates/horizon-mod-sdk/examples/coords-hud/src/lib.rs). Logic
that does not touch the API (formatting, maths) is ordinary Rust and unit-tests
on the host with `cargo test`.

### The manifest

```toml
id = "coords_hud"            # [a-z0-9_]{1,64}: settings key, log name
name = "Coordinates HUD"
version = "0.1.0"
description = "..."
authors = ["..."]
api = "0.1"                  # the API the SDK targets
side = "client"              # client | server | both
capabilities = ["game-read", "hud", "chat"]
experimental = []            # experimental capabilities it uses, e.g. ["ui"]
```

Unknown keys are refused, so a typo is an error, not a silently ignored
permission. An experimental capability (today `ui`, `render` and `input`) may change in any release
([stability](api/stability.md)): a mod uses it only by naming it in
`experimental` as well, so nobody depends on it by accident.

### Content mods (API 0.2)

A mod with `api = "0.2"` and capability `registries` may carry `assets/` and
`data/` in the `.hmod`, and needs no `mod.wasm` if it has no code
(`cargo horizon-mod new --template content` starts one; `build` in a folder
with no `Cargo.toml` packs the manifest, `assets/` and `data/`, and `check`
reads it without a trial run). It adds
blocks and items by declaring them in `data/<id>/horizon/block/*.json` and
`data/<id>/horizon/item/*.json`, each copying a vanilla template
([modpacks.md](modpacks.md#registries-02-data-driven-blocks-and-items)):

```json
{ "template": "minecraft:iron_block",
  "properties": { "hardness": 4.0, "light": 3, "sounds": "minecraft:amethyst_block" },
  "item": true }
```

A block takes its template's shape unless it gives its own, in pixels with
a block 16 across as NeoForge's `Block.box` takes them: `shape` is the
outline players pick and see and, unless `collision_shape` says otherwise,
what entities collide with (`[]`: nothing). Either is a list of boxes for
every state, or boxes by state, keyed by properties as a state writes them;
a state takes the key it matches with the most properties, and one no key
matches keeps the template's shape:

```json
{ "template": "minecraft:iron_chain",
  "shape": { "axis=x": [[0, 6, 6, 16, 10, 10]],
             "axis=y": [[6, 0, 6, 10, 16, 10]],
             "axis=z": [[6, 6, 0, 10, 10, 16]] } }
```

The boxes are joined as `Shapes.or` joins them, and the outline's edges are
drawn as vanilla draws a shape's. Light, the faces a block hides on its
neighbours and what it supports stay the template's: a template with the
properties the shape is keyed by and that hides nothing (a chain, an end
rod, iron bars) fits a small shape best. The gears example's `gears:shaft`
is a chain with a thicker shape along its axis.

The fence: files go in the mod's own namespaces (`assets/<id>/`,
`data/<id>/`, and those its manifest lists), plus additions to vanilla tags
(`data/minecraft/tags/`) and to the convention tags NeoForge and Fabric mods
share (`data/c/tags/`).
Templates and `sounds` name vanilla blocks; unknown keys are refused.

Some Java mods ship their files under other names than their mod id (Towns
and Towers, mod id `t_and_t`, under `kaisyn` and `towns_and_towers`). A
content mod lists those in its manifest:

```toml
id = "t_and_t"
api = "0.2"
capabilities = ["registries"]
namespaces = ["kaisyn", "towns_and_towers"]
```

A listed namespace is the mod's as its id is: its `assets/` and `data/`
files load, and the game reads its tags, recipes, names and loot. Blocks and
items are still declared under `data/<id>/horizon/` only. `minecraft`, `c`
and `horizon` cannot be listed. Two mods that are on may not own the same
namespace (one's id or listed namespace being the other's): the game keeps
the first in load order and leaves the other's content out, with
"mods A and B both use the namespace N; turn one of them off" in the log. Paths
use vanilla's resource characters, at most 65,536 files, 16 MiB each and
512 MiB in all. A mod whose content is refused stays off, with the reason on
its row. The game reads this content into its block and item tables
(`horizon_client::mod_content`, docs/modpacks.md). A tag entry naming an id
nobody registers (a block a port left out, another mod's item) is dropped,
so the rest of the tag still loads; vanilla would drop the whole tag.

Tags merge as a data pack's do. Every mod's file for one tag (vanilla's
`data/minecraft/tags/`, the convention `data/c/tags/`, or another mod's) adds
its values after vanilla's and the mods' before it, in load order. A file with
`"replace": true` first drops what came before it, vanilla's entries and the
earlier mods', and the mods after it still add theirs, as a higher data pack
does. Since the result depends on load order, the game says so in the log:
"mod towns replaces data/minecraft/tags/worldgen/biome/has_structure/village_plains.json:
vanilla's entries and those of adds are left out, ...". Tags are the only
`minecraft` files a mod may hold: its other content stays in its own
namespaces.

A mod adds to structure sets and template pools that are not its own
(vanilla's villages, another mod's houses) with a set or pool file in its own
namespace holding `horizon:add_to`, a key vanilla data never has. Its entries
are appended to each set or pool named, after every mod's own files, in mod
and file order; the file is no set or pool itself. A name nobody defines is
skipped.

```json
{ "horizon:add_to": "minecraft:villages",
  "structures": [ { "structure": "mymod:village_beach", "weight": 10 } ] }
```

A pool addition holds `"elements"` as a pool does. This is what lithostitched's
`add_structure_set_entries` and `add_template_pool_elements` modifiers do on
NeoForge and Fabric, and `horizon-port-pack carry` writes them from those.

A structure (and any of its pool elements, in place of the structure's) may
hold `horizon:enhanced_terrain_adaptation`: MoogsStructureLib's terrain
kernel, which carves or buries terrain around the
pieces on top of vanilla's `terrain_adaptation`. `kernel_size` and
`kernel_distance` shape the kernel; `top` and `bottom` (`carve`, `bury` or
`none`) act above and below a piece's floor; `bottom_offset`, `padding`
(`x`, `top`, `bottom`, `z`) and MoogsStructureLib's `band` (`bottom`, `top`,
`piece_heights`) move the box it measures from. On a structure,
`rigid_junctions` adds rigid pieces' junctions as well and
`inflate` grows the start's box by the kernel radius. `kernel_size` 0 is
none.

```json
{ "horizon:enhanced_terrain_adaptation": { "kernel_size": 24, "kernel_distance": 24,
    "top": "carve", "bottom": "none", "rigid_junctions": true, "inflate": true } }
```

### Server mods (API 0.3, experimental)

A server mod runs where the world is simulated: on the dedicated server, on a
LAN host and on single player's own server, never in a player's window. Its
manifest says `side = "server"` and `api = "0.3"` (and only a server mod may);
its component targets the WIT world `horizon-server-mod` instead of
`horizon-mod`. Experimental: it may change in any release
([stability](api/stability.md)).

| Interface | Capability | Functions |
|---|---|---|
| `log` | always | as for client mods |
| `storage` | `storage` | as for client mods, in the server's `horizon-mods/` |
| `players` | `players` | `online()`: id, name, position, dimension, game mode, health of each player on; `tell(id, text)`, `broadcast(text)`: system messages after the mod's name, 8 per call |
| `world` | `world` | `get-block(dimension, x, y, z)`: the state in a loaded chunk, `minecraft:oak_log[axis=y]`; `get-biome(dimension, x, y, z)`: `minecraft:plains`, ...; `set-blocks(dimension, changes)`: up to 4096 per call, checked at once, made as `/setblock` makes them (neighbours, light, block entities, every player told) after the call, before that dimension's next tick; `scan(dimension, min, max)` (API level 15): the non-air blocks of a box of up to 32768 positions in loaded chunks, as a palette of states and `(x, y, z, palette index)` entries, for a mod that picks up a structure |

| Hook (export `server-events`) | When |
|---|---|
| `init() -> result<registration, string>` | once, after loading |
| `tick(tick)` | each server tick |
| `player-joined(player)`, `player-left(player)` | `registration.joins` |
| `chat(player, text)` | `registration.chat`: each chat message a player sends |
| `command(player, name, args, values)` | `/name args` typed by a player, for a command it registered, with its arguments' values (below); `ok(some)` is that player's feedback, `err` their red error |
| `block-broken(player, pos, state)`, `block-placed(player, pos, state)` | `registration.blocks`, with the state as `minecraft:oak_log[axis=y]` |

Capabilities that act on a local player (`game-read`, `chat`, `hud`) are for
client mods; `players` is for server mods. In Rust: `impl
horizon_mod_sdk::server::ServerMod` and `export_server_mod!`; the example is
[`welcome`](../crates/horizon-mod-sdk/examples/welcome/src/lib.rs), and with
`world`, [`beacons`](../crates/horizon-mod-sdk/examples/beacons/src/lib.rs).

**Typed commands.** Each command in `registration.commands` lists its
arguments, in order: a name and a kind, required or optional (optional ones
last). The server adds them to the command tree every player receives, after
vanilla's commands, so a player's chat box completes and colours them as it
does vanilla's (player names, coordinates, bounds); when a player sends one,
the server parses the line the way vanilla's parsers do and gives the mod the
values, by argument name, or answers with vanilla's error (`Integer must not
be more than 64, found 65`, `No player was found`) without calling the mod.

| Kind | Takes | Value | Chat box parser |
|---|---|---|---|
| `word` | one word: letters, digits, `_ - . +` | `word(string)` | `brigadier:string` (word) |
| `text` | the rest of the line, only last | `text(string)` | `brigadier:string` (greedy) |
| `integer(min, max)` | a whole number within the bounds given | `integer(s32)` | `brigadier:integer` |
| `number` | a number | `number(f64)` | `brigadier:double` |
| `boolean` | `true` or `false` | `boolean(bool)` | `brigadier:bool` |
| `player` | an online player's name, `@s` or `@p` (whoever typed it) | `player(player)` | `minecraft:entity` (one player) |
| `block-pos` | `x y z`, each a whole number, `~` or `~n` (from whoever typed it) | `block-pos(block-pos)` | `minecraft:block_pos` |

A command with no arguments takes nothing after its name. In Rust,
`Registration::command_with("beacon", [Argument::integer("height", Some(1),
Some(64)).optional()])`, read with `args.integer("height")`; `command(name)`
alone takes free text, an optional `text` argument named `args`
(`args.text()` is the line as typed either way).

**Capability `world`.** Reads are of the server's loaded chunks as they are
during the call: the game lends a server mod's call the levels' chunk maps
(moved, not copied, and only while a running mod has `world`), so a read
costs a map lookup and no copy of the world. A dimension with no chunk loaded
at a position reads as none. Writes go through the same code as `/setblock`,
so block updates, light, block entities and every client (LAN guests,
dedicated server players) see them as vanilla's; the parity harnesses test
that code. Changes for a dimension whose level isn't ticking wait for it
(in-frame single player ticks only the player's), up to 65,536 blocks; a
change in a chunk that isn't loaded when it is made is skipped, as `/setblock`
refuses one.

**Capability `block-entities` (API level 8, experimental).** The base for
ports of mods with machines, such as Create: bytes a mod keeps at the
positions of its own blocks, and a tick and a use hook for them.
`block-entities.get/set/remove(dimension, x, y, z, ...)` hold up to 64 KiB at
a position and 65,536 positions per mod; `set` is refused where the block
standing there is not in one of the mod's namespaces (its id and the manifest's
`namespaces`). The mod's `block-events` export (the SDK exports it for every
server mod, with `ServerMod::ticking_blocks`, `block_tick` and `block_used`)
lists the blocks it wants ticked, once after `init`; the game then calls
`block-tick(dimension, pos, state)` every server tick for each position
where the mod keeps data, if its chunk is loaded and a ticking block still
stands there (otherwise the data is dropped), at most 4096 positions per mod
per tick. A server mod built before this export still loads and hears
nothing of blocks. A server mod may also carry its own blocks: capability
`registries` is allowed beside `api = "0.3"`, with `data/<id>/horizon/block/`
as for content mods; the example is
[`gears`](../crates/horizon-mod-sdk/examples/gears/src/lib.rs).

The game keeps the bytes in the world's folder, in
`horizon-mod-data/<mod id>.blocks` (written every 10 seconds when changed
and when the world saves), outside the vanilla save, so no chunk format
changes. They are handed back to the mod when a world opens or the set of
running mods changes. `block-used` runs when a player (the window's own or a
LAN guest) right-clicks a block of a mod's namespace with an empty-handed
use, not sneaking with full hands: the mod returns true to take the click,
and the game then does nothing else with it. Vanilla blocks never reach it.

### Capability `containers`

**API level 11, experimental, server mods.** A block of the mod's own that
holds items and opens as a chest. `containers.register(block)` (or `register-sized(block, slots)`, API level 20, with
5, 9 or 27 slots opening as a hopper, a dispenser or a chest) (`mymod:crate`,
no properties; at most 64 blocks) makes it one; `open(player, dimension, x, y,
z)` opens it for a player (the window's own or a LAN guest) after the call,
from `block-used` for instance; `slots(dimension, x, y, z)` reads its 27 slots
as of the tick's start (item and count, no components) and `set-slot(...,
slot, stack)` writes one after the call. The game keeps the items in a chest
block entity at the position, saved with the chunk. In this first step it is
a 27-slot chest only (no custom menu; `set-name` gives it a title; hoppers
should work with it as with any block entity holding items, untested), and only the dimension the world is simulating is reachable.
How the chest entity of a block that is not a vanilla chest saves and loads
is not yet proven: keep what matters in `block-entities` too until it is.

### Neighbour events (capability `block-entities`)

A server mod's `ServerMod::block_neighbor_changed(dimension, pos, state, from,
from_state)` is called when a block next to one of its blocks changes: `pos` is
the mod's block (a position where it keeps data with `block-entities.set`),
`from` the block that changed (any block, a player's or a piston's or the
mod's own). It comes at the start of the next server tick, at most 1024 calls
a tick to a mod; built before this, a mod simply hears nothing. A block that
should send a redstone signal can be two blocks of the mod, one with template
`minecraft:redstone_block`, swapped with `world.set-blocks`.

### Giving a redstone signal

`redstone.set-signal(dimension, x, y, z, level)` (API level 21) makes a block of the
mod's give a signal of 0 to 15 to its neighbours at the tick's end, until the
mod sets another: the analog signal a trapped chest gives. Only a block declared
with the template `minecraft:trapped_chest` has one, and its container
openers share the number, so a mod keeps such a block for signals only. It is
not saved: the mod sets it again after a load. The gears example's
`gears:gauge` gives 5 for each item lying on it.

### Pushing players and items

`players.push(player, x, y, z)` and `items.push(id, x, y, z)` (API level 19)
add a velocity, in blocks per tick, at the tick's end, each component clamped
to -4 to 4, at most 256 together with `items.take` a call. A guest of the
world is sent the motion as vanilla sends a knockback. With `items.nearby` and
`players.online` a ticking block moves whatever stands above it: the gears
example's `gears:belt` carries items and players east.

### Capability `redstone`

**API level 12, experimental, server mods.** `redstone.power(dimension, x, y,
z)` is the strongest signal (0 to 15) reaching a block of the mod's from its
neighbours (vanilla's best neighbour signal), as of the start of the tick. It
is known only at positions where the mod keeps data (`block-entities.set`) or
has a container, in the dimension the world is simulating, at most 4096 a
tick; elsewhere it is `none`. There is no event for a change yet: a mod reads
it from `block-tick`, and a block has to be a ticking one. It does not make a
block a signal source.

### Capability `effects`

**API level 14, experimental, server mods.** `effects.play-sound(dimension, x,
y, z, event, volume, pitch)` plays a sound event at a block (volume 0 to 4,
pitch 0.5 to 2) and `level-event(dimension, x, y, z, id, data)` triggers a
vanilla level event: 2000 smoke, 2004 spawner flames, 2005 bone meal
particles, 2009 water evaporating (any other id is refused; data 0 to 15). Both
run at the tick's end through the game's own level events, so LAN guests see
and hear them as they do a vanilla block's; at most 64 a call, and only the
dimension the world is simulating plays.

### Capability `items`

**API level 13, experimental, server mods.** `items.drop(dimension, x, y, z,
item, count)` drops a stack (1 to 99, any item id, a mod's own too) as an item
entity at a position, at the tick's end, with a short pickup delay; at most 64
a call. Only the dimension the world is simulating drops; others are ignored.
The game does not check the item id: an unknown one drops as an unknown item.

`items.nearby(dimension, x, y, z, radius)` (API level 17) lists the item
entities within 16 blocks of a point (id, item, count, position; nearest
first, at most 256) as of the tick's start, and `items.take(id)` removes one
at the tick's end, so a mod's machine can carry items in. Only the simulated
dimension has item entities to list.

`containers.set-slot-rule(dimension, x, y, z, slot, allowed)` (API level 22) limits
what a player may put in a slot of a registered container: `some(items)` allows
only those item ids, an empty list nothing (a slot players can only take from),
none lifts the rule. It holds for clicks, drags and shift-clicks, local and
guests'; hoppers and the mod itself are not held to it. Not saved: the mod sets
its rules again after a load (the gears crate does at each open). A progress bar or
a screen of its own is not part of this.

### Turning parts (capability `block-entities`, `visuals`)

**API level 39.** `visuals.set(dimension, x, y, z, parts)` draws up to 16 model parts at one of
the mod's own blocks, replacing what it drew there; no parts draws nothing. Each part is a block
state's model (`mymod:shaft[axis=x]`, a vanilla one too) with its corner at `offset` blocks from
the block's, turning about `axis` (0 x, 1 y, 2 z) through `pivot` at `speed` degrees a tick from
`angle`, as Flywheel's instances draw Create's shafts and cogs. The server sends a position's parts
to the players whose games have its chunk once, on `horizon:block_visual` after the chunk, and again
only when they change; each game turns them every frame from the tick they were set, with that
frame's fraction of a tick, so a thousand shafts cost no packets while they turn. Parts set in one
tick at the same speed turn together. They are saved with the world beside the block data, and gone
when a player breaks the block or the game forgets its data. They are drawn as falling blocks are,
lit by the block's light. In the SDK, `BlockEntity::visual` fills a `Visual`
(`rotating(state, axis, speed)`, `model(state, offset)`; `block::rpm` turns revolutions a minute
into degrees a tick) after each tick, sent only when it changed; a part whose speed changes starts
again from its `angle`. The gears example's counter turns an iron trapdoor above it while it runs.

### Moving structures and networks

`entities.spawn-block(dimension, x, y, z, state)` (API level 24) places a block state as an
entity, drawn full size as a falling block is, with no gravity and never landing; the
mod moves it with `move-to` and takes it away with `remove`, and it is never saved. On it the
SDK builds `horizon_mod_sdk::contraption::Contraption`: `assemble(dimension, min, max)` takes
the non-air blocks of a box (at most 1024) out of the world as such entities, `move_by(delta)`
moves them together a fraction of a block at a time, and `disassemble()` puts them back as
blocks at the nearest whole position (capabilities `world` and `entities`). That is a piston,
a gantry, an elevator or a simple flying machine; the gears example's `gears:lift` raises the
3 x 3 x 3 box above it three blocks when used.

`entities.rotate(handle, yaw, pitch)` (API level 25) turns a block entity about its centre, pitch
about x then yaw about y, so a block with `axis=x` spins about any axis; it reaches clients as the
entity's rotation. `entities.set-solid(handle, solid)` (API level 26) makes it solid for players as a
boat is, its full cube: they stand on it and walk into it, and one standing on it rides along as it
moves (a guest is sent a teleport that keeps their motion); one it moves into from the side is not
pushed. It
reaches clients as a silent falling block with no gravity. `Contraption::set_rotation(yaw, pitch)`
turns a whole structure about the centre of its box and `set_solid` makes it walkable: the gears
example's `gears:mill` spins the 1 x 3 x 3 wheel above it, and its lift carries players.

**Structures (API level 40).** `structures.create(dimension, blocks, pose)` makes up to 4096
blocks one object (64 per mod), `set-pose(id, pose)` moves and turns it and `remove(id)` takes it
away. A pose is the world position of the structure's (0, 0, 0) corner and a quaternion it turns by
about that corner, any angle. Players' games get its blocks once, before the first chunks of its
level (`horizon:structure`), build its model once, and then draw it moved and turned between its
last two poses, so a structure costs one small packet a tick while it moves and none while it
stands. The game saves structures with the world; one still out of the grid when the world loads is
put back where it stood, turned to the nearest quarter turns about the middle of its blocks, so each
block gets a place of its own and none is lost. `horizon_mod_sdk::structure::Structure` has
`Contraption`'s methods on top (`assemble`, `move_by`, `set_rotation`, `disassemble`, which puts the
blocks back turned to the nearest quarter turns): the gears example's mill turns its wheel as one.
Players do not stand on structures or collide with them yet, and their blocks do nothing while they
move; that comes next ([Roadmap](#roadmap)).

`horizon_mod_sdk::network` walks networks of connected blocks in the mod's own code:
`propagate(sources, limit, link)` carries a value (a shaft's speed, reversed by a gear, doubled
by a cog) from each source to each member and marks a conflict where two disagree;
`members(start, limit, connects)` lists one pipe or cable network. A mod keeps what flows
(stress, fluid amounts) in `block-entities` data and shows it with entities and effects.

### Blocks and block entities as types

The SDK gives a server mod's blocks the shape NeoForge's have, over the interfaces above and with
no change to them. `horizon_mod_sdk::level` has `BlockPos` (`above`, `relative(direction, n)`, ...),
`Direction`, `Axis`, `BlockState` (`parse("mymod:gear[axis=x]")`, `get_value`, `set_value`,
`get_axis`) and `Level`, one dimension: `get_block_state`, `set_block`, `remove_block`,
`destroy_block`, `get_biome`, `get_best_neighbor_signal`, `play_sound`, `level_event`, `players`,
`get_block_entity` and `set_block_entity`, each needing the capability of the interface it calls.
`horizon_mod_sdk::block` has the `Block` trait (`use_without_item`, `use_item_on`, `set_placed_by`,
`player_will_destroy`, `neighbor_changed`, returning `InteractionResult`) and `BlockEntity` (`new`,
`load_additional`, `save_additional`, `tick`, and `SYNCED` for one players' games get, see
[Synced block data](#capability-net)). A mod registers them at `init`:
`block::register("mymod:press", Press)` and `block::register_entity::<PressData>("mymod:press")`.
`export_server_mod!` then calls them for those blocks before the mod's own hooks, which hear every
other block and the clicks a block passes (`InteractionResult::Pass`).

A block entity is made when a player places its block and kept as the bytes `block-entities` holds
there, so it is ticked, saved and told of neighbour changes as those are; after its tick it is saved
again when its bytes changed. A block the mod sets itself gets its entity from `set_block_entity` in
a later call, once the block stands. The gears example's `gears:drill` (a `BlockEntity` counting
its ticks) and `gears:deployer` (a `Block`) are written this way.

### Players: inventories and the console

`players.inventory(player)` (API level 33) is a player's 41 slots as of the tick's start (0 to 8 the
hotbar, 9 to 35 the rest, 36 to 39 the armor from feet to head, 40 the off hand) and
`players.selected-slot(player)` the hotbar slot they hold.

Capability `console` (experimental) runs any command as the server's console, with an operator's
rights, at the tick's end, as a command block would: `console.run(line)`, at most 256 a call, no
feedback to players and a line per mod in the server's log. The SDK's `horizon_mod_sdk::player_ops`
builds the usual ones with the player named by UUID and every id checked: `give`, `take`, `effect`,
`clear_effects`, `experience`, `teleport` (to another dimension too) and `damage`. The gears example
gives a diamond and speed to a player who waves a stick and counts the sticks they carry.

### Capability `stacks`: data on items

**API level 34, experimental, server mods.** What a stack carries beyond its item and count, as
NeoForge's data components: damage, a custom name, enchantments, and the mod's own bytes (at most 16
KiB), kept in vanilla's `minecraft:custom_data` under `horizon:<mod id>` so they stay with the stack
wherever it goes, saves included. `stacks.player-stack(player, slot)` reads one of a player's 41
slots as of the tick's start and `set-player-stack` writes it after the call (a guest is sent their
inventory again); `container-stack` and `set-container-stack` read and write an inventory's slot at
once. A mod sees and writes only its own bytes, and what other mods keep on a stack stays when its
item does. The gears stick counts how often it was waved and is named for it.

### Capability `menus`: screens of the mod's own

**API level 32, experimental, server mods.** As NeoForge's menus and screens: a mod opens one of
its registered containers with `menus.open(player, dimension, x, y, z, layout)`, where `layout`
says the screen's size, where each menu slot is drawn (the container's slots first, then the
player's 27 main and 9 hotbar slots; a slot left out is not drawn), and its widgets: labels,
progress bars filled as one data value is to another, and buttons. `menus.set-data(player,
index, value)` sets one of the screen's 32 data values (`ContainerData`), sent to the client
when it changes; a button pressed reaches `ServerMod::menu_button(player, id)` (`clickMenuButton`).

The menu itself is the container's ordinary one, so clicks, shift-clicks and the slot rules work
as they do in a chest; the layout goes to Horizon's clients on the `horizon:menu` channel and they
draw it, and a vanilla client sees a chest. The panel is drawn in vanilla's container colours;
textures of the mod's own are not drawn yet. The gears example opens its crate as a press with
two slots, a moving bar and a button.

**API level 35.** A screen with no block behind it (a toolbox, a book, a schedule, Create's
filter screens): `menus.open-panel(player, layout)` draws the layout's widgets on a panel over
the dimmed world (its slots are not drawn). Widgets include text fields (`text-field`: id,
position, width, starting value, at most 256 characters); what the player typed reaches
`ServerMod::menu_text(player, id, value)` when they press Enter, and for every field before a
button press and as the screen closes. `menus.close(player)` closes the mod's screen, and
`ServerMod::menu_closed(player)` hears the player leave one (Esc, or leaving its container). A
vanilla client sees nothing of a panel. The gears example's stick, waved while sneaking, opens a
notebook whose note is kept on the player in attachments.

### Items used and blocks broken

`ServerMod::item_used(player, item, sneaking, target)` hears a player's right click with the item
in their main hand, on a block (`target`: dimension, position, face 0 to 5 and state) or in the
air, before anything vanilla does with it. It goes to every server mod, those whose namespace the
item or the block is in first, until one takes it (true): a mod's wrench works on any block, a mod's
machine hears any item used on it, and a mod can give a vanilla item a use. The gears example shows a gears block's state when a stick is used on it.

`world.break-block(dimension, x, y, z, tool, enchantments, into)` (API level 29) breaks a block as
a player holding `tool` with those enchantments (fortune, silk touch) would, after the call: its
break particles and sound, a container's contents spilled, air in its place, and its loot table's
drops when the tool is right for it. The drops go into the inventory at `into`, and what does not
fit, or all without one, drops where the block stood. Bedrock and other unbreakable blocks are
left. The gears example's `gears:drill` mines the block under it once a second into the inventory
above it.

`world.use-item(dimension, x, y, z, face, item)` (API level 31) uses an item on a block as a player
facing that face would, after the call, as a deployer does: the block's own answer to the item (a
lever pulled, a door opened, a note block tuned), and when it has none, a block item's block placed
against the face where there is air. Items' own uses (bone meal, flint and steel, buckets) are not
run yet. The gears example's `gears:deployer` uses cobblestone on the block above it when used.

### Messages between mods

Every server mod, with no capability, can write to another running server mod (API level 30):
`peers.send(mod, data)` (at most 64 KiB, 64 a call) reaches its `ServerMod::peer_message(from,
data)` at the start of the next server tick, and `peers.running(mod)` says whether it runs. That is
how one mod offers others an API: a request, then an answer back the next tick. The gears deployer
writes to itself and says so in chat.

### Capability `transfer`: items, fluids and energy between blocks

**API level 28, experimental, server mods.** What NeoForge's capabilities (`IItemHandler`,
`IFluidHandler`, `IEnergyStorage`) are to Forge mods: a way for one mod's machine to move things
in and out of another's blocks and vanilla's, at once, during the call (`simulate` only asks).

- Items: `item-slots`, `insert-item` (like stacks first, then empty slots; returns what did not fit)
  and `extract-item` reach any inventory in the simulated dimension: chests, barrels, shulker boxes,
  hoppers, dispensers, droppers, mod containers, and a furnace by `face` as a hopper does (its top
  takes what it smelts, its sides fuel only, its bottom gives the result).
- Fluids: `add-tank(pos, capacity)` gives one of the mod's own blocks a tank (millibuckets, one
  fluid at a time); any mod reads it with `get-tank` and moves fluid with `fill` and `drain`.
- Energy: `add-battery(pos, capacity)`, `get-battery`, `receive-energy`, `extract-energy`, in any
  unit the mods agree on.

Tanks and batteries are saved with the world (among the mods' block data, as `@transfer`) and
go when their block does; `remove-storage` takes them off. The gears example's counter charges
a battery and puts what it presses into the inventory under it, dropping what does not fit.

### Data attachments

Every server mod, with no capability, can keep bytes on a player (by UUID, online or not), a
dimension or a chunk (API level 27): `attachments.get(target, key)`, `set` (at most 64 KiB, 65536
values a mod), `remove` and `keys`, as NeoForge's data attachments. They are saved with the world
beside the mod's block data; a mod sees only its own, and sends what clients need with `net`. The
gears example counts on each player the counters they placed.

### Reading the mod's own files

Every mod, with no capability, can read what its package carries under `data/` and
`assets/` (API level 23): `files.read(path)` gives a file (at most 1 MiB) or none,
`files.paths(prefix)` the paths that start with a prefix (at most 1024, sorted). Nothing
outside `data/` and `assets/` of the mod's own package can be read.

### The game's recipes

Every server mod, with no capability, asks the game about its recipes through
`recipe-manager` (API level 36), as NeoForge's `RecipeManager`:

- `craft(grid, width)`: the crafting table's recipe for a 1x1 to 3x3 grid, matched as vanilla
  matches it, with its id, result and what stays in each cell (a bucket);
- `cook(kind, item)`: the `minecraft:smelting`, `blasting`, `smoking` or `campfire_cooking`
  recipe taking an item, with its result, experience and time;
- `stonecut(item)`: the stonecutter's recipes for an item;
- `of-type(kind)`: every recipe file of a type the game does not run itself (`gears:pressing`),
  from the `data/<namespace>/recipe/` folders (subfolders too) of every content mod, as JSON;
- `test-ingredient(ingredient, item)`: whether an ingredient as recipe files write one
  (`minecraft:iron_ingot`, `#minecraft:logs`, a list) takes an item, tags included.

A mod with capability `registries` puts recipe files in its package's `data/<namespace>/recipe/`:
vanilla's types join the game's recipes (the crafting table, furnaces, the recipe book), and its
own types are its machines', which another mod's or a pack's data adds to as with Create's
`create:pressing`. The gears example's counter presses what `gears:pressing` files say
(`data/gears/recipe/pressing/`) and smelts what no press recipe takes; a stick used on a crafting
table crafts once from the first nine slots of the chest below it; `gears:counter` has a crafting
recipe of its own.

`cargo horizon-mod test` answers `of-type` from the package's own recipe files and tests
ingredients by item id only; it has no vanilla recipes or tags.

### Capability `entities`

**API level 10, experimental, server mods.** A first, small step toward mod
entities: `entities.spawn(dimension, x, y, z, yaw, item)` places a marker
(returns the mod's own handle, at most 1024 live), `move-to(handle, ...)`,
`set-item(handle, item)` and `remove(handle)` change it. The game draws it as
an invisible armor stand holding `item` in its hand (a mod's own item too), so
it follows the stand's rendering for players and LAN guests. It has no hitbox,
AI or physics, and is **not saved**: after a restart the mod places its
entities again (from `block-entities` data or `storage`). Changes apply at
the tick's end, in the dimension the world is simulating; others are dropped.
A mod turned off mid-session leaves its entities until the world closes.

`entities.set-slot(handle, slot, item)` (API level 18) puts an item in any of
`mainhand`, `offhand`, `head`, `chest`, `legs`, `feet` (none empties it): a
block's item on the head is drawn as the block, so a mod's own block can stand
in the world as a model. `entities.set-pose(handle, part, x, y, z)` turns
`head`, `body`, `left-arm`, `right-arm`, `left-leg` or `right-leg` in degrees,
as an armor stand's pose does: a spinning gear is a block on the head turned a
few degrees each tick.

#### Entity types of the mod's own

**API level 37, experimental, server mods.** As NeoForge's `EntityType`s, under capability
`entities`: `entity-types.register(def)` names a type in one of the mod's namespaces with its
model (an item worn on the head, a block's item drawn as the block), size (an armor stand's, or a
small one's), gravity and health. `spawn(kind, dimension, x, y, z, yaw)` gives back the entity's
id, which stays the same after a restart. The game keeps each entity: it moves it by its velocity
against the blocks (gravity if its type has it; on the ground it slows as a mob does), saves it
with the world in `horizon-mod-data/entities.json` with the mod's own bytes (`set-data`, at most
4 KiB), and takes players' hits off its health. The mod gives it its behaviour:

- `entity-events.tick(entities)` each server tick, with the mod's entities in the simulated
  dimension after they moved, where the mod calls `set-velocity`, `move-to`, `set-pose` (the
  model turns with `head`), `set-model`, `hurt` or `remove`;
- `attacked(id, player, damage)` when a player hits one: true takes the hit, false lets the game
  take the damage off its health;
- `interacted(id, player, item)` when a player right-clicks one;
- `died(id, kind)` when its health reaches 0 (or it falls out of the world), before it goes.

`get(id)` and `all()` read them as of the tick's start. Every client, vanilla's too, sees each one
as an invisible armor stand wearing its model, with the armor stand's hitbox; at most 256 a mod.
In the gears example, a stick used while sneaking on one of its blocks lets out a `gears:roller`
that rolls after the nearest player, spins its gear, hops and tells its age when clicked, and drops
three iron nuggets when broken.

### Capability `net`

A mod with a client half and a server half (two mods with the same `id`, one
`side = "client"` and one `side = "server"`) talks between them with
`net.send(player, data)`: up to 32 KiB a message and 64 messages a hook call.
A client mod passes `none` and the message goes to the server it plays on. A
server mod passes a player's id (`players.online`), or `none` for every player.
The receiver hears it as `Mod::net_message` (client) or
`ServerMod::net_message` (server, with the sender's id) on its next tick. The
messages travel as custom payloads on the channel `horizon:mod/<mod id>`:
in single player and on a LAN host they stay in the game or go to the guests'
connections, and a client mod playing on a server reaches that server's mod
of the same id. Vanilla servers ignore the payloads. A mod built before `net`
(no `net-events` export) hears nothing. **Experimental:**
`experimental = ["net"]`.

**Synced block data (API level 38).** What a server half keeps at a block
with `block-entities.set-synced` (as `set`, with the same limits) also goes
to every player whose game has the block's chunk: at once, whenever it
changes, and when the chunk reaches a player later, as NeoForge sends a block
entity's update tag. The client half reads it with `block-data.get(x, y, z)`
and `block-data.in-box(min, max)` (at most 256 positions, nearest the box's
minimum corner first), as of its tick, in the player's dimension; a chunk the
player's game forgets takes its data with it. A later `set` at the position
keeps the bytes on the server only, and `remove` takes them from the players'
games too. They travel as custom payloads on `horizon:block_data`, sent after
the chunk they belong to, so every change is a packet to each player near
it: a block syncs what players see change (a gauge's reading, a tank's
level), not every counter it keeps. Synced entries stay synced when the world
is saved and loaded. In-frame single player has no connection: the window's
client half reads its own server half's synced data directly. In the SDK, a
`BlockEntity` with `const SYNCED: bool = true` is saved this way; the gears
example's counter syncs its seconds.

**Where they run.** The game that simulates the world starts the server mods
that are on, and only that one:

| Mode | Runs server mods | Players they see |
|---|---|---|
| Single player (default) | the server thread | the window's player, a connection like any other |
| Single player `--in-frame` | the window's own game, beside its client mods | the window's player |
| Open to LAN, Play with Friends | the host's server thread (or its window, in-frame) | the host and every guest |
| Dedicated server (`--dedicated`, `horizon-server`) | the server | every connected player |

A window that is a client of a server (single player's window, a LAN guest,
any multiplayer server) never runs them; its Mods page lists a server mod with
the others, and its switch takes effect the next time a world opens. The mods
live in `horizon-mods/` of the game folder, or of the server's folder on a
dedicated server, and are turned on as client mods are: Options > Mods, or
`mod.<id>:true` in `horizon-mods.txt`. `--dev-mod <file>` runs one always
on and reloads it when its file changes, on a server too.

**When hooks run.** At the end of each server tick the game tells the server
mods who joined and who left since the last one, then what players said,
broke and placed, in order, then calls `tick`. A command a player types
reaches its mod at once, whatever the player's permission level, and its
answer goes to that player. `tell` reaches one player, the window's own
chat for the in-frame player and a system message for a connection;
`broadcast` reaches every player and the server's log. With no server mod
on, each of these points costs one branch (measured in the PR that added
them).

**Not yet.** Block events are a player's own breaking and placing, not
explosions, pistons or other mods. Command arguments don't take entities,
items or block states yet, and `^` coordinates; in-frame single player's chat
box lists server mods' commands by name only (the server still parses their
arguments).

## Publish your mod

Horizon's registry lists mods, modpacks (`.hpack`) and resource packs made for Horizon, each reviewed before it is
published. Players find them at [playhorizon.dev/mods](https://playhorizon.dev/mods) and in the launcher (Mods, then
**Horizon** beside Modrinth), which checks every file before it installs it.

1. Run `cargo horizon-mod check` on the file (above): the registry runs the same checks.
2. Put the source somewhere public, under a licence that lets players run it: reviewers read it, and the file must be
   built from it.
3. Open [playhorizon.dev/mods/submit](https://playhorizon.dev/mods/submit), sign in with GitHub (it reads only your
   public profile) and fill in the page: the file, its name and summary, licence, source address, and optionally a
   minisign public key with the file's `.minisig`.
4. The form checks the file again and opens a pull request in
   [horizon-rust/horizon-mods](https://github.com/horizon-rust/horizon-mods) that adds `mods/<id>.toml`, with you as its
   owner. That repository's checks run the file in the sandbox and post their report on the pull request; answer the
   reviewers there.
5. A reviewer merges it: within minutes the file is a release of horizon-mods, and the project is on the site and in
   the launcher.

Once it is published, its page and its card in the launcher show its downloads (GitHub's counts of its files,
refreshed every hour) and its rating: players signed in with GitHub on its page give it 1 to 5 stars, one rating per
account, which they can change. Its owners can't rate it, and nobody sees who rated.

An update is the same form with a new version; only the project's `owners` can publish one, signed with its key if it
has one. A published file never changes. Without the form, open the pull request by hand (the repository's README
says how). The formats: [api/mod-index.md](api/mod-index.md).

### Horizon's official mods

The Horizon extras that are mods now, and Horizon's own recipe viewer, live in `mods/official/<name>/`, one crate each in the game's workspace, so
`cargo clippy --workspace` and pr_ready cover them. They use only the public API, like any other mod, with the same
options on their settings page as the extra had and the same look. Each is published by a maintainer: build it on
`main` with the toolchain CI uses (`cd mods/official/<name> && cargo horizon-mod build`), then open a pull request in
horizon-mods with its `mods/<id>.toml` (owners `ludovic111`, `source` pointing at the folder) and the file's
SHA-256. The reviewer builds it the same way and merges only when the hashes match: the `.wasm` holds no local paths,
so the same rustc and `Cargo.lock` give the same file.

| Mod | Id | Replaces | Settings (storage key: values) |
|---|---|---|---|
| FPS Counter | `fps_overlay` | Extras > FPS counter | `corner`: `top_left` (default), `top_right`, `bottom_left`, `bottom_right`; `lows`: `false` (default), `true` |
| Keystrokes | `keystrokes` | Extras > Keystrokes | `corner`: as above, default `top_left`; `cps`: `true` (default), `false` |
| Zoom | `zoom` | Extras > Zoom | `level`: `2`, `3`, `4` (default), `6`, `8`, `10`; `smooth`: `true` (default), `false`; `sensitivity` (slower turning): `true` (default), `false`. The key is the game's: `key.zoom.zoom` in `horizon-mods.txt`, default `key.keyboard.z` |
| Minimap | `minimap` | Extras > Minimap | `corner`: as above, default `top_right`; `size`: `64`, `96` (default), `128`; `shape`: `square` (default), `round`; `zoom` (blocks per pixel): `0.5`, `1` (default), `2`, `4`; `rotate`: `false` (default), `true`. The game draws the map (`render.set-minimap`) |
| Far View | `far_view` | Graphics > LOD Distance and Far Detail | `distance` (chunks): `64`, `128`, `256` (default), `512`, `1024`; `detail`: `full` (default), `reduced`. The game builds and draws the distant terrain (`render.set-far-terrain`); the player's LOD Distance, when on, wins |
| Recipes | `recipes` | nothing (new) | `give` (a click on the item panel gives the item in creative mode: one, a stack with a right click): `false` (default), `true`. The keys are the game's: `key.recipes.recipe` (default `key.keyboard.r`) and `key.recipes.uses` (default `key.keyboard.u`) in `horizon-mods.txt`. Every item sits in the item panel (`browser`) with a search box (words in the name or id, `@namespace`, `#tag`); R or a click shows an item's recipes, U or a right click its uses, a tab per station, and + fills the grid with a recipe the player knows (`recipes.place`). On servers that are not Horizon it shows the game's own 26.3 recipes; known recipes and + follow the server's recipe book |

## Versions and compatibility

- The API is the WIT package `horizon:mods@0.1.0`. New functions and
  interfaces arrive in patch versions (0.1.1, ...); a game serving 0.1.N loads
  mods built for 0.1.0 to 0.1.N. A breaking change makes `0.2` (and later
  `1.0`); the game then links both versions side by side for at least one
  release cycle, so existing mods keep working while authors move.
- The manifest's `api` names the line it needs (`0.1`); the game refuses a
  line it does not serve with a clear message.
- Within a line, each function has an API level, the one it arrived at
  ([stability](api/stability.md#mod-api-levels)). A mod needs the level of
  the newest function it calls, read from its file: an older Horizon refuses
  it with "made for a newer Horizon: update Horizon", and the registry lists
  the level so installers take a version the game can run.
  `cargo horizon-mod check` prints it ("needs mod API level 1 or newer").
- Horizon follows Minecraft releases; the mod API does not have to. Vanilla
  concepts mods see (dimensions, block and item ids) are the game's own ids,
  so a version port changes the data, not the API.

## Multiplayer

- **Client mods** (`side = "client"`, all of 0.1) run in the player's game,
  in single player and on any server, vanilla or Horizon. They see only what
  the client already knows (the snapshot), like F3 does, and act only on the
  local screen and chat. A server cannot see them.
- **Server mods** (`side = "server"`, API 0.3) run where the world is
  simulated: single player's server thread (or the window, in-frame), a LAN
  host and the dedicated server ([Server mods](#server-mods-api-03-experimental)).
  They use only vanilla messages, so any client, vanilla or Horizon, can join.
- **Both** (`side = "both"`): a server mod with a client half (new blocks,
  their textures, a custom screen). On join the Horizon server announces its
  mods (id, version, SHA-256) on a `horizon:mods` custom payload channel; a
  Horizon client with matching client halves enables them for that session, or
  offers to download them from the same source the launcher uses. Vanilla
  clients are refused only if the server says its mods need a client.
- Registries (new blocks and items) are server-owned and synced like vanilla's
  registry sync, so ids match on both ends.

## Distribution

- **The file.** `.hmod`: a zip holding `horizon-mod.toml`, `mod.wasm` and,
  later, `assets/` and `data/`. `mod.wasm` is capped at 32 MiB.
- **Integrity.** Every download is checked against the SHA-256 the index
  lists before the file reaches `horizon-mods/`.
- **Signing.** Authors sign `.hmod` files with an Ed25519 key (minisign
  format: a `.hmod.minisig` beside it); the index pins each project's public
  key, so an update signed by another key is refused. Unsigned local files
  stay allowed, with an "unsigned" mark on the Mods page.
- **Where mods come from.** Horizon's registry ([Publish your mod](#publish-your-mod)): reviewed pull requests in
  [horizon-rust/horizon-mods](https://github.com/horizon-rust/horizon-mods), files as its release assets, and an
  `index.json` on GitHub Pages that the launcher and the website read. A file the registry takes down
  (`revoked.toml`) is refused by the launcher and turned off where installed.
- **The launcher.** A Mods tab per installation: browse and search the index,
  install, update, turn on and off (it writes the same `horizon-mods.txt`),
  see capabilities before installing. The launcher's CLI and MCP tools gain
  `mods list|search|install|remove|enable|disable`, so agents can set up a
  modded game the same way players do.

## Roadmap

1. Design (this page) and the Java research.
2. SDK, example mod, runtime, `cargo horizon-mod`, the game's Mods page: API 0.1.
3. `storage` (done), `keys`, `target`, `inventory-read`, `ui`: enough for the
   first native ports (zoom presets, AppleSkin-style food HUD, Jade-style
   tooltips).
4. Launcher Mods tab, the index, signatures; the Mods page shows cost and
   update state.
5. `recipes`, `world-read`, `render`: recipe viewer and schematics ports.
6. Server mods, `registries`, `net`, data-driven content from `.hmod`
   `data/` and `assets/`, multiplayer sync.

Modpacks ([modpacks.md](modpacks.md)) move data-driven `registries` first:
API 0.2 is content-only `.hmod`s (blocks, items, recipes, loot, models and
textures), then server hooks (0.3) and block entities, menus, capabilities and
`net` (0.4), in the order the ATM10 inventory needs them.
