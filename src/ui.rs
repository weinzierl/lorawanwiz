//! Header, toolbar, tabs, step panels, scroll, tooltips, footer.

use bevy::ecs::message::MessageReader;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonState;
use bevy::prelude::*;

use crate::math::{ChirpDirection, SymbolKind};
use crate::state::{
    parse_hex, AudioSettings, ChirpAnimator, CodingRate, CryptoEdit, CryptoFocus, DecodeView,
    InputsDirty, LorawanInputs, PipelineOutput,
};

const PANEL_BG: Color = Color::srgb(0.10, 0.11, 0.14);
const HEADER_BG: Color = Color::srgb(0.13, 0.14, 0.18);
const TOOLBAR_BG: Color = Color::srgb(0.16, 0.17, 0.21);
// Footer reads as a chrome band paired with the toolbar. Picked a warmer
// dark tone so it differs from PAGE_BG (cool dark) and TAB_BG (near-black).
const FOOTER_BG: Color = Color::srgb(0.14, 0.13, 0.11);
const TOOLBAR_BORDER: Color = Color::srgb(0.30, 0.32, 0.38);
const FOOTER_BORDER: Color = Color::srgb(0.28, 0.26, 0.22);
const PANEL_FG: Color = Color::srgb(0.92, 0.94, 0.97);
const ACCENT: Color = Color::srgb(0.40, 0.75, 1.00);
const MUTED_FG: Color = Color::srgb(0.55, 0.58, 0.65);
const LINK: Color = Color::srgb(0.55, 0.80, 1.00);
const BUTTON_BG: Color = Color::srgb(0.22, 0.24, 0.30);
const BUTTON_HOVER: Color = Color::srgb(0.28, 0.32, 0.40);
const BUTTON_DISABLED: Color = Color::srgb(0.18, 0.20, 0.24);
const TAB_BG: Color = Color::srgb(0.05, 0.06, 0.08);
const TAB_BUTTON_BG: Color = Color::srgb(0.10, 0.11, 0.14);
const PAGE_BG: Color = Color::srgb(0.06, 0.07, 0.09);
const FIELD_FOCUS: Color = Color::srgb(0.30, 0.40, 0.55);
const ERROR_RED: Color = Color::srgb(0.90, 0.40, 0.45);
const OK_GREEN: Color = Color::srgb(0.55, 0.85, 0.55);
const MUTED_RED: Color = Color::srgb(0.85, 0.50, 0.45);

const FONT_S: f32 = 13.0;
const FONT_M: f32 = 15.0;
const FONT_L: f32 = 18.0;
const FONT_XL: f32 = 22.0;

const VOLUME_BTN_WIDTH: f32 = 80.0;
const MUTE_BTN_WIDTH: f32 = 110.0;
const PLAY_BTN_WIDTH: f32 = 90.0;
const STOP_BTN_WIDTH: f32 = 90.0;
const FILE_BTN_WIDTH: f32 = 90.0;
const DECODE_BTN_WIDTH: f32 = 130.0;

const COPYRIGHT_TEXT: &str = "Copyright L. Weinzierl, 2026";
const HOMEPAGE_URL: &str = "https://weinzierlweb.com";

#[derive(Component)]
pub struct UiRoot;

#[derive(Component, Copy, Clone)]
pub struct StepsContainer(pub Tab);

#[derive(Component, Copy, Clone)]
pub enum CycleControl {
    Sf,
    Bw,
    Cr,
    FCnt,
    FPort,
}

#[derive(Component)]
pub struct MessageField;

#[derive(Component)]
pub struct MessageFieldText;

#[derive(Component)]
pub struct MessageByteCount;

#[derive(Component)]
pub struct PlayAudioButton;

#[derive(Component)]
pub struct PlayAudioLabel;

#[derive(Component)]
pub struct StopAudioButton;

#[derive(Component)]
pub struct VolumeButton;

#[derive(Component)]
pub struct VolumeLabel;

#[derive(Component)]
pub struct MuteButton;

#[derive(Component)]
pub struct MuteLabel;

#[derive(Component)]
pub struct SaveButton;

#[derive(Component)]
pub struct LoadButton;

#[derive(Component)]
pub struct ExportButton;

#[derive(Component)]
pub struct DecodeButton;

#[derive(Component)]
pub struct DecodeLabel;

#[derive(Component)]
pub struct HomepageLink;

#[derive(Resource, Default)]
pub struct MessageFieldFocus(pub bool);

#[derive(Component)]
pub struct LiveLabel(pub LabelKind);

#[derive(Copy, Clone)]
pub enum LabelKind {
    Sf,
    Bw,
    Cr,
    DevAddr,
    FCnt,
    FPort,
    AppSkey,
    NwkSkey,
}

#[derive(Component, Copy, Clone)]
pub struct CryptoField(pub CryptoFocus);

#[derive(Component, Copy, Clone)]
pub struct CryptoFieldText(pub CryptoFocus);

#[derive(Component, Copy, Clone)]
pub struct CryptoFieldStatus(pub CryptoFocus);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Tab {
    Inputs,
    Plaintext,
    Frame,
    Modulation,
    About,
}

#[derive(Resource)]
pub struct ActiveTab(pub Tab);

impl Default for ActiveTab {
    fn default() -> Self {
        Self(Tab::Inputs)
    }
}

#[derive(Component, Copy, Clone)]
pub struct TabButton(pub Tab);

#[derive(Component, Copy, Clone)]
pub struct TabContent(pub Tab);

pub fn set_window_title(mut windows: Query<&mut Window>) {
    for mut w in &mut windows {
        let title = format!(
            "lorawanwiz v{} - LoRaWAN modulation visualizer - {} - {}",
            env!("CARGO_PKG_VERSION"),
            COPYRIGHT_TEXT,
            HOMEPAGE_URL,
        );
        if w.title != title {
            w.title = title;
        }
    }
}

