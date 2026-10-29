//! Capability `visuals`: model parts a server mod draws at its blocks, each
//! turning at a constant speed, sent to players' games once and turned by
//! them every frame (what Flywheel's instances do for Create). This is the
//! form both ends share: the parts, and the payload that carries them.

/// The most parts at one position.
pub const PARTS: usize = 16;

/// One model drawn at a block position.
#[derive(Clone, Debug, PartialEq)]
pub struct VisualPart {
    /// The block state whose model is drawn, `mymod:shaft[axis=x]`.
    pub state: String,
    /// The axis it turns about: 0 x, 1 y, 2 z.
    pub axis: u8,
    /// The point it turns about, in blocks from the position's corner.
    pub pivot: [f32; 3],
    /// Where its model's corner is, in blocks from the position's corner.
    pub offset: [f32; 3],
    /// Degrees a tick, counterclockwise looking along the axis towards its
    /// negative end; negative turns the other way.
    pub speed: f32,
    /// Its angle, in degrees, at the tick the parts were set.
    pub angle: f32,
    /// Blocks a tick the whole part moves, from the tick it was set.
    pub velocity: [f32; 3],
    /// `state` names an item (`minecraft:iron_ingot`), drawn as an item
    /// frame shows it, a block across, its centre half a block from `offset`.
    pub item: bool,
    /// How large it is drawn, about its model's centre: 1 is as it is.
    pub scale: f32,
    /// A texture running across it (`visuals.set-scrolls`).
    pub scroll: Option<Scroll>,
    /// A unit quaternion (x, y, z, w) turning its model about `pivot`
    /// before its spin about `axis` (`visuals.set-orientations`).
    pub orientation: Option<[f32; 4]>,
    /// A box of fluid instead (`visuals.set-fluids`): `state` names the
    /// fluid, and the box is from `min` to `max` in blocks from the
    /// position's corner. Kept after the parts, never among them.
    pub fluid: Option<FluidBox>,
}

/// A box of a fluid drawn at a block: a tank's or a basin's content.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FluidBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl VisualPart {
    /// A box of `fluid` from `min` to `max` (`visuals.set-fluids`).
    pub fn fluid(fluid: &str, min: [f32; 3], max: [f32; 3]) -> Self {
        VisualPart {
            state: fluid.to_owned(),
            axis: 1,
            pivot: [0.0; 3],
            offset: [0.0; 3],
            speed: 0.0,
            angle: 0.0,
            velocity: [0.0; 3],
            item: false,
            scale: 1.0,
            scroll: None,
            orientation: None,
            fluid: Some(FluidBox { min, max }),
        }
    }
}

/// The most fluid boxes at one position.
pub const FLUIDS: usize = 16;

/// A quaternion as a mod gave it, made unit length; none for one that is
/// not finite or too short to turn by.
pub fn unit_quaternion(q: [f32; 4]) -> Option<[f32; 4]> {
    let len = q.iter().map(|c| c * c).sum::<f32>().sqrt();
    (q.iter().all(|c| c.is_finite()) && len > 1e-4).then(|| q.map(|c| c / len))
}

/// A texture running across a part as it works, as Create's belts'
/// (`SpriteShiftEntry`): the faces its model draws with sprite `from` are
/// drawn with `to`, at the same place in it, moved down by an offset that
/// grows with the game time.
#[derive(Clone, Debug, PartialEq)]
pub struct Scroll {
    /// A sprite of the part's model, `create:block/belt/belt`.
    pub from: String,
    /// The sprite drawn in its place, `create:block/belt/belt_scroll`.
    pub to: String,
    /// `to`'s heights a tick it runs down; negative runs up.
    pub speed: f32,
    /// The part of `to`'s height it runs over before it starts again (more
    /// than 0, at most 1): 0.5 for a texture that repeats every half.
    pub span: f32,
}

