//! Waveforms tab: real-amplitude time-domain plots, parallel to the
//! Modulation tab's frequency-trace plots.
//!
//! Why this exists alongside `visualization.rs`:
//!
//!   * The Modulation tab plots instantaneous frequency over time and
//!     uses a tidy shortcut for the dechirping product (frequency
//!     addition modulo BW). Pedagogically clean, mathematically a
//!     coincidence of representation.
//!   * The Waveforms tab plots actual real-valued amplitude samples
//!     over time. The product row is a literal sample-by-sample
//!     multiplication of the signal samples and the reference samples.
//!     The audience sees the sum-and-difference identity in action: a
//!     fast carrier modulated by a slow beat envelope at the
//!     difference frequency, which encodes the symbol value.
//!   * When Decode is on, a fourth row shows the FFT magnitude of the
//!     product, sliced to the bins covering 0..audio_top_hz so the
//!     dechirping peak is visible in a meaningful position. Header and
//!     payload columns get a small "→ 0xNN" annotation showing the
//!     symbol value recovered from the peak bin, closing the loop with
//!     the input symbol value shown above the signal row.
//!
//! Sample resolution comes from the audio-rescaled chirps (3 kHz top,
//! 80 ms per symbol, 22050 Hz sample rate => 1764 samples per chirp).
//! That's enough to plot the actual sinusoid without aliasing, and it
//! matches what playback sounds like, which is a coherent story. The
//! FFT bin spacing at this rate is 12.5 Hz; at SF7 (128 symbols) the
//! peak resolution is comfortably better than one symbol value per
//! bin, but at SF12 (4096 symbols) multiple symbol values land in the
//! same bin so the recovered annotation is a rough approximation of
//! the true symbol. A real receiver at SF12 uses a much longer
//! integration window to get the necessary resolution.
//!
//! Both this module and `visualization.rs` share the same camera and
//! the same `CanvasView`, so panning is preserved when switching tabs
//! (you stay at the same horizontal symbol position). They occupy the
//! same world-space region and rely on visibility to show only one at
//! a time. This module owns its own mesh/text entities (with their
//! own marker components), its own animation system, and its own
//! visibility-refresh system. It deliberately makes the smallest
//! possible touch to `visualization.rs`: extending the pan/zoom and
//! visibility tab checks to include `Tab::TimeDomain`.

use bevy::asset::RenderAssetUsages;
use bevy::color::palettes::css::{DODGER_BLUE, LIME, ORANGE, SLATE_GRAY};
use bevy::ecs::system::ParamSet;
use bevy::mesh::{Indices, Mesh, PrimitiveTopology};
use bevy::prelude::*;
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

use crate::math::{generate_audio_chirp_samples, ChirpDirection, SymbolKind};
use crate::state::{
    ChirpAnimator, DecodeView, LorawanInputs, PipelineOutput, AUDIO_SAMPLE_RATE_HZ,
    AUDIO_TARGET_T_SYM_S, AUDIO_TARGET_TOP_HZ,
};

const CHIRP_WIDTH_PX: f32 = 90.0;
const SINGLE_ROW_HEIGHT_PX: f32 = 280.0;
/// Per-row height when decode mode shows four rows (signal,
/// reference, product, FFT). Tighter than the old three-row decode
/// height to keep the canvas from overflowing the viewport.
const DECODE_ROW_HEIGHT_PX: f32 = 115.0;
const DECODE_ROW_GAP_PX: f32 = 16.0;
/// Vertical space below the FFT row for per-symbol peak annotations
/// ("→ 0xXX"). Shown only for header/payload symbols.
const FFT_ANNOTATION_GAP_PX: f32 = 16.0;
const TOP_PADDING_PX: f32 = 30.0;
const HEADER_HEIGHT_PX: f32 = 130.0;
const LINE_THICKNESS_PX: f32 = 2.0;
const SEPARATOR_THICKNESS_PX: f32 = 1.0;
const AXIS_THICKNESS_PX: f32 = 2.0;
const LABEL_FONT_SIZE: f32 = 11.0;
const AXIS_LABEL_FONT_SIZE: f32 = 12.0;