pub fn build_ui(mut commands: Commands) {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            UiRoot,
        ))
        .with_children(|root| {
            // === Title bar ===
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(10.0)),
                    column_gap: Val::Px(12.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Baseline,
                    ..default()
                },
                BackgroundColor(HEADER_BG),
            ))
            .with_children(|h| {
                h.spawn((
                    Text::new("lorawanwiz"),
                    TextFont { font_size: FONT_XL, ..default() },
                    TextColor(ACCENT),
                ));
                h.spawn((
                    Text::new("LoRaWAN modulation visualizer"),
                    TextFont { font_size: FONT_M, ..default() },
                    TextColor(PANEL_FG),
                ));
            });

            // === Toolbar ===
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect {
                        left: Val::Px(16.0),
                        right: Val::Px(16.0),
                        top: Val::Px(8.0),
                        bottom: Val::Px(8.0),
                    },
                    border: UiRect::bottom(Val::Px(1.0)),
                    column_gap: Val::Px(8.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(TOOLBAR_BG),
                BorderColor::all(TOOLBAR_BORDER),
            ))
            .with_children(|t| {
                volume_button(t);
                mute_button(t);
                play_button(t);
                stop_button(t);
                // Spacer between audio controls and view-toggle controls.
                t.spawn(Node { width: Val::Px(16.0), ..default() });
                decode_button(t);
                // Spacer between view-toggle and file controls.
                t.spawn(Node { width: Val::Px(16.0), ..default() });
                save_button(t);
                load_button(t);
                export_button(t);
            });

            // === Tab bar ===
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                    column_gap: Val::Px(6.0),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(TAB_BG),
            ))
            .with_children(|bar| {
                tab_button(bar, Tab::Inputs, "Inputs");
                tab_button(bar, Tab::Plaintext, "Plaintext");
                tab_button(bar, Tab::Frame, "Frame");
                tab_button(bar, Tab::Modulation, "Modulation");
                tab_button(bar, Tab::About, "About");
            });

            // === Tab content area ===
            root.spawn(Node {
                width: Val::Percent(100.0),
                flex_grow: 1.0,
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|content| {
                build_inputs_tab(content);
                build_plaintext_tab(content);
                build_frame_tab(content);
                build_modulation_tab(content);
                build_about_tab(content);
            });

            // === Footer ===
            root.spawn((
                Node {
                    width: Val::Percent(100.0),
                    padding: UiRect::axes(Val::Px(16.0), Val::Px(6.0)),
                    border: UiRect::top(Val::Px(1.0)),
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(FOOTER_BG),
                BorderColor::all(FOOTER_BORDER),
            ))
            .with_children(|f| {
                f.spawn(Node {
                    flex_basis: Val::Percent(33.3),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn((
                        Text::new(COPYRIGHT_TEXT),
                        TextFont { font_size: FONT_S, ..default() },
                        TextColor(MUTED_FG),
                    ));
                });

                f.spawn(Node {
                    flex_basis: Val::Percent(33.3),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn((
                        Button,
                        Node {
                            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
                            ..default()
                        },
                        BackgroundColor(Color::NONE),
                        HomepageLink,
                        Tooltip("Click to open weinzierlweb.com in your browser."),
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(HOMEPAGE_URL),
                            TextFont { font_size: FONT_S, ..default() },
                            TextColor(LINK),
                        ));
                    });
                });

                f.spawn(Node {
                    flex_basis: Val::Percent(33.3),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Row,
                    justify_content: JustifyContent::FlexEnd,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|c| {
                    c.spawn((
                        Text::new(format!("v{}", env!("CARGO_PKG_VERSION"))),
                        TextFont { font_size: FONT_S, ..default() },
                        TextColor(MUTED_FG),
                    ));
                });
            });
        });
}

fn build_inputs_tab(content: &mut ChildSpawnerCommands) {
    content
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(14.0),
                max_width: Val::Px(900.0),
                ..default()
            },
            BackgroundColor(PAGE_BG),
            ScrollPosition::default(),
            StepsContainer(Tab::Inputs),
            TabContent(Tab::Inputs),
        ))
        .with_children(|p| {
            section_heading(p, "Message");
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(10.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                input_label(row, "msg");
                message_field(row);
                byte_count_label(row);
            });

            section_heading(p, "Modulation parameters");
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                row_gap: Val::Px(8.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                input_label(row, "SF");
                cycle_button(row, CycleControl::Sf, LabelKind::Sf,
                    "Spreading Factor (7-12). Higher SF means more chirps per symbol: lower data rate but longer range. Click to cycle.");
                input_label(row, "BW");
                cycle_button(row, CycleControl::Bw, LabelKind::Bw,
                    "Bandwidth in kHz (125, 250, 500). Wider BW means higher data rate but more channel use and worse sensitivity. Click to cycle.");
                input_label(row, "CR");
                cycle_button(row, CycleControl::Cr, LabelKind::Cr,
                    "Coding Rate (4/5..4/8). Forward error correction overhead. 4/8 is most robust, 4/5 is fastest. Click to cycle.");
            });

            section_heading(p, "Frame parameters");
            p.spawn(Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(14.0),
                row_gap: Val::Px(8.0),
                flex_wrap: FlexWrap::Wrap,
                ..default()
            })
            .with_children(|row| {
                input_label(row, "FCnt");
                cycle_button(row, CycleControl::FCnt, LabelKind::FCnt,
                    "Frame Counter. Increments per uplink, prevents replay. Different FCnt = different keystream and MIC for the same plaintext. Click to increment.");
                input_label(row, "FPort");
                cycle_button(row, CycleControl::FPort, LabelKind::FPort,
                    "Frame Port (1-9 here). Application port number, like a UDP port for LoRaWAN. Click to cycle.");
            });

            section_heading(p, "Crypto context (editable)");
            p.spawn((
                Text::new("Hex digits only. Whitespace and ':' allowed and ignored. Click a field to focus, type to edit, Backspace to delete. Changes apply when the field is filled to the expected length."),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(MUTED_FG),
            ));
            crypto_row(p, CryptoFocus::DevAddr, "DevAddr", 4,
                "Device address, 4 bytes (8 hex digits). Identifies the device on the LoRaWAN network. Used in the Ai block and the MIC.");
            crypto_row(p, CryptoFocus::AppSKey, "AppSKey", 16,
                "Application Session Key, 16 bytes (32 hex digits). Used to encrypt and decrypt FRMPayload via AES-CTR.");
            crypto_row(p, CryptoFocus::NwkSKey, "NwkSKey", 16,
                "Network Session Key, 16 bytes (32 hex digits). Used to compute the CMAC-AES-128 MIC over the wire frame.");
            p.spawn((
                Text::new("Defaults are public test vectors. Do not paste real production keys here."),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(MUTED_FG),
            ));
        });
}