impl Scroll {
    /// How far down `to` it has run at game time `ticks` (a fraction
    /// between), in `to`'s heights: from 0 up to `span`.
    pub fn offset_at(&self, ticks: f64) -> f32 {
        let span = f64::from(self.span);
        let cycles = f64::from(self.speed) * ticks / span;
        ((cycles - cycles.floor()) * span) as f32
    }
    /// Checks what a mod gave: two sprite names, a finite speed of at most
    /// 16 heights a tick, a span within (0, 1].
    pub fn check(&self) -> Result<(), String> {
        let name = |s: &str| !s.is_empty() && s.len() <= 256;
        if !name(&self.from) || !name(&self.to) {
            return Err("a scroll names two sprites".into());
        }
        if !self.speed.is_finite() || self.speed.abs() > 16.0 {
            return Err("a scroll's speed is at most 16 heights a tick".into());
        }
        if !(self.span > 0.0 && self.span <= 1.0) {
            return Err("a scroll's span is more than 0 and at most 1".into());
        }
        Ok(())
    }
}

impl VisualPart {
    /// How far it has moved `ticks` after the tick it was set (a fraction
    /// between), kept within the 16 blocks a part may reach.
    pub fn moved_at(&self, ticks: f64) -> [f32; 3] {
        self.velocity
            .map(|v| (f64::from(v) * ticks).clamp(-16.0, 16.0) as f32)
    }
    /// Its angle `ticks` after the tick it was set (a fraction between).
    pub fn angle_at(&self, ticks: f64) -> f32 {
        ((f64::from(self.angle) + f64::from(self.speed) * ticks) % 360.0) as f32
    }
    /// Checks what a mod gave: a finite speed, angle, pivot and offset
    /// within a few blocks, an axis.
    pub fn check(&self) -> Result<(), String> {
        let near = |v: &[f32; 3]| v.iter().all(|c| c.is_finite() && c.abs() <= 16.0);
        if self.axis > 2 {
            return Err(format!("axis {}: 0 x, 1 y, 2 z", self.axis));
        }
        if !near(&self.pivot) || !near(&self.offset) {
            return Err("pivot and offset are within 16 blocks".into());
        }
        if !self.speed.is_finite() || !self.angle.is_finite() || self.speed.abs() > 360.0 {
            return Err("speed is at most 360 degrees a tick, angle a number".into());
        }
        if !self
            .velocity
            .iter()
            .all(|c| c.is_finite() && c.abs() <= 16.0)
        {
            return Err("velocity is at most 16 blocks a tick".into());
        }
        if self.state.is_empty() || self.state.len() > 256 {
            return Err("a part names a block state or an item".into());
        }
        if !(self.scale.is_finite() && self.scale > 0.0 && self.scale <= 4.0) {
            return Err("scale is more than 0 and at most 4".into());
        }
        if let Some(scroll) = &self.scroll {
            scroll.check()?;
        }
        if let Some(q) = self.orientation
            && unit_quaternion(q).is_none()
        {
            return Err("an orientation is a quaternion of finite numbers, not all 0".into());
        }
        if let Some(b) = &self.fluid {
            let inside = |v: &[f32; 3]| {
                v.iter()
                    .all(|c| c.is_finite() && (-16.0..=32.0).contains(c))
            };
            if !inside(&b.min) || !inside(&b.max) || (0..3).any(|k| b.min[k] > b.max[k]) {
                return Err("a fluid box is from min to max, each -16 to 32 blocks".into());
            }
        }
        Ok(())
    }
}

