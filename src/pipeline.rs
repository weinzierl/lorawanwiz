//! Bridge between the math module and Bevy resources.

use bevy::prelude::*;

use crate::math::{self, ChirpDirection, ChirpWaveform, FCTRL_NONE, MHDR_UNCONFIRMED_UP};
use crate::state::{
    AUDIO_SAMPLE_RATE_HZ, AUDIO_TARGET_TOP_HZ, AUDIO_TARGET_T_SYM_S, ChirpAnimator,
    InputsDirty, LorawanInputs, PipelineOutput,
};

pub fn run_pipeline(
    inputs: Res<LorawanInputs>,
    mut dirty: ResMut<InputsDirty>,
    mut output: ResMut<PipelineOutput>,
    mut animator: ResMut<ChirpAnimator>,
) {
    if !inputs.is_changed() && !dirty.0 {
        return;
    }
    dirty.0 = false;

    let plaintext = math::message_to_bytes(&inputs.message);

    let (ciphertext, encrypt_steps) = math::encrypt_payload_with_steps(
        &plaintext,
        &inputs.app_skey,
        inputs.f_cnt,
        &inputs.dev_addr,
    );

    let mic = math::compute_mic(
        MHDR_UNCONFIRMED_UP,
        &inputs.dev_addr,
        FCTRL_NONE,
        inputs.f_cnt,
        inputs.f_port,
        &ciphertext,
        &inputs.nwk_skey,
    );

    let frame = math::build_lorawan_frame(
        MHDR_UNCONFIRMED_UP,
        &inputs.dev_addr,
        FCTRL_NONE,
        inputs.f_cnt,
        inputs.f_port,
        &ciphertext,
        &mic,
    );

    let symbols = math::build_symbol_stream(&frame, inputs.sf);

    const VIS_SAMPLE_RATE_HZ: u32 = 8_000;
    let chirps: Vec<ChirpWaveform> = symbols
        .iter()
        .map(|s| match s.direction {
            ChirpDirection::Up => {
                ChirpWaveform::upchirp(s.raw, inputs.sf, inputs.bw_hz, VIS_SAMPLE_RATE_HZ)
            }
            ChirpDirection::Down => {
                ChirpWaveform::downchirp(inputs.sf, inputs.bw_hz, VIS_SAMPLE_RATE_HZ)
            }
        })
        .collect();

    let audio_chirp_samples: Vec<Vec<f32>> = symbols
        .iter()
        .map(|s| {
            math::generate_audio_chirp_samples(
                s.raw,
                inputs.sf,
                AUDIO_TARGET_TOP_HZ,
                AUDIO_TARGET_T_SYM_S,
                AUDIO_SAMPLE_RATE_HZ,
                s.direction,
            )
        })
        .collect();

    let mut audio = Vec::with_capacity(audio_chirp_samples.iter().map(|c| c.len()).sum());
    let fade_len = 32;
    for chirp_samples in &audio_chirp_samples {
        let n = chirp_samples.len();
        for (i, &s) in chirp_samples.iter().enumerate() {
            let env = if i < fade_len {
                i as f32 / fade_len as f32
            } else if i + fade_len >= n {
                (n - i) as f32 / fade_len as f32
            } else {
                1.0
            };
            audio.push(s * env * 0.4);
        }
    }

    let audio_duration_s = AUDIO_TARGET_T_SYM_S * symbols.len() as f32;

    *output = PipelineOutput {
        plaintext,
        ciphertext,
        encrypt_steps,
        mic,
        frame,
        symbols,
        chirps,
        audio_samples: audio,
        audio_duration_s,
    };

    animator.current_index = 0;
    animator.playing = false;
}