fn build_plaintext_tab(content: &mut ChildSpawnerCommands) {
    content.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            overflow: Overflow::scroll_y(),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(PAGE_BG),
        ScrollPosition::default(),
        StepsContainer(Tab::Plaintext),
        TabContent(Tab::Plaintext),
    ));
}

fn build_frame_tab(content: &mut ChildSpawnerCommands) {
    content.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            display: Display::None,
            overflow: Overflow::scroll_y(),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(12.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(PAGE_BG),
        ScrollPosition::default(),
        StepsContainer(Tab::Frame),
        TabContent(Tab::Frame),
    ));
}

fn build_modulation_tab(content: &mut ChildSpawnerCommands) {
    content
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(12.0)),
                row_gap: Val::Px(8.0),
                ..default()
            },
            TabContent(Tab::Modulation),
        ))
        .with_children(|m| {
            m.spawn((
                Text::new("Baseband chirp visualization"),
                TextFont { font_size: FONT_L, ..default() },
                TextColor(ACCENT),
            ));
            m.spawn((
                Text::new(
                    "Per-chirp label: type letter (P/S/H/D) + index, then the raw symbol value.\n\
                     Pan: click and drag.   Zoom: mouse wheel, or pinch on a Mac trackpad.\n\
                     View resets when you leave this tab. Click Play in the toolbar to start playback;\n\
                     the highlight bar tracks the audible chirp. Stop ends playback immediately."
                ),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(MUTED_FG),
            ));
        });
}

fn build_about_tab(content: &mut ChildSpawnerCommands) {
    content
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                display: Display::None,
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(20.0)),
                row_gap: Val::Px(10.0),
                max_width: Val::Px(900.0),
                ..default()
            },
            BackgroundColor(PAGE_BG),
            ScrollPosition::default(),
            StepsContainer(Tab::About),
            TabContent(Tab::About),
        ))
        .with_children(|a| {
            about_heading(a, "About");
            about_para(a,
                "lorawanwiz is an educational tool that visualizes the LoRaWAN v1.1 uplink path. Type a message, pick SF/BW/CR, and watch the same plaintext travel through encryption, MIC, framing, symbol packing, and chirp modulation."
            );
            about_para(a, COPYRIGHT_TEXT);
            about_para(a, HOMEPAGE_URL);

            about_heading(a, "What's real");
            about_para(a,
                "AES-128 in LoRaWAN's CTR-style construction with the Ai block. CMAC-AES-128 for the MIC over a B0 prefix and the message. The full wire frame layout. Gray-coded SF-bit symbols. Linear baseband upchirps that wrap modulo BW."
            );
            about_heading(a, "What's simplified");
            about_para(a,
                "No whitening, no Hamming FEC, no interleaving. No CRC. The preamble is shown as 8 downchirps of value 0; real LoRa preamble is 8 upchirps followed by 2 sync symbols and a 2.25-symbol downchirp SFD. LoRaWAN 1.1 splits the MIC across two network keys; this demo uses one NwkSKey, matching 1.0.x."
            );
            about_heading(a, "Audio");
            about_para(a,
                "Real LoRa chirps are 125-500 kHz, well above hearing. The audio button plays each chirp at a fixed audible target (top frequency 3 kHz, 80 ms per symbol) regardless of SF/BW, so every config sounds at the same comfortable pace. Visualization keeps the true LoRa numbers."
            );
            about_para(a,
                "Volume cycles 25%, 50%, 75%, 100%. Sound toggles mute. Both apply live: changes during a playback take effect immediately. Stop ends playback at any time."
            );
            about_heading(a, "Save and Load");
            about_para(a,
                "Save writes the current message, modulation parameters, frame parameters, crypto keys, and audio settings to a RON file. Load reads such a file back. RON is a human-readable format, so files can be hand-edited if you want."
            );
            about_heading(a, "Pan and zoom");
            about_para(a,
                "On the Modulation tab: click and drag to pan. Mouse wheel zooms; on a Mac trackpad, pinch zooms. Switching tabs resets the view."
            );
            about_heading(a, "Max payload");
            about_para(a,
                "The byte counter next to the message field shows how many bytes the current SF/BW combination allows in FRMPayload (EU868). DR0-DR2 (SF12-SF10 at 125 kHz) are limited to 51 bytes, DR3 (SF9) to 115, and DR4-DR5 (SF8-SF7) to 222."
            );
        });
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn input_label(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_string()),
        TextFont { font_size: FONT_M, ..default() },
        TextColor(MUTED_FG),
    ));
}

fn section_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_string()),
        TextFont { font_size: FONT_L, ..default() },
        TextColor(ACCENT),
        Node {
            margin: UiRect { top: Val::Px(8.0), ..default() },
            ..default()
        },
    ));
}

fn cycle_button(
    parent: &mut ChildSpawnerCommands,
    control: CycleControl,
    kind: LabelKind,
    tooltip: &'static str,
) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                width: Val::Px(80.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            control,
            Tooltip(tooltip),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("..."),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
                LiveLabel(kind),
            ));
        });
}

