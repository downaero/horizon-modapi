//! Gears: a server mod with its own block. `gears:counter` keeps the seconds
//! it has stood as four bytes at its position (capability `block-entities`);
//! the server ticks it and saves the bytes with the world. A gear (an iron
//! ingot, capability `entities`) floats above each counter placed.
use horizon_mod_sdk::{
    export_server_mod,
    server::{
        BlockPos, Player, Pos, Registration, ServerMod, block_entities, containers, effects,
        entities, items, net, players, redstone,
    },
};

const COUNTER: &str = "gears:counter";
const CRATE: &str = "gears:crate";
const BELT: &str = "gears:belt";
const GAUGE: &str = "gears:gauge";
const DIMENSION: &str = "minecraft:overworld";

struct Gears {
    /// Ticks seen by each counter since its last write.
    ticks: Vec<([i32; 3], u32)>,
    /// The floating gear above each counter (capability `entities`).
    gears: Vec<([i32; 3], u32)>,
    /// The turn of the gears, in degrees (capability `entities`).
    spin: f32,
}

impl ServerMod for Gears {
    fn init() -> Result<(Self, Registration), String> {
        let this = Self {
            ticks: Vec::new(),
            gears: Vec::new(),
            spin: 0.0,
        };
        // The crate holds items (capability `containers`) and opens as a chest.
        containers::register(CRATE)?;
        Ok((this, Registration::new().on_blocks()))
    }

    fn block_placed(&mut self, _player: &Player, pos: BlockPos, state: &str) {
        if state.starts_with(BELT) || state.starts_with(GAUGE) {
            // Data under it, so the server ticks it.
            let _ = block_entities::set(DIMENSION, pos.x, pos.y, pos.z, &[0]);
        }
        if state.starts_with(COUNTER) {
            let _ = block_entities::set(DIMENSION, pos.x, pos.y, pos.z, &0u32.to_le_bytes());
            let at = [
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 1.1,
                f64::from(pos.z) + 0.5,
            ];
            if let Ok(handle) =
                entities::spawn(DIMENSION, at[0], at[1], at[2], 0.0, "minecraft:iron_ingot")
            {
                // The counter's own block on the head, turned each tick.
                entities::set_slot(handle, "head", Some(COUNTER));
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
        vec![COUNTER.into(), BELT.into(), GAUGE.into()]
    }

    fn block_tick(&mut self, dimension: &str, pos: Pos, state: &str) {
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
        self.spin = (self.spin + 4.0) % 360.0;
        if let Some(&(_, handle)) = self.gears.iter().find(|(p, _)| *p == at) {
            entities::set_pose(handle, "head", 0.0, self.spin, 0.0);
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
        // The counter swallows iron ingots lying next to it (capability
        // `items`): it counts them in the nuggets it gives back.
        let mut swallowed = 0;
        for item in items::nearby(
            dimension,
            f64::from(pos.x) + 0.5,
            f64::from(pos.y) + 0.5,
            f64::from(pos.z) + 0.5,
            2.0,
        ) {
            if item.item == "minecraft:iron_ingot" && items::take(item.id) {
                swallowed += item.count;
            }
        }
        if swallowed > 0 {
            let _ = items::drop(
                dimension,
                f64::from(pos.x) + 0.5,
                f64::from(pos.y) + 1.0,
                f64::from(pos.z) + 0.5,
                "minecraft:iron_nugget",
                (swallowed * 9).min(99),
            );
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
        if state.starts_with(CRATE) {
            let _ = containers::set_name(dimension, pos.x, pos.y, pos.z, Some("Gear Crate"));
            return containers::open(player, dimension, pos.x, pos.y, pos.z);
        }
        if !state.starts_with(COUNTER) {
            return false;
        }
        let seconds = block_entities::get(dimension, pos.x, pos.y, pos.z)
            .and_then(|d| <[u8; 4]>::try_from(d).ok())
            .map_or(0, u32::from_le_bytes);
        players::tell(player, &format!("This counter has run for {seconds} s"));
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
