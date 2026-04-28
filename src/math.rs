//! Pure-Rust LoRaWAN math: payload encryption, MIC, framing, modulation.
//!
//! No Bevy dependencies. Everything returns owned values or primitives so the
//! Bevy layer can copy them into resources without lifetime juggling.
//!
//! References:
//!   LoRaWAN 1.1 Specification (LoRa Alliance, October 2017)
//!     Section 4.3.3.1: FRMPayload encryption (AES-CTR-like)
//!     Section 4.4:     Message Integrity Code (CMAC)
//!   Semtech SX1276 datasheet, chapter 4 (LoRa physical layer)
//!
//! Simplifications vs. real LoRaWAN PHY:
//!   * No whitening, no interleaving, no Hamming FEC. Bytes are packed into
//!     SF-bit symbols by simple bitstream packing. A real receiver could not
//!     decode the resulting waveform, but the chirp structure is correct and
//!     that is what we want to show on stage.
//!   * No CRC.
//!   * The "preamble" in our visualization uses 8 symbols of value 0 marked
//!     as downchirps. Real LoRa preamble is 8 upchirps of value 0 followed
//!     by 2 sync symbols and 2.25 downchirps as the SFD. We follow the
//!     project spec wording but document the discrepancy here.

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use cmac::{Cmac, Mac};

// ----------------------------------------------------------------------------
// LoRaWAN protocol constants
// ----------------------------------------------------------------------------

/// MHDR for an unconfirmed data uplink: MType=010, RFU=000, Major=00.
pub const MHDR_UNCONFIRMED_UP: u8 = 0x40;

/// FCtrl with no ADR, no ACK, no FOpts, no FPending.
pub const FCTRL_NONE: u8 = 0x00;

/// LoRaWAN public-network sync symbols, transmitted after the preamble.
pub const SYNC_SYMBOL_1: u16 = 0x14;
pub const SYNC_SYMBOL_2: u16 = 0x24;

/// Number of preamble symbols. LoRaWAN class A typically uses 8.
pub const PREAMBLE_LEN: usize = 8;

/// AES block size in bytes.
pub const AES_BLOCK_SIZE: usize = 16;

/// Direction byte in the AES-CTR Ai block: 0 = uplink, 1 = downlink.
pub const DIR_UPLINK: u8 = 0x00;

// ----------------------------------------------------------------------------
// Step 2: text to bytes
// ----------------------------------------------------------------------------

/// UTF-8 encode a message and return owned bytes.
pub fn message_to_bytes(msg: &str) -> Vec<u8> {
    msg.as_bytes().to_vec()
}

// ----------------------------------------------------------------------------
// Step 3: FRMPayload encryption (LoRaWAN 1.1 section 4.3.3.1)
// ----------------------------------------------------------------------------

/// One block of the keystream computation, exposed for visualization.
#[derive(Clone, Debug)]
pub struct EncryptStep {
    /// 1-based block index i.
    pub block_index: u8,
    /// The Ai block before encryption.
    pub a_block: [u8; AES_BLOCK_SIZE],
    /// Si = AES(AppSKey, Ai), the keystream block.
    pub s_block: [u8; AES_BLOCK_SIZE],
    /// Plaintext slice covered by this block (1..=16 bytes).
    pub plaintext: Vec<u8>,
    /// Ciphertext = plaintext XOR Si (same length as plaintext).
    pub ciphertext: Vec<u8>,
}

/// Encrypt FRMPayload with AppSKey using LoRaWAN's AES-CTR-like construction.
///
/// For each block index i (1-based) the Ai block is built as:
///   [ 0x01, 0x00, 0x00, 0x00, 0x00, Dir,
///     DevAddr (LE 4), FCnt (LE 32), 0x00, i ]
///
/// The keystream Si = AES_ECB_encrypt(AppSKey, Ai). The ciphertext is the
/// XOR of the plaintext with the concatenation of Si blocks, truncated to
/// the plaintext length. CTR mode is its own inverse, so calling this on
/// ciphertext yields the plaintext again.
pub fn encrypt_payload(
    payload: &[u8],
    app_skey: &[u8; 16],
    f_cnt: u32,
    dev_addr: &[u8; 4],
) -> Vec<u8> {
    encrypt_payload_with_steps(payload, app_skey, f_cnt, dev_addr).0
}

