//! Step 7 visualization: chirp canvas with mesh polylines, separators,
//! axis labels, per-symbol labels, sound-synced highlight, plus pan/zoom.

use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::css::{DODGER_BLUE, LIME, ORANGE, SLATE_GRAY};
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::ParamSet;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::{MouseButton, MouseScrollUnit, MouseWheel};
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};

use crate::math::{ChirpDirection, SymbolKind};
use crate::state::{CanvasView, ChirpAnimator, DecodeView, LorawanInputs, PipelineOutput};

const CHIRP_WIDTH_PX: f32 = 90.0;
const CANVAS_HEIGHT_PX: f32 = 320.0;
/// Per-row height when decode mode shows three rows. Picked to keep
/// each individual row readable while not making the total canvas
/// dwarf the rest of the modulation tab.
const DECODE_ROW_HEIGHT_PX: f32 = 200.0;
/// Vertical gap between rows in decode mode.
const DECODE_ROW_GAP_PX: f32 = 18.0;
const TOP_PADDING_PX: f32 = 30.0;
const HEADER_HEIGHT_PX: f32 = 130.0;
const TAB_BAR_HEIGHT_PX: f32 = 40.0;
const LINE_THICKNESS_PX: f32 = 3.0;
const SEPARATOR_THICKNESS_PX: f32 = 1.0;
const AXIS_THICKNESS_PX: f32 = 2.0;
const LABEL_FONT_SIZE: f32 = 11.0;
const AXIS_LABEL_FONT_SIZE: f32 = 12.0;

const MIN_ZOOM: f32 = 0.2;
const MAX_ZOOM: f32 = 4.0;
const ZOOM_STEP_LINE: f32 = 0.15;
const ZOOM_STEP_PIXEL: f32 = 0.005;

/// Drag tracking for the chirp canvas. Owned by visualization.rs because
/// it is part of the canvas interaction state.
#[derive(Resource, Default)]
pub struct DragState {
    /// True while a drag is in progress.
    pub active: bool,
    /// Cursor position last frame (window coordinates), used to compute
    /// per-frame deltas without relying on MouseMotion which can be
    /// suppressed by Bevy UI input handling.
    pub last_cursor: Vec2,
}

#[derive(Component)]
pub struct ChirpCanvas;

#[derive(Component)]
pub struct ChirpMesh {
    pub symbol_index: usize,
}

#[derive(Component)]
pub struct ChirpHighlight;

#[derive(Component)]
pub struct ChirpAxis;

#[derive(Component)]
pub struct ChirpSeparator;

#[derive(Component)]
pub struct ChirpLabel;

pub fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.06, 0.07, 0.09)),
            ..default()
        },
    ));
}