/// One position's parts for a client: the mod id (u16 length and bytes),
/// x y z (i32), the tick their angles are measured from (i64: the tick they
/// were set, or 0 for parts set in phase) and the tick they were set
/// (i64), then each part's state (u16 length and bytes), axis (u8), whether it
/// is an item (u8), pivot, offset, speed, angle, velocity and scale (f32),
/// little-endian. Fluid boxes follow when there are any: 0xFFFD (u16, no
/// state's length), their count (u8), then for each the fluid (u16 length
/// and bytes) and min and max (f32 each). Orientations follow when a part
/// has one: 0xFFFE, their count (u8), then for each the part's index (u8)
/// and x, y, z, w (f32). Then scrolls, when a part has one: 0xFFFF, then for
/// each the part's index (u8), from and to (u16 length and bytes), speed and
/// span (f32), to the end. No parts: none drawn there any more.
pub fn encode(
    mod_id: &str,
    pos: [i32; 3],
    start: i64,
    set_at: i64,
    parts: &[VisualPart],
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((mod_id.len() as u16).to_le_bytes());
    out.extend(mod_id.as_bytes());
    for c in pos {
        out.extend(c.to_le_bytes());
    }
    out.extend(start.to_le_bytes());
    out.extend(set_at.to_le_bytes());
    for p in parts.iter().filter(|p| p.fluid.is_none()) {
        out.extend((p.state.len() as u16).to_le_bytes());
        out.extend(p.state.as_bytes());
        out.push(p.axis);
        out.push(u8::from(p.item));
        let more = [&p.speed, &p.angle];
        let last = [&p.scale];
        for v in (p.pivot.iter().chain(&p.offset).chain(more))
            .chain(&p.velocity)
            .chain(last)
        {
            out.extend(v.to_le_bytes());
        }
    }
    let fluids: Vec<(&str, FluidBox)> = parts
        .iter()
        .filter_map(|p| Some((p.state.as_str(), p.fluid?)))
        .collect();
    if !fluids.is_empty() {
        out.extend(FLUID_BOXES.to_le_bytes());
        out.push(fluids.len() as u8);
        for (fluid, b) in fluids {
            out.extend((fluid.len() as u16).to_le_bytes());
            out.extend(fluid.as_bytes());
            for c in b.min.iter().chain(&b.max) {
                out.extend(c.to_le_bytes());
            }
        }
    }
    let turned: Vec<(usize, [f32; 4])> = parts
        .iter()
        .enumerate()
        .filter_map(|(i, p)| Some((i, p.orientation?)))
        .collect();
    if !turned.is_empty() {
        out.extend(ORIENTATIONS.to_le_bytes());
        out.push(turned.len() as u8);
        for (i, q) in turned {
            out.push(i as u8);
            for c in q {
                out.extend(c.to_le_bytes());
            }
        }
    }
    if parts.iter().any(|p| p.scroll.is_some()) {
        out.extend(SCROLLS.to_le_bytes());
        for (i, p) in parts.iter().enumerate() {
            let Some(s) = &p.scroll else { continue };
            out.push(i as u8);
            for name in [&s.from, &s.to] {
                out.extend((name.len() as u16).to_le_bytes());
                out.extend(name.as_bytes());
            }
            out.extend(s.speed.to_le_bytes());
            out.extend(s.span.to_le_bytes());
        }
    }
    out
}

/// Where [`encode`]'s scrolls start: a length no state has.
const SCROLLS: u16 = 0xFFFF;
/// Where [`encode`]'s orientations start, before the scrolls.
const ORIENTATIONS: u16 = 0xFFFE;
/// Where [`encode`]'s fluid boxes start, before the orientations.
const FLUID_BOXES: u16 = 0xFFFD;

