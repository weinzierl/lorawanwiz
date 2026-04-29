//! Library crate for the LoRaWAN visualizer. The binary in main.rs is a
//! tiny shim that runs `App::new().add_plugins(LorawanwizPlugin).run()`.

pub mod audio;
pub mod export;
pub mod math;
pub mod persistence;
pub mod pipeline;
pub mod state;
pub mod time_visualization;
pub mod ui;
pub mod visualization;

use bevy::prelude::*;

pub struct LorawanwizPlugin;

impl Plugin for LorawanwizPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<state::LorawanInputs>()
            .init_resource::<state::InputsDirty>()
            .init_resource::<state::PipelineOutput>()
            .init_resource::<state::ChirpAnimator>()
            .init_resource::<state::CanvasView>()
            .init_resource::<state::CryptoEdit>()
            .init_resource::<state::AudioSettings>()
            .init_resource::<state::DecodeView>()
            .init_resource::<ui::MessageFieldFocus>()
            .init_resource::<ui::ActiveTab>()
            .init_resource::<audio::AudioState>()
            .init_resource::<visualization::DragState>()
            .add_systems(
                Startup,
                (
                    visualization::setup_camera,
                    ui::build_ui,
                    ui::setup_tooltip,
                    ui::set_window_title,
                ),
            )
            .add_systems(Update, pipeline::run_pipeline)
            .add_systems(
                Update,
                (
                    ui::handle_button_hover,
                    ui::handle_homepage_hover,
                    ui::handle_homepage_click,
                    ui::handle_cycle_clicks,
                    ui::handle_volume_click,
                    ui::handle_mute_click,
                    ui::handle_field_focus,
                    ui::handle_message_typing,
                    ui::handle_crypto_typing,
                    ui::refresh_message_field_visual,
                    ui::refresh_crypto_field_visual,
                    ui::handle_scroll,
                    ui::handle_tab_clicks,
                    ui::refresh_tab_visibility,
                ),
            )
            .add_systems(
                Update,
                (
                    ui::refresh_labels,
                    ui::refresh_crypto_field_labels,
                    ui::refresh_audio_button_labels,
                    ui::rebuild_step_panels,
                    ui::handle_tooltips,
                    visualization::rebuild_chirp_canvas,
                    visualization::animate_chirps,
                    visualization::refresh_canvas_visibility,
                    visualization::handle_canvas_input,
                    visualization::apply_canvas_view,
                    visualization::reset_canvas_view_on_tab_change,
                    audio::handle_play_button,
                    audio::handle_stop_button,
                    audio::tick_animator,
                    audio::apply_audio_settings,
                ),
            )
            .add_systems(
                Update,
                (
                    persistence::handle_save_click,
                    persistence::handle_load_click,
                    persistence::poll_pending_load,
                    export::handle_export_click,
                    ui::handle_decode_click,
                    ui::refresh_decode_label,
                    time_visualization::rebuild_time_canvas,
                    time_visualization::animate_time_chirps,
                    time_visualization::refresh_time_canvas_visibility,
                ),
            )
            .add_systems(Startup, mark_dirty_at_start);
    }
}

fn mark_dirty_at_start(mut dirty: ResMut<state::InputsDirty>) {
    dirty.0 = true;
}
