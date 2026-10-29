//! Gears: a server mod with its own block. `gears:counter` keeps the seconds
//! it has stood as four bytes at its position (capability `block-entities`);
//! the server ticks it and saves the bytes with the world, and sends them to
//! the players near it for a client half to read. A gear (an iron
//! trapdoor drawn by players' games, capability `visuals`) turns above each
//! counter while it runs, an iron ingot lying on it.
use horizon_mod_sdk::{
    block::{self, Block, BlockEntity, BlockHitResult, InteractionResult},
    contraption::{Contraption, ROTATION_STEP},
    export_server_mod,
    level::{Axis, BlockState, Direction, Level},
    player_ops,
    server::{
        BlockPos, BlockTarget, Living, Player, Pos, Registration, ServerMod, attachments,
        block_entities, containers, effects, entities, entity_types, items, menus, net, peers,
        players, recipe_manager, redstone, stacks, transfer, visuals, world,
    },
    structure::Structure,
};

const COUNTER: &str = "gears:counter";
const CRATE: &str = "gears:crate";
const BELT: &str = "gears:belt";
const GAUGE: &str = "gears:gauge";
const LIFT: &str = "gears:lift";
const MILL: &str = "gears:mill";
/// The mod's own entity type: a small gear that rolls after players.
const ROLLER: &str = "gears:roller";
const DRILL: &str = "gears:drill";
const DEPLOYER: &str = "gears:deployer";
/// Ticks a lift takes to raise its load three blocks.
const LIFT_TICKS: u32 = 60;
const DIMENSION: &str = "minecraft:overworld";

/// The press's recipes, `gears:pressing` files any content mod carries in
/// `data/<namespace>/recipe/` (this one's in `data/gears/recipe/pressing/`):
/// the ingredient as the file writes it, the output and its count.
fn read_press() -> Vec<(String, String, u32)> {
    recipe_manager::of_type("gears:pressing")
        .iter()
        .filter_map(|file| {
            let v: serde_json::Value = serde_json::from_str(&file.json).ok()?;
            let result = &v["result"];
            Some((
                v["ingredient"].to_string(),
                result["id"].as_str()?.to_owned(),
                result["count"].as_u64().unwrap_or(1) as u32,
            ))
        })
        .collect()
}

/// The chest under a crafting table crafts once from its first nine slots,
/// as a 3 by 3 grid (`recipe_manager::craft`): the result and what the
/// recipe leaves go to its other slots.
fn auto_craft(player: &str, t: &BlockTarget) {
    let (d, x, y, z) = (t.dimension.as_str(), t.x, t.y - 1, t.z);
    let grid: Vec<Option<recipe_manager::ItemCount>> = (0..9)
        .map(|slot| {
            stacks::container_stack(d, x, y, z, slot).map(|s| recipe_manager::ItemCount {
                item: s.item,
                count: s.count,
            })
        })
        .collect();
    let Some(crafted) = recipe_manager::craft(&grid, 3) else {
        players::tell(player, "The chest below crafts nothing");
        return;
    };
    let mut made = vec![crafted.output.clone()];
    for slot in 0..9u32 {
        let Some(mut s) = stacks::container_stack(d, x, y, z, slot) else {
            continue;
        };
        s.count -= 1;
        let _ = stacks::set_container_stack(d, x, y, z, slot, (s.count > 0).then_some(&s));
        if let Some(Some(rest)) = crafted.remainders.get(slot as usize) {
            made.push(rest.clone());
        }
    }
    // Into the first free slot past the grid; dropped when there is none.
    for m in made {
        let free = (9..27).find(|&slot| stacks::container_stack(d, x, y, z, slot).is_none());
        let put = free.is_some_and(|slot| {
            let stack = stacks::Stack {
                item: m.item.clone(),
                count: m.count,
                damage: 0,
                name: None,
                enchantments: Vec::new(),
                data: None,
            };
            stacks::set_container_stack(d, x, y, z, slot, Some(&stack))
        });
        if !put {
            let _ = items::drop(
                d,
                f64::from(t.x) + 0.5,
                f64::from(t.y) + 1.0,
                f64::from(t.z) + 0.5,
                &m.item,
                m.count,
            );
        }
    }
    players::tell(
        player,
        &format!(
            "Crafted {} {} ({})",
            crafted.output.count, crafted.output.item, crafted.recipe
        ),
    );
}

