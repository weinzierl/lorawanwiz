//! Native binary entry point. The browser side is built via `cargo build
//! --target wasm32-unknown-unknown` and the resulting .wasm is loaded by
//! the bundled `index.html`; that path doesn't go through this file.

use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResolution};
use lorawanwiz::LorawanwizPlugin;

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!(
                        "lorawanwiz v{} - LoRaWAN modulation visualizer - Copyright L. Weinzierl, 2026 - https://weinzierlweb.com",
                        env!("CARGO_PKG_VERSION")
                    ),
                    // Tall enough to fit title + toolbar + tab bar +
                    // content + footer without the footer copyright
                    // getting cut off. 16:10 keeps the canvas roomy.
                    // WindowResolution::new takes u32 in Bevy 0.18.
                    resolution: WindowResolution::new(1280, 800),
                    present_mode: PresentMode::AutoVsync,
                    ..default()
                }),
                ..default()
            }),
        )
        .add_plugins(LorawanwizPlugin)
        .run();
}
