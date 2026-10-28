//! Networks of connected blocks, for server mods: shafts and gears turning
//! together, pipes carrying a fluid, cables carrying power. The mod says
//! which neighbours connect and what each source gives; this walks the
//! network and tells each member what reaches it. Plain Rust: it reads
//! nothing from the game itself.
use std::collections::{HashMap, VecDeque};

/// A block position.
pub type Pos = [i32; 3];

/// The six neighbours of a position, in a fixed order.
pub fn neighbours(p: Pos) -> [Pos; 6] {
    [
        [p[0] + 1, p[1], p[2]],
        [p[0] - 1, p[1], p[2]],
        [p[0], p[1] + 1, p[2]],
        [p[0], p[1] - 1, p[2]],
        [p[0], p[1], p[2] + 1],
        [p[0], p[1], p[2] - 1],
    ]
}

/// What reaches each member of a network from its sources.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reached {
    /// Each member reached, with the value that reached it.
    pub values: HashMap<Pos, f64>,
    /// Two sources gave values that disagree where they met (a gearbox
    /// asked to turn both ways): the network is stuck, as a mod may show.
    pub conflict: bool,
}

/// Walks from each source through `link(from, to)`, which gives the value at
/// `to` from the value at `from` (`Some(v)`: a shaft passes it on, a gear
/// reverses it, a cog doubles it) or `None` when the two do not connect. At
/// most `limit` members are visited; a member reached twice with values
/// further apart than `1e-6` marks a conflict.
pub fn propagate(
    sources: &[(Pos, f64)],
    limit: usize,
    mut link: impl FnMut(Pos, Pos, f64) -> Option<f64>,
) -> Reached {
    let mut out = Reached::default();
    let mut queue: VecDeque<(Pos, f64)> = sources.iter().copied().collect();
    while let Some((p, v)) = queue.pop_front() {
        if let Some(&seen) = out.values.get(&p) {
            if (seen - v).abs() > 1e-6 {
                out.conflict = true;
            }
            continue;
        }
        if out.values.len() >= limit {
            break;
        }
        out.values.insert(p, v);
        for n in neighbours(p) {
            if let Some(next) = link(p, n, v) {
                queue.push_back((n, next));
            }
        }
    }
    out
}

/// The members connected to `start` (through `connects(a, b)`), at most
/// `limit`: a pipe network sharing one fluid, a cable sharing one charge.
pub fn members(start: Pos, limit: usize, mut connects: impl FnMut(Pos, Pos) -> bool) -> Vec<Pos> {
    propagate(&[(start, 0.0)], limit, |a, b, _| {
        connects(a, b).then_some(0.0)
    })
    .values
    .into_keys()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shaft_line_turns_together_and_a_gear_reverses() {
        // Shafts along x from 0 to 4; a gear at 5 turns the other way.
        let shaft = |p: Pos| p[1] == 0 && p[2] == 0 && (0..=5).contains(&p[0]);
        // Crossing between a shaft and the gear, either way, reverses.
        let r = propagate(&[([0, 0, 0], 16.0)], 100, |from, to, v| {
            shaft(to).then(|| {
                if (from[0] == 5) != (to[0] == 5) {
                    -v
                } else {
                    v
                }
            })
        });
        assert_eq!(r.values.len(), 6);
        assert_eq!(r.values[&[4, 0, 0]], 16.0);
        assert_eq!(r.values[&[5, 0, 0]], -16.0);
        assert!(!r.conflict);
    }

    #[test]
    fn two_sources_that_disagree_stick_the_network() {
        let line = |p: Pos| p[1] == 0 && p[2] == 0 && (0..=3).contains(&p[0]);
        let r = propagate(&[([0, 0, 0], 8.0), ([3, 0, 0], -8.0)], 100, |_, to, v| {
            line(to).then_some(v)
        });
        assert!(r.conflict);
    }

    #[test]
    fn members_stop_at_the_limit_and_at_gaps() {
        let pipe = |p: Pos| p[1] == 0 && p[2] == 0 && p[0] != 3;
        let all = members([0, 0, 0], 100, |_, b| pipe(b) && (0..10).contains(&b[0]));
        assert_eq!(all.len(), 3);
        assert_eq!(members([0, 0, 0], 2, |_, b| pipe(b)).len(), 2);
    }
}