/// The crate's screen: its slots 0 and 1 as a press's input and output, the
/// player's inventory where a container screen has it, a bar and a button.
fn press_screen() -> menus::Layout {
    use menus::{Button, Label, Layout, Progress, SlotPos, Widget};
    let mut slots = vec![
        SlotPos {
            slot: 0,
            x: 56,
            y: 35,
        },
        SlotPos {
            slot: 1,
            x: 116,
            y: 35,
        },
    ];
    // The crate has 27 slots; the player's 27 main and 9 hotbar follow.
    for k in 0..27 {
        slots.push(SlotPos {
            slot: 27 + k,
            x: 8 + (k as i32 % 9) * 18,
            y: 84 + (k as i32 / 9) * 18,
        });
    }
    for k in 0..9 {
        slots.push(SlotPos {
            slot: 54 + k,
            x: 8 + k as i32 * 18,
            y: 142,
        });
    }
    Layout {
        width: 176,
        height: 166,
        slots,
        widgets: vec![
            Widget::Label(Label {
                x: 8,
                y: 6,
                text: "Gear Press".into(),
                color: 0x40_40_40,
            }),
            Widget::Label(Label {
                x: 8,
                y: 72,
                text: "Inventory".into(),
                color: 0x40_40_40,
            }),
            Widget::Progress(Progress {
                x: 79,
                y: 39,
                width: 26,
                height: 8,
                value: 0,
                max: 1,
                vertical: false,
                color: 0x55_AA_55,
                background: 0x8B_8B_8B,
            }),
            Widget::Button(Button {
                id: 1,
                x: 8,
                y: 52,
                width: 40,
                height: 16,
                label: "Ping".into(),
            }),
        ],
    }
}

/// The notebook: its note in a field, Save and Close.
fn notebook(note: &str) -> menus::Layout {
    use menus::{Button, Label, Layout, TextField, Widget};
    Layout {
        width: 200,
        height: 86,
        slots: Vec::new(),
        widgets: vec![
            Widget::Label(Label {
                x: 8,
                y: 8,
                text: "Notebook".into(),
                color: 0x40_40_40,
            }),
            Widget::TextField(TextField {
                id: 1,
                x: 8,
                y: 24,
                width: 184,
                value: note.into(),
                max_length: 64,
            }),
            Widget::Button(Button {
                id: 2,
                x: 8,
                y: 56,
                width: 60,
                height: 20,
                label: "Save".into(),
            }),
            Widget::Button(Button {
                id: 3,
                x: 74,
                y: 56,
                width: 60,
                height: 20,
                label: "Close".into(),
            }),
        ],
    }
}

struct Gears {
    /// Ticks seen by each counter since its last write.
    ticks: Vec<([i32; 3], u32)>,
    /// The floating gear above each counter (capability `entities`).
    gears: Vec<([i32; 3], bool)>,
    /// The turn of the gears, in degrees (capability `entities`).
    /// The recipes of the counter's press: (ingredient, output, count), read
    /// at the first tick that needs them (capability-free `recipe-manager`).
    press: Vec<(String, String, u32)>,
    /// Each lift on its way up: its position, the load and the ticks gone.
    lifts: Vec<([i32; 3], Contraption, u32)>,
    /// What each player last typed into the notebook, before Save.
    drafts: Vec<(String, String)>,
    /// The players who opened a crate's screen, fed its progress bar.
    viewers: Vec<String>,
    /// Each mill turning: its position, its wheel and the wheel's turn.
    mills: Vec<([i32; 3], Structure, f32)>,
}

/// The gear over a counter: an iron trapdoor's model a tenth of a block above
/// it, turning about its upright axis while the counter runs, with an iron
/// ingot lying on it and turning with it (`visuals`).
fn gear(dimension: &str, x: i32, y: i32, z: i32, running: bool) {
    let speed = if running { 2.0 * ROTATION_STEP } else { 0.0 };
    let part = visuals::Part {
        state: "minecraft:iron_trapdoor[half=bottom]".into(),
        axis: 1,
        pivot: (0.5, 1.1, 0.5),
        offset: (0.0, 1.1, 0.0),
        speed,
        angle: 0.0,
        velocity: (0.0, 0.0, 0.0),
        item: false,
        scale: 1.0,
    };
    // An iron ingot lying on the gear, half size as an item frame shows it,
    // turning with it about the upright axis.
    let ingot = visuals::Part {
        state: "minecraft:iron_ingot".into(),
        axis: 1,
        pivot: (0.5, 1.32, 0.5),
        offset: (0.0, 0.82, 0.0),
        speed,
        angle: 0.0,
        velocity: (0.0, 0.0, 0.0),
        item: true,
        scale: 0.5,
    };
    let _ = visuals::set(dimension, x, y, z, &[part, ingot]);
    // Laid flat first, a quarter turn about x (`set-orientations`).
    let [qx, qy, qz, qw] = block::rotation_degrees(Axis::X, 90.0);
    let flat = visuals::Orientation {
        part: 1,
        rotation: (qx, qy, qz, qw),
    };
    let _ = visuals::set_orientations(dimension, x, y, z, &[flat]);
}