fn message_field(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                width: Val::Px(480.0),
                min_width: Val::Px(160.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            MessageField,
            Tooltip("The plaintext message that will be encrypted and modulated. Click to focus, then type. Backspace to delete. Field length adapts to the EU868 max payload for the current SF/BW."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("hello"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
                MessageFieldText,
            ));
        });
}

fn byte_count_label(parent: &mut ChildSpawnerCommands) {
    parent.spawn((
        Text::new("0 / 51 B"),
        TextFont { font_size: FONT_S, ..default() },
        TextColor(MUTED_FG),
        MessageByteCount,
    ));
}

fn crypto_row(
    parent: &mut ChildSpawnerCommands,
    field: CryptoFocus,
    label: &'static str,
    byte_len: usize,
    tooltip: &'static str,
) {
    let width_px = byte_len as f32 * 22.0 + 24.0;
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(10.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(MUTED_FG),
                Node { min_width: Val::Px(72.0), ..default() },
            ));
            row.spawn((
                Button,
                Node {
                    width: Val::Px(width_px),
                    padding: UiRect::axes(Val::Px(8.0), Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(BUTTON_BG),
                CryptoField(field),
                Tooltip(tooltip),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("..."),
                    TextFont { font_size: FONT_M, ..default() },
                    TextColor(PANEL_FG),
                    CryptoFieldText(field),
                ));
            });
            row.spawn((
                Text::new("ok"),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(OK_GREEN),
                CryptoFieldStatus(field),
            ));
        });
}

fn volume_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                width: Val::Px(VOLUME_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            VolumeButton,
            Tooltip("Volume level. Click to cycle through 25%, 50%, 75%, 100%."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("75%"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
                VolumeLabel,
            ));
        });
}

fn mute_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                width: Val::Px(MUTE_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            MuteButton,
            Tooltip("Click to toggle mute. When muted, playback is silent but the highlight bar still moves."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Sound: on"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
                MuteLabel,
            ));
        });
}

fn play_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                width: Val::Px(PLAY_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(ACCENT),
            PlayAudioButton,
            Tooltip("Play the chirp sequence. The highlight bar in the Modulation tab tracks the audible chirp. Disabled while a playback is running."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Play"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(Color::BLACK),
                PlayAudioLabel,
            ));
        });
}

fn stop_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                width: Val::Px(STOP_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            StopAudioButton,
            Tooltip("Stop the current playback immediately. Resets the highlight bar."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Stop"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
            ));
        });
}

fn save_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                width: Val::Px(FILE_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            SaveButton,
            Tooltip("Save current message, modulation, frame, crypto, and audio settings to a RON file."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Save"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
            ));
        });
}

fn load_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                width: Val::Px(FILE_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            LoadButton,
            Tooltip("Load a RON file produced by Save and apply all of its settings."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Load"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
            ));
        });
}

fn export_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                width: Val::Px(FILE_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            ExportButton,
            Tooltip("Export the entire LoRaWAN flow as a Typst document. Compile with `typst compile lorawanwiz_export.typ` to get a PDF."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Export"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
            ));
        });
}

fn decode_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(10.0), Val::Px(6.0)),
                width: Val::Px(DECODE_BTN_WIDTH),
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(BUTTON_BG),
            DecodeButton,
            Tooltip("Toggle the decoding visualization. When on, the chirp canvas adds two rows: the conjugate reference downchirp, and the product (signal times conjugate reference) which would feed an FFT in a real receiver. Click again to hide them."),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new("Decode: off"),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
                DecodeLabel,
            ));
        });
}

fn tab_button(parent: &mut ChildSpawnerCommands, tab: Tab, label: &str) {
    parent
        .spawn((
            Button,
            Node {
                padding: UiRect::axes(Val::Px(14.0), Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(TAB_BUTTON_BG),
            TabButton(tab),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(label.to_string()),
                TextFont { font_size: FONT_M, ..default() },
                TextColor(PANEL_FG),
            ));
        });
}

fn about_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_string()),
        TextFont { font_size: FONT_L, ..default() },
        TextColor(ACCENT),
        Node { margin: UiRect::top(Val::Px(8.0)), ..default() },
    ));
}

fn about_para(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_string()),
        TextFont { font_size: FONT_M, ..default() },
        TextColor(PANEL_FG),
    ));
}

// ---------------------------------------------------------------------------
// Audio button handlers
// ---------------------------------------------------------------------------

pub fn handle_volume_click(
    q: Query<&Interaction, (Changed<Interaction>, With<VolumeButton>)>,
    mut settings: ResMut<AudioSettings>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            settings.cycle_volume();
        }
    }
}

pub fn handle_mute_click(
    q: Query<&Interaction, (Changed<Interaction>, With<MuteButton>)>,
    mut settings: ResMut<AudioSettings>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            settings.muted = !settings.muted;
        }
    }
}

pub fn handle_decode_click(
    q: Query<&Interaction, (Changed<Interaction>, With<DecodeButton>)>,
    mut decode: ResMut<DecodeView>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            decode.enabled = !decode.enabled;
        }
    }
}

pub fn refresh_decode_label(
    decode: Res<DecodeView>,
    mut q: Query<&mut Text, With<DecodeLabel>>,
) {
    if !decode.is_changed() {
        return;
    }
    let s = if decode.enabled { "Decode: on" } else { "Decode: off" };
    for mut t in &mut q {
        if t.0 != s {
            t.0 = s.to_string();
        }
    }
}

