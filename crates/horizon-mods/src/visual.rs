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
}

impl VisualPart {
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
        if self.state.is_empty() || self.state.len() > 256 {
            return Err("a part names a block state".into());
        }
        Ok(())
    }
}

/// One position's parts for a client: the mod id (u16 length and bytes),
/// x y z (i32), the tick they were set (i64), then each part's state (u16
/// length and bytes), axis (u8), pivot, offset, speed and angle (f32),
/// little-endian. No parts: none drawn there any more.
pub fn encode(mod_id: &str, pos: [i32; 3], start: i64, parts: &[VisualPart]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend((mod_id.len() as u16).to_le_bytes());
    out.extend(mod_id.as_bytes());
    for c in pos {
        out.extend(c.to_le_bytes());
    }
    out.extend(start.to_le_bytes());
    for p in parts {
        out.extend((p.state.len() as u16).to_le_bytes());
        out.extend(p.state.as_bytes());
        out.push(p.axis);
        for v in p.pivot.iter().chain(&p.offset).chain([&p.speed, &p.angle]) {
            out.extend(v.to_le_bytes());
        }
    }
    out
}

/// [`encode`]'s inverse; none for damaged bytes.
pub fn decode(bytes: &[u8]) -> Option<(String, [i32; 3], i64, Vec<VisualPart>)> {
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
    let mut parts = Vec::new();
    while !b.is_empty() {
        if parts.len() == PARTS {
            return None;
        }
        let n = u16::from_le_bytes(take(&mut b, 2)?.try_into().ok()?) as usize;
        let state = std::str::from_utf8(take(&mut b, n)?).ok()?.to_owned();
        let axis = take(&mut b, 1)?[0];
        let mut f = [0f32; 8];
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
        };
        part.check().ok()?;
        parts.push(part);
    }
    Some((id, pos, start, parts))
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
        }
    }

    #[test]
    fn parts_round_trip_and_damage_is_refused() {
        let parts = [shaft(6.0), shaft(-3.0)];
        let bytes = encode("gears", [1, -60, 9], 1234, &parts);
        assert_eq!(
            decode(&bytes),
            Some(("gears".into(), [1, -60, 9], 1234, parts.to_vec()))
        );
        assert_eq!(decode(&encode("gears", [0; 3], 0, &[])).unwrap().3, []);
        assert_eq!(decode(&bytes[..bytes.len() - 1]), None);
        assert_eq!(
            decode(&encode("gears", [0; 3], 0, &[shaft(f32::NAN)])),
            None
        );
    }

    #[test]
    fn a_part_turns_from_its_angle_at_its_speed() {
        assert_eq!(shaft(6.0).angle_at(0.0), 45.0);
        assert_eq!(shaft(6.0).angle_at(2.5), 60.0);
        assert_eq!(shaft(6.0).angle_at(60.0), 45.0);
    }
}