pub fn rebuild_chirp_canvas(
    output: Res<PipelineOutput>,
    inputs: Res<LorawanInputs>,
    active: Res<crate::ui::ActiveTab>,
    decode: Res<DecodeView>,
    windows: Query<&Window>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    existing_meshes: Query<Entity, With<ChirpMesh>>,
    existing_axes: Query<Entity, With<ChirpAxis>>,
    existing_separators: Query<Entity, With<ChirpSeparator>>,
    existing_labels: Query<Entity, With<ChirpLabel>>,
    existing_highlights: Query<Entity, With<ChirpHighlight>>,
) {
    if !output.is_changed() && !decode.is_changed() {
        return;
    }

    for e in existing_meshes
        .iter()
        .chain(existing_axes.iter())
        .chain(existing_separators.iter())
        .chain(existing_labels.iter())
        .chain(existing_highlights.iter())
    {
        commands.entity(e).despawn();
    }

    if output.chirps.is_empty() {
        return;
    }

    let initial_visibility = if active.0 == crate::ui::Tab::Modulation {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let _window_h = windows
        .single()
        .map(|w| w.resolution.height())
        .unwrap_or(800.0);

    let total_width = output.chirps.len() as f32 * CHIRP_WIDTH_PX;
    let x0 = -total_width * 0.5;
    let t_sym_ms = (1u32 << inputs.sf) as f32 / inputs.bw_hz * 1000.0;

    // Pre-compute the reference downchirp's frequency trace once, since
    // the same value-0 conjugate (downchirp) is the dechirping reference
    // for every column. We borrow the length and bandwidth from the
    // first chirp so the reference matches the per-symbol sample count
    // exactly for product computation.
    let first_chirp = &output.chirps[0];
    let ref_trace = make_reference_trace(first_chirp.freq_trace.len(), inputs.bw_hz);

    if decode.enabled {
        // Three rows, stacked vertically. Top: signal, middle: reference,
        // bottom: product. Y math computes each row's center.
        let rows_total_h =
            DECODE_ROW_HEIGHT_PX * 3.0 + DECODE_ROW_GAP_PX * 2.0;
        let canvas_center_y = -HEADER_HEIGHT_PX * 0.5;
        let canvas_top_y = canvas_center_y + rows_total_h * 0.5;

        let signal_cy = canvas_top_y - DECODE_ROW_HEIGHT_PX * 0.5;
        let ref_cy = signal_cy - DECODE_ROW_HEIGHT_PX - DECODE_ROW_GAP_PX;
        let prod_cy = ref_cy - DECODE_ROW_HEIGHT_PX - DECODE_ROW_GAP_PX;

        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            inputs.bw_hz,
            x0,
            signal_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Signal,
            &ref_trace,
            initial_visibility,
        );
        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            inputs.bw_hz,
            x0,
            ref_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Reference,
            &ref_trace,
            initial_visibility,
        );
        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            inputs.bw_hz,
            x0,
            prod_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Product,
            &ref_trace,
            initial_visibility,
        );

        // Per-symbol header labels (kind letter + index + raw value) go
        // above the top (signal) row only.
        spawn_chirp_header_labels(
            &mut commands,
            &output,
            x0,
            signal_cy + DECODE_ROW_HEIGHT_PX * 0.5 + 12.0,
            initial_visibility,
        );

        // X-axis time annotation under the bottom (product) row.
        spawn_text_label(
            &mut commands,
            format!(
                "time ({} symbols, T_sym = {:.3} ms)",
                output.chirps.len(),
                t_sym_ms
            ),
            Vec2::new(
                x0 + total_width * 0.5,
                prod_cy - DECODE_ROW_HEIGHT_PX * 0.5 - 22.0,
            ),
            AXIS_LABEL_FONT_SIZE,
            Color::srgb(0.40, 0.75, 1.00),
            initial_visibility,
            ChirpAxis,
        );

        // Highlight bar spans all three rows.
        let highlight_h = rows_total_h;
        let highlight_cy =
            (signal_cy + prod_cy) * 0.5; // midpoint between top and bottom centers
        let highlight = build_rect_mesh(CHIRP_WIDTH_PX, highlight_h);
        commands.spawn((
            Mesh2d(meshes.add(highlight)),
            MeshMaterial2d(materials.add(Color::srgba(1.0, 0.95, 0.55, 0.18))),
            Transform::from_xyz(x0 + CHIRP_WIDTH_PX * 0.5, highlight_cy, 0.5),
            Visibility::Hidden,
            ChirpHighlight,
        ));
    } else {
        // Single-row layout (original behavior). Centered the same way
        // as before to avoid any visible jump when toggling.
        let canvas_center_y = -HEADER_HEIGHT_PX * 0.5;
        let cy = canvas_center_y;

        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            inputs.bw_hz,
            x0,
            cy,
            CANVAS_HEIGHT_PX,
            RowKind::Signal,
            &ref_trace,
            initial_visibility,
        );

        spawn_chirp_header_labels(
            &mut commands,
            &output,
            x0,
            cy + CANVAS_HEIGHT_PX * 0.5 + 12.0,
            initial_visibility,
        );

        spawn_text_label(
            &mut commands,
            format!(
                "time ({} symbols, T_sym = {:.3} ms)",
                output.chirps.len(),
                t_sym_ms
            ),
            Vec2::new(x0 + total_width * 0.5, cy - CANVAS_HEIGHT_PX * 0.5 - 22.0),
            AXIS_LABEL_FONT_SIZE,
            Color::srgb(0.40, 0.75, 1.00),
            initial_visibility,
            ChirpAxis,
        );

        let highlight = build_rect_mesh(CHIRP_WIDTH_PX, CANVAS_HEIGHT_PX);
        commands.spawn((
            Mesh2d(meshes.add(highlight)),
            MeshMaterial2d(materials.add(Color::srgba(1.0, 0.95, 0.55, 0.22))),
            Transform::from_xyz(x0 + CHIRP_WIDTH_PX * 0.5, cy, 0.5),
            Visibility::Hidden,
            ChirpHighlight,
        ));
    }
}

