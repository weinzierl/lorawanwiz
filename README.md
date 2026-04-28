# lorawanwiz

LoRaWAN modulation visualizer for technical talks. Built in Rust on Bevy 0.18,
runs natively on Linux/macOS/Windows and in the browser via WASM.

## What it shows

Four tabs:

- **Plaintext**: input summary, UTF-8 bytes, AES-128 CTR-style encryption with
  per-block A_i, S_i, plaintext, and ciphertext.
- **Frame**: CMAC-AES-128 MIC, assembled wire frame, symbol stream with
  preamble/sync/header/payload labels and Gray-coded values.
- **Modulation**: live baseband chirp canvas. Each transmitted symbol becomes
  one chirp, drawn on a frequency vs. time grid, with the currently-playing
  chirp highlighted as the animation cycles.
- **About**: what's faithful to the spec, what's simplified, why the audio
  sounds the way it does, and a key-safety note.

## Build

```sh
just test          # run unit tests for the math module
just native        # cargo run, opens a desktop window
just wasm          # produce dist/ ready for any static web server
just serve         # serve dist/ on http://localhost:8080
just install-linux # add the .desktop entry and SVG icon for the current user
```

The WASM build needs `wasm-bindgen-cli`. Optional `wasm-opt` shrinks the bundle.

## Architecture

```
src/
  math.rs            pure-Rust LoRaWAN math, no Bevy types, fully tested
  state.rs           Bevy resources: LorawanInputs, PipelineOutput, animator
  pipeline.rs        glue system, runs math on every input change
  ui.rs              input bar, tabs, step panels, scroll, tooltips
  visualization.rs   2D mesh chirp canvas with animation, tab-aware visibility
  audio.rs           web-sys AudioContext (WASM) and cpal output stream (native)
  lib.rs             plugin assembling all systems
  main.rs            binary shim
```

The math module has no Bevy dependencies and ships with 22 tests covering
encryption round-trips, CMAC determinism, frame layout, symbol packing at
SF 7/8/12, Gray-code round-trips, and chirp generation.

## Audio

LoRa chirps span 125-500 kHz which is well above hearing. The Play audio
button generates a parallel chirp set with a fixed audible target frequency
(3 kHz top) and fixed symbol duration (80 ms), independent of the chosen
SF/BW. Visualization always shows the true LoRa numbers; only audio is
rescaled.

## Deliberate simplifications

Compared to a real LoRaWAN PHY:

* No whitening, Hamming FEC, or interleaving on the symbol path. A real
  receiver could not decode this waveform, but the chirp structure is
  faithful and the encryption + MIC match the spec.
* No CRC.
* The "preamble" uses 8 downchirps of value 0. Real LoRa preamble is 8
  upchirps followed by 2 sync symbols and a 2.25-symbol SFD made of
  downchirps.
* LoRaWAN 1.1 splits MIC computation across FNwkSIntKey and SNwkSIntKey
  for uplinks. We use a single NwkSKey (LoRaWAN 1.0.x style) for clarity.
* All keys are public test vectors. Do not put real keys in a demo.

## License

MIT OR Apache-2.0 at your option.
