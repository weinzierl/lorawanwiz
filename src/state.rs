//! Bevy resources for the visualizer.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::math::{ChirpWaveform, EncryptStep, LabeledSymbol};

pub const DEFAULT_APP_SKEY: [u8; 16] = [
    0x2B, 0x7E, 0x15, 0x16, 0x28, 0xAE, 0xD2, 0xA6,
    0xAB, 0xF7, 0x15, 0x88, 0x09, 0xCF, 0x4F, 0x3C,
];
pub const DEFAULT_NWK_SKEY: [u8; 16] = [
    0x99, 0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22,
    0x11, 0x00, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF,
];
pub const DEFAULT_DEV_ADDR: [u8; 4] = [0x26, 0x01, 0x1F, 0x88];
pub const DEFAULT_F_CNT: u32 = 1;
pub const DEFAULT_F_PORT: u8 = 1;
pub const DEFAULT_MESSAGE: &str = "hello";
pub const DEFAULT_SF: u8 = 7;
pub const DEFAULT_BW_HZ: f32 = 125_000.0;

pub const AUDIO_SAMPLE_RATE_HZ: u32 = 22_050;
pub const AUDIO_TARGET_TOP_HZ: f32 = 3_000.0;
pub const AUDIO_TARGET_T_SYM_S: f32 = 0.080;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CodingRate {
    FourFive,
    FourSix,
    FourSeven,
    FourEight,
}

impl CodingRate {
    pub fn label(self) -> &'static str {
        match self {
            CodingRate::FourFive => "4/5",
            CodingRate::FourSix => "4/6",
            CodingRate::FourSeven => "4/7",
            CodingRate::FourEight => "4/8",
        }
    }
    pub fn all() -> [CodingRate; 4] {
        [
            CodingRate::FourFive,
            CodingRate::FourSix,
            CodingRate::FourSeven,
            CodingRate::FourEight,
        ]
    }
}

#[derive(Resource, Clone, Debug, Serialize, Deserialize)]
pub struct LorawanInputs {
    pub message: String,
    pub sf: u8,
    pub bw_hz: f32,
    pub coding_rate: CodingRate,
    pub dev_addr: [u8; 4],
    pub f_cnt: u32,
    pub f_port: u8,
    pub app_skey: [u8; 16],
    pub nwk_skey: [u8; 16],
}

impl Default for LorawanInputs {
    fn default() -> Self {
        Self {
            message: DEFAULT_MESSAGE.to_string(),
            sf: DEFAULT_SF,
            bw_hz: DEFAULT_BW_HZ,
            coding_rate: CodingRate::FourFive,
            dev_addr: DEFAULT_DEV_ADDR,
            f_cnt: DEFAULT_F_CNT,
            f_port: DEFAULT_F_PORT,
            app_skey: DEFAULT_APP_SKEY,
            nwk_skey: DEFAULT_NWK_SKEY,
        }
    }
}

impl LorawanInputs {
    pub fn max_app_payload_bytes(&self) -> usize {
        let bw_khz = (self.bw_hz / 1000.0) as u32;
        match (self.sf, bw_khz) {
            (12, 125) | (11, 125) | (10, 125) => 51,
            (9, 125) => 115,
            (8, 125) | (7, 125) => 222,
            (12, 250) | (11, 250) | (10, 250) => 51,
            (9, 250) => 115,
            (8, 250) | (7, 250) => 222,
            (12, 500) | (11, 500) | (10, 500) => 51,
            (9, 500) => 115,
            (8, 500) | (7, 500) => 222,
            _ => 51,
        }
    }
}

#[derive(Resource, Default)]
pub struct InputsDirty(pub bool);

#[derive(Resource, Default, Clone)]
pub struct PipelineOutput {
    pub plaintext: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub encrypt_steps: Vec<EncryptStep>,
    pub mic: [u8; 4],
    pub frame: Vec<u8>,
    pub symbols: Vec<LabeledSymbol>,
    pub chirps: Vec<ChirpWaveform>,
    pub audio_samples: Vec<f32>,
    pub audio_duration_s: f32,
}

#[derive(Resource, Default)]
pub struct ChirpAnimator {
    pub current_index: usize,
    pub playing: bool,
    pub elapsed_s: f32,
}

#[derive(Resource)]
pub struct CanvasView {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for CanvasView {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CryptoFocus {
    None,
    DevAddr,
    AppSKey,
    NwkSKey,
}

#[derive(Resource)]
pub struct CryptoEdit {
    pub focus: CryptoFocus,
    pub dev_addr_text: String,
    pub app_skey_text: String,
    pub nwk_skey_text: String,
}

impl Default for CryptoEdit {
    fn default() -> Self {
        Self {
            focus: CryptoFocus::None,
            dev_addr_text: hex_string(&DEFAULT_DEV_ADDR),
            app_skey_text: hex_string(&DEFAULT_APP_SKEY),
            nwk_skey_text: hex_string(&DEFAULT_NWK_SKEY),
        }
    }
}

pub fn hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02X}", b));
    }
    s
}

pub fn parse_hex(text: &str, n: usize) -> Option<Vec<u8>> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if cleaned.len() != n * 2 {
        return None;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let byte = u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16).ok()?;
        out.push(byte);
    }
    Some(out)
}

#[derive(Resource, Serialize, Deserialize, Clone)]
pub struct AudioSettings {
    pub volume: f32,
    pub muted: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: 0.75,
            muted: false,
        }
    }
}

impl AudioSettings {
    pub fn effective_gain(&self) -> f32 {
        if self.muted {
            0.0
        } else {
            self.volume
        }
    }

    pub fn cycle_volume(&mut self) {
        self.volume = match (self.volume * 100.0).round() as i32 {
            v if v < 38 => 0.50,
            v if v < 63 => 0.75,
            v if v < 88 => 1.00,
            _ => 0.25,
        };
    }

    pub fn volume_label(&self) -> String {
        format!("{}%", (self.volume * 100.0).round() as i32)
    }
}

/// Toggleable view-state for the modulation tab. When on, the chirp
/// canvas shows three rows (signal, reference, product) instead of
/// just the signal. The product row is the pointwise sum (modulo BW)
/// of the signal's instantaneous frequency and the conjugate
/// reference's instantaneous frequency, which is the dechirping
/// operation expressed in the frequency-trace domain. No FFT is run.
#[derive(Resource, Default)]
pub struct DecodeView {
    pub enabled: bool,
}

/// Persistent snapshot of all user-facing settings, written to and read
/// from RON files via the Save / Load buttons.
#[derive(Serialize, Deserialize, Clone)]
pub struct SavedState {
    pub version: u32,
    pub inputs: LorawanInputs,
    pub audio: AudioSettings,
}

impl SavedState {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn from_resources(inputs: &LorawanInputs, audio: &AudioSettings) -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            inputs: inputs.clone(),
            audio: audio.clone(),
        }
    }
}