/// Which of the three decode-mode rows we're drawing. Affects the
/// frequency curve, the row label, and the line color.
#[derive(Copy, Clone)]
enum RowKind {
    Signal,
    Reference,
    Product,
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    output: &PipelineOutput,
    bw_hz: f32,
    x0: f32,
    cy: f32,
    row_height: f32,
    row_kind: RowKind,
    ref_trace: &[f32],
    initial_visibility: Visibility,
) {
    let total_width = output.chirps.len() as f32 * CHIRP_WIDTH_PX;
    let y_top = cy + row_height * 0.5;
    let y_bot = cy - row_height * 0.5;

    // X-axis baseline for this row.
    let axis_mesh = build_rect_mesh(total_width, AXIS_THICKNESS_PX);
    commands.spawn((
        Mesh2d(meshes.add(axis_mesh)),
        MeshMaterial2d(materials.add(Color::from(SLATE_GRAY))),
        Transform::from_xyz(x0 + total_width * 0.5, y_bot, 0.0),
        initial_visibility,
        ChirpAxis,
    ));
    // Y-axis vertical line for this row.
    let y_axis_mesh = build_rect_mesh(AXIS_THICKNESS_PX, row_height);
    commands.spawn((
        Mesh2d(meshes.add(y_axis_mesh)),
        MeshMaterial2d(materials.add(Color::from(SLATE_GRAY))),
        Transform::from_xyz(x0, cy, 0.0),
        initial_visibility,
        ChirpAxis,
    ));

    // Y-axis labels (BW at top, 0 Hz at bottom) and the row name on
    // the far left.
    let bw_khz = bw_hz / 1000.0;
    spawn_text_label(
        commands,
        format!("{:.0} kHz", bw_khz),
        Vec2::new(x0 - 36.0, y_top - 8.0),
        AXIS_LABEL_FONT_SIZE,
        Color::srgb(0.55, 0.58, 0.65),
        initial_visibility,
        ChirpAxis,
    );
    spawn_text_label(
        commands,
        "0 Hz".to_string(),
        Vec2::new(x0 - 30.0, y_bot + 8.0),
        AXIS_LABEL_FONT_SIZE,
        Color::srgb(0.55, 0.58, 0.65),
        initial_visibility,
        ChirpAxis,
    );
    let row_label = match row_kind {
        RowKind::Signal => "signal",
        RowKind::Reference => "reference",
        RowKind::Product => "product",
    };
    spawn_text_label(
        commands,
        row_label.to_string(),
        Vec2::new(x0 - 32.0, cy),
        AXIS_LABEL_FONT_SIZE,
        Color::srgb(0.40, 0.75, 1.00),
        initial_visibility,
        ChirpAxis,
    );

    // Per-chirp meshes and separators for this row.
    for (i, chirp) in output.chirps.iter().enumerate() {
        let sym = &output.symbols[i];
        let color = match row_kind {
            RowKind::Signal => color_for(sym.kind, chirp.direction),
            RowKind::Reference => Color::srgb(0.55, 0.58, 0.65),
            RowKind::Product => Color::srgb(0.95, 0.78, 0.45),
        };

        let cx = x0 + i as f32 * CHIRP_WIDTH_PX + CHIRP_WIDTH_PX * 0.5;

        // Build the frequency trace this row needs from the per-symbol
        // signal trace (which already exists) and the shared reference
        // trace. Only the signal row uses the chirp's own trace
        // directly; the other two are derived.
        let trace: Vec<f32> = match row_kind {
            RowKind::Signal => chirp.freq_trace.clone(),
            RowKind::Reference => ref_trace.to_vec(),
            RowKind::Product => product_trace(&chirp.freq_trace, ref_trace, bw_hz),
        };

        let mesh = build_chirp_mesh(
            &trace,
            bw_hz,
            CHIRP_WIDTH_PX * 0.92,
            row_height - TOP_PADDING_PX,
        );

        commands.spawn((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(cx, cy, 1.0),
            initial_visibility,
            ChirpMesh { symbol_index: i },
        ));

        if i + 1 < output.chirps.len() {
            let sep_x = x0 + (i + 1) as f32 * CHIRP_WIDTH_PX;
            let sep_mesh = build_rect_mesh(SEPARATOR_THICKNESS_PX, row_height);
            commands.spawn((
                Mesh2d(meshes.add(sep_mesh)),
                MeshMaterial2d(materials.add(Color::srgba(1.0, 1.0, 1.0, 0.10))),
                Transform::from_xyz(sep_x, cy, 0.2),
                initial_visibility,
                ChirpSeparator,
            ));
        }
    }
}

