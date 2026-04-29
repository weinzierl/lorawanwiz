//! Audio playback for the chirp sequence.
//!
//! Native: cpal output stream, kept alive on AudioState.
//! WASM:   web-sys AudioContext + GainNode + BufferSource, kept alive on
//! AudioState so we can support live volume changes (GainNode) and Stop
//! (calling source.stop()).

use bevy::prelude::*;

use crate::state::{AUDIO_TARGET_T_SYM_S, AudioSettings, ChirpAnimator, PipelineOutput};
#[cfg(target_arch = "wasm32")]
use crate::state::AUDIO_SAMPLE_RATE_HZ;
use crate::ui::{PlayAudioButton, StopAudioButton};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, atomic::{AtomicU32, Ordering}};

#[derive(Resource, Default)]
pub struct AudioState {
    #[cfg(target_arch = "wasm32")]
    ctx: Option<web_sys::AudioContext>,
    #[cfg(target_arch = "wasm32")]
    current_gain: Option<web_sys::GainNode>,
    #[cfg(target_arch = "wasm32")]
    current_source: Option<web_sys::AudioBufferSourceNode>,

    #[cfg(not(target_arch = "wasm32"))]
    stream: Option<NativeStream>,
    #[cfg(not(target_arch = "wasm32"))]
    gain: Option<Arc<AtomicU32>>,
}

pub fn handle_play_button(
    q: Query<&Interaction, (Changed<Interaction>, With<PlayAudioButton>)>,
    output: Res<PipelineOutput>,
    settings: Res<AudioSettings>,
    mut audio: ResMut<AudioState>,
    mut animator: ResMut<ChirpAnimator>,
) {
    for i in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        if animator.playing {
            continue;
        }
        let gain = settings.effective_gain();
        if play(&mut audio, &output.audio_samples, gain) {
            animator.playing = true;
            animator.elapsed_s = 0.0;
            animator.current_index = 0;
        }
    }
}

pub fn handle_stop_button(
    q: Query<&Interaction, (Changed<Interaction>, With<StopAudioButton>)>,
    mut audio: ResMut<AudioState>,
    mut animator: ResMut<ChirpAnimator>,
) {
    for i in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        stop(&mut audio);
        animator.playing = false;
        animator.elapsed_s = 0.0;
        animator.current_index = 0;
    }
}

pub fn tick_animator(
    time: Res<Time>,
    output: Res<PipelineOutput>,
    mut animator: ResMut<ChirpAnimator>,
) {
    if !animator.playing {
        return;
    }
    if output.audio_duration_s <= 0.0 || output.chirps.is_empty() {
        animator.playing = false;
        animator.elapsed_s = 0.0;
        animator.current_index = 0;
        return;
    }
    animator.elapsed_s += time.delta_secs();
    if animator.elapsed_s >= output.audio_duration_s {
        animator.playing = false;
        animator.elapsed_s = 0.0;
        animator.current_index = 0;
        return;
    }
    let idx = (animator.elapsed_s / AUDIO_TARGET_T_SYM_S) as usize;
    animator.current_index = idx.min(output.chirps.len().saturating_sub(1));
}

pub fn apply_audio_settings(settings: Res<AudioSettings>, audio: ResMut<AudioState>) {
    if !settings.is_changed() {
        return;
    }
    let gain = settings.effective_gain();
    apply_gain(audio, gain);
}