pub fn refresh_audio_button_labels(
    settings: Res<AudioSettings>,
    animator: Res<ChirpAnimator>,
    mut volume_label_q: Query<&mut Text, (With<VolumeLabel>, Without<MuteLabel>, Without<PlayAudioLabel>)>,
    mut mute_label_q: Query<(&mut Text, &mut TextColor), (With<MuteLabel>, Without<VolumeLabel>, Without<PlayAudioLabel>)>,
    mut play_label_q: Query<&mut Text, (With<PlayAudioLabel>, Without<VolumeLabel>, Without<MuteLabel>)>,
    mut play_bg_q: Query<&mut BackgroundColor, With<PlayAudioButton>>,
) {
    if !settings.is_changed() && !animator.is_changed() {
        return;
    }
    let vol = settings.volume_label();
    for mut t in &mut volume_label_q {
        if t.0 != vol {
            t.0 = vol.clone();
        }
    }
    let (mute_text, mute_color) = if settings.muted {
        ("Sound: off".to_string(), MUTED_RED)
    } else {
        ("Sound: on".to_string(), PANEL_FG)
    };
    for (mut t, mut c) in &mut mute_label_q {
        if t.0 != mute_text {
            t.0 = mute_text.clone();
        }
        if c.0 != mute_color {
            c.0 = mute_color;
        }
    }
    let play_text = if animator.playing { "Playing..." } else { "Play" };
    for mut t in &mut play_label_q {
        if t.0 != play_text {
            t.0 = play_text.to_string();
        }
    }
    let play_bg_color = if animator.playing { BUTTON_DISABLED } else { ACCENT };
    for mut bg in &mut play_bg_q {
        if bg.0 != play_bg_color {
            bg.0 = play_bg_color;
        }
    }
}

pub fn handle_homepage_click(
    q: Query<&Interaction, (Changed<Interaction>, With<HomepageLink>)>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            open_url(HOMEPAGE_URL);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn open_url(url: &str) {
    if let Some(window) = web_sys::window() {
        let _ = window.open_with_url_and_target(url, "_blank");
    }
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "linux"))]
fn open_url(url: &str) {
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "macos"))]
fn open_url(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[cfg(all(not(target_arch = "wasm32"), target_os = "windows"))]
fn open_url(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "linux"),
    not(target_os = "macos"),
    not(target_os = "windows"),
))]
fn open_url(_url: &str) {
    eprintln!("open_url: unsupported platform");
}

// ---------------------------------------------------------------------------
// Interaction systems
// ---------------------------------------------------------------------------

pub fn handle_button_hover(
    mut q: Query<
        (&Interaction, &mut BackgroundColor),
        (
            Changed<Interaction>,
            With<Button>,
            Without<PlayAudioButton>,
            Without<MessageField>,
            Without<TabButton>,
            Without<CryptoField>,
            Without<HomepageLink>,
        ),
    >,
) {
    for (i, mut bg) in &mut q {
        bg.0 = match *i {
            Interaction::Hovered => BUTTON_HOVER,
            _ => BUTTON_BG,
        };
    }
}

pub fn handle_homepage_hover(
    mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<HomepageLink>)>,
) {
    for (i, mut bg) in &mut q {
        bg.0 = match *i {
            Interaction::Hovered => Color::srgba(0.40, 0.75, 1.00, 0.15),
            _ => Color::NONE,
        };
    }
}

pub fn handle_cycle_clicks(
    q: Query<(&Interaction, &CycleControl), (Changed<Interaction>, With<Button>)>,
    mut inputs: ResMut<LorawanInputs>,
    mut dirty: ResMut<InputsDirty>,
) {
    for (i, control) in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        match control {
            CycleControl::Sf => {
                inputs.sf = if inputs.sf >= 12 { 7 } else { inputs.sf + 1 };
            }
            CycleControl::Bw => {
                inputs.bw_hz = match inputs.bw_hz as u32 {
                    125_000 => 250_000.0,
                    250_000 => 500_000.0,
                    _ => 125_000.0,
                };
            }
            CycleControl::Cr => {
                let all = CodingRate::all();
                let idx = all.iter().position(|c| *c == inputs.coding_rate).unwrap_or(0);
                inputs.coding_rate = all[(idx + 1) % all.len()];
            }
            CycleControl::FCnt => {
                inputs.f_cnt = inputs.f_cnt.wrapping_add(1);
            }
            CycleControl::FPort => {
                inputs.f_port = if inputs.f_port >= 9 { 1 } else { inputs.f_port + 1 };
            }
        }
        dirty.0 = true;
    }
}

pub fn handle_field_focus(
    msg_q: Query<&Interaction, (Changed<Interaction>, With<MessageField>)>,
    crypto_q: Query<(&Interaction, &CryptoField), Changed<Interaction>>,
    other_buttons: Query<
        &Interaction,
        (
            Changed<Interaction>,
            With<Button>,
            Without<MessageField>,
            Without<CryptoField>,
        ),
    >,
    mut msg_focus: ResMut<MessageFieldFocus>,
    mut crypto: ResMut<CryptoEdit>,
) {
    for i in &msg_q {
        if *i == Interaction::Pressed {
            msg_focus.0 = true;
            crypto.focus = CryptoFocus::None;
        }
    }
    for (i, field) in &crypto_q {
        if *i == Interaction::Pressed {
            msg_focus.0 = false;
            crypto.focus = field.0;
        }
    }
    for i in &other_buttons {
        if *i == Interaction::Pressed {
            msg_focus.0 = false;
            crypto.focus = CryptoFocus::None;
        }
    }
}

pub fn handle_message_typing(
    mut events: MessageReader<KeyboardInput>,
    msg_focus: Res<MessageFieldFocus>,
    crypto: Res<CryptoEdit>,
    mut inputs: ResMut<LorawanInputs>,
    mut dirty: ResMut<InputsDirty>,
) {
    if !msg_focus.0 || crypto.focus != CryptoFocus::None {
        if !msg_focus.0 && crypto.focus == CryptoFocus::None {
            events.clear();
        }
        return;
    }
    let max = inputs.max_app_payload_bytes();
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => {
                inputs.message.pop();
                dirty.0 = true;
            }
            Key::Character(s) => {
                if inputs.message.len() + s.len() <= max {
                    inputs.message.push_str(s.as_str());
                    dirty.0 = true;
                }
            }
            Key::Space => {
                if inputs.message.len() + 1 <= max {
                    inputs.message.push(' ');
                    dirty.0 = true;
                }
            }
            _ => {}
        }
    }
}