/// A belt's rubber, a block's sixteenth on top of it (`gears:belt_strip`,
/// never placed), running east as fast as the belt carries: its
/// `gears:block/belt` faces drawn from `gears:block/belt_scroll`, the same
/// ribs twice as tall, half a height a block (`visuals.set-scrolls`).
fn belt_strip(pos: BlockPos) {
    let strip = visuals::Part {
        state: "gears:belt_strip".into(),
        axis: 1,
        pivot: (0.5, 0.5, 0.5),
        offset: (0.0, 1.0, 0.0),
        speed: 0.0,
        angle: 0.0,
        velocity: (0.0, 0.0, 0.0),
        item: false,
        scale: 1.0,
    };
    let (x, y, z) = (pos.x, pos.y, pos.z);
    let _ = visuals::set(DIMENSION, x, y, z, &[strip]);
    let scroll = visuals::Scroll {
        part: 0,
        from: "gears:block/belt".into(),
        to: "gears:block/belt_scroll".into(),
        speed: -0.02,
        span: 0.5,
    };
    let _ = visuals::set_scrolls(DIMENSION, x, y, z, &[scroll]);
}

/// A crate's water (its tank, or the lowest crate's under it), drawn on its
/// lid: a quarter block deep when the tank is full.
fn crate_water(dimension: &str, x: i32, y: i32, z: i32) {
    let Some(tank) = transfer::get_tank(dimension, x, y, z) else {
        return;
    };
    let boxes: Vec<visuals::FluidBox> = tank
        .content
        .filter(|_| tank.capacity > 0)
        .map(|f| {
            let depth = 0.25 * f.amount as f32 / tank.capacity as f32;
            visuals::FluidBox {
                fluid: f.fluid,
                min: (0.0625, 1.0, 0.0625),
                max: (0.9375, 1.0 + depth, 0.9375),
            }
        })
        .into_iter()
        .collect();
    let _ = visuals::set_fluids(dimension, x, y, z, &boxes);
}

/// A drill's block entity: the ticks since it last mined.
struct Drill {
    ticks: u8,
}

impl BlockEntity for Drill {
    fn new(_pos: BlockPos, _state: &BlockState) -> Self {
        Self { ticks: 0 }
    }
    fn load_additional(&mut self, data: &[u8]) {
        self.ticks = data.first().copied().unwrap_or(0);
    }
    fn save_additional(&self, out: &mut Vec<u8>) {
        out.push(self.ticks);
    }
    /// It mines the block under it as an iron pickaxe would, a second for
    /// each point of its hardness (`world.block-info`), its cracks showing
    /// as it goes (`effects.destroy-progress`), the drops into the
    /// inventory above it (`world.break-block`). Leaves (`#minecraft:leaves`,
    /// `world.block-tags`) it cuts as shears do, so they drop themselves.
    /// What nothing breaks (bedrock) it leaves; air it leaves too, hurting
    /// what stands there every half second (`entities::hurt`).
    fn tick(&mut self, level: &Level, pos: BlockPos, _state: &BlockState) {
        let under = pos.below();
        let d = level.dimension();
        let below = world::get_block(d, under.x, under.y, under.z).unwrap_or_default();
        if level.get_block_state(under).is_air() {
            self.ticks = (self.ticks + 1) % 10;
            if self.ticks == 0 {
                let (x, y, z) = (under.x, under.y, under.z);
                let near = entities::living_nearby(
                    d,
                    f64::from(x) + 0.5,
                    f64::from(y),
                    f64::from(z) + 0.5,
                    1.0,
                );
                let inside = |l: &Living| {
                    l.x.floor() as i32 == x && l.y.floor() as i32 == y && l.z.floor() as i32 == z
                };
                for l in near.iter().filter(|l| inside(l)) {
                    entities::hurt(l.id, 2.0, "minecraft:generic");
                }
            }
            return;
        }
        let info = world::block_info(&below).filter(|i| i.hardness >= 0.0);
        let Some(info) = info else {
            self.ticks = 0;
            return;
        };
        let needed = ((info.hardness * 20.0) as u32).clamp(1, 250) as u8;
        self.ticks = self.ticks.saturating_add(1);
        let stage = (u32::from(self.ticks) * 10 / u32::from(needed)).min(9) as i8;
        let _ = effects::destroy_progress(d, under.x, under.y, under.z, 0, stage);
        if self.ticks < needed {
            return;
        }
        self.ticks = 0;
        let _ = effects::destroy_progress(d, under.x, under.y, under.z, 0, -1);
        let tool = match BlockState::parse(&below).is_tag("minecraft:leaves") {
            true => "minecraft:shears",
            false => "minecraft:iron_pickaxe",
        };
        let _ = world::break_block(
            d,
            under.x,
            under.y,
            under.z,
            Some(tool),
            &[],
            Some((pos.x, pos.y + 1, pos.z)),
        );
    }
}

