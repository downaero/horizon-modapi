//! Gears: a server mod with its own block. `gears:counter` keeps the seconds
//! it has stood as four bytes at its position (capability `block-entities`);
//! the server ticks it and saves the bytes with the world. A gear (an iron
//! ingot, capability `entities`) floats above each counter placed.
use horizon_mod_sdk::{
    export_server_mod,
    server::{
        BlockPos, Player, Pos, Registration, ServerMod, block_entities, containers, entities, net,
        players, redstone,
    },
};

const COUNTER: &str = "gears:counter";
const CRATE: &str = "gears:crate";
const DIMENSION: &str = "minecraft:overworld";

struct Gears {
    /// Ticks seen by each counter since its last write.
    ticks: Vec<([i32; 3], u32)>,
    /// The floating gear above each counter (capability `entities`).
    gears: Vec<([i32; 3], u32)>,
}

impl ServerMod for Gears {
    fn init() -> Result<(Self, Registration), String> {
        let this = Self {
            ticks: Vec::new(),
            gears: Vec::new(),
        };
        // The crate holds items (capability `containers`) and opens as a chest.
        containers::register(CRATE)?;
        Ok((this, Registration::new().on_blocks()))
    }

    fn block_placed(&mut self, _player: &Player, pos: BlockPos, state: &str) {
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

    fn ticking_blocks(&self) -> Vec<String> {
        vec![COUNTER.into()]
    }

    fn block_tick(&mut self, dimension: &str, pos: Pos, _state: &str) {
        // A redstone signal stops the counter (capability `redstone`).
        if redstone::power(dimension, pos.x, pos.y, pos.z).is_some_and(|p| p > 0) {
            return;
        }
        let at = [pos.x, pos.y, pos.z];
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
    }

    fn block_used(&mut self, player: &str, dimension: &str, pos: Pos, state: &str) -> bool {
        if state.starts_with(CRATE) {
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