/// Like [`encrypt_payload`] but also returns per-block intermediate values
/// for the visualization layer to render.
pub fn encrypt_payload_with_steps(
    payload: &[u8],
    app_skey: &[u8; 16],
    f_cnt: u32,
    dev_addr: &[u8; 4],
) -> (Vec<u8>, Vec<EncryptStep>) {
    let cipher = Aes128::new(app_skey.into());
    let mut out = Vec::with_capacity(payload.len());
    let mut steps = Vec::new();

    if payload.is_empty() {
        return (out, steps);
    }

    let block_count = payload.len().div_ceil(AES_BLOCK_SIZE);

    for i in 0..block_count {
        let block_index = (i + 1) as u8;
        let mut a_i = build_ai_block(block_index, f_cnt, dev_addr);
        let a_before = a_i;

        cipher.encrypt_block((&mut a_i).into());
        let s_i = a_i;

        let start = i * AES_BLOCK_SIZE;
        let end = (start + AES_BLOCK_SIZE).min(payload.len());
        let pt_slice = &payload[start..end];
        let mut ct_slice = Vec::with_capacity(pt_slice.len());
        for (b, k) in pt_slice.iter().zip(s_i.iter()) {
            ct_slice.push(b ^ k);
        }
        out.extend_from_slice(&ct_slice);

        steps.push(EncryptStep {
            block_index,
            a_block: a_before,
            s_block: s_i,
            plaintext: pt_slice.to_vec(),
            ciphertext: ct_slice,
        });
    }

    (out, steps)
}

/// Build the Ai block per LoRaWAN 1.1 section 4.3.3.1.
fn build_ai_block(i: u8, f_cnt: u32, dev_addr: &[u8; 4]) -> [u8; AES_BLOCK_SIZE] {
    let mut a = [0u8; AES_BLOCK_SIZE];
    a[0] = 0x01;
    a[5] = DIR_UPLINK;
    // DevAddr is little-endian on the wire and in the Ai block.
    a[6] = dev_addr[3];
    a[7] = dev_addr[2];
    a[8] = dev_addr[1];
    a[9] = dev_addr[0];
    let f_cnt_le = f_cnt.to_le_bytes();
    a[10..14].copy_from_slice(&f_cnt_le);
    a[15] = i;
    a
}

// ----------------------------------------------------------------------------
// Step 4: MIC (LoRaWAN 1.1 section 4.4)
// ----------------------------------------------------------------------------

/// Compute the 4-byte Message Integrity Code over a LoRaWAN uplink frame.
///
/// The MIC covers a B0 prefix block plus the message:
///   B0 = [ 0x49, 0x00, 0x00, 0x00, 0x00, Dir,
///          DevAddr (LE), FCnt (LE 32), 0x00, len(msg) ]
///   msg = MHDR || DevAddr (LE) || FCtrl || FCnt (LE 16) || FPort || FRMPayload
///
/// CMAC is computed over B0 || msg with NwkSKey, and the first 4 bytes are
/// taken as the MIC.
///
/// Note: LoRaWAN 1.1 actually uses two MIC keys (FNwkSIntKey, SNwkSIntKey)
/// for uplinks. We use a single NwkSKey here for clarity; this matches
/// LoRaWAN 1.0.x behavior and is what most demos show.
pub fn compute_mic(
    mhdr: u8,
    dev_addr: &[u8; 4],
    f_ctrl: u8,
    f_cnt: u32,
    f_port: u8,
    encrypted_payload: &[u8],
    nwk_skey: &[u8; 16],
) -> [u8; 4] {
    let mut msg = Vec::with_capacity(1 + 4 + 1 + 2 + 1 + encrypted_payload.len());
    msg.push(mhdr);
    msg.extend_from_slice(&dev_addr_le(dev_addr));
    msg.push(f_ctrl);
    msg.extend_from_slice(&(f_cnt as u16).to_le_bytes());
    msg.push(f_port);
    msg.extend_from_slice(encrypted_payload);

    let mut b0 = [0u8; AES_BLOCK_SIZE];
    b0[0] = 0x49;
    b0[5] = DIR_UPLINK;
    b0[6..10].copy_from_slice(&dev_addr_le(dev_addr));
    b0[10..14].copy_from_slice(&f_cnt.to_le_bytes());
    b0[15] = msg.len() as u8;

    let mut mac = <Cmac<Aes128> as Mac>::new_from_slice(nwk_skey)
        .expect("AES-128 key length is fixed");
    mac.update(&b0);
    mac.update(&msg);
    let tag = mac.finalize().into_bytes();

    let mut mic = [0u8; 4];
    mic.copy_from_slice(&tag[..4]);
    mic
}

