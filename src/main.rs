use bevy::prelude::*;
use lorawanwiz::LorawanwizPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "lorawanwiz - LoRaWAN modulation visualizer".to_string(),
                canvas: Some("#bevy".to_string()),
                fit_canvas_to_parent: true,
                prevent_default_event_handling: false,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(LorawanwizPlugin)
        .run();
}
