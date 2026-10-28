//! Gears: a server mod with its own block. `gears:counter` keeps the seconds
//! it has stood as four bytes at its position (capability `block-entities`);
//! the server ticks it and saves the bytes with the world. A gear (an iron
//! ingot, capability `entities`) floats above each counter placed.
use horizon_mod_sdk::{
    contraption::{Contraption, ROTATION_STEP},
    export_server_mod,
    server::{
        BlockPos, BlockTarget, Player, Pos, Registration, ServerMod, attachments, block_entities,
        containers, effects, entities, files, items, net, players, redstone, transfer, world,
    },
};

const COUNTER: &str = "gears:counter";
const CRATE: &str = "gears:crate";
const BELT: &str = "gears:belt";
const GAUGE: &str = "gears:gauge";
const LIFT: &str = "gears:lift";
const MILL: &str = "gears:mill";
const DRILL: &str = "gears:drill";
/// Ticks a lift takes to raise its load three blocks.
const LIFT_TICKS: u32 = 60;
const DIMENSION: &str = "minecraft:overworld";

/// The press recipes the mod carries as data, one `input output count` a line.
fn read_press() -> Vec<(String, String, u32)> {
    let Some(bytes) = files::read("data/gears/recipes/press.txt") else {
        return Vec::new();
    };
    String::from_utf8_lossy(&bytes)
        .lines()
        .filter_map(|line| {
            let mut w = line.split_whitespace();
            Some((
                w.next()?.to_owned(),
                w.next()?.to_owned(),
                w.next()?.parse().ok()?,
            ))
        })
        .collect()
}

struct Gears {
    /// Ticks seen by each counter since its last write.
    ticks: Vec<([i32; 3], u32)>,
    /// The floating gear above each counter (capability `entities`).
    gears: Vec<([i32; 3], u32)>,
    /// The turn of the gears, in degrees (capability `entities`).
    spin: f32,
    /// The recipes of the counter's press: (input, output, count) read from
    /// the mod's own `data/gears/recipes/press.txt` (capability-free `files`).
    press: Vec<(String, String, u32)>,
    /// Each lift on its way up: its position, the load and the ticks gone.
    lifts: Vec<([i32; 3], Contraption, u32)>,
    /// Ticks the drills have seen.
    drill_ticks: u32,
    /// Each mill turning: its position, its wheel and the wheel's turn.
    mills: Vec<([i32; 3], Contraption, f32)>,
}

impl ServerMod for Gears {
    fn init() -> Result<(Self, Registration), String> {
        let this = Self {
            ticks: Vec::new(),
            gears: Vec::new(),
            spin: 0.0,
            press: read_press(),
            lifts: Vec::new(),
            drill_ticks: 0,
            mills: Vec::new(),
        };
        // The crate holds items (capability `containers`) and opens as a chest.
        containers::register(CRATE)?;
        Ok((this, Registration::new().on_blocks()))
    }

    fn block_placed(&mut self, player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(BELT)
            || state.starts_with(GAUGE)
            || state.starts_with(LIFT)
            || state.starts_with(MILL)
            || state.starts_with(DRILL)
        {
            // Data under it, so the server ticks it.
            let _ = block_entities::set(DIMENSION, pos.x, pos.y, pos.z, &[0]);
        }
        if state.starts_with(COUNTER) {
            let _ = block_entities::set(DIMENSION, pos.x, pos.y, pos.z, &0u32.to_le_bytes());
            // A battery on it, charged each second it runs (capability
            // `transfer`), and the placer's count of counters, saved on them
            // (`attachments`).
            let _ = transfer::add_battery(DIMENSION, pos.x, pos.y, pos.z, 1000);
            let me = attachments::Target::Player(player.id.clone());
            let placed = attachments::get(&me, "counters")
                .and_then(|d| <[u8; 4]>::try_from(d).ok())
                .map_or(0, u32::from_le_bytes);
            let _ = attachments::set(&me, "counters", &(placed + 1).to_le_bytes());
            // A gear above it: an iron trapdoor as a block entity, turned
            // each tick (drawn smoothly between ticks on every client).
            let at = [
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 1.1,
                f64::from(pos.z) + 0.5,
            ];
            if let Ok(handle) = entities::spawn_block(
                DIMENSION,
                at[0],
                at[1],
                at[2],
                "minecraft:iron_trapdoor[half=bottom]",
            ) {
                self.gears.push(([pos.x, pos.y, pos.z], handle));
            }
        }
    }

    fn block_broken(&mut self, _player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(COUNTER) {
            let at = [pos.x, pos.y, pos.z];
            self.gears.retain(|&(p, handle)| {
                let gone = p == at;
                if gone {
                    entities::remove(handle);
                }
                !gone
            });
        }
    }

    /// A stick used on one of the mod's blocks shows its state and takes
    /// the click (`item_used`).
    fn item_used(
        &mut self,
        player: &str,
        item: &str,
        _sneaking: bool,
        target: Option<&BlockTarget>,
    ) -> bool {
        match target {
            Some(t) if item == "minecraft:stick" && t.state.starts_with("gears:") => {
                players::tell(player, &format!("This is {}", t.state));
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
            DRILL.into(),
        ]
    }

    fn block_tick(&mut self, dimension: &str, pos: Pos, state: &str) {
        if state.starts_with(DRILL) {
            // Once a second it mines the block under it as an iron pickaxe
            // would, the drops into the inventory above it (`world.break-block`).
            self.drill_ticks = self.drill_ticks.wrapping_add(1);
            if self.drill_ticks.is_multiple_of(20) {
                let _ = world::break_block(
                    dimension,
                    pos.x,
                    pos.y - 1,
                    pos.z,
                    Some("minecraft:iron_pickaxe"),
                    Some((pos.x, pos.y + 1, pos.z)),
                );
            }
            return;
        }
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
            return;
        }
        // A redstone signal stops the counter (capability `redstone`).
        if redstone::power(dimension, pos.x, pos.y, pos.z).is_some_and(|p| p > 0) {
            return;
        }
        let at = [pos.x, pos.y, pos.z];
        // Two steps of the rotation clients are sent (360/256 degrees each) a
        // tick, so every tick turns it the same.
        self.spin = (self.spin + 2.0 * ROTATION_STEP) % 360.0;
        if let Some(&(_, handle)) = self.gears.iter().find(|(p, _)| *p == at) {
            entities::rotate(handle, self.spin, 0.0);
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
        let _ = block_entities::set(dimension, pos.x, pos.y, pos.z, &(seconds + 1).to_le_bytes());
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
            if let Some((_, out, n)) = self.press.iter().find(|(i, _, _)| *i == item.item)
                && items::take(item.id)
            {
                gives.push((out.clone(), (item.count * n).min(99)));
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
            match Contraption::assemble(dimension, min, max) {
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
            // Slot 0 takes iron ingots only; slot 1 is only for taking from.
            let ingots = Some(vec!["minecraft:iron_ingot".to_string()]);
            let _ = containers::set_slot_rule(dimension, pos.x, pos.y, pos.z, 0, ingots.as_deref());
            let _ = containers::set_slot_rule(dimension, pos.x, pos.y, pos.z, 1, Some(&[]));
            return containers::open(player, dimension, pos.x, pos.y, pos.z);
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
