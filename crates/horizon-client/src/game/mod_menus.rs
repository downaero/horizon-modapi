//! Screens of a server mod's own (capability `menus`), sent on
//! [`horizon_mods::menu_layout::CHANNEL`]: a container menu the server opened
//! as usual, drawn here with the mod's layout (its slots where the mod put
//! them, labels, progress bars fed by the menu's data values, buttons whose
//! presses go back to the mod); or a panel with no container behind it, over
//! the dimmed world, with text fields too. A vanilla client shows the
//! container as a chest and no panel.
use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_mods::menu_layout::{CHANNEL, MenuLayout, MenuMessage, MenuWidget, PANEL};
use winit::event::{KeyEvent, MouseButton};
use winit::keyboard::{KeyCode, PhysicalKey};

/// The screen a mod sent.
pub(super) struct ModMenu {
    /// The container menu it draws ([`PANEL`] for a panel): a window id from
    /// the server, or none for one this window opened itself.
    pub window: Option<i32>,
    pub layout: MenuLayout,
    /// `ContainerData`: the values the bars read.
    pub data: [i32; 32],
    /// A panel: no container behind it.
    pub panel: bool,
    /// What each text field holds now, by id.
    pub fields: Vec<(u32, String)>,
    /// The text field being typed into.
    pub focus: Option<u32>,
}

impl ModMenu {
    pub(super) fn new(window: Option<i32>, layout: MenuLayout, panel: bool) -> Self {
        let fields = layout
            .widgets
            .iter()
            .filter_map(|w| match w {
                MenuWidget::TextField { id, value, .. } => Some((*id, value.clone())),
                _ => None,
            })
            .collect();
        Self {
            window,
            layout,
            data: [0; 32],
            panel,
            fields,
            focus: None,
        }
    }
}