#[derive(Component)]
pub struct TimeChirpMesh {
    pub symbol_index: usize,
}

#[derive(Component)]
pub struct TimeChirpHighlight;

#[derive(Component)]
pub struct TimeChirpAxis;

#[derive(Component)]
pub struct TimeChirpSeparator;

#[derive(Component)]
pub struct TimeChirpLabel;

pub fn rebuild_time_canvas(
    output: Res<PipelineOutput>,
    inputs: Res<LorawanInputs>,
    active: Res<crate::ui::ActiveTab>,
    decode: Res<DecodeView>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    existing_meshes: Query<Entity, With<TimeChirpMesh>>,
    existing_axes: Query<Entity, With<TimeChirpAxis>>,
    existing_separators: Query<Entity, With<TimeChirpSeparator>>,
    existing_labels: Query<Entity, With<TimeChirpLabel>>,
    existing_highlights: Query<Entity, With<TimeChirpHighlight>>,
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

    let initial_visibility = if active.0 == crate::ui::Tab::TimeDomain {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };

    let total_width = output.chirps.len() as f32 * CHIRP_WIDTH_PX;
    let x0 = -total_width * 0.5;
    let t_sym_ms = AUDIO_TARGET_T_SYM_S * 1000.0;

    // Reference samples: basic value-0 upchirp at the audible scale.
    // Same for every column. Computed once and reused.
    let reference_samples = generate_audio_chirp_samples(
        0,
        inputs.sf,
        AUDIO_TARGET_TOP_HZ,
        AUDIO_TARGET_T_SYM_S,
        AUDIO_SAMPLE_RATE_HZ,
        ChirpDirection::Up,
    );

    if decode.enabled {
        // Four rows: signal, reference, product, FFT. Plus a small
        // strip below FFT for per-symbol peak annotations.
        let rows_total_h = DECODE_ROW_HEIGHT_PX * 4.0 + DECODE_ROW_GAP_PX * 3.0;
        let canvas_center_y = -HEADER_HEIGHT_PX * 0.5;
        let canvas_top_y = canvas_center_y + rows_total_h * 0.5;

        let signal_cy = canvas_top_y - DECODE_ROW_HEIGHT_PX * 0.5;
        let ref_cy = signal_cy - DECODE_ROW_HEIGHT_PX - DECODE_ROW_GAP_PX;
        let prod_cy = ref_cy - DECODE_ROW_HEIGHT_PX - DECODE_ROW_GAP_PX;
        let fft_cy = prod_cy - DECODE_ROW_HEIGHT_PX - DECODE_ROW_GAP_PX;

        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            &inputs,
            x0,
            signal_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Signal,
            &reference_samples,
            initial_visibility,
        );
        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            &inputs,
            x0,
            ref_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Reference,
            &reference_samples,
            initial_visibility,
        );
        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            &inputs,
            x0,
            prod_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Product,
            &reference_samples,
            initial_visibility,
        );
        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            &inputs,
            x0,
            fft_cy,
            DECODE_ROW_HEIGHT_PX,
            RowKind::Fft,
            &reference_samples,
            initial_visibility,
        );

        spawn_chirp_header_labels(
            &mut commands,
            &output,
            x0,
            signal_cy + DECODE_ROW_HEIGHT_PX * 0.5 + 12.0,
            initial_visibility,
        );

        // Per-symbol peak annotations under the FFT row, shown only
        // for header and payload symbols where the FFT actually
        // resolves a clean peak.
        spawn_fft_peak_annotations(
            &mut commands,
            &output,
            &inputs,
            &reference_samples,
            x0,
            fft_cy - DECODE_ROW_HEIGHT_PX * 0.5 - FFT_ANNOTATION_GAP_PX * 0.5,
            initial_visibility,
        );

        spawn_text_label(
            &mut commands,
            format!(
                "time ({} symbols, audible T_sym = {:.0} ms)",
                output.chirps.len(),
                t_sym_ms
            ),
            Vec2::new(
                x0 + total_width * 0.5,
                fft_cy - DECODE_ROW_HEIGHT_PX * 0.5 - FFT_ANNOTATION_GAP_PX - 22.0,
            ),
            AXIS_LABEL_FONT_SIZE,
            Color::srgb(0.40, 0.75, 1.00),
            initial_visibility,
            TimeChirpAxis,
        );

        let highlight_h = rows_total_h;
        let highlight_cy = (signal_cy + fft_cy) * 0.5;
        let highlight = build_rect_mesh(CHIRP_WIDTH_PX, highlight_h);
        commands.spawn((
            Mesh2d(meshes.add(highlight)),
            MeshMaterial2d(materials.add(Color::srgba(1.0, 0.95, 0.55, 0.18))),
            Transform::from_xyz(x0 + CHIRP_WIDTH_PX * 0.5, highlight_cy, 0.5),
            Visibility::Hidden,
            TimeChirpHighlight,
        ));
    } else {
        let canvas_center_y = -HEADER_HEIGHT_PX * 0.5;
        let cy = canvas_center_y;

        draw_row(
            &mut commands,
            &mut meshes,
            &mut materials,
            &output,
            &inputs,
            x0,
            cy,
            SINGLE_ROW_HEIGHT_PX,
            RowKind::Signal,
            &reference_samples,
            initial_visibility,
        );

        spawn_chirp_header_labels(
            &mut commands,
            &output,
            x0,
            cy + SINGLE_ROW_HEIGHT_PX * 0.5 + 12.0,
            initial_visibility,
        );

        spawn_text_label(
            &mut commands,
            format!(
                "time ({} symbols, audible T_sym = {:.0} ms)",
                output.chirps.len(),
                t_sym_ms
            ),
            Vec2::new(
                x0 + total_width * 0.5,
                cy - SINGLE_ROW_HEIGHT_PX * 0.5 - 22.0,
            ),
            AXIS_LABEL_FONT_SIZE,
            Color::srgb(0.40, 0.75, 1.00),
            initial_visibility,
            TimeChirpAxis,
        );

        let highlight = build_rect_mesh(CHIRP_WIDTH_PX, SINGLE_ROW_HEIGHT_PX);
        commands.spawn((
            Mesh2d(meshes.add(highlight)),
            MeshMaterial2d(materials.add(Color::srgba(1.0, 0.95, 0.55, 0.22))),
            Transform::from_xyz(x0 + CHIRP_WIDTH_PX * 0.5, cy, 0.5),
            Visibility::Hidden,
            TimeChirpHighlight,
        ));
    }
}

