//! Screens of a server mod's own (capability `menus`): a container menu the
//! server opened as usual, drawn here with the layout the mod sent on
//! [`horizon_mods::menu_layout::CHANNEL`]: its slots where the mod put them,
//! labels, progress bars fed by the menu's data values, and buttons whose
//! presses go back to the mod. A vanilla client shows the same menu as a
//! chest.
use super::Game;
use horizon_client::hud::{HudBatch, rgba};
use horizon_mods::menu_layout::{CHANNEL, MenuLayout, MenuMessage, MenuWidget};

/// The screen a mod sent for the container menu open now.
pub(super) struct ModMenu {
    /// The container menu it draws: a window id from the server, or none
    /// for one this window opened itself.
    pub window: Option<i32>,
    pub layout: MenuLayout,
    /// `ContainerData`: the values the bars read.
    pub data: [i32; 32],
}

impl Game<'_> {
    /// The mod's screen, while the container menu it was sent for is open.
    fn mod_menu(&self) -> Option<&ModMenu> {
        let m = self.mod_menu.as_ref()?;
        self.container?;
        let open = match (m.window, self.remote.as_ref()) {
            (Some(w), Some(r)) => r.menu.as_ref().is_some_and(|(window, _)| *window == w),
            (None, None) => true,
            _ => false,
        };
        open.then_some(m)
    }

    /// The size of the mod's screen open now.
    pub(super) fn mod_menu_size(&self) -> Option<(f32, f32)> {
        self.mod_menu()
            .map(|m| (m.layout.width as f32, m.layout.height as f32))
    }

    /// Where a slot of the menu is drawn on the mod's screen: container slots
    /// (300 on) first in the menu's order, then the 27 main and 9 hotbar
    /// slots. A slot the layout leaves out is drawn nowhere.
    pub(super) fn mod_menu_slot_position(&self, i: usize) -> Option<(f32, f32)> {
        let m = self.mod_menu()?;
        let size = self.container_size();
        let slot = match i {
            300..500 => i - 300,
            9..36 => size + (i - 9),
            0..9 => size + 27 + i,
            _ => return None,
        };
        Some(
            m.layout
                .slots
                .iter()
                .find(|(s, _, _)| *s as usize == slot)
                .map_or((-10_000.0, -10_000.0), |&(_, x, y)| (x as f32, y as f32)),
        )
    }

    /// Draws the mod's screen behind the slots' items: a panel, each slot's
    /// frame, labels, bars and buttons. False when none is open.
    pub(super) fn draw_mod_menu(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some(m) = self.mod_menu() else {
            return false;
        };
        let atlas = &self.gui.atlas;
        let (w, h) = (m.layout.width as f32, m.layout.height as f32);
        // A panel in vanilla's container colours: a dark edge, a light face.
        batch.fill(atlas, ox, oy, w, h, rgba(85, 85, 85, 255));
        batch.fill(
            atlas,
            ox + 1.0,
            oy + 1.0,
            w - 2.0,
            h - 2.0,
            rgba(198, 198, 198, 255),
        );
        for &(_, x, y) in &m.layout.slots {
            let (x, y) = (ox + x as f32, oy + y as f32);
            batch.fill(atlas, x - 1.0, y - 1.0, 18.0, 18.0, rgba(55, 55, 55, 255));
            batch.fill(atlas, x, y, 17.0, 17.0, rgba(255, 255, 255, 255));
            batch.fill(atlas, x, y, 16.0, 16.0, rgba(139, 139, 139, 255));
        }
        let colour = |c: u32| rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255);
        let hovered = |x: f32, y: f32, w: f32, h: f32| {
            let (cx, cy) = self.cursor;
            cx >= x && cx < x + w && cy >= y && cy < y + h
        };
        for widget in &m.layout.widgets {
            match widget {
                MenuWidget::Label { x, y, text, color } => {
                    batch.text(
                        atlas,
                        text,
                        ox + *x as f32,
                        oy + *y as f32,
                        colour(*color),
                        false,
                    );
                }
                MenuWidget::Progress {
                    x,
                    y,
                    width,
                    height,
                    value,
                    max,
                    vertical,
                    color,
                    background,
                } => {
                    let (x, y) = (ox + *x as f32, oy + *y as f32);
                    let (bw, bh) = (*width as f32, *height as f32);
                    batch.fill(atlas, x, y, bw, bh, colour(*background));
                    let v = m.data[*value as usize].max(0) as f32;
                    let full = m.data[*max as usize].max(1) as f32;
                    let part = (v / full).clamp(0.0, 1.0);
                    if *vertical {
                        let fh = (bh * part).round();
                        batch.fill(atlas, x, y + bh - fh, bw, fh, colour(*color));
                    } else {
                        batch.fill(atlas, x, y, (bw * part).round(), bh, colour(*color));
                    }
                }
                MenuWidget::Button {
                    x,
                    y,
                    width,
                    height,
                    label,
                    ..
                } => {
                    let (x, y) = (ox + *x as f32, oy + *y as f32);
                    let (bw, bh) = (*width as f32, *height as f32);
                    let face = if hovered(x, y, bw, bh) {
                        rgba(120, 140, 200, 255)
                    } else {
                        rgba(110, 110, 110, 255)
                    };
                    batch.fill(atlas, x, y, bw, bh, rgba(0, 0, 0, 255));
                    batch.fill(atlas, x + 1.0, y + 1.0, bw - 2.0, bh - 2.0, face);
                    let tw = HudBatch::text_width(atlas, label);
                    batch.text(
                        atlas,
                        label,
                        x + (bw - tw) / 2.0,
                        y + (bh - 8.0) / 2.0,
                        rgba(255, 255, 255, 255),
                        true,
                    );
                }
            }
        }
        true
    }

    /// A left click on one of the mod's screen's buttons: sent to the mod
    /// (the server's, or this window's own). True when one was clicked.
    pub(super) fn mod_menu_press(&mut self) -> bool {
        let Some(r) = self.renderer.as_ref() else {
            return false;
        };
        let batch = HudBatch::new([r.config.width, r.config.height]);
        let (ox, oy) = self.inventory_origin(&batch);
        let Some(m) = self.mod_menu() else {
            return false;
        };
        let (cx, cy) = self.cursor;
        let hit = m.layout.widgets.iter().find_map(|w| match w {
            MenuWidget::Button {
                id,
                x,
                y,
                width,
                height,
                ..
            } => {
                let (x, y) = (ox + *x as f32, oy + *y as f32);
                (cx >= x && cx < x + *width as f32 && cy >= y && cy < y + *height as f32)
                    .then_some(*id)
            }
            _ => None,
        });
        let Some(id) = hit else {
            return false;
        };
        let window = m.window;
        self.audio.play_button_click();
        match window {
            Some(window) => {
                if let Ok(data) = serde_json::to_vec(&MenuMessage::Button { window, id }) {
                    self.send(horizon_protocol::packets::play::Serverbound::CustomPayload(
                        horizon_protocol::packets::configuration::CustomPayload {
                            channel: CHANNEL.into(),
                            data,
                        },
                    ));
                }
            }
            None => self.local_menu_button(id),
        }
        true
    }

    /// What the server this window plays on sent on [`CHANNEL`]: a screen for
    /// the menu it opened, or a data value of it. False for another channel.
    pub(super) fn remote_menu_message(&mut self, channel: &str, data: &[u8]) -> bool {
        if channel != CHANNEL {
            return false;
        }
        match serde_json::from_slice::<MenuMessage>(data) {
            Ok(MenuMessage::Open { window, layout }) => {
                self.mod_menu = Some(ModMenu {
                    window: Some(window),
                    layout,
                    data: [0; 32],
                });
            }
            Ok(MenuMessage::Data {
                window,
                index,
                value,
            }) => {
                if let Some(m) = self.mod_menu.as_mut()
                    && m.window == Some(window)
                    && let Some(cell) = m.data.get_mut(index as usize)
                {
                    *cell = value;
                }
            }
            Ok(MenuMessage::Button { .. }) | Err(_) => {}
        }
        true
    }
}