#[cfg(target_arch = "wasm32")]
fn apply_gain(audio: ResMut<AudioState>, gain: f32) {
    if let Some(g) = audio.current_gain.as_ref() {
        g.gain().set_value(gain.clamp(0.0, 1.0));
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn apply_gain(audio: ResMut<AudioState>, gain: f32) {
    if let Some(g) = audio.gain.as_ref() {
        g.store(gain.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
}

#[cfg(target_arch = "wasm32")]
fn play(audio: &mut AudioState, samples: &[f32], gain: f32) -> bool {
    if samples.is_empty() {
        return false;
    }
    if audio.ctx.is_none() {
        match web_sys::AudioContext::new() {
            Ok(ctx) => audio.ctx = Some(ctx),
            Err(e) => {
                web_sys::console::warn_1(&format!("AudioContext::new failed: {:?}", e).into());
                return false;
            }
        }
    }
    let ctx = audio.ctx.as_ref().unwrap();

    let buffer = match ctx.create_buffer(1, samples.len() as u32, AUDIO_SAMPLE_RATE_HZ as f32) {
        Ok(b) => b,
        Err(e) => {
            web_sys::console::warn_1(&format!("create_buffer failed: {:?}", e).into());
            return false;
        }
    };

    let mut owned = samples.to_vec();
    if let Err(e) = buffer.copy_to_channel(&mut owned, 0) {
        web_sys::console::warn_1(&format!("copy_to_channel failed: {:?}", e).into());
        return false;
    }

    let source = match ctx.create_buffer_source() {
        Ok(s) => s,
        Err(e) => {
            web_sys::console::warn_1(&format!("create_buffer_source failed: {:?}", e).into());
            return false;
        }
    };
    source.set_buffer(Some(&buffer));

    let gain_node = match ctx.create_gain() {
        Ok(g) => g,
        Err(e) => {
            web_sys::console::warn_1(&format!("create_gain failed: {:?}", e).into());
            return false;
        }
    };
    gain_node.gain().set_value(gain.clamp(0.0, 1.0));

    let source_node: &web_sys::AudioNode = source.as_ref();
    let gain_as_node: &web_sys::AudioNode = gain_node.as_ref();
    if let Err(e) = source_node.connect_with_audio_node(gain_as_node) {
        web_sys::console::warn_1(&format!("source.connect failed: {:?}", e).into());
        return false;
    }

    let dest = ctx.destination();
    let dest_node: &web_sys::AudioNode = dest.as_ref();
    if let Err(e) = gain_as_node.connect_with_audio_node(dest_node) {
        web_sys::console::warn_1(&format!("gain.connect failed: {:?}", e).into());
        return false;
    }

    if let Err(e) = source.start() {
        web_sys::console::warn_1(&format!("start failed: {:?}", e).into());
        return false;
    }

    audio.current_gain = Some(gain_node);
    audio.current_source = Some(source);
    true
}

#[cfg(target_arch = "wasm32")]
fn stop(audio: &mut AudioState) {
    if let Some(source) = audio.current_source.take() {
        // `AudioBufferSourceNode::stop()` is flagged deprecated in
        // recent web-sys for Safari-compat reasons; the no-arg form is
        // still functionally correct on every browser we care about,
        // and the parent-class alternatives carry the same flag, so we
        // just suppress the warning here.
        #[allow(deprecated)]
        let _ = source.stop();
    }
    audio.current_gain = None;
}

#[cfg(not(target_arch = "wasm32"))]
pub struct NativeStream {
    _stream: cpal::Stream,
}

#[cfg(not(target_arch = "wasm32"))]
unsafe impl Send for NativeStream {}
#[cfg(not(target_arch = "wasm32"))]
unsafe impl Sync for NativeStream {}

#[cfg(not(target_arch = "wasm32"))]
fn play(audio: &mut AudioState, samples: &[f32], gain: f32) -> bool {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::sync::Mutex;

    if samples.is_empty() {
        return false;
    }

    audio.stream = None;
    audio.gain = None;

    let host = cpal::default_host();
    let Some(device) = host.default_output_device() else {
        eprintln!("audio: no default output device");
        return false;
    };
    let config = match device.default_output_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("audio: default_output_config failed: {:?}", e);
            return false;
        }
    };

    let device_sample_rate = config.sample_rate().0 as f32;
    let device_channels = config.channels() as usize;

    let src_rate = crate::state::AUDIO_SAMPLE_RATE_HZ as f32;
    let ratio = device_sample_rate / src_rate;
    let resampled_len = ((samples.len() as f32) * ratio) as usize;
    let mut resampled = Vec::with_capacity(resampled_len);
    for i in 0..resampled_len {
        let src_idx = (i as f32) / ratio;
        let i0 = src_idx.floor() as usize;
        let i1 = (i0 + 1).min(samples.len().saturating_sub(1));
        let t = src_idx - i0 as f32;
        let s0 = samples.get(i0).copied().unwrap_or(0.0);
        let s1 = samples.get(i1).copied().unwrap_or(0.0);
        resampled.push(s0 * (1.0 - t) + s1 * t);
    }

    let cursor = Arc::new(Mutex::new(0usize));
    let buffer = Arc::new(resampled);
    let gain_atomic = Arc::new(AtomicU32::new(gain.clamp(0.0, 1.0).to_bits()));
    let err_fn = |e| eprintln!("audio: stream error: {:?}", e);

    let stream_result = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let buffer = buffer.clone();
            let cursor = cursor.clone();
            let gain_atomic = gain_atomic.clone();
            device.build_output_stream(
                &config.config(),
                move |out: &mut [f32], _| {
                    let g = f32::from_bits(gain_atomic.load(Ordering::Relaxed));
                    let mut c = cursor.lock().unwrap();
                    for frame in out.chunks_mut(device_channels) {
                        let s = buffer.get(*c).copied().unwrap_or(0.0) * g;
                        for sample in frame.iter_mut() {
                            *sample = s;
                        }
                        *c += 1;
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::I16 => {
            let buffer = buffer.clone();
            let cursor = cursor.clone();
            let gain_atomic = gain_atomic.clone();
            device.build_output_stream(
                &config.config(),
                move |out: &mut [i16], _| {
                    let g = f32::from_bits(gain_atomic.load(Ordering::Relaxed));
                    let mut c = cursor.lock().unwrap();
                    for frame in out.chunks_mut(device_channels) {
                        let s = buffer.get(*c).copied().unwrap_or(0.0) * g;
                        let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                        for sample in frame.iter_mut() {
                            *sample = v;
                        }
                        *c += 1;
                    }
                },
                err_fn,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let buffer = buffer.clone();
            let cursor = cursor.clone();
            let gain_atomic = gain_atomic.clone();
            device.build_output_stream(
                &config.config(),
                move |out: &mut [u16], _| {
                    let g = f32::from_bits(gain_atomic.load(Ordering::Relaxed));
                    let mut c = cursor.lock().unwrap();
                    for frame in out.chunks_mut(device_channels) {
                        let s = buffer.get(*c).copied().unwrap_or(0.0) * g;
                        let v = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16;
                        for sample in frame.iter_mut() {
                            *sample = v;
                        }
                        *c += 1;
                    }
                },
                err_fn,
                None,
            )
        }
        other => {
            eprintln!("audio: unsupported sample format {:?}", other);
            return false;
        }
    };

    let stream = match stream_result {
        Ok(s) => s,
        Err(e) => {
            eprintln!("audio: build_output_stream failed: {:?}", e);
            return false;
        }
    };
    if let Err(e) = stream.play() {
        eprintln!("audio: stream.play failed: {:?}", e);
        return false;
    }
    audio.stream = Some(NativeStream { _stream: stream });
    audio.gain = Some(gain_atomic);
    true
}

#[cfg(not(target_arch = "wasm32"))]
fn stop(audio: &mut AudioState) {
    audio.stream = None;
    audio.gain = None;
}
