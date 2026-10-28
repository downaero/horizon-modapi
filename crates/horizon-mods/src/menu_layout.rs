//! A screen of a mod's own (capability `menus`): what a server sends a
//! client to draw over a container menu, apart from the runtime so a
//! client that runs no mods still draws it.

/// A screen of a mod's own (capability `menus`), as its clients draw it.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MenuLayout {
    pub width: u32,
    pub height: u32,
    /// (menu slot, x, y).
    pub slots: Vec<(u32, i32, i32)>,
    pub widgets: Vec<MenuWidget>,
}
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum MenuWidget {
    Label {
        x: i32,
        y: i32,
        text: String,
        color: u32,
    },
    Progress {
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        value: u32,
        max: u32,
        vertical: bool,
        color: u32,
        background: u32,
    },
    Button {
        id: u32,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
        label: String,
    },
    TextField {
        id: u32,
        x: i32,
        y: i32,
        width: u32,
        value: String,
        max_length: u32,
    },
}

/// The custom payload channel a screen's layout, data and buttons go on.
pub const CHANNEL: &str = "horizon:menu";

/// What goes on [`CHANNEL`], as JSON.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum MenuMessage {
    /// To the client: the screen of the container menu `window`.
    Open { window: i32, layout: MenuLayout },
    /// To the client: data value `index` of the screen of `window`.
    Data { window: i32, index: u32, value: i32 },
    /// To the server: button `id` pressed on the screen of `window`.
    Button { window: i32, id: u32 },
    /// To the client: a screen with no container behind it ([`PANEL`]).
    Panel { layout: MenuLayout },
    /// To the client: the mod closed its screen.
    Close,
    /// To the server: what the player typed into field `id`.
    Text { window: i32, id: u32, value: String },
    /// To the server: the player closed the screen of `window`.
    Closed { window: i32 },
}

/// The window a panel (a screen with no container) goes by.
pub const PANEL: i32 = -1;
