//! Entities of server mods' own types (`entity-types`, docs/mods.md): the
//! game moves them, keeps their health and saves them with the world; the
//! mod gives them their behaviour. Each one in the simulated dimension is
//! shown as an invisible armor stand wearing its model on its head.
use super::Game;

/// Where a world keeps every server mod's typed entities:
/// `<world>/horizon-mod-data/entities.json`.
#[cfg(feature = "mods")]
const ENTITIES_FILE: &str = "entities.json";

/// A player's hit or click on a typed entity, for the next tick.
#[cfg(feature = "mods")]
pub(super) enum TypedEvent {
    Attacked {
        mod_id: String,
        id: u32,
        player: String,
        damage: f32,
    },
    Interacted {
        mod_id: String,
        id: u32,
        player: String,
        item: String,
    },
}

/// `UUID.toString()`, as mods name players.
pub(super) fn uuid_text(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

#[cfg(not(feature = "mods"))]
impl Game<'_> {
    pub(super) fn is_typed_stand(&self, _stand: u32) -> bool {
        false
    }
    pub(super) fn typed_attack(&mut self, _stand: u32, _player: String, _damage: f32) -> bool {
        false
    }
    pub(super) fn typed_interact(&mut self, _stand: u32, _player: String, _item: String) -> bool {
        false
    }
    pub(super) fn flush_typed_entities(&mut self) {}
}