/// [`encode`]'s inverse; none for damaged bytes.
pub fn decode(bytes: &[u8]) -> Option<(String, [i32; 3], i64, i64, Vec<VisualPart>)> {
    fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
        let (head, rest) = bytes.split_at_checked(n)?;
        *bytes = rest;
        Some(head)
    }
    let mut b = bytes;
    let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?) as usize;
    let id = std::str::from_utf8(take(&mut b, n)?).ok()?.to_owned();
    let mut pos = [0i32; 3];
    for c in &mut pos {
        *c = i32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
    }
    let start = i64::from_le_bytes(take(&mut b, 8)?.try_into().ok()?);
    let set_at = i64::from_le_bytes(take(&mut b, 8)?.try_into().ok()?);
    let mut parts = Vec::new();
    let mut turned = Vec::new();
    let mut fluids = Vec::new();
    while !b.is_empty() {
        let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?);
        if n == SCROLLS {
            break;
        }
        if n == FLUID_BOXES {
            let count = take(&mut b, 1)?[0];
            if usize::from(count) > FLUIDS {
                return None;
            }
            for _ in 0..count {
                let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?) as usize;
                let fluid = std::str::from_utf8(take(&mut b, n)?).ok()?.to_owned();
                let mut f = [0f32; 6];
                for v in &mut f {
                    *v = f32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
                }
                let part = VisualPart::fluid(&fluid, [f[0], f[1], f[2]], [f[3], f[4], f[5]]);
                part.check().ok()?;
                fluids.push(part);
            }
            continue;
        }
        if n == ORIENTATIONS {
            for _ in 0..take(&mut b, 1)?[0] {
                let i = usize::from(take(&mut b, 1)?[0]);
                let mut q = [0f32; 4];
                for c in &mut q {
                    *c = f32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
                }
                // Unit already: checked, kept as sent.
                unit_quaternion(q)?;
                turned.push((i, q));
            }
            continue;
        }
        if parts.len() == PARTS {
            return None;
        }
        let n = usize::from(n);
        let state = std::str::from_utf8(take(&mut b, n)?).ok()?.to_owned();
        let axis = take(&mut b, 1)?[0];
        let item = match take(&mut b, 1)?[0] {
            0 => false,
            1 => true,
            _ => return None,
        };
        let mut f = [0f32; 12];
        for v in &mut f {
            *v = f32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
        }
        let part = VisualPart {
            state,
            axis,
            pivot: [f[0], f[1], f[2]],
            offset: [f[3], f[4], f[5]],
            speed: f[6],
            angle: f[7],
            velocity: [f[8], f[9], f[10]],
            item,
            scale: f[11],
            scroll: None,
            orientation: None,
            fluid: None,
        };
        part.check().ok()?;
        parts.push(part);
    }
    for (i, q) in turned {
        parts.get_mut(i)?.orientation = Some(q);
    }
    while !b.is_empty() {
        let i = usize::from(take(&mut b, 1)?[0]);
        let mut names = [String::new(), String::new()];
        for name in &mut names {
            let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?) as usize;
            *name = std::str::from_utf8(take(&mut b, n)?).ok()?.to_owned();
        }
        let speed = f32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
        let span = f32::from_le_bytes(take(&mut b, 4)?.try_into().ok()?);
        let [from, to] = names;
        let scroll = Scroll {
            from,
            to,
            speed,
            span,
        };
        scroll.check().ok()?;
        parts.get_mut(i)?.scroll = Some(scroll);
    }
    parts.extend(fluids);
    Some((id, pos, start, set_at, parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shaft(speed: f32) -> VisualPart {
        VisualPart {
            state: "gears:shaft[axis=x]".into(),
            axis: 0,
            pivot: [0.5; 3],
            offset: [0.0; 3],
            speed,
            angle: 45.0,
            velocity: [0.0, -0.25, 0.0],
            item: false,
            scale: 1.0,
            scroll: None,
            orientation: None,
            fluid: None,
        }
    }

    #[test]
    fn parts_round_trip_and_damage_is_refused() {
        let ingot = VisualPart {
            state: "minecraft:iron_ingot".into(),
            item: true,
            scale: 0.5,
            ..shaft(0.0)
        };
        let parts = [shaft(6.0), shaft(-3.0), ingot];
        let bytes = encode("gears", [1, -60, 9], 0, 1234, &parts);
        assert_eq!(
            decode(&bytes),
            Some(("gears".into(), [1, -60, 9], 0, 1234, parts.to_vec()))
        );
        assert_eq!(decode(&encode("gears", [0; 3], 0, 0, &[])).unwrap().4, []);
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(
            decode(&encode("gears", [0; 3], 0, 0, &[shaft(f32::NAN)])),
            None
        );
    }

    fn belt() -> Scroll {
        Scroll {
            from: "create:block/belt/belt".into(),
            to: "create:block/belt/belt_scroll".into(),
            speed: 0.125,
            span: 0.5,
        }
    }

    /// Scrolls follow the parts, and bytes written before scrolls were read
    /// as they were.
    #[test]
    fn scrolls_round_trip_after_the_parts() {
        let mut parts = [shaft(0.0), shaft(1.0)];
        let old = encode("gears", [0; 3], 0, 7, &parts);
        assert_eq!(decode(&old).unwrap().4, parts.to_vec());
        parts[1].scroll = Some(belt());
        let bytes = encode("gears", [0; 3], 0, 7, &parts);
        assert_eq!(decode(&bytes).unwrap().4, parts.to_vec());
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        parts[1].scroll.as_mut().unwrap().span = 0.0;
        assert_eq!(decode(&encode("gears", [0; 3], 0, 7, &parts)), None);
    }

    /// Orientations come before the scrolls and are read with them.
    #[test]
    fn orientations_round_trip_with_scrolls() {
        let half = std::f32::consts::FRAC_1_SQRT_2;
        let mut parts = [shaft(0.0), shaft(1.0), shaft(2.0)];
        parts[0].orientation = Some([half, 0.0, 0.0, half]);
        parts[2].orientation = Some([0.0, 0.0, 0.0, 1.0]);
        parts[1].scroll = Some(belt());
        let bytes = encode("gears", [0; 3], 0, 7, &parts);
        assert_eq!(decode(&bytes).unwrap().4, parts.to_vec());
        parts[1].scroll = None;
        assert_eq!(
            decode(&encode("gears", [0; 3], 0, 7, &parts)).unwrap().4,
            parts.to_vec()
        );
        assert_eq!(
            unit_quaternion([0.0, 0.0, 0.0, 2.0]),
            Some([0.0, 0.0, 0.0, 1.0])
        );
        assert_eq!(unit_quaternion([0.0; 4]), None);
        assert_eq!(unit_quaternion([f32::NAN, 0.0, 0.0, 1.0]), None);
    }

    /// Fluid boxes come after the parts, before their orientations and
    /// scrolls, which keep their indices.
    #[test]
    fn fluid_boxes_round_trip_after_the_parts() {
        let mut parts = vec![shaft(0.0), shaft(1.0)];
        parts[1].scroll = Some(belt());
        parts[0].orientation = Some([0.0, 0.0, 0.0, 1.0]);
        parts.push(VisualPart::fluid(
            "minecraft:water",
            [0.0; 3],
            [3.0, 1.5, 3.0],
        ));
        parts.push(VisualPart::fluid(
            "create:honey",
            [-1.0, 0.0, -1.0],
            [2.0, 0.25, 2.0],
        ));
        let bytes = encode("gears", [0; 3], 0, 7, &parts);
        assert_eq!(decode(&bytes).unwrap().4, parts);
        let only = vec![VisualPart::fluid("minecraft:lava", [0.0; 3], [1.0; 3])];
        assert_eq!(
            decode(&encode("gears", [0; 3], 0, 7, &only)).unwrap().4,
            only
        );
        let past = VisualPart::fluid("minecraft:lava", [0.0; 3], [40.0, 1.0, 1.0]);
        assert!(past.check().is_err());
        let inside_out = VisualPart::fluid("minecraft:lava", [1.0; 3], [0.0; 3]);
        assert!(inside_out.check().is_err());
    }

    #[test]
    fn a_scroll_runs_over_its_span_and_starts_again() {
        let s = belt();
        assert_eq!(s.offset_at(0.0), 0.0);
        assert_eq!(s.offset_at(2.0), 0.25);
        assert_eq!(s.offset_at(4.0), 0.0);
        assert_eq!(s.offset_at(6.0), 0.25);
        let up = Scroll {
            speed: -0.125,
            ..belt()
        };
        assert_eq!(up.offset_at(2.0), 0.25);
        assert_eq!(up.offset_at(1.0), 0.375);
    }

    #[test]
    fn a_part_moves_by_its_velocity_within_its_reach() {
        assert_eq!(shaft(0.0).moved_at(2.0), [0.0, -0.5, 0.0]);
        assert_eq!(shaft(0.0).moved_at(1000.0), [0.0, -16.0, 0.0]);
        let mut p = shaft(0.0);
        p.velocity = [0.0, 17.0, 0.0];
        assert!(p.check().is_err());
    }

    #[test]
    fn a_part_turns_from_its_angle_at_its_speed() {
        assert_eq!(shaft(6.0).angle_at(0.0), 45.0);
        assert_eq!(shaft(6.0).angle_at(2.5), 60.0);
        assert_eq!(shaft(6.0).angle_at(60.0), 45.0);
    }
}