pub fn handle_crypto_typing(
    mut events: MessageReader<KeyboardInput>,
    mut crypto: ResMut<CryptoEdit>,
    mut inputs: ResMut<LorawanInputs>,
    mut dirty: ResMut<InputsDirty>,
) {
    if crypto.focus == CryptoFocus::None {
        return;
    }
    let (target_len_bytes, target_text_len) = match crypto.focus {
        CryptoFocus::DevAddr => (4, 8),
        CryptoFocus::AppSKey => (16, 32),
        CryptoFocus::NwkSKey => (16, 32),
        CryptoFocus::None => return,
    };

    let buffer: &mut String = match crypto.focus {
        CryptoFocus::DevAddr => &mut crypto.dev_addr_text,
        CryptoFocus::AppSKey => &mut crypto.app_skey_text,
        CryptoFocus::NwkSKey => &mut crypto.nwk_skey_text,
        CryptoFocus::None => return,
    };

    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match &ev.logical_key {
            Key::Backspace => {
                buffer.pop();
            }
            Key::Character(s) => {
                for c in s.chars() {
                    if !c.is_ascii_hexdigit() && c != ':' && c != '-' {
                        continue;
                    }
                    if buffer.chars().filter(|x| x.is_ascii_hexdigit()).count() >= target_text_len
                        && c.is_ascii_hexdigit()
                    {
                        continue;
                    }
                    buffer.push(c.to_ascii_uppercase());
                }
            }
            _ => {}
        }
    }

    if let Some(bytes) = parse_hex(buffer.as_str(), target_len_bytes) {
        match crypto.focus {
            CryptoFocus::DevAddr => {
                let arr: [u8; 4] = bytes.try_into().unwrap();
                if inputs.dev_addr != arr {
                    inputs.dev_addr = arr;
                    dirty.0 = true;
                }
            }
            CryptoFocus::AppSKey => {
                let arr: [u8; 16] = bytes.try_into().unwrap();
                if inputs.app_skey != arr {
                    inputs.app_skey = arr;
                    dirty.0 = true;
                }
            }
            CryptoFocus::NwkSKey => {
                let arr: [u8; 16] = bytes.try_into().unwrap();
                if inputs.nwk_skey != arr {
                    inputs.nwk_skey = arr;
                    dirty.0 = true;
                }
            }
            CryptoFocus::None => {}
        }
    }
}

pub fn refresh_labels(
    inputs: Res<LorawanInputs>,
    mut q: Query<(&LiveLabel, &mut Text)>,
    mut field_q: Query<&mut Text, (With<MessageFieldText>, Without<LiveLabel>, Without<MessageByteCount>, Without<CryptoFieldText>, Without<CryptoFieldStatus>, Without<VolumeLabel>, Without<MuteLabel>, Without<PlayAudioLabel>)>,
    mut count_q: Query<
        &mut Text,
        (With<MessageByteCount>, Without<LiveLabel>, Without<MessageFieldText>, Without<CryptoFieldText>, Without<CryptoFieldStatus>, Without<VolumeLabel>, Without<MuteLabel>, Without<PlayAudioLabel>),
    >,
    mut field_node_q: Query<&mut Node, With<MessageField>>,
) {
    for (kind, mut text) in &mut q {
        let s = match kind.0 {
            LabelKind::Sf => format!("SF{}", inputs.sf),
            LabelKind::Bw => format!("{} kHz", (inputs.bw_hz / 1000.0) as u32),
            LabelKind::Cr => inputs.coding_rate.label().to_string(),
            LabelKind::DevAddr => format!(
                "{:02X}{:02X}{:02X}{:02X}",
                inputs.dev_addr[0], inputs.dev_addr[1], inputs.dev_addr[2], inputs.dev_addr[3]
            ),
            LabelKind::FCnt => format!("0x{:04X}", inputs.f_cnt as u16),
            LabelKind::FPort => format!("{}", inputs.f_port),
            LabelKind::AppSkey => abbreviated_hex(&inputs.app_skey),
            LabelKind::NwkSkey => abbreviated_hex(&inputs.nwk_skey),
        };
        if text.0 != s {
            text.0 = s;
        }
    }

    for mut t in &mut field_q {
        if t.0 != inputs.message {
            t.0 = inputs.message.clone();
        }
    }

    let max = inputs.max_app_payload_bytes();
    let used = inputs.message.len();
    let count_str = format!("{} / {} B", used, max);
    for mut t in &mut count_q {
        if t.0 != count_str {
            t.0 = count_str.clone();
        }
    }

    let char_w = 9.0;
    let target_w = (max as f32 * char_w + 16.0).clamp(160.0, 720.0);
    for mut node in &mut field_node_q {
        let new_w = Val::Px(target_w);
        if node.width != new_w {
            node.width = new_w;
        }
    }
}