#[cfg(feature = "mods")]
impl Game<'_> {
    /// The mod and id of the typed entity an armor stand shows.
    fn typed_of_stand(&self, stand: u32) -> Option<(String, u32)> {
        self.mods
            .typed_stands
            .iter()
            .find(|(_, s)| **s == stand)
            .map(|(k, _)| k.clone())
    }

    /// Whether this armor stand shows a typed entity: those are never saved
    /// as stands, nor moved or burnt as one.
    pub(super) fn is_typed_stand(&self, stand: u32) -> bool {
        self.mods.typed_stands.values().any(|&s| s == stand)
    }

    /// A player hit the typed entity this stand shows: heard next tick.
    /// False when the stand shows none.
    pub(super) fn typed_attack(&mut self, stand: u32, player: String, damage: f32) -> bool {
        let Some((mod_id, id)) = self.typed_of_stand(stand) else {
            return false;
        };
        if self.mods.typed_events.len() < 256 {
            self.mods.typed_events.push(TypedEvent::Attacked {
                mod_id,
                id,
                player,
                damage,
            });
        }
        true
    }

    /// A player right-clicked the typed entity this stand shows.
    pub(super) fn typed_interact(&mut self, stand: u32, player: String, item: String) -> bool {
        let Some((mod_id, id)) = self.typed_of_stand(stand) else {
            return false;
        };
        if self.mods.typed_events.len() < 256 {
            self.mods.typed_events.push(TypedEvent::Interacted {
                mod_id,
                id,
                player,
                item,
            });
        }
        true
    }

    /// Reads the world's typed entities when the world changed.
    pub(super) fn sync_typed_entities(&mut self) {
        if self.mods.typed_root.as_ref() == Some(&self.world_root) {
            return;
        }
        self.mods.typed.clear();
        self.mods.typed_dirty = false;
        for (_, stand) in self.mods.typed_stands.drain() {
            self.armor_stands.retain(|a| a.id != stand);
        }
        let path = self
            .world_root
            .join(super::server_mods::BLOCK_DATA_DIR)
            .join(ENTITIES_FILE);
        if !self.world_root.as_os_str().is_empty()
            && let Ok(bytes) = std::fs::read(&path)
        {
            match serde_json::from_slice::<Vec<horizon_mods::TypedEntity>>(&bytes) {
                Ok(list) => self.mods.typed = list,
                Err(e) => log::warn!("Horizon: {} not read: {e}", path.display()),
            }
        }
        self.mods.typed_root = Some(self.world_root.clone());
    }

    /// Writes every typed entity to the world's folder.
    pub(super) fn flush_typed_entities(&mut self) {
        if !self.mods.typed_dirty || self.world_root.as_os_str().is_empty() {
            return;
        }
        let dir = self.world_root.join(super::server_mods::BLOCK_DATA_DIR);
        let result = (|| -> std::io::Result<()> {
            let path = dir.join(ENTITIES_FILE);
            if self.mods.typed.is_empty() {
                if path.exists() {
                    std::fs::remove_file(path)?;
                }
                return Ok(());
            }
            std::fs::create_dir_all(&dir)?;
            let tmp = dir.join(format!("{ENTITIES_FILE}.tmp"));
            std::fs::write(&tmp, serde_json::to_vec(&self.mods.typed)?)?;
            std::fs::rename(&tmp, path)
        })();
        match result {
            Ok(()) => self.mods.typed_dirty = false,
            Err(e) => log::warn!("Horizon: could not save server mods' entities: {e}"),
        }
    }

    /// Moves the typed entities of the simulated dimension a tick, as a mob
    /// with no will of its own: its velocity against the blocks, gravity if
    /// its type has it, then drag. Their stands follow.
    pub(super) fn tick_typed_entities(&mut self) {
        use horizon_core::geometry::{Aabb, Vec3};
        let dim = self.dim.id().to_owned();
        let ticking = self.entity_ticking_area();
        let void = f64::from(self.dim.min_y()) - 64.0;
        let mut scene = horizon_client::collisions::SceneCollisions {
            world: &self.world,
            binding: self.collision,
            subject: None,
        };
        for e in self.mods.typed.iter_mut().filter(|e| e.dimension == dim) {
            if !ticking(e.pos[0].floor() as i32, e.pos[2].floor() as i32) {
                continue;
            }
            let s = if e.small { 0.5 } else { 1.0 };
            let (w, h) = (0.25 * s, 1.975 * s);
            let [x, y, z] = e.pos;
            let bounds = Aabb::new(Vec3::new(x - w, y, z - w), Vec3::new(x + w, y + h, z + w));
            let wanted = Vec3::new(e.velocity[0], e.velocity[1], e.velocity[2]);
            if let Ok(moved) = scene.resolve(bounds, wanted, 0.0, e.on_ground) {
                e.pos = [x + moved.x, y + moved.y, z + moved.z];
                e.on_ground = wanted.y < 0.0 && moved.y > wanted.y;
                let stop = |w: f64, m: f64| if (w - m).abs() > 1e-9 { 0.0 } else { w };
                e.velocity = [
                    stop(wanted.x, moved.x),
                    stop(wanted.y, moved.y),
                    stop(wanted.z, moved.z),
                ];
            }
            if e.gravity {
                e.velocity[1] = (e.velocity[1] - 0.08) * 0.98;
            } else {
                e.velocity[1] *= 0.98;
            }
            // LivingEntity.travel: 0.6 slipperiness times 0.91 on the ground.
            let drag = if e.on_ground { 0.546 } else { 0.91 };
            e.velocity[0] *= drag;
            e.velocity[2] *= drag;
            if e.pos[1] < void {
                e.health = 0.0;
                e.max_health = e.max_health.max(1.0);
            }
        }
        if self.mods.typed.iter().any(|e| e.dimension == dim) {
            self.mods.typed_dirty = true;
        }
        self.show_typed_entities();
    }

    /// Gives each typed entity of the simulated dimension its stand, where
    /// it is now; takes away the stands of those gone or elsewhere.
    fn show_typed_entities(&mut self) {
        use horizon_client::{entities::ArmorStand, items::ItemStack};
        use horizon_core::geometry::Vec3;
        let dim = self.dim.id().to_owned();
        let live: std::collections::HashSet<(String, u32)> = self
            .mods
            .typed
            .iter()
            .filter(|e| e.dimension == dim)
            .map(|e| (e.mod_id.clone(), e.id))
            .collect();
        let gone: Vec<(String, u32)> = self
            .mods
            .typed_stands
            .keys()
            .filter(|k| !live.contains(*k))
            .cloned()
            .collect();
        for k in gone {
            if let Some(stand) = self.mods.typed_stands.remove(&k) {
                self.armor_stands.retain(|a| a.id != stand);
            }
        }
        for e in self.mods.typed.iter().filter(|e| e.dimension == dim) {
            let key = (e.mod_id.clone(), e.id);
            let at = Vec3::new(e.pos[0], e.pos[1], e.pos[2]);
            let stand = self.mods.typed_stands.get(&key).copied();
            let i = stand.and_then(|s| self.armor_stands.iter().position(|a| a.id == s));
            let i = match i {
                Some(i) => i,
                None => {
                    self.next_entity += 1;
                    let mut a = ArmorStand::new(self.next_entity, at, e.yaw);
                    a.invisible = true;
                    a.no_base_plate = true;
                    a.small = e.small;
                    self.mods.typed_stands.insert(key, a.id);
                    self.armor_stands.push(a);
                    self.armor_stands.len() - 1
                }
            };
            let a = &mut self.armor_stands[i];
            a.position = at;
            a.velocity = Vec3::new(e.velocity[0], e.velocity[1], e.velocity[2]);
            a.on_ground = e.on_ground;
            a.yaw = e.yaw;
            if a.equipment[5].as_ref().is_none_or(|s| *s.item != *e.model) {
                a.equipment[5] = Some(ItemStack::new(e.model.as_str(), 1));
            }
        }
    }

    /// The tick's hits and clicks, the deaths, then each mod's entities'
    /// tick; with the world lent.
    pub(super) fn deliver_typed_events(&mut self) {
        for event in std::mem::take(&mut self.mods.typed_events) {
            match event {
                TypedEvent::Attacked {
                    mod_id,
                    id,
                    player,
                    damage,
                } => {
                    if self.mods.mods.entity_attacked(&mod_id, id, &player, damage) {
                        continue;
                    }
                    if let Some(e) = self
                        .mods
                        .typed
                        .iter_mut()
                        .find(|e| e.mod_id == mod_id && e.id == id && e.max_health > 0.0)
                    {
                        e.health = (e.health - damage).max(0.0);
                        self.mods.typed_dirty = true;
                    }
                }
                TypedEvent::Interacted {
                    mod_id,
                    id,
                    player,
                    item,
                } => self
                    .mods
                    .mods
                    .entity_interacted(&mod_id, id, &player, &item),
            }
        }
        let dead: Vec<(String, u32, String)> = self
            .mods
            .typed
            .iter()
            .filter(|e| e.max_health > 0.0 && e.health <= 0.0)
            .map(|e| (e.mod_id.clone(), e.id, e.kind.clone()))
            .collect();
        for (mod_id, id, kind) in dead {
            self.mods.mods.entity_died(&mod_id, id, &kind);
            self.mods
                .typed
                .retain(|e| !(e.mod_id == mod_id && e.id == id));
            self.mods.typed_dirty = true;
        }
        let dim = self.dim.id().to_owned();
        let mut owners: Vec<&str> = self
            .mods
            .typed
            .iter()
            .filter(|e| e.dimension == dim)
            .map(|e| e.mod_id.as_str())
            .collect();
        owners.sort_unstable();
        owners.dedup();
        let owners: Vec<String> = owners.into_iter().map(str::to_owned).collect();
        for mod_id in owners {
            let list: Vec<horizon_mods::TypedEntity> = self
                .mods
                .typed
                .iter()
                .filter(|e| e.dimension == dim && e.mod_id == mod_id)
                .cloned()
                .collect();
            self.mods.mods.entity_tick(&mod_id, &list);
        }
    }

    /// Makes the changes server mods queued for their typed entities.
    pub(super) fn apply_typed_ops(&mut self) {
        use horizon_mods::TypedOp;
        let ops = std::mem::take(&mut self.mods.typed_ops);
        if ops.is_empty() {
            return;
        }
        self.mods.typed_dirty = true;
        for (mod_id, op) in ops {
            let find = |typed: &mut Vec<horizon_mods::TypedEntity>, id: u32| {
                typed
                    .iter_mut()
                    .position(|e| e.mod_id == mod_id && e.id == id)
            };
            match op {
                TypedOp::Spawn(e) => {
                    let own = self
                        .mods
                        .typed
                        .iter()
                        .filter(|t| t.mod_id == mod_id)
                        .count();
                    let taken = find(&mut self.mods.typed, e.id).is_some();
                    if e.mod_id == mod_id && !taken && own < horizon_mods::TYPED_PER_MOD {
                        self.mods.typed.push(e);
                    }
                }
                TypedOp::Velocity { id, velocity } => {
                    if let Some(i) = find(&mut self.mods.typed, id) {
                        self.mods.typed[i].velocity = velocity;
                    }
                }
                TypedOp::Move { id, pos, yaw } => {
                    if let Some(i) = find(&mut self.mods.typed, id) {
                        let e = &mut self.mods.typed[i];
                        e.pos = pos;
                        e.yaw = yaw;
                    }
                }
                TypedOp::Data { id, data } => {
                    if let Some(i) = find(&mut self.mods.typed, id) {
                        self.mods.typed[i].data = data;
                    }
                }
                TypedOp::Hurt { id, amount } => {
                    if let Some(i) = find(&mut self.mods.typed, id) {
                        let e = &mut self.mods.typed[i];
                        if e.max_health > 0.0 {
                            e.health = (e.health - amount).max(0.0);
                        }
                    }
                }
                TypedOp::Model { id, item } => {
                    if let Some(i) = find(&mut self.mods.typed, id) {
                        self.mods.typed[i].model = item;
                    }
                }
                TypedOp::Pose { id, part, rotation } => {
                    if let Some(&stand) = self.mods.typed_stands.get(&(mod_id.clone(), id))
                        && let Some(a) = self.armor_stands.iter_mut().find(|a| a.id == stand)
                        && let Some(cell) = a.pose.get_mut(part)
                    {
                        *cell = rotation;
                    }
                }
                TypedOp::Remove { id } => {
                    self.mods
                        .typed
                        .retain(|e| !(e.mod_id == mod_id && e.id == id));
                }
            }
        }
        self.show_typed_entities();
    }
}