/// Reverse a 4-byte DevAddr to little-endian wire order.
fn dev_addr_le(dev_addr: &[u8; 4]) -> [u8; 4] {
    [dev_addr[3], dev_addr[2], dev_addr[1], dev_addr[0]]
}

// ----------------------------------------------------------------------------
// Step 5: Frame assembly
// ----------------------------------------------------------------------------

/// Assemble a full LoRaWAN PHY payload (data uplink) with little-endian
/// DevAddr and 16-bit FCnt on the wire.
///
/// Layout:
///   MHDR (1) | DevAddr (4 LE) | FCtrl (1) | FCnt (2 LE) | FPort (1)
///   | FRMPayload (N) | MIC (4)
///
/// Total size = 13 + N.
pub fn build_lorawan_frame(
    mhdr: u8,
    dev_addr: &[u8; 4],
    f_ctrl: u8,
    f_cnt: u32,
    f_port: u8,
    encrypted_payload: &[u8],
    mic: &[u8; 4],
) -> Vec<u8> {
    let mut frame = Vec::with_capacity(13 + encrypted_payload.len());
    frame.push(mhdr);
    frame.extend_from_slice(&dev_addr_le(dev_addr));
    frame.push(f_ctrl);
    frame.extend_from_slice(&(f_cnt as u16).to_le_bytes());
    frame.push(f_port);
    frame.extend_from_slice(encrypted_payload);
    frame.extend_from_slice(mic);
    frame
}

// ----------------------------------------------------------------------------
// Step 6: Bytes -> SF-bit symbols
// ----------------------------------------------------------------------------

/// Pack a byte stream into SF-bit symbols, MSB-first.
///
/// At SF=8 each byte becomes one symbol. At SF=7, eight bytes (64 bits)
/// pack into ceil(64/7) = 10 symbols with 6 bits of zero padding in the
/// last symbol. SF=12 packs 8 bits into 12-bit symbols leaving 4 zero bits
/// in each symbol's low end.
///
/// This is teaching-grade packing; real LoRaWAN applies whitening, Hamming
/// FEC and interleaving before this step.
pub fn bytes_to_symbols(bytes: &[u8], sf: u8) -> Vec<u16> {
    assert!((7..=12).contains(&sf), "SF must be 7..=12");
    if bytes.is_empty() {
        return Vec::new();
    }
    let sf = sf as u32;
    let total_bits = bytes.len() as u32 * 8;
    let symbol_count = total_bits.div_ceil(sf) as usize;
    let mask: u16 = ((1u32 << sf) - 1) as u16;

    let mut out = Vec::with_capacity(symbol_count);
    for s in 0..symbol_count {
        let bit_start = s as u32 * sf;
        let mut sym: u16 = 0;
        for k in 0..sf {
            let bit_idx = bit_start + k;
            if bit_idx >= total_bits {
                break;
            }
            let byte = bytes[(bit_idx / 8) as usize];
            // MSB of the byte is bit 7.
            let bit = (byte >> (7 - (bit_idx % 8) as u8)) & 1;
            sym |= (bit as u16) << (sf - 1 - k);
        }
        out.push(sym & mask);
    }
    out
}

/// Standard reflected binary Gray code: g = s XOR (s >> 1).
pub fn apply_gray_coding(symbol: u16) -> u16 {
    symbol ^ (symbol >> 1)
}