pub fn refresh_crypto_field_labels(
    crypto: Res<CryptoEdit>,
    mut text_q: Query<(&CryptoFieldText, &mut Text), Without<CryptoFieldStatus>>,
    mut status_q: Query<(&CryptoFieldStatus, &mut Text, &mut TextColor), Without<CryptoFieldText>>,
) {
    for (field, mut text) in &mut text_q {
        let s = match field.0 {
            CryptoFocus::DevAddr => crypto.dev_addr_text.clone(),
            CryptoFocus::AppSKey => crypto.app_skey_text.clone(),
            CryptoFocus::NwkSKey => crypto.nwk_skey_text.clone(),
            CryptoFocus::None => continue,
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for (field, mut text, mut color) in &mut status_q {
        let (buf, n) = match field.0 {
            CryptoFocus::DevAddr => (&crypto.dev_addr_text, 4),
            CryptoFocus::AppSKey => (&crypto.app_skey_text, 16),
            CryptoFocus::NwkSKey => (&crypto.nwk_skey_text, 16),
            CryptoFocus::None => continue,
        };
        let parsed = parse_hex(buf, n).is_some();
        let hex_count = buf.chars().filter(|c| c.is_ascii_hexdigit()).count();
        let needed = n * 2;
        let (s, c) = if parsed {
            ("ok".to_string(), OK_GREEN)
        } else {
            (format!("{}/{} digits", hex_count, needed), ERROR_RED)
        };
        if text.0 != s {
            text.0 = s;
        }
        if color.0 != c {
            color.0 = c;
        }
    }
}

fn abbreviated_hex(bytes: &[u8]) -> String {
    let head: String = bytes.iter().take(3).map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("");
    let tail: String = bytes.iter().rev().take(2).rev().map(|b| format!("{:02X}", b)).collect::<Vec<_>>().join("");
    format!("{}...{}", head, tail)
}

#[derive(Component)]
pub struct StepPanel(pub usize);

pub fn rebuild_step_panels(
    inputs: Res<LorawanInputs>,
    output: Res<PipelineOutput>,
    mut commands: Commands,
    container_q: Query<(Entity, &StepsContainer)>,
    existing: Query<Entity, With<StepPanel>>,
) {
    if !inputs.is_changed() && !output.is_changed() {
        return;
    }
    for e in &existing {
        commands.entity(e).despawn();
    }

    let mut plaintext_container = None;
    let mut frame_container = None;
    for (e, sc) in &container_q {
        match sc.0 {
            Tab::Plaintext => plaintext_container = Some(e),
            Tab::Frame => frame_container = Some(e),
            _ => {}
        }
    }

    if let Some(container) = plaintext_container {
        commands.entity(container).with_children(|c| {
            step_panel(c, 1, "Input summary", |b| {
                kv(b, "message", &format!("\"{}\"", inputs.message));
                kv(b, "msg bytes", &format!("{} / {} B (EU868 max for SF{}/{} kHz)",
                    inputs.message.len(), inputs.max_app_payload_bytes(),
                    inputs.sf, (inputs.bw_hz / 1000.0) as u32));
                kv(b, "SF / BW / CR", &format!(
                    "SF{} / {} kHz / {}",
                    inputs.sf, (inputs.bw_hz / 1000.0) as u32, inputs.coding_rate.label()
                ));
                kv(b, "DevAddr", &format!(
                    "{:02X}{:02X}{:02X}{:02X}",
                    inputs.dev_addr[0], inputs.dev_addr[1],
                    inputs.dev_addr[2], inputs.dev_addr[3]
                ));
                kv(b, "FCnt / FPort", &format!("0x{:04X} / {}", inputs.f_cnt as u16, inputs.f_port));
                kv(b, "AppSKey", &abbreviated_hex(&inputs.app_skey));
                kv(b, "NwkSKey", &abbreviated_hex(&inputs.nwk_skey));
            });

            step_panel(c, 2, "Plaintext bytes (UTF-8)", |b| {
                kv(b, "ASCII", &format!("\"{}\"", inputs.message));
                kv(b, "hex", &hex_bytes(&output.plaintext));
            });

            step_panel(c, 3, "FRMPayload encryption (AES-128 CTR-style)", |b| {
                kv(b, "plaintext ", &hex_bytes(&output.plaintext));
                kv(b, "ciphertext", &hex_bytes(&output.ciphertext));
                for s in &output.encrypt_steps {
                    kv(b, &format!("A{}", s.block_index), &hex_bytes(&s.a_block));
                    kv(b, &format!("S{}", s.block_index), &hex_bytes(&s.s_block));
                    kv(b, &format!("PT{}", s.block_index), &hex_bytes(&s.plaintext));
                    kv(b, &format!("CT{}", s.block_index), &hex_bytes(&s.ciphertext));
                }
            });
        });
    }

    if let Some(container) = frame_container {
        commands.entity(container).with_children(|c| {
            step_panel(c, 4, "Frame MIC (CMAC-AES-128, NwkSKey)", |b| {
                kv(b, "MIC", &hex_bytes(&output.mic));
                kv(b, "covers", "MHDR | DevAddr(LE) | FCtrl | FCnt(LE16) | FPort | FRMPayload");
            });

            step_panel(c, 5, "Complete LoRaWAN frame", |b| {
                kv(b, "bytes", &hex_bytes(&output.frame));
                kv(b, "size",  &format!("{} B", output.frame.len()));
            });

            step_panel(c, 6, "Preamble | Sync | Header | Payload symbols", |b| {
                let mut summary = String::new();
                let mut last_kind: Option<SymbolKind> = None;
                let mut group_start = 0usize;
                for (i, sym) in output.symbols.iter().enumerate() {
                    if last_kind != Some(sym.kind) {
                        if let Some(k) = last_kind {
                            summary.push_str(&format!(
                                "  {:?}: {} symbol(s) [{}..{}]\n",
                                k, i - group_start, group_start, i - 1
                            ));
                        }
                        last_kind = Some(sym.kind);
                        group_start = i;
                    }
                }
                if let Some(k) = last_kind {
                    let i = output.symbols.len();
                    summary.push_str(&format!(
                        "  {:?}: {} symbol(s) [{}..{}]\n",
                        k, i - group_start, group_start, i - 1
                    ));
                }
                kv(b, "groups", summary.trim_end());
                kv(b, "total", &format!("{} symbols", output.symbols.len()));

                let mut sample = String::new();
                for sym in output.symbols.iter().take(12) {
                    let dir = match sym.direction {
                        ChirpDirection::Up => "^",
                        ChirpDirection::Down => "v",
                    };
                    sample.push_str(&format!(
                        "[{:>2}] {} raw=0x{:03X} gray=0x{:03X} ({:?})\n",
                        sym.index, dir, sym.raw, sym.gray, sym.kind
                    ));
                }
                kv(b, "first 12", sample.trim_end());
            });

            step_panel(c, 7, "Baseband chirps", |b| {
                kv(b, "T_sym", &format!("{:.3} ms", (1u32 << inputs.sf) as f32 / inputs.bw_hz * 1000.0));
                kv(b, "chirps queued", &format!("{}", output.chirps.len()));
                kv(b, "see", "Modulation tab for the chirp visualization (drag pan, wheel zoom)");
            });
        });
    }
}

fn step_panel(
    parent: &mut ChildSpawnerCommands,
    n: usize,
    title: &str,
    body: impl FnOnce(&mut ChildSpawnerCommands),
) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(10.0)),
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(PANEL_BG),
            StepPanel(n),
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(format!("Step {}: {}", n, title)),
                TextFont { font_size: FONT_L, ..default() },
                TextColor(ACCENT),
            ));
            body(p);
        });
}

