//! What a server mod does to a player, through capability `console`: the
//! vanilla commands for it, every id checked so nothing else can be slipped
//! into the line. A player is given by UUID, as `players` gives them, and
//! named by their name in the command (vanilla's player-only commands take
//! no UUID). Each runs at the end of the tick, as `console::run` does.
use crate::server::{console, players};

/// The name of the online player with this UUID.
fn named(player: &str) -> Result<String, String> {
    players::online()
        .into_iter()
        .find(|p| p.id == player)
        .map(|p| p.name)
        .ok_or_else(|| format!("no player {player} online"))
}

/// An id (`minecraft:diamond`, `mymod:gear`) or a UUID, nothing else: no
/// spaces, brackets or other command syntax.
fn plain(s: &str) -> Result<&str, String> {
    let ok = !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_:./-".contains(&b));
    if ok {
        Ok(s)
    } else {
        Err(format!("{s:?} is not a plain id"))
    }
}

/// Gives `count` of `item` to the player (`/give`); what does not fit drops.
pub fn give(player: &str, item: &str, count: u32) -> Result<(), String> {
    console::run(&format!(
        "give {} {} {}",
        plain(&named(player)?)?,
        plain(item)?,
        count.max(1)
    ))
}

/// Takes up to `count` of `item` from the player (`/clear`).
pub fn take(player: &str, item: &str, count: u32) -> Result<(), String> {
    console::run(&format!(
        "clear {} {} {count}",
        plain(&named(player)?)?,
        plain(item)?
    ))
}

/// Gives an effect for `seconds` at `amplifier` (0 is level I), hiding its
/// particles or not (`/effect give`).
pub fn effect(
    player: &str,
    effect: &str,
    seconds: u32,
    amplifier: u8,
    hide_particles: bool,
) -> Result<(), String> {
    console::run(&format!(
        "effect give {} {} {} {amplifier} {hide_particles}",
        plain(&named(player)?)?,
        plain(effect)?,
        seconds.clamp(1, 1_000_000)
    ))
}

/// Takes every effect off the player (`/effect clear`).
pub fn clear_effects(player: &str) -> Result<(), String> {
    console::run(&format!("effect clear {}", plain(&named(player)?)?))
}

/// Adds experience points, or levels (`/xp add`); negative takes away.
pub fn experience(player: &str, amount: i32, levels: bool) -> Result<(), String> {
    let unit = if levels { "levels" } else { "points" };
    console::run(&format!(
        "xp add {} {amount} {unit}",
        plain(&named(player)?)?
    ))
}

/// Moves the player (`/execute in .. run tp`), to another dimension too.
pub fn teleport(player: &str, dimension: &str, x: f64, y: f64, z: f64) -> Result<(), String> {
    if ![x, y, z].iter().all(|v| v.is_finite()) {
        return Err("a position that is not a number".into());
    }
    console::run(&format!(
        "execute in {} run tp {} {x} {y} {z}",
        plain(dimension)?,
        plain(&named(player)?)?
    ))
}

/// Hurts the player by `amount` (half hearts) as a generic damage (`/damage`).
pub fn damage(player: &str, amount: f32) -> Result<(), String> {
    if !amount.is_finite() || amount <= 0.0 {
        return Err("a positive amount".into());
    }
    console::run(&format!("damage {} {amount}", plain(&named(player)?)?))
}