/// Inverse of [`apply_gray_coding`]. Useful for tests and for receivers.
pub fn inverse_gray_coding(mut g: u16) -> u16 {
    let mut s = g;
    while g > 0 {
        g >>= 1;
        s ^= g;
    }
    s
}

// ----------------------------------------------------------------------------
// Step 7: Chirp generation
// ----------------------------------------------------------------------------

/// Map a symbol value to its starting frequency offset within the band.
///
/// Symbol 0 starts at the bottom of the band (0 in the baseband convention
/// used here). The frequency wraps modulo BW across the symbol duration,
/// producing the characteristic LoRa chirp.
pub fn symbol_to_frequency_offset(symbol: u16, sf: u8, bw_hz: f32) -> f32 {
    let n: f32 = (1u32 << sf as u32) as f32;
    (symbol as f32 / n) * bw_hz
}

/// Direction of a baseband chirp.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ChirpDirection {
    /// Frequency rises with time (data symbols).
    Up,
    /// Frequency falls with time (preamble SFD).
    Down,
}

/// A baseband chirp waveform plus its frequency-vs-time trace.
///
/// `samples` are unit-amplitude sine values you can play through an audio
/// device. `freq_trace` is the instantaneous frequency at each sample,
/// suitable for plotting on a time/frequency axis.
#[derive(Clone, Debug)]
pub struct ChirpWaveform {
    pub samples: Vec<f32>,
    pub freq_trace: Vec<f32>,
    pub sample_rate_hz: u32,
    pub bw_hz: f32,
    pub sf: u8,
    pub symbol: u16,
    pub direction: ChirpDirection,
}

impl ChirpWaveform {
    pub fn upchirp(symbol: u16, sf: u8, bw_hz: f32, sample_rate_hz: u32) -> Self {
        Self::generate(symbol, sf, bw_hz, sample_rate_hz, ChirpDirection::Up)
    }

    pub fn downchirp(sf: u8, bw_hz: f32, sample_rate_hz: u32) -> Self {
        Self::generate(0, sf, bw_hz, sample_rate_hz, ChirpDirection::Down)
    }

    fn generate(
        symbol: u16,
        sf: u8,
        bw_hz: f32,
        sample_rate_hz: u32,
        direction: ChirpDirection,
    ) -> Self {
        let n = (1u32 << sf as u32) as f32;
        let t_sym = n / bw_hz;
        let sample_count = ((t_sym * sample_rate_hz as f32).ceil() as usize).max(1);
        let dt = 1.0 / sample_rate_hz as f32;

        let f_start_up = (symbol as f32 / n) * bw_hz;

        let mut samples = Vec::with_capacity(sample_count);
        let mut freq_trace = Vec::with_capacity(sample_count);
        let mut phase: f32 = 0.0;

        for i in 0..sample_count {
            let t = i as f32 * dt;

            let f_inst = match direction {
                ChirpDirection::Up => {
                    // Linear sweep from f_start to f_start + BW, wrapping at BW.
                    let f = f_start_up + (bw_hz / t_sym) * t;
                    f.rem_euclid(bw_hz)
                }
                ChirpDirection::Down => {
                    // Linear sweep from BW down to 0.
                    let f = bw_hz - (bw_hz / t_sym) * t;
                    f.clamp(0.0, bw_hz)
                }
            };
            freq_trace.push(f_inst);

            // Numerical phase integration is fine for visualization at the
            // sample rates we use here. For a reference TX you would
            // integrate the linear chirp analytically.
            phase += 2.0 * std::f32::consts::PI * f_inst * dt;
            samples.push(phase.sin());
        }

        Self {
            samples,
            freq_trace,
            sample_rate_hz,
            bw_hz,
            sf,
            symbol,
            direction,
        }
    }

    pub fn duration_s(&self) -> f32 {
        self.samples.len() as f32 / self.sample_rate_hz as f32
    }
}

/// Convenience: generate the raw sample buffer for a chirp without the trace.
pub fn generate_chirp_samples(
    symbol: u16,
    sf: u8,
    bw_hz: f32,
    sample_rate_hz: u32,
) -> Vec<f32> {
    ChirpWaveform::upchirp(symbol, sf, bw_hz, sample_rate_hz).samples
}