/// Per-symbol header labels (kind letter + index + raw value) above
/// a row. Drawn as `ChirpLabel` so they hide/show with the canvas.
fn spawn_chirp_header_labels(
    commands: &mut Commands,
    output: &PipelineOutput,
    x0: f32,
    label_y: f32,
    initial_visibility: Visibility,
) {
    for (i, chirp) in output.chirps.iter().enumerate() {
        let sym = &output.symbols[i];
        let color = color_for(sym.kind, chirp.direction);
        let cx = x0 + i as f32 * CHIRP_WIDTH_PX + CHIRP_WIDTH_PX * 0.5;
        let kind_letter = match sym.kind {
            SymbolKind::Preamble => "P",
            SymbolKind::Sync => "S",
            SymbolKind::Header => "H",
            SymbolKind::Payload => "D",
        };
        let label = format!("{}{}\n0x{:X}", kind_letter, i, sym.raw);
        spawn_text_label(
            commands,
            label,
            Vec2::new(cx, label_y),
            LABEL_FONT_SIZE,
            color,
            initial_visibility,
            ChirpLabel,
        );
    }
}

/// Frequency trace of the conjugate of the basic value-0 upchirp,
/// which is the dechirping reference. Linear sweep from BW down to 0
/// over `n` samples. Same shape as a downchirp's `freq_trace`.
fn make_reference_trace(n: usize, bw_hz: f32) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(n);
    let denom = (n.saturating_sub(1)) as f32;
    for i in 0..n {
        let t = if denom == 0.0 { 0.0 } else { i as f32 / denom };
        let f = bw_hz * (1.0 - t);
        out.push(f.clamp(0.0, bw_hz));
    }
    out
}

/// Pointwise sum of the signal and reference frequency traces, modulo
/// BW. This represents the time-domain product (multiplication of two
/// complex chirps adds their instantaneous frequencies). For a value-v
/// upchirp signal against the value-0 downchirp reference, the result
/// is a flat horizontal line at v · (BW / 2^SF). For a downchirp
/// signal it produces a wrapped double-rate downchirp instead.
fn product_trace(signal: &[f32], reference: &[f32], bw_hz: f32) -> Vec<f32> {
    let n = signal.len().min(reference.len());
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let f = (signal[i] + reference[i]).rem_euclid(bw_hz);
        out.push(f);
    }
    out
}

fn spawn_text_label(
    commands: &mut Commands,
    text: String,
    pos: Vec2,
    size: f32,
    color: Color,
    visibility: Visibility,
    marker: impl Component,
) {
    commands.spawn((
        Text2d::new(text),
        TextFont {
            font_size: size,
            ..default()
        },
        TextColor(color),
        Transform::from_xyz(pos.x, pos.y, 0.4),
        visibility,
        marker,
    ));
}

fn color_for(kind: SymbolKind, dir: ChirpDirection) -> Color {
    if dir == ChirpDirection::Down {
        return Color::from(SLATE_GRAY);
    }
    match kind {
        SymbolKind::Preamble => Color::from(SLATE_GRAY),
        SymbolKind::Sync => Color::from(ORANGE),
        SymbolKind::Header => Color::from(LIME),
        SymbolKind::Payload => Color::from(DODGER_BLUE),
    }
}