/// Clicked on its top, uses cobblestone on the block above as a player would (`world.use-item`):
/// a lever there flips, else cobblestone goes on top. It tells itself
/// through `peers`, heard next tick.
struct Deployer;

impl Block for Deployer {
    fn use_without_item(
        &self,
        _state: &BlockState,
        level: &Level,
        pos: BlockPos,
        player: &str,
        hit: &BlockHitResult,
    ) -> InteractionResult {
        // Only a click on its top works it (`clicked-face`).
        if hit.direction != Some(Direction::Up) {
            let face = hit.direction.map_or("an unknown face", Direction::get_name);
            players::tell(
                player,
                &format!("The deployer works from its top, not {face}"),
            );
            return InteractionResult::Consume;
        }
        let above = pos.above();
        let _ = world::use_item(
            level.dimension(),
            above.x,
            above.y,
            above.z,
            1,
            "minecraft:cobblestone",
        );
        let _ = peers::send("gears", b"deployed");
        InteractionResult::Success
    }
}

impl ServerMod for Gears {
    fn init() -> Result<(Self, Registration), String> {
        // The drill and the deployer as types of their own (`block`).
        block::register_entity::<Drill>(DRILL)?;
        block::register(DEPLOYER, Deployer)?;
        let this = Self {
            ticks: Vec::new(),
            gears: Vec::new(),
            press: Vec::new(),
            lifts: Vec::new(),
            viewers: Vec::new(),
            drafts: Vec::new(),
            mills: Vec::new(),
        };
        // The crate holds items (capability `containers`) and opens as a chest.
        containers::register(CRATE)?;
        // An entity type of the mod's own (capability `entities`): a gear
        // the size of a small armor stand, worn as the counter's block.
        entity_types::register(&entity_types::EntityType {
            id: ROLLER.into(),
            model: COUNTER.into(),
            small: true,
            gravity: true,
            health: 10.0,
        })?;
        Ok((this, Registration::new().on_blocks().on_tick()))
    }

    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(BELT)
            || state.starts_with(GAUGE)
            || state.starts_with(LIFT)
            || state.starts_with(MILL)
        {
            // Data under it, so the server ticks it.
            let _ = block_entities::set(DIMENSION, pos.x, pos.y, pos.z, &[0]);
        }
        if state.starts_with(CRATE) {
            // Hoppers put one iron ingot at a time in slot 0 and never take it
            // back out (`set-hopper-rule`), as a depot takes an item only
            // while empty.
            let rule = containers::HopperRule {
                insert: true,
                extract: false,
                max: 1,
            };
            let _ = containers::set_hopper_rule(DIMENSION, pos.x, pos.y, pos.z, 0, Some(rule));
            // A tank of 4 buckets, filled by a water bucket used on it.
            let _ = transfer::add_tank(DIMENSION, pos.x, pos.y, pos.z, 4000);
            // Crates stacked are one: each above the lowest hands out its
            // items and tank (`transfer::forward`), as Create's vaults do.
            let mut lowest = pos.y;
            while world::get_block(DIMENSION, pos.x, lowest - 1, pos.z)
                .is_some_and(|s| s.starts_with(CRATE))
            {
                lowest -= 1;
            }
            if lowest != pos.y {
                let to = Some((pos.x, lowest, pos.z));
                let _ = transfer::forward(DIMENSION, pos.x, pos.y, pos.z, to);
            }
        }
        if state.starts_with(BELT) {
            belt_strip(pos);
        }
        if state.starts_with(COUNTER) {
            // Synced: players' games have its seconds too (`block_data`).
            let _ = block_entities::set_synced(DIMENSION, pos.x, pos.y, pos.z, &0u32.to_le_bytes());
            // A battery on it, charged each second it runs (capability
            // `transfer`), and the placer's count of counters, saved on them
            // (`attachments`).
            let _ = transfer::add_battery(DIMENSION, pos.x, pos.y, pos.z, 1000);
            let me = attachments::Target::Player(player.id.clone());
            let placed = attachments::get(&me, "counters")
                .and_then(|d| <[u8; 4]>::try_from(d).ok())
                .map_or(0, u32::from_le_bytes);
            let _ = attachments::set(&me, "counters", &(placed + 1).to_le_bytes());
            // A gear above it, turning while the counter runs.
            gear(DIMENSION, pos.x, pos.y, pos.z, true);
            self.gears.push(([pos.x, pos.y, pos.z], true));
        }
    }

    /// The game takes the gear away with its block.
    fn block_broken(&mut self, _player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(COUNTER) {
            self.gears.retain(|(p, _)| *p != [pos.x, pos.y, pos.z]);
        }
    }

    /// Each server tick the presses' bars move on.
    fn tick(&mut self, tick: u64) {
        let progress = (tick % 40) as i32;
        for v in &self.viewers {
            menus::set_data(v, 0, progress);
            menus::set_data(v, 1, 40);
        }
    }

    /// The press screen's button, and the notebook's.
    fn menu_button(&mut self, player: &str, id: u32) {
        match id {
            1 => {
                players::tell(player, "The press says hello");
            }
            2 => {
                let draft = self
                    .drafts
                    .iter()
                    .find(|(p, _)| p == player)
                    .map(|(_, d)| d.clone())
                    .unwrap_or_default();
                let me = attachments::Target::Player(player.to_owned());
                let _ = attachments::set(&me, "note", draft.as_bytes());
                players::tell(player, &format!("Saved: {draft}"));
            }
            3 => menus::close(player),
            _ => {}
        }
    }

    /// What the player typed into the notebook.
    fn menu_text(&mut self, player: &str, _id: u32, value: &str) {
        self.drafts.retain(|(p, _)| p != player);
        self.drafts.push((player.to_owned(), value.to_owned()));
    }

    /// Each tick the rollers roll toward the nearest player within 16
    /// blocks, turn their gear and count their age in their own bytes.
    fn entity_tick(&mut self, rollers: &[entity_types::Entity]) {
        let online = players::online();
        for r in rollers {
            let age = <[u8; 4]>::try_from(r.data.as_slice()).map_or(0, u32::from_le_bytes) + 1;
            entity_types::set_data(r.id, &age.to_le_bytes());
            entity_types::set_pose(r.id, "head", (age * 9 % 360) as f32, 0.0, 0.0);
            let near = online
                .iter()
                .filter(|p| p.dimension == r.dimension)
                .map(|p| (p.x - r.x, p.z - r.z))
                .filter(|(dx, dz)| dx * dx + dz * dz < 256.0)
                .min_by(|a, b| (a.0 * a.0 + a.1 * a.1).total_cmp(&(b.0 * b.0 + b.1 * b.1)));
            let Some((dx, dz)) = near else { continue };
            let d = (dx * dx + dz * dz).sqrt();
            if d < 2.0 || !r.on_ground {
                continue;
            }
            let yaw = (-dx.atan2(dz)).to_degrees() as f32;
            entity_types::move_to(r.id, r.x, r.y, r.z, yaw);
            entity_types::set_velocity(r.id, dx / d * 0.15, r.vy, dz / d * 0.15);
        }
    }

    /// A roller right-clicked hops and says how old it is.
    fn entity_interacted(&mut self, id: u32, player: &str, _item: &str) {
        let Some(r) = entity_types::get(id) else {
            return;
        };
        let age = <[u8; 4]>::try_from(r.data.as_slice()).map_or(0, u32::from_le_bytes);
        entity_types::set_velocity(id, r.vx, 0.5, r.vz);
        players::tell(
            player,
            &format!("Roller #{id}: {} ticks old, {} health", age, r.health),
        );
    }

    /// A roller broken drops three iron nuggets where it stood.
    fn entity_died(&mut self, id: u32, _kind: &str) {
        if let Some(r) = entity_types::get(id) {
            let _ = items::drop(
                &r.dimension,
                r.x,
                r.y + 0.3,
                r.z,
                "minecraft:iron_nugget",
                3,
            );
        }
    }

    /// The player left a screen of the mod's.
    fn menu_closed(&mut self, player: &str) {
        self.viewers.retain(|v| v != player);
    }

    /// A message from a server mod (here, from itself).
    fn peer_message(&mut self, from: &str, data: &[u8]) {
        players::broadcast(&format!(
            "heard {} from {from}",
            String::from_utf8_lossy(data)
        ));
    }

    /// A gear crate used on a chicken catches it: the chicken is gone, no
    /// drops (`entities::discard`), as Create's burner catches a blaze.
    fn item_used_on_entity(
        &mut self,
        player: &str,
        item: &str,
        _sneaking: bool,
        target: &Living,
    ) -> bool {
        if item != CRATE || target.kind != "minecraft:chicken" {
            return false;
        }
        if entities::discard(target.id) {
            players::tell(player, "Caught a chicken");
        }
        true
    }

    /// A stick used on one of the mod's blocks shows its state and takes
    /// the click (`item_used`).
    fn item_used(
        &mut self,
        player: &str,
        item: &str,
        sneaking: bool,
        target: Option<&BlockTarget>,
    ) -> bool {
        match target {
            Some(t) if item == "minecraft:stick" && t.state == "minecraft:crafting_table" => {
                auto_craft(player, t);
                true
            }
            // A water bucket pours a bucket into a crate's tank, which shows
            // as water on its lid (`visuals::set_fluids`).
            Some(t) if item == "minecraft:water_bucket" && t.state.starts_with(CRATE) => {
                let water = transfer::FluidStack {
                    fluid: "minecraft:water".into(),
                    amount: 1000,
                };
                transfer::fill(&t.dimension, t.x, t.y, t.z, &water, false);
                crate_water(&t.dimension, t.x, t.y, t.z);
                true
            }
            // Sneaking, a stick on one of the mod's blocks lets a roller out.
            Some(t) if item == "minecraft:stick" && sneaking && t.state.starts_with("gears:") => {
                let spawned = entity_types::spawn(
                    ROLLER,
                    &t.dimension,
                    f64::from(t.x) + 0.5,
                    f64::from(t.y) + 1.0,
                    f64::from(t.z) + 0.5,
                    0.0,
                );
                match spawned {
                    Ok(id) => players::tell(player, &format!("Roller #{id} is out")),
                    Err(e) => players::tell(player, &format!("No roller: {e}")),
                };
                true
            }
            Some(t) if item == "minecraft:stick" && t.state.starts_with("gears:") => {
                players::tell(player, &format!("This is {}", t.state));
                true
            }
            // A stick waved in the air: a diamond and a burst of speed
            // (capability `console`, through the SDK's `player_ops`), and
            // the sticks they carry counted (`players::inventory`).
            // Sneaking, a notebook: a panel with no block behind it, its
            // note kept on the player (capabilities `menus`, attachments).
            None if item == "minecraft:stick" && sneaking => {
                let me = attachments::Target::Player(player.to_owned());
                let note = attachments::get(&me, "note")
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                menus::open_panel(player, &notebook(&note));
                true
            }
            None if item == "minecraft:stick" => {
                let sticks: u32 = players::inventory(player)
                    .unwrap_or_default()
                    .iter()
                    .flatten()
                    .filter(|s| s.item == "minecraft:stick")
                    .map(|s| s.count)
                    .sum();
                // The stick keeps how often it was waved, in the mod's own
                // bytes on it, and is named for it (capability `stacks`).
                if let Some(slot) = players::selected_slot(player)
                    && let Some(mut held) = stacks::player_stack(player, slot)
                {
                    let waves = held
                        .data
                        .as_deref()
                        .and_then(|d| <[u8; 4]>::try_from(d).ok())
                        .map_or(0, u32::from_le_bytes)
                        + 1;
                    held.data = Some(waves.to_le_bytes().to_vec());
                    held.name = Some(format!("Wand ({waves})"));
                    stacks::set_player_stack(player, slot, Some(&held));
                }
                let _ = player_ops::give(player, "minecraft:diamond", 1);
                let _ = player_ops::effect(player, "minecraft:speed", 5, 1, false);
                players::tell(
                    player,
                    &format!("You carry {sticks} sticks; here is a diamond"),
                );
                true
            }
            _ => false,
        }
    }

    /// A block next to a counter changed (`block_neighbor_changed`): it rings.
    fn block_neighbor_changed(
        &mut self,
        dimension: &str,
        pos: Pos,
        _state: &str,
        _from: Pos,
        _from_state: &str,
    ) {
        let _ = effects::play_sound(
            dimension,
            pos.x,
            pos.y,
            pos.z,
            "minecraft:block.note_block.bell",
            1.0,
            1.0,
        );
    }

    fn ticking_blocks(&self) -> Vec<String> {
        vec![
            COUNTER.into(),
            BELT.into(),
            GAUGE.into(),
            LIFT.into(),
            MILL.into(),
        ]
    }

    fn block_tick(&mut self, dimension: &str, pos: Pos, state: &str) {
        if state.starts_with(MILL) {
            // A mill turns its wheel 3 degrees a tick about x.
            let at = [pos.x, pos.y, pos.z];
            if let Some((_, wheel, turn)) = self.mills.iter_mut().find(|(p, _, _)| *p == at) {
                *turn = (*turn + 2.0 * ROTATION_STEP) % 360.0;
                wheel.set_rotation(0.0, *turn);
            }
            return;
        }
        if state.starts_with(LIFT) {
            // A lift on its way raises its load 3/60 of a block a tick, then
            // sets it down as blocks (`horizon_mod_sdk::contraption`).
            let at = [pos.x, pos.y, pos.z];
            if let Some(i) = self.lifts.iter().position(|(p, _, _)| *p == at) {
                self.lifts[i]
                    .1
                    .move_by([0.0, 3.0 / f64::from(LIFT_TICKS), 0.0]);
                self.lifts[i].2 += 1;
                if self.lifts[i].2 >= LIFT_TICKS {
                    let (_, load, _) = self.lifts.remove(i);
                    let _ = load.disassemble();
                }
            }
            return;
        }
        if state.starts_with(GAUGE) {
            // Five of signal for each item lying on it (capability
            // `redstone`'s output: a trapped chest's analog signal).
            let found = items::nearby(
                dimension,
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 1.2,
                f64::from(pos.z) + 0.5,
                0.7,
            );
            let level = (found.len() as u8).saturating_mul(5).min(15);
            let _ = redstone::set_signal(dimension, pos.x, pos.y, pos.z, level);
            // Comparators read one for each item (`set-comparator-signal`).
            let count = Some((found.len() as u8).min(15));
            let _ = redstone::set_comparator_signal(dimension, pos.x, pos.y, pos.z, count);
            return;
        }
        if state.starts_with(BELT) {
            // A belt moves what stands on it east (capabilities `items`
            // and `players`).
            let (x, y, z) = (
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 1.2,
                f64::from(pos.z) + 0.5,
            );
            for item in items::nearby(dimension, x, y, z, 0.8) {
                items::push(item.id, 0.04, 0.0, 0.0);
            }
            for p in players::online() {
                let on = p.dimension == dimension
                    && (p.x - x).abs() < 0.6
                    && (p.z - z).abs() < 0.6
                    && (p.y - (y - 0.2)).abs() < 0.6;
                if on {
                    players::push(&p.id, 0.06, 0.0, 0.0);
                }
            }
            // Mobs standing on it too (`entities::push`).
            for m in entities::living_nearby(dimension, x, y - 0.2, z, 0.8) {
                if m.player.is_none() && (m.x - x).abs() < 0.6 && (m.z - z).abs() < 0.6 {
                    entities::push(m.id, 0.06, 0.0, 0.0);
                }
            }
            return;
        }
        // A redstone signal stops the counter (capability `redstone`), and
        // its gear: players' games are told only when that changes.
        let at = [pos.x, pos.y, pos.z];
        let running = !redstone::power(dimension, pos.x, pos.y, pos.z).is_some_and(|p| p > 0);
        match self.gears.iter_mut().find(|(p, _)| *p == at) {
            Some((_, was)) if *was == running => {}
            Some((_, was)) => {
                *was = running;
                gear(dimension, pos.x, pos.y, pos.z, running);
            }
            None => {
                self.gears.push((at, running));
                gear(dimension, pos.x, pos.y, pos.z, running);
            }
        }
        if !running {
            return;
        }
        let seen = match self.ticks.iter_mut().find(|(p, _)| *p == at) {
            Some((_, n)) => n,
            None => {
                self.ticks.push((at, 0));
                &mut self.ticks.last_mut().expect("just pushed").1
            }
        };
        *seen += 1;
        if *seen < 20 {
            return;
        }
        *seen = 0;
        let seconds = block_entities::get(dimension, pos.x, pos.y, pos.z)
            .and_then(|d| <[u8; 4]>::try_from(d).ok())
            .map_or(0, u32::from_le_bytes);
        let bytes = (seconds + 1).to_le_bytes();
        let _ = block_entities::set_synced(dimension, pos.x, pos.y, pos.z, &bytes);
        transfer::receive_energy(dimension, pos.x, pos.y, pos.z, 10, false);
        // The counter swallows iron ingots lying next to it (capability
        // `items`): it counts them in the nuggets it gives back.
        let mut gives: Vec<(String, u32)> = Vec::new();
        for item in items::nearby(
            dimension,
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 0.5,
            f64::from(pos.z) + 0.5,
            2.0,
        ) {
            if self.press.is_empty() {
                self.press = read_press();
            }
            let pressed = self
                .press
                .iter()
                .find(|(i, _, _)| recipe_manager::test_ingredient(i, &item.item))
                .map(|(_, out, n)| (out.clone(), *n));
            // What no press recipe takes, it smelts as a furnace would.
            let made = pressed.or_else(|| {
                recipe_manager::cook("minecraft:smelting", &item.item)
                    .map(|c| (c.output.item, c.output.count))
            });
            if let Some((out, n)) = made
                && items::take(item.id)
            {
                // Bits of what it swallowed fly off, as from a press.
                let particle = format!("minecraft:item{{item:\"{}\"}}", item.item);
                let (cx, cy, cz) = (
                    f64::from(pos.x) + 0.5,
                    f64::from(pos.y) + 1.0,
                    f64::from(pos.z) + 0.5,
                );
                let _ =
                    effects::particles(dimension, cx, cy, cz, &particle, (0.2, 0.1, 0.2), 0.05, 8);
                gives.push((out, (item.count * n).min(99)));
            }
        }
        for (out, count) in gives {
            // Into the inventory below through its top, as a hopper would;
            // what does not fit drops (capability `transfer`).
            let stack = transfer::ItemStack {
                item: out.clone(),
                count,
            };
            let left = transfer::insert_item(
                dimension,
                pos.x,
                pos.y - 1,
                pos.z,
                Some(transfer::Face::Up),
                &stack,
                false,
            );
            if left > 0 {
                let _ = items::drop(
                    dimension,
                    f64::from(pos.x) + 0.5,
                    f64::from(pos.y) + 1.0,
                    f64::from(pos.z) + 0.5,
                    &out,
                    left,
                );
            }
        }
        // A minute of running makes an iron nugget (capability `items`).
        if (seconds + 1).is_multiple_of(60) {
            let _ = effects::play_sound(
                dimension,
                pos.x,
                pos.y,
                pos.z,
                "minecraft:block.anvil.use",
                0.5,
                1.5,
            );
            let _ = effects::level_event(dimension, pos.x, pos.y + 1, pos.z, 2000, 1);
            let _ = items::drop(
                dimension,
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 1.0,
                f64::from(pos.z) + 0.5,
                "minecraft:iron_nugget",
                1,
            );
        }
    }

    fn block_used(&mut self, player: &str, dimension: &str, pos: Pos, state: &str) -> bool {
        if state.starts_with(MILL) {
            // The wheel above (one block across x, 3 x 3 in y and z) turns,
            // or is set down again as blocks.
            let at = [pos.x, pos.y, pos.z];
            if let Some(i) = self.mills.iter().position(|(p, _, _)| *p == at) {
                let (_, wheel, _) = self.mills.remove(i);
                let _ = wheel.disassemble();
                return true;
            }
            let min = [pos.x, pos.y + 1, pos.z - 1];
            let max = [pos.x, pos.y + 3, pos.z + 1];
            // One structure: players' games draw the wheel as one model.
            match Structure::assemble(dimension, min, max) {
                Ok(wheel) if !wheel.is_empty() => self.mills.push((at, wheel, 0.0)),
                Ok(_) => {}
                Err(e) => {
                    players::tell(player, &format!("The mill is stuck: {e}"));
                }
            }
            return true;
        }
        if state.starts_with(LIFT) {
            // The 3 x 3 x 3 box above the lift goes up three blocks.
            let at = [pos.x, pos.y, pos.z];
            if self.lifts.iter().any(|(p, _, _)| *p == at) {
                return true;
            }
            let min = [pos.x - 1, pos.y + 1, pos.z - 1];
            let max = [pos.x + 1, pos.y + 3, pos.z + 1];
            match Contraption::assemble(dimension, min, max) {
                Ok(load) if !load.is_empty() => {
                    // Players ride it: its blocks are solid.
                    load.set_solid(true);
                    self.lifts.push((at, load, 0));
                }
                Ok(_) => {}
                Err(e) => {
                    players::tell(player, &format!("The lift is stuck: {e}"));
                }
            }
            return true;
        }
        if state.starts_with(CRATE) {
            let _ = containers::set_name(dimension, pos.x, pos.y, pos.z, Some("Gear Crate"));
            // Slot 0 takes ingots only, every mod's too (`recipe-manager.items`);
            // slot 1 is only for taking from.
            let ingots: Vec<String> = recipe_manager::items()
                .into_iter()
                .filter(|i| i.ends_with("_ingot"))
                .collect();
            let ingots = Some(ingots);
            let _ = containers::set_slot_rule(dimension, pos.x, pos.y, pos.z, 0, ingots.as_deref());
            let _ = containers::set_slot_rule(dimension, pos.x, pos.y, pos.z, 1, Some(&[]));
            // Opened as a press of its own (capability `menus`): its input
            // and output slots, a progress bar fed each tick and a button.
            let opened = menus::open(player, dimension, pos.x, pos.y, pos.z, &press_screen());
            if opened && !self.viewers.iter().any(|v| v == player) {
                self.viewers.push(player.to_owned());
            }
            return opened;
        }
        if !state.starts_with(COUNTER) {
            return false;
        }
        let seconds = block_entities::get(dimension, pos.x, pos.y, pos.z)
            .and_then(|d| <[u8; 4]>::try_from(d).ok())
            .map_or(0, u32::from_le_bytes);
        let charge = transfer::get_battery(dimension, pos.x, pos.y, pos.z).map_or(0, |b| b.stored);
        let placed = attachments::get(&attachments::Target::Player(player.to_owned()), "counters")
            .and_then(|d| <[u8; 4]>::try_from(d).ok())
            .map_or(0, u32::from_le_bytes);
        players::tell(
            player,
            &format!(
                "This counter has run for {seconds} s, holds {charge} energy; you placed {placed} counters"
            ),
        );
        true
    }

    /// A client half asking for a position's counter (three i32 LE) gets
    /// the seconds back (capability `net`).
    fn net_message(&mut self, player: &str, data: &[u8]) {
        let Ok(raw) = <[u8; 12]>::try_from(data) else {
            return;
        };
        let at = |i: usize| i32::from_le_bytes(raw[i * 4..i * 4 + 4].try_into().expect("4 bytes"));
        if let Some(seconds) = block_entities::get(DIMENSION, at(0), at(1), at(2)) {
            let _ = net::send(Some(player), &seconds);
        }
    }
}

export_server_mod!(Gears);