fn kv(parent: &mut ChildSpawnerCommands, k: &str, v: &str) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(format!("{}:", k)),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(MUTED_FG),
                Node { min_width: Val::Px(110.0), ..default() },
            ));
            row.spawn((
                Text::new(v.to_string()),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(PANEL_FG),
            ));
        });
}

fn hex_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".to_string();
    }
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && i % 16 == 0 {
            s.push('\n');
        } else if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{:02X}", b));
    }
    s
}

const SCROLL_SPEED_PX: f32 = 40.0;

pub fn handle_scroll(
    mut wheel: MessageReader<bevy::input::mouse::MouseWheel>,
    active: Res<ActiveTab>,
    mut q: Query<(&mut ScrollPosition, &ComputedNode, &Children, &TabContent), With<StepsContainer>>,
    children_q: Query<&ComputedNode>,
) {
    if active.0 == Tab::Modulation {
        wheel.clear();
        return;
    }
    let mut dy = 0.0;
    for ev in wheel.read() {
        match ev.unit {
            bevy::input::mouse::MouseScrollUnit::Line => dy += ev.y * SCROLL_SPEED_PX,
            bevy::input::mouse::MouseScrollUnit::Pixel => dy += ev.y,
        }
    }
    if dy == 0.0 {
        return;
    }
    for (mut sp, viewport, children, tab_content) in &mut q {
        if tab_content.0 != active.0 {
            continue;
        }
        let viewport_h = viewport.size().y;
        let mut content_h = 0.0;
        for child in children.iter() {
            if let Ok(cn) = children_q.get(child) {
                content_h += cn.size().y;
            }
        }
        let max_scroll = (content_h - viewport_h).max(0.0);
        let new_y = (sp.y - dy).clamp(0.0, max_scroll);
        sp.y = new_y;
    }
}

pub fn handle_tab_clicks(
    q: Query<(&Interaction, &TabButton), Changed<Interaction>>,
    mut active: ResMut<ActiveTab>,
) {
    for (i, btn) in &q {
        if *i == Interaction::Pressed {
            active.0 = btn.0;
        }
    }
}

pub fn refresh_tab_visibility(
    active: Res<ActiveTab>,
    mut content_q: Query<(&TabContent, &mut Node)>,
    mut button_q: Query<(&TabButton, &mut BackgroundColor)>,
) {
    if !active.is_changed() {
        return;
    }
    for (tab, mut node) in &mut content_q {
        node.display = if tab.0 == active.0 { Display::Flex } else { Display::None };
    }
    for (btn, mut bg) in &mut button_q {
        bg.0 = if btn.0 == active.0 { ACCENT } else { TAB_BUTTON_BG };
    }
}

pub fn refresh_message_field_visual(
    focus: Res<MessageFieldFocus>,
    mut q: Query<&mut BackgroundColor, With<MessageField>>,
) {
    if !focus.is_changed() {
        return;
    }
    for mut bg in &mut q {
        bg.0 = if focus.0 { FIELD_FOCUS } else { BUTTON_BG };
    }
}

pub fn refresh_crypto_field_visual(
    crypto: Res<CryptoEdit>,
    mut q: Query<(&CryptoField, &mut BackgroundColor)>,
) {
    if !crypto.is_changed() {
        return;
    }
    for (field, mut bg) in &mut q {
        bg.0 = if crypto.focus == field.0 { FIELD_FOCUS } else { BUTTON_BG };
    }
}

#[derive(Component, Clone)]
pub struct Tooltip(pub &'static str);

#[derive(Component)]
pub struct TooltipPanel;

#[derive(Component)]
pub struct TooltipText;

pub fn setup_tooltip(mut commands: Commands) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                padding: UiRect::all(Val::Px(8.0)),
                max_width: Val::Px(280.0),
                display: Display::None,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.92)),
            GlobalZIndex(1000),
            TooltipPanel,
        ))
        .with_children(|p| {
            p.spawn((
                Text::new(""),
                TextFont { font_size: FONT_S, ..default() },
                TextColor(PANEL_FG),
                TooltipText,
            ));
        });
}

pub fn handle_tooltips(
    windows: Query<&Window>,
    hovered: Query<(&Interaction, &Tooltip), With<Button>>,
    mut panel_q: Query<&mut Node, With<TooltipPanel>>,
    mut text_q: Query<&mut Text, With<TooltipText>>,
) {
    let mut active: Option<&'static str> = None;
    for (i, t) in &hovered {
        if *i == Interaction::Hovered || *i == Interaction::Pressed {
            active = Some(t.0);
            break;
        }
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let cursor = window.cursor_position();

    for mut node in &mut panel_q {
        match (active, cursor) {
            (Some(_), Some(pos)) => {
                node.display = Display::Flex;
                node.left = Val::Px(pos.x + 14.0);
                node.top = Val::Px(pos.y + 14.0);
            }
            _ => {
                node.display = Display::None;
            }
        }
    }
    for mut t in &mut text_q {
        let s = active.unwrap_or("");
        if t.0 != s {
            t.0 = s.to_string();
        }
    }
}