fn build_chirp_mesh(freq_trace: &[f32], bw_hz: f32, width: f32, height: f32) -> Mesh {
    let n = freq_trace.len();
    let mut positions: Vec<[f32; 3]> = Vec::with_capacity(n * 2);
    let mut indices: Vec<u32> = Vec::with_capacity((n.saturating_sub(1)) * 6);

    if n < 2 {
        return Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_indices(Indices::U32(indices));
    }

    let half = LINE_THICKNESS_PX * 0.5;

    let to_xy = |i: usize| -> (f32, f32) {
        let t = i as f32 / (n - 1) as f32;
        let x = -width * 0.5 + t * width;
        let y = -height * 0.5 + (freq_trace[i] / bw_hz).clamp(0.0, 1.0) * height;
        (x, y)
    };

    for i in 0..n {
        let (x, y) = to_xy(i);

        let (xp, yp) = if i == 0 { to_xy(0) } else { to_xy(i - 1) };
        let (xn, yn) = if i == n - 1 { to_xy(n - 1) } else { to_xy(i + 1) };
        let mut tx = xn - xp;
        let mut ty = yn - yp;
        let len = (tx * tx + ty * ty).sqrt().max(1e-6);
        tx /= len;
        ty /= len;
        let nx = -ty;
        let ny = tx;

        positions.push([x + nx * half, y + ny * half, 0.0]);
        positions.push([x - nx * half, y - ny * half, 0.0]);
    }

    for i in 0..(n - 1) {
        let a = (i * 2) as u32;
        let b = a + 1;
        let c = a + 2;
        let d = a + 3;
        let dy = (freq_trace[i + 1] - freq_trace[i]).abs();
        if dy > bw_hz * 0.5 {
            continue;
        }
        indices.extend_from_slice(&[a, b, c, b, d, c]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

fn build_rect_mesh(width: f32, height: f32) -> Mesh {
    let w = width * 0.5;
    let h = height * 0.5;
    let positions = vec![
        [-w, -h, 0.0],
        [w, -h, 0.0],
        [w, h, 0.0],
        [-w, h, 0.0],
    ];
    let indices = vec![0u32, 1, 2, 0, 2, 3];
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

pub fn animate_chirps(
    animator: Res<ChirpAnimator>,
    output: Res<PipelineOutput>,
    active: Res<crate::ui::ActiveTab>,
    mut highlight_q: Query<&mut Transform, With<ChirpHighlight>>,
) {
    let Ok(mut transform) = highlight_q.single_mut() else {
        return;
    };
    if !animator.playing
        || active.0 != crate::ui::Tab::Modulation
        || output.chirps.is_empty()
    {
        return;
    }
    let total_width = output.chirps.len() as f32 * CHIRP_WIDTH_PX;
    let x0 = -total_width * 0.5;
    let cx = x0 + animator.current_index as f32 * CHIRP_WIDTH_PX + CHIRP_WIDTH_PX * 0.5;
    transform.translation.x = cx;
}

pub fn refresh_canvas_visibility(
    active: Res<crate::ui::ActiveTab>,
    animator: Res<ChirpAnimator>,
    mut params: ParamSet<(
        Query<
            &mut Visibility,
            Or<(
                With<ChirpMesh>,
                With<ChirpAxis>,
                With<ChirpSeparator>,
                With<ChirpLabel>,
            )>,
        >,
        Query<&mut Visibility, With<ChirpHighlight>>,
    )>,
) {
    let on_modulation = active.0 == crate::ui::Tab::Modulation;
    let decor_target = if on_modulation {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in params.p0().iter_mut() {
        if *v != decor_target {
            *v = decor_target;
        }
    }

    let highlight_target = if on_modulation && animator.playing {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in params.p1().iter_mut() {
        if *v != highlight_target {
            *v = highlight_target;
        }
    }
}

// ---------------------------------------------------------------------------
// Pan / zoom
// ---------------------------------------------------------------------------

/// Bindings:
///   * Click and drag (left or right mouse): pan
///   * Mouse wheel: zoom
///   * Two-finger trackpad swipe: zoom (treated as wheel)
///   * Pinch / Ctrl+wheel / Cmd+wheel: zoom
///
/// We track drag state ourselves rather than polling MouseButton::pressed
/// because the latter goes wrong on the web: if the user releases the
/// button outside the canvas, the mouseup never reaches the WASM app and
/// the button appears stuck pressed.
///
/// We compute pan deltas from window cursor positions (not MouseMotion)
/// because Bevy UI input can suppress motion events that fall over UI
/// nodes.
pub fn handle_canvas_input(
    active: Res<crate::ui::ActiveTab>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Query<&Window>,
    mut wheel: MessageReader<MouseWheel>,
    mut drag: ResMut<DragState>,
    mut view: ResMut<CanvasView>,
) {
    if active.0 != crate::ui::Tab::Modulation && active.0 != crate::ui::Tab::TimeDomain {
        wheel.clear();
        if drag.active {
            drag.active = false;
        }
        return;
    }

    let Ok(window) = windows.single() else {
        wheel.clear();
        return;
    };
    let cursor = window.cursor_position();

    // Drag state machine.
    let any_button_held = buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right);
    let any_button_just_pressed =
        buttons.just_pressed(MouseButton::Left) || buttons.just_pressed(MouseButton::Right);
    let any_button_just_released =
        buttons.just_released(MouseButton::Left) || buttons.just_released(MouseButton::Right);

    // Start a drag: just-pressed AND cursor is inside the canvas area
    // (below header + tab bar).
    if any_button_just_pressed {
        if let Some(pos) = cursor {
            let canvas_top = HEADER_HEIGHT_PX + TAB_BAR_HEIGHT_PX;
            if pos.y > canvas_top {
                drag.active = true;
                drag.last_cursor = pos;
            }
        }
    }

    // End a drag: explicit release, OR the cursor left the window
    // (cursor == None), OR the button is no longer held.
    if any_button_just_released || cursor.is_none() || !any_button_held {
        if drag.active {
            drag.active = false;
        }
    }

    // Compute pan delta from cursor motion while dragging.
    if drag.active {
        if let Some(pos) = cursor {
            let dx = pos.x - drag.last_cursor.x;
            let dy = pos.y - drag.last_cursor.y;
            // Window Y is down-positive; world Y is up-positive.
            view.pan.x -= dx * view.zoom;
            view.pan.y += dy * view.zoom;
            drag.last_cursor = pos;
        }
    }

    // Zoom from wheel events. All wheel input zooms; pan is exclusively
    // drag-based. Ctrl/Cmd is irrelevant to behavior but kept for the
    // "pinch" mental model.
    let _modifier_held = keys.pressed(KeyCode::ControlLeft)
        || keys.pressed(KeyCode::ControlRight)
        || keys.pressed(KeyCode::SuperLeft)
        || keys.pressed(KeyCode::SuperRight);

    let mut zoom_delta = 0.0f32;
    for ev in wheel.read() {
        match ev.unit {
            MouseScrollUnit::Line => zoom_delta += ev.y * ZOOM_STEP_LINE,
            MouseScrollUnit::Pixel => zoom_delta += ev.y * ZOOM_STEP_PIXEL,
        }
    }
    if zoom_delta.abs() > 0.0 {
        let factor = (-zoom_delta).exp();
        view.zoom = (view.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    }
}

pub fn apply_canvas_view(
    view: Res<CanvasView>,
    mut camera_q: Query<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    if !view.is_changed() {
        return;
    }
    let Ok((mut transform, mut projection)) = camera_q.single_mut() else {
        return;
    };
    transform.translation.x = view.pan.x;
    transform.translation.y = view.pan.y;
    if let Projection::Orthographic(ortho) = projection.as_mut() {
        ortho.scale = view.zoom;
    }
}

pub fn reset_canvas_view_on_tab_change(
    active: Res<crate::ui::ActiveTab>,
    mut view: ResMut<CanvasView>,
    mut drag: ResMut<DragState>,
) {
    if !active.is_changed() {
        return;
    }
    if active.0 != crate::ui::Tab::Modulation && active.0 != crate::ui::Tab::TimeDomain {
        view.pan = Vec2::ZERO;
        view.zoom = 1.0;
        drag.active = false;
    }
}