impl Game<'_> {
    /// The mod's screen for a container, while the container menu it was
    /// sent for is open.
    fn mod_menu(&self) -> Option<&ModMenu> {
        let m = self.mod_menu.as_ref().filter(|m| !m.panel)?;
        self.container?;
        let open = match (m.window, self.remote.as_ref()) {
            (Some(w), Some(r)) => r.menu.as_ref().is_some_and(|(window, _)| *window == w),
            (None, None) => true,
            _ => false,
        };
        open.then_some(m)
    }

    /// The mod's panel, while it shows.
    fn mod_panel(&self) -> Option<&ModMenu> {
        self.mod_menu
            .as_ref()
            .filter(|m| m.panel && self.screen == Screen::ModPanel)
    }

    /// The size of the mod's container screen open now.
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

    /// A layout drawn at (`ox`, `oy`): a panel in vanilla's container
    /// colours, each slot's frame (a container's), labels, bars, buttons and
    /// text fields.
    fn draw_layout(&self, batch: &mut HudBatch, m: &ModMenu, ox: f32, oy: f32) {
        let atlas = &self.gui.atlas;
        let (w, h) = (m.layout.width as f32, m.layout.height as f32);
        batch.fill(atlas, ox, oy, w, h, rgba(85, 85, 85, 255));
        batch.fill(
            atlas,
            ox + 1.0,
            oy + 1.0,
            w - 2.0,
            h - 2.0,
            rgba(198, 198, 198, 255),
        );
        if !m.panel {
            for &(_, x, y) in &m.layout.slots {
                let (x, y) = (ox + x as f32, oy + y as f32);
                batch.fill(atlas, x - 1.0, y - 1.0, 18.0, 18.0, rgba(55, 55, 55, 255));
                batch.fill(atlas, x, y, 17.0, 17.0, rgba(255, 255, 255, 255));
                batch.fill(atlas, x, y, 16.0, 16.0, rgba(139, 139, 139, 255));
            }
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
                MenuWidget::TextField {
                    id, x, y, width, ..
                } => {
                    // EditBox: a black box, a grey edge (white with focus).
                    let (x, y) = (ox + *x as f32, oy + *y as f32);
                    let bw = *width as f32;
                    let focused = m.focus == Some(*id);
                    let edge = if focused {
                        rgba(255, 255, 255, 255)
                    } else {
                        rgba(160, 160, 160, 255)
                    };
                    batch.fill(atlas, x, y, bw, 20.0, edge);
                    batch.fill(atlas, x + 1.0, y + 1.0, bw - 2.0, 18.0, rgba(0, 0, 0, 255));
                    let value = m
                        .fields
                        .iter()
                        .find(|(f, _)| f == id)
                        .map_or("", |(_, v)| v.as_str());
                    // The end of a long value, as an edit box scrolls to it.
                    let mut shown: &str = value;
                    while !shown.is_empty() && HudBatch::text_width(atlas, shown) > bw - 10.0 {
                        let mut chars = shown.chars();
                        chars.next();
                        shown = chars.as_str();
                    }
                    let text = if focused {
                        format!("{shown}_")
                    } else {
                        shown.to_owned()
                    };
                    batch.text(
                        atlas,
                        &text,
                        x + 4.0,
                        y + 6.0,
                        rgba(224, 224, 224, 255),
                        true,
                    );
                }
            }
        }
    }

    /// Draws the mod's container screen behind the slots' items. False when
    /// none is open.
    pub(super) fn draw_mod_menu(&self, batch: &mut HudBatch, ox: f32, oy: f32) -> bool {
        let Some(m) = self.mod_menu() else {
            return false;
        };
        self.draw_layout(batch, m, ox, oy);
        true
    }

    /// The panel's top left corner: centred.
    fn mod_panel_origin(&self, batch: &HudBatch, m: &ModMenu) -> (f32, f32) {
        batch.centered_origin(m.layout.width as f32, m.layout.height as f32)
    }

    /// Draws the mod's panel over the dimmed world.
    pub(super) fn draw_mod_panel(&mut self, batch: &mut HudBatch) {
        let Some(m) = self.mod_panel() else {
            // The panel went: back to the world.
            self.screen = Screen::Playing;
            self.capture(true);
            return;
        };
        batch.fill(
            &self.gui.atlas,
            0.0,
            0.0,
            batch.width,
            batch.height,
            rgba(16, 16, 16, 160),
        );
        let (ox, oy) = self.mod_panel_origin(batch, m);
        self.draw_layout(batch, m, ox, oy);
    }

    /// The widget under the cursor on a layout at (`ox`, `oy`): a button's
    /// id, or a text field's.
    fn layout_hit(m: &ModMenu, cursor: (f32, f32), ox: f32, oy: f32) -> Option<(bool, u32)> {
        let (cx, cy) = cursor;
        let inside =
            |x: f32, y: f32, w: f32, h: f32| cx >= x && cx < x + w && cy >= y && cy < y + h;
        m.layout.widgets.iter().find_map(|w| match w {
            MenuWidget::Button {
                id,
                x,
                y,
                width,
                height,
                ..
            } => inside(
                ox + *x as f32,
                oy + *y as f32,
                *width as f32,
                *height as f32,
            )
            .then_some((true, *id)),
            MenuWidget::TextField {
                id, x, y, width, ..
            } => {
                inside(ox + *x as f32, oy + *y as f32, *width as f32, 20.0).then_some((false, *id))
            }
            _ => None,
        })
    }

    /// Sends a message about the mod's screen to the mod: to the server this
    /// window plays on, or to this window's own server mods.
    fn send_menu(&mut self, window: Option<i32>, message: MenuMessage) {
        match window {
            Some(_) => {
                if let Ok(data) = serde_json::to_vec(&message) {
                    self.send(horizon_protocol::packets::play::Serverbound::CustomPayload(
                        horizon_protocol::packets::configuration::CustomPayload {
                            channel: CHANNEL.into(),
                            data,
                        },
                    ));
                }
            }
            None => self.local_menu_message(message),
        }
    }

    /// A left click on one of the mod's container screen's buttons. True
    /// when one was clicked.
    pub(super) fn mod_menu_press(&mut self) -> bool {
        let Some(r) = self.renderer.as_ref() else {
            return false;
        };
        let batch = HudBatch::new([r.config.width, r.config.height]);
        let (ox, oy) = self.inventory_origin(&batch);
        let Some(m) = self.mod_menu() else {
            return false;
        };
        let Some((true, id)) = Self::layout_hit(m, self.cursor, ox, oy) else {
            return false;
        };
        let window = m.window;
        self.audio.play_button_click();
        self.send_menu(
            window,
            MenuMessage::Button {
                window: window.unwrap_or(0),
                id,
            },
        );
        true
    }

    /// A click on the mod's panel: a button to the mod, a text field focused.
    pub(super) fn mod_panel_click(&mut self, button: MouseButton) {
        if button != MouseButton::Left {
            return;
        }
        let Some(r) = self.renderer.as_ref() else {
            return;
        };
        let batch = HudBatch::new([r.config.width, r.config.height]);
        let Some(m) = self.mod_panel() else {
            return;
        };
        let (ox, oy) = self.mod_panel_origin(&batch, m);
        let hit = Self::layout_hit(m, self.cursor, ox, oy);
        let window = m.window;
        let fields = m.fields.clone();
        match hit {
            Some((true, id)) => {
                self.audio.play_button_click();
                // The fields' texts first, so the button's action has them.
                for (field, value) in fields {
                    self.send_menu(
                        window,
                        MenuMessage::Text {
                            window: PANEL,
                            id: field,
                            value,
                        },
                    );
                }
                self.send_menu(window, MenuMessage::Button { window: PANEL, id });
            }
            Some((false, id)) => {
                if let Some(m) = self.mod_menu.as_mut() {
                    m.focus = Some(id);
                }
            }
            None => {
                if let Some(m) = self.mod_menu.as_mut() {
                    m.focus = None;
                }
            }
        }
    }

    /// A key on the mod's panel: typing into the focused field, Backspace,
    /// Enter sending it to the mod (Escape closes the panel elsewhere).
    pub(super) fn mod_panel_key(&mut self, event: &KeyEvent) {
        if event.state != winit::event::ElementState::Pressed {
            return;
        }
        let Some(m) = self.mod_menu.as_mut().filter(|m| m.panel) else {
            return;
        };
        let Some(focus) = m.focus else {
            return;
        };
        let max = m
            .layout
            .widgets
            .iter()
            .find_map(|w| match w {
                MenuWidget::TextField { id, max_length, .. } if *id == focus => Some(*max_length),
                _ => None,
            })
            .unwrap_or(256) as usize;
        let window = m.window;
        let Some((_, value)) = m.fields.iter_mut().find(|(id, _)| *id == focus) else {
            return;
        };
        match event.physical_key {
            PhysicalKey::Code(KeyCode::Backspace) => {
                value.pop();
            }
            PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                let value = value.clone();
                self.send_menu(
                    window,
                    MenuMessage::Text {
                        window: PANEL,
                        id: focus,
                        value,
                    },
                );
            }
            _ => {
                if let Some(text) = event.text.as_deref() {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if value.chars().count() < max {
                            value.push(c);
                        }
                    }
                }
            }
        }
    }

    /// Closes the mod's panel: back to the world. `tell` (Escape) sends the
    /// mod every field's text and that it closed.
    pub(super) fn close_mod_panel(&mut self, tell: bool) {
        let Some(m) = self.mod_menu.take() else {
            return;
        };
        if self.screen == Screen::ModPanel {
            self.screen = Screen::Playing;
            if self.player.is_some() {
                self.capture(true);
            }
        }
        if tell && m.panel {
            for (id, value) in m.fields {
                self.send_menu(
                    m.window,
                    MenuMessage::Text {
                        window: PANEL,
                        id,
                        value,
                    },
                );
            }
            self.send_menu(m.window, MenuMessage::Closed { window: PANEL });
        }
    }

    /// Opens a panel a mod sent: over the world, the pointer free.
    pub(super) fn open_mod_panel(&mut self, window: Option<i32>, layout: MenuLayout) {
        self.mod_menu = Some(ModMenu::new(window, layout, true));
        self.screen = Screen::ModPanel;
        self.capture(false);
    }

    /// What the server this window plays on sent on [`CHANNEL`]: a screen for
    /// the menu it opened, a panel, a data value, or a close. False for
    /// another channel.
    pub(super) fn remote_menu_message(&mut self, channel: &str, data: &[u8]) -> bool {
        if channel != CHANNEL {
            return false;
        }
        match serde_json::from_slice::<MenuMessage>(data) {
            Ok(MenuMessage::Open { window, layout }) => {
                self.mod_menu = Some(ModMenu::new(Some(window), layout, false));
            }
            Ok(MenuMessage::Panel { layout }) => self.open_mod_panel(Some(PANEL), layout),
            Ok(MenuMessage::Close) => {
                if self.mod_menu.as_ref().is_some_and(|m| m.panel) {
                    self.close_mod_panel(false);
                }
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
            Ok(
                MenuMessage::Button { .. } | MenuMessage::Text { .. } | MenuMessage::Closed { .. },
            )
            | Err(_) => {}
        }
        true
    }
}