#[derive(Copy, Clone)]
enum RowKind {
    Signal,
    Reference,
    Product,
    Fft,
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    output: &PipelineOutput,
    inputs: &LorawanInputs,
    x0: f32,
    cy: f32,
    row_height: f32,
    row_kind: RowKind,
    reference_samples: &[f32],
    initial_visibility: Visibility,
) {
    let total_width = output.chirps.len() as f32 * CHIRP_WIDTH_PX;
    let y_top = cy + row_height * 0.5;
    let y_bot = cy - row_height * 0.5;
    let cy_zero = cy; // amplitude 0 sits at the row's vertical center

    // X-axis baseline. For amplitude rows it's at the row center
    // (zero-crossing). For the FFT row, magnitudes are non-negative
    // so the baseline is at the bottom.
    let baseline_y = match row_kind {
        RowKind::Fft => y_bot,
        _ => cy_zero,
    };
    let axis_mesh = build_rect_mesh(total_width, AXIS_THICKNESS_PX);
    commands.spawn((
        Mesh2d(meshes.add(axis_mesh)),
        MeshMaterial2d(materials.add(Color::from(SLATE_GRAY))),
        Transform::from_xyz(x0 + total_width * 0.5, baseline_y, 0.0),
        initial_visibility,
        TimeChirpAxis,
    ));
    // Y-axis vertical line at the left edge.
    let y_axis_mesh = build_rect_mesh(AXIS_THICKNESS_PX, row_height);
    commands.spawn((
        Mesh2d(meshes.add(y_axis_mesh)),
        MeshMaterial2d(materials.add(Color::from(SLATE_GRAY))),
        Transform::from_xyz(x0, cy, 0.0),
        initial_visibility,
        TimeChirpAxis,
    ));

    // Y-axis tick labels. For amplitude rows: +1 / 0 / -1 (centered).
    // For FFT row: max / 0 (bottom-up, magnitude scale).
    match row_kind {
        RowKind::Fft => {
            spawn_text_label(
                commands,
                "max".to_string(),
                Vec2::new(x0 - 22.0, y_top - 8.0),
                AXIS_LABEL_FONT_SIZE,
                Color::srgb(0.55, 0.58, 0.65),
                initial_visibility,
                TimeChirpAxis,
            );
            spawn_text_label(
                commands,
                "0".to_string(),
                Vec2::new(x0 - 14.0, y_bot + 8.0),
                AXIS_LABEL_FONT_SIZE,
                Color::srgb(0.55, 0.58, 0.65),
                initial_visibility,
                TimeChirpAxis,
            );
        }
        _ => {
            spawn_text_label(
                commands,
                "+1".to_string(),
                Vec2::new(x0 - 22.0, y_top - 8.0),
                AXIS_LABEL_FONT_SIZE,
                Color::srgb(0.55, 0.58, 0.65),
                initial_visibility,
                TimeChirpAxis,
            );
            spawn_text_label(
                commands,
                "0".to_string(),
                Vec2::new(x0 - 14.0, cy_zero),
                AXIS_LABEL_FONT_SIZE,
                Color::srgb(0.55, 0.58, 0.65),
                initial_visibility,
                TimeChirpAxis,
            );
            spawn_text_label(
                commands,
                "-1".to_string(),
                Vec2::new(x0 - 22.0, y_bot + 8.0),
                AXIS_LABEL_FONT_SIZE,
                Color::srgb(0.55, 0.58, 0.65),
                initial_visibility,
                TimeChirpAxis,
            );
        }
    }
    let row_label = match row_kind {
        RowKind::Signal => "signal",
        RowKind::Reference => "reference",
        RowKind::Product => "product",
        RowKind::Fft => "FFT",
    };
    spawn_text_label(
        commands,
        row_label.to_string(),
        Vec2::new(x0 - 36.0, cy),
        AXIS_LABEL_FONT_SIZE,
        Color::srgb(0.40, 0.75, 1.00),
        initial_visibility,
        TimeChirpAxis,
    );

    for (i, chirp) in output.chirps.iter().enumerate() {
        let sym = &output.symbols[i];
        let color = match row_kind {
            RowKind::Signal => color_for(sym.kind, chirp.direction),
            RowKind::Reference => Color::srgb(0.55, 0.58, 0.65),
            RowKind::Product => Color::srgb(0.95, 0.78, 0.45),
            RowKind::Fft => Color::srgb(0.65, 0.85, 0.55),
        };

        let cx = x0 + i as f32 * CHIRP_WIDTH_PX + CHIRP_WIDTH_PX * 0.5;

        let signal_samples = generate_audio_chirp_samples(
            chirp.symbol,
            inputs.sf,
            AUDIO_TARGET_TOP_HZ,
            AUDIO_TARGET_T_SYM_S,
            AUDIO_SAMPLE_RATE_HZ,
            chirp.direction,
        );

        let mesh = match row_kind {
            RowKind::Signal => build_amplitude_mesh(
                &signal_samples,
                CHIRP_WIDTH_PX * 0.92,
                row_height - TOP_PADDING_PX,
            ),
            RowKind::Reference => build_amplitude_mesh(
                reference_samples,
                CHIRP_WIDTH_PX * 0.92,
                row_height - TOP_PADDING_PX,
            ),
            RowKind::Product => {
                let n = signal_samples.len().min(reference_samples.len());
                let product: Vec<f32> = (0..n)
                    .map(|j| signal_samples[j] * reference_samples[j])
                    .collect();
                build_amplitude_mesh(
                    &product,
                    CHIRP_WIDTH_PX * 0.92,
                    row_height - TOP_PADDING_PX,
                )
            }
            RowKind::Fft => {
                let n = signal_samples.len().min(reference_samples.len());
                let product: Vec<f32> = (0..n)
                    .map(|j| signal_samples[j] * reference_samples[j])
                    .collect();
                let mags = compute_fft_magnitudes(&product);
                let max_bin = max_fft_bin_for(product.len());
                let slice_len = (max_bin + 1).min(mags.len());
                let slice = &mags[..slice_len];
                build_unipolar_mesh(
                    slice,
                    CHIRP_WIDTH_PX * 0.92,
                    row_height - TOP_PADDING_PX,
                )
            }
        };

        commands.spawn((
            Mesh2d(meshes.add(mesh)),
            MeshMaterial2d(materials.add(color)),
            Transform::from_xyz(cx, cy, 1.0),
            initial_visibility,
            TimeChirpMesh { symbol_index: i },
        ));

        if i + 1 < output.chirps.len() {
            let sep_x = x0 + (i + 1) as f32 * CHIRP_WIDTH_PX;
            let sep_mesh = build_rect_mesh(SEPARATOR_THICKNESS_PX, row_height);
            commands.spawn((
                Mesh2d(meshes.add(sep_mesh)),
                MeshMaterial2d(materials.add(Color::srgba(1.0, 1.0, 1.0, 0.10))),
                Transform::from_xyz(sep_x, cy, 0.2),
                initial_visibility,
                TimeChirpSeparator,
            ));
        }
    }
}

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
            TimeChirpLabel,
        );
    }
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