/// Generate audible chirp samples with explicit target frequency and duration.
///
/// `target_top_hz` is the top of the audible band (the chirp sweeps from
/// some fraction of this down or up to it). `target_t_sym_s` is how long
/// the chirp should last. Both are independent of the real LoRa BW/SF, so
/// every configuration sounds at a comfortable rate.
pub fn generate_audio_chirp_samples(
    symbol: u16,
    sf: u8,
    target_top_hz: f32,
    target_t_sym_s: f32,
    sample_rate_hz: u32,
    direction: ChirpDirection,
) -> Vec<f32> {
    assert!(target_top_hz > 0.0 && target_t_sym_s > 0.0);
    let n = (1u32 << sf as u32) as f32;
    let sample_count = ((target_t_sym_s * sample_rate_hz as f32).ceil() as usize).max(1);
    let dt = 1.0 / sample_rate_hz as f32;
    let f_start_up = (symbol as f32 / n) * target_top_hz;

    let mut out = Vec::with_capacity(sample_count);
    let mut phase: f32 = 0.0;
    for i in 0..sample_count {
        let t = i as f32 * dt;
        let f_inst = match direction {
            ChirpDirection::Up => {
                let f = f_start_up + (target_top_hz / target_t_sym_s) * t;
                f.rem_euclid(target_top_hz)
            }
            ChirpDirection::Down => {
                let f = target_top_hz - (target_top_hz / target_t_sym_s) * t;
                f.clamp(0.0, target_top_hz)
            }
        };
        phase += 2.0 * std::f32::consts::PI * f_inst * dt;
        out.push(phase.sin());
    }
    out
}

// ----------------------------------------------------------------------------
// Step 6 helper: full symbol stream with metadata for the visualizer
// ----------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    Preamble,
    Sync,
    Header,
    Payload,
}

#[derive(Clone, Debug)]
pub struct LabeledSymbol {
    pub index: usize,
    pub kind: SymbolKind,
    pub raw: u16,
    pub gray: u16,
    pub direction: ChirpDirection,
}