/// Build a polyline mesh for an amplitude trace in [-1, 1]. Maps the
/// y-axis to the amplitude range with 0 at the row's vertical center.
fn build_amplitude_mesh(samples: &[f32], width: f32, height: f32) -> Mesh {
    let n = samples.len();
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
        let y = samples[i].clamp(-1.0, 1.0) * (height * 0.5);
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

/// Compute FFT magnitudes of a real-valued time-domain signal. Returns
/// `|X[k]|` for k = 0..N where N is the input length. The first half
/// of bins covers 0..Nyquist; for our purposes only the lowest bins
/// matter (the slow difference frequency).
fn compute_fft_magnitudes(samples: &[f32]) -> Vec<f32> {
    if samples.is_empty() {
        return Vec::new();
    }
    let n = samples.len();
    let mut planner: FftPlanner<f32> = FftPlanner::new();
    let fft = planner.plan_fft_forward(n);
    let mut buffer: Vec<Complex<f32>> = samples
        .iter()
        .map(|&x| Complex { re: x, im: 0.0 })
        .collect();
    fft.process(&mut buffer);
    buffer.iter().map(|c| c.norm()).collect()
}

/// Highest FFT bin index whose center frequency is at or below the
/// audible top frequency. We only plot bins 0..=this bin so the
/// dechirping result fills the row instead of being squeezed into a
/// thin sliver against a mostly empty spectrum.
fn max_fft_bin_for(n_samples: usize) -> usize {
    if n_samples == 0 {
        return 0;
    }
    let bin_spacing = AUDIO_SAMPLE_RATE_HZ as f32 / n_samples as f32;
    (AUDIO_TARGET_TOP_HZ / bin_spacing).round() as usize
}

/// Build a polyline mesh for a non-negative trace mapped to [0, 1]
/// where 0 is at the bottom of the row and the maximum value is at
/// the top. Auto-normalizes by the max so the peak always reaches the
/// row top.
fn build_unipolar_mesh(values: &[f32], width: f32, height: f32) -> Mesh {
    let n = values.len();
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

    let max_v = values.iter().copied().fold(0.0_f32, f32::max).max(1e-9);
    let half = LINE_THICKNESS_PX * 0.5;

    let to_xy = |i: usize| -> (f32, f32) {
        let t = i as f32 / (n - 1) as f32;
        let x = -width * 0.5 + t * width;
        let norm = (values[i] / max_v).clamp(0.0, 1.0);
        // Map [0, 1] to [-height/2, +height/2] (zero at row bottom,
        // peak at row top).
        let y = -height * 0.5 + norm * height;
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
        indices.extend_from_slice(&[a, b, c, b, d, c]);
    }

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_indices(Indices::U32(indices))
}

/// Per-symbol peak-bin annotations under the FFT row. For each header
/// or payload symbol, find the FFT magnitude peak in the relevant bin
/// range, convert it to a recovered symbol value, and label it as
/// `→ 0xNN`. Skipped for preamble and sync symbols where the FFT
/// shows a broad spectrum rather than a clean peak.
fn spawn_fft_peak_annotations(
    commands: &mut Commands,
    output: &PipelineOutput,
    inputs: &LorawanInputs,
    reference_samples: &[f32],
    x0: f32,
    label_y: f32,
    initial_visibility: Visibility,
) {
    for (i, chirp) in output.chirps.iter().enumerate() {
        let sym = &output.symbols[i];
        let show = matches!(sym.kind, SymbolKind::Header | SymbolKind::Payload);
        if !show {
            continue;
        }
        let signal_samples = generate_audio_chirp_samples(
            chirp.symbol,
            inputs.sf,
            AUDIO_TARGET_TOP_HZ,
            AUDIO_TARGET_T_SYM_S,
            AUDIO_SAMPLE_RATE_HZ,
            chirp.direction,
        );
        let n = signal_samples.len().min(reference_samples.len());
        if n == 0 {
            continue;
        }
        let product: Vec<f32> = (0..n)
            .map(|j| signal_samples[j] * reference_samples[j])
            .collect();
        let mags = compute_fft_magnitudes(&product);
        let max_bin = max_fft_bin_for(product.len());
        let slice_len = (max_bin + 1).min(mags.len());
        if slice_len == 0 {
            continue;
        }
        // argmax over the relevant slice
        let (peak_bin, _peak_mag) = mags[..slice_len]
            .iter()
            .enumerate()
            .fold((0usize, 0.0f32), |acc, (k, &v)| {
                if v > acc.1 {
                    (k, v)
                } else {
                    acc
                }
            });
        // Convert the peak bin to a symbol value using the same
        // mapping the upchirp encoding uses: bin / max_bin gives the
        // fraction of the audible band, multiply by 2^SF to recover
        // the symbol value (rounded to nearest integer).
        let recovered = if max_bin == 0 {
            0u16
        } else {
            let n_symbols = 1u32 << inputs.sf as u32;
            let frac = peak_bin as f32 / max_bin as f32;
            (frac * n_symbols as f32).round() as u32 as u16
        };

        let cx = x0 + i as f32 * CHIRP_WIDTH_PX + CHIRP_WIDTH_PX * 0.5;
        let label = format!("\u{2192} 0x{:X}", recovered);
        let color = color_for(sym.kind, chirp.direction);
        spawn_text_label(
            commands,
            label,
            Vec2::new(cx, label_y),
            LABEL_FONT_SIZE,
            color,
            initial_visibility,
            TimeChirpLabel,
        );
    }
}

pub fn animate_time_chirps(
    animator: Res<ChirpAnimator>,
    output: Res<PipelineOutput>,
    active: Res<crate::ui::ActiveTab>,
    mut highlight_q: Query<&mut Transform, With<TimeChirpHighlight>>,
) {
    let Ok(mut transform) = highlight_q.single_mut() else {
        return;
    };
    if !animator.playing
        || active.0 != crate::ui::Tab::TimeDomain
        || output.chirps.is_empty()
    {
        return;
    }
    let total_width = output.chirps.len() as f32 * CHIRP_WIDTH_PX;
    let x0 = -total_width * 0.5;
    let cx = x0 + animator.current_index as f32 * CHIRP_WIDTH_PX + CHIRP_WIDTH_PX * 0.5;
    transform.translation.x = cx;
}

pub fn refresh_time_canvas_visibility(
    active: Res<crate::ui::ActiveTab>,
    animator: Res<ChirpAnimator>,
    mut params: ParamSet<(
        Query<
            &mut Visibility,
            Or<(
                With<TimeChirpMesh>,
                With<TimeChirpAxis>,
                With<TimeChirpSeparator>,
                With<TimeChirpLabel>,
            )>,
        >,
        Query<&mut Visibility, With<TimeChirpHighlight>>,
    )>,
) {
    let on_time = active.0 == crate::ui::Tab::TimeDomain;
    let decor_target = if on_time {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in params.p0().iter_mut() {
        if *v != decor_target {
            *v = decor_target;
        }
    }

    let highlight_target = if on_time && animator.playing {
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