/// Build the symbol stream that would be transmitted: preamble, sync, then
/// frame bytes packed into SF-bit symbols. The first byte of the frame
/// (MHDR) is treated as the "header" for visualization purposes.
pub fn build_symbol_stream(frame: &[u8], sf: u8) -> Vec<LabeledSymbol> {
    let mut out = Vec::new();
    let mut idx = 0usize;

    for _ in 0..PREAMBLE_LEN {
        out.push(LabeledSymbol {
            index: idx,
            kind: SymbolKind::Preamble,
            raw: 0,
            gray: 0,
            direction: ChirpDirection::Down,
        });
        idx += 1;
    }

    for &raw in &[SYNC_SYMBOL_1, SYNC_SYMBOL_2] {
        out.push(LabeledSymbol {
            index: idx,
            kind: SymbolKind::Sync,
            raw,
            gray: apply_gray_coding(raw),
            direction: ChirpDirection::Up,
        });
        idx += 1;
    }

    if frame.is_empty() {
        return out;
    }

    let header_syms = bytes_to_symbols(&frame[..1], sf);
    for raw in header_syms {
        out.push(LabeledSymbol {
            index: idx,
            kind: SymbolKind::Header,
            raw,
            gray: apply_gray_coding(raw),
            direction: ChirpDirection::Up,
        });
        idx += 1;
    }

    if frame.len() > 1 {
        let payload_syms = bytes_to_symbols(&frame[1..], sf);
        for raw in payload_syms {
            out.push(LabeledSymbol {
                index: idx,
                kind: SymbolKind::Payload,
                raw,
                gray: apply_gray_coding(raw),
                direction: ChirpDirection::Up,
            });
            idx += 1;
        }
    }

    out
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_key(s: &str) -> [u8; 16] {
        assert_eq!(s.len(), 32);
        let mut k = [0u8; 16];
        for i in 0..16 {
            k[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        k
    }

    #[test]
    fn message_to_bytes_basic() {
        assert_eq!(message_to_bytes("hello"), vec![0x68, 0x65, 0x6c, 0x6c, 0x6f]);
        assert_eq!(message_to_bytes(""), Vec::<u8>::new());
    }

    #[test]
    fn gray_coding_round_trip() {
        for s in 0u16..4096 {
            assert_eq!(inverse_gray_coding(apply_gray_coding(s)), s);
        }
    }

    #[test]
    fn gray_coding_known_values() {
        assert_eq!(apply_gray_coding(0), 0);
        assert_eq!(apply_gray_coding(1), 1);
        assert_eq!(apply_gray_coding(2), 3);
        assert_eq!(apply_gray_coding(3), 2);
        assert_eq!(apply_gray_coding(4), 6);
        assert_eq!(apply_gray_coding(7), 4);
    }

    #[test]
    fn encrypt_then_decrypt_is_identity() {
        let key = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0x12, 0x34, 0x56, 0x78];
        let pt = b"hello world, this crosses 16 bytes";
        let ct = encrypt_payload(pt, &key, 1, &dev);
        let pt2 = encrypt_payload(&ct, &key, 1, &dev);
        assert_eq!(pt2, pt);
    }

    #[test]
    fn encrypt_changes_with_fcnt() {
        let key = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0x12, 0x34, 0x56, 0x78];
        let pt = b"hello";
        let c1 = encrypt_payload(pt, &key, 1, &dev);
        let c2 = encrypt_payload(pt, &key, 2, &dev);
        assert_ne!(c1, c2);
    }

    #[test]
    fn encrypt_changes_with_devaddr() {
        let key = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let pt = b"hello";
        let c1 = encrypt_payload(pt, &key, 1, &[1, 2, 3, 4]);
        let c2 = encrypt_payload(pt, &key, 1, &[5, 6, 7, 8]);
        assert_ne!(c1, c2);
    }

    #[test]
    fn encrypt_with_steps_block_count() {
        let key = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0x12, 0x34, 0x56, 0x78];
        let pt = b"hello, longer than 16 bytes please";
        let plain = encrypt_payload(pt, &key, 7, &dev);
        let (with_steps, steps) = encrypt_payload_with_steps(pt, &key, 7, &dev);
        assert_eq!(plain, with_steps);
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0].block_index, 1);
        assert_eq!(steps[2].block_index, 3);
    }

    #[test]
    fn encrypt_empty_payload_returns_empty() {
        let key = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0; 4];
        assert!(encrypt_payload(&[], &key, 0, &dev).is_empty());
    }

    #[test]
    fn ai_block_layout_matches_spec() {
        let dev = [0x01, 0x02, 0x03, 0x04];
        let a = build_ai_block(1, 0x0000_0007, &dev);
        assert_eq!(a[0], 0x01);
        assert_eq!(&a[1..5], &[0, 0, 0, 0]);
        assert_eq!(a[5], 0x00); // uplink
        assert_eq!(&a[6..10], &[0x04, 0x03, 0x02, 0x01]);
        assert_eq!(&a[10..14], &[0x07, 0x00, 0x00, 0x00]);
        assert_eq!(a[14], 0x00);
        assert_eq!(a[15], 0x01);
    }

    #[test]
    fn mic_is_deterministic_and_4_bytes() {
        let nwk = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0x12, 0x34, 0x56, 0x78];
        let ct = vec![0xaa, 0xbb, 0xcc];
        let m1 = compute_mic(0x40, &dev, 0x00, 1, 1, &ct, &nwk);
        let m2 = compute_mic(0x40, &dev, 0x00, 1, 1, &ct, &nwk);
        assert_eq!(m1, m2);
        assert_eq!(m1.len(), 4);
    }

    #[test]
    fn mic_changes_with_payload() {
        let nwk = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0x12, 0x34, 0x56, 0x78];
        let m1 = compute_mic(0x40, &dev, 0x00, 1, 1, &[1, 2, 3], &nwk);
        let m2 = compute_mic(0x40, &dev, 0x00, 1, 1, &[1, 2, 4], &nwk);
        assert_ne!(m1, m2);
    }

    #[test]
    fn mic_changes_with_fcnt() {
        let nwk = hex_key("2b7e151628aed2a6abf7158809cf4f3c");
        let dev = [0x12, 0x34, 0x56, 0x78];
        let pl = [0xaa, 0xbb];
        let m1 = compute_mic(0x40, &dev, 0x00, 1, 1, &pl, &nwk);
        let m2 = compute_mic(0x40, &dev, 0x00, 2, 1, &pl, &nwk);
        assert_ne!(m1, m2);
    }

    #[test]
    fn frame_layout_sizes() {
        let f = build_lorawan_frame(
            0x40,
            &[1, 2, 3, 4],
            0x00,
            0x0001,
            0x01,
            &[0xaa; 5],
            &[0; 4],
        );
        assert_eq!(f.len(), 1 + 4 + 1 + 2 + 1 + 5 + 4);
        assert_eq!(f[0], 0x40);
    }

    #[test]
    fn frame_devaddr_is_little_endian() {
        let f = build_lorawan_frame(
            0x40,
            &[0x12, 0x34, 0x56, 0x78],
            0x00,
            0,
            1,
            &[],
            &[0; 4],
        );
        assert_eq!(&f[1..5], &[0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn bytes_to_symbols_sf8_byte_aligned() {
        let syms = bytes_to_symbols(&[0xab, 0xcd], 8);
        assert_eq!(syms, vec![0xab, 0xcd]);
    }

    #[test]
    fn bytes_to_symbols_sf7_packs_bits() {
        let syms = bytes_to_symbols(&[0xff, 0xff], 7);
        assert_eq!(syms.len(), 3);
        assert_eq!(syms[0], 0x7f);
        assert_eq!(syms[1], 0x7f);
        assert_eq!(syms[2], 0x60);
    }

    #[test]
    fn bytes_to_symbols_sf12_padded() {
        let syms = bytes_to_symbols(&[0xab], 12);
        assert_eq!(syms.len(), 1);
        // 0xab packed MSB-first into 12 bits: 1010_1011_0000 = 0xab0.
        assert_eq!(syms[0], 0xab0);
    }

    #[test]
    fn frequency_offset_basics() {
        assert_eq!(symbol_to_frequency_offset(0, 7, 125_000.0), 0.0);
        let half = symbol_to_frequency_offset(64, 7, 125_000.0);
        assert!((half - 62_500.0).abs() < 1.0);
    }

    #[test]
    fn upchirp_has_expected_length() {
        // T_sym at SF=7, BW=125k is 2^7 / 125_000 = 1.024 ms.
        // At 8.7 kHz sample rate: ceil(1.024e-3 * 8700) = 9 samples.
        let c = ChirpWaveform::upchirp(0, 7, 125_000.0, 8_700);
        assert_eq!(c.samples.len(), 9);
        assert_eq!(c.freq_trace.len(), 9);
    }

    #[test]
    fn upchirp_starts_at_correct_offset() {
        let c = ChirpWaveform::upchirp(64, 7, 125_000.0, 100_000);
        let f0 = c.freq_trace[0];
        assert!((f0 - 62_500.0).abs() < 100.0);
    }

    #[test]
    fn build_symbol_stream_has_preamble_and_sync() {
        let frame = vec![0x40, 0x01, 0x02, 0x03];
        let stream = build_symbol_stream(&frame, 8);
        assert!(stream.len() >= PREAMBLE_LEN + 2 + 1);
        for i in 0..PREAMBLE_LEN {
            assert_eq!(stream[i].kind, SymbolKind::Preamble);
            assert_eq!(stream[i].raw, 0);
            assert_eq!(stream[i].direction, ChirpDirection::Down);
        }
        assert_eq!(stream[PREAMBLE_LEN].kind, SymbolKind::Sync);
        assert_eq!(stream[PREAMBLE_LEN].raw, SYNC_SYMBOL_1);
        assert_eq!(stream[PREAMBLE_LEN + 1].raw, SYNC_SYMBOL_2);
        assert_eq!(stream[PREAMBLE_LEN + 2].kind, SymbolKind::Header);
    }

    #[test]
    fn build_symbol_stream_empty_frame_just_preamble_and_sync() {
        let stream = build_symbol_stream(&[], 8);
        assert_eq!(stream.len(), PREAMBLE_LEN + 2);
    }
}
