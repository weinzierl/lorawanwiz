//! Export button. Produces a Typst document describing the entire
//! LoRaWAN flow for the current inputs: parameters, plaintext bytes,
//! AES-CTR encryption blocks, MIC, complete frame, symbol stream,
//! chirp summary. The user picks a destination via the same dialog
//! plumbing used for Save.
//!
//! Typst was chosen because it gives nice typography for technical
//! documents with minimal markup: native math, raw blocks for hex,
//! tables, and a sensible default font stack ("Libertinus Serif",
//! "DejaVu Sans Mono"). Output is a `.typ` source file; the user runs
//! `typst compile lorawanwiz_export.typ` to get a PDF, or opens it in
//! the Typst web app.
//!
//! The platform plumbing (native rfd / WASM Blob+anchor) is shared
//! with `persistence.rs` via small helpers in this module rather than
//! exposing those primitives publicly there.

use bevy::prelude::*;

use crate::math::{ChirpDirection, SymbolKind};
use crate::state::{
    AUDIO_TARGET_T_SYM_S, AUDIO_TARGET_TOP_HZ, AudioSettings, LorawanInputs, PipelineOutput,
};
use crate::ui::ExportButton;

#[cfg(not(target_arch = "wasm32"))]
const TYP_EXTENSION: &str = "typ";
const DEFAULT_FILENAME: &str = "lorawanwiz_export.typ";

pub fn handle_export_click(
    q: Query<&Interaction, (Changed<Interaction>, With<ExportButton>)>,
    inputs: Res<LorawanInputs>,
    output: Res<PipelineOutput>,
    audio: Res<AudioSettings>,
) {
    for i in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        let text = build_typst_document(&inputs, &output, &audio);
        save_to_user_chosen_path(text);
    }
}

// ---------------------------------------------------------------------------
// Document builder
// ---------------------------------------------------------------------------

fn build_typst_document(
    inputs: &LorawanInputs,
    output: &PipelineOutput,
    audio: &AudioSettings,
) -> String {
    let mut s = String::with_capacity(8192);

    // Header with global styling. Choosing a serif body face for
    // readability and a mono face for hex blocks; both are bundled
    // with Typst, so users don't need extra fonts installed.
    write_preamble(&mut s, inputs);

    // Cover content (no separate cover-page break needed; Typst will
    // start at the top of page 1).
    write_cover(&mut s, inputs, output);
    s.push_str("\n#pagebreak()\n\n");

    write_inputs_section(&mut s, inputs);
    s.push_str("\n#pagebreak()\n\n");

    write_plaintext_section(&mut s, inputs, output);
    s.push_str("\n#pagebreak()\n\n");

    write_encryption_section(&mut s, output);
    s.push_str("\n#pagebreak()\n\n");

    write_mic_and_frame_section(&mut s, output);
    s.push_str("\n#pagebreak()\n\n");

    write_symbols_section(&mut s, output);
    s.push_str("\n#pagebreak()\n\n");

    write_chirps_section(&mut s, inputs, output, audio);
    s.push_str("\n#pagebreak()\n\n");

    write_about_section(&mut s);

    s
}

fn write_preamble(s: &mut String, inputs: &LorawanInputs) {
    let title = "LoRaWAN modulation flow";
    let author = "lorawanwiz";

    s.push_str(&format!(
        "#set document(title: \"{}\", author: \"{}\")\n",
        typst_string(title),
        typst_string(author),
    ));
    s.push_str(
        "#set page(\n\
        \x20\x20paper: \"a4\",\n\
        \x20\x20margin: (top: 2.4cm, bottom: 2.4cm, left: 2.2cm, right: 2.2cm),\n\
        \x20\x20header: context {\n\
        \x20\x20\x20\x20if counter(page).get().first() > 1 {\n\
        \x20\x20\x20\x20\x20\x20set text(size: 8.5pt, fill: rgb(\"#777777\"))\n\
        \x20\x20\x20\x20\x20\x20[#emph[lorawanwiz] · LoRaWAN modulation flow #h(1fr) ",
    );
    // Header right-side identifier for the run: DevAddr + FCnt
    s.push_str(&format!(
        "DevAddr {dev_addr} · FCnt 0x{fcnt:04X}]\n\
        \x20\x20\x20\x20\x20\x20line(length: 100%, stroke: 0.5pt + rgb(\"#dddddd\"))\n\
        \x20\x20\x20\x20}}\n\
        \x20\x20}},\n",
        dev_addr = format_dev_addr(&inputs.dev_addr),
        fcnt = inputs.f_cnt as u16,
    ));
    s.push_str(
        "\x20\x20footer: context {\n\
        \x20\x20\x20\x20set text(size: 8.5pt, fill: rgb(\"#777777\"))\n\
        \x20\x20\x20\x20[https://weinzierlweb.com #h(1fr) #counter(page).display()]\n\
        \x20\x20},\n\
        )\n",
    );
    s.push_str("#set text(size: 10.5pt)\n");
    s.push_str("#set par(justify: true, leading: 0.65em)\n");
    s.push_str("#set heading(numbering: none)\n");
    s.push_str(
        "#show heading.where(level: 1): it => {\n\
        \x20\x20set text(size: 22pt, fill: rgb(\"#1f4e79\"), weight: \"bold\")\n\
        \x20\x20block(below: 0.8em, it.body)\n\
        }\n",
    );
    s.push_str(
        "#show heading.where(level: 2): it => {\n\
        \x20\x20set text(size: 14pt, fill: rgb(\"#1f4e79\"), weight: \"semibold\")\n\
        \x20\x20block(above: 1.2em, below: 0.5em, it.body)\n\
        }\n",
    );
    s.push_str(
        "#show heading.where(level: 3): it => {\n\
        \x20\x20set text(size: 11pt, fill: rgb(\"#333333\"), weight: \"semibold\")\n\
        \x20\x20block(above: 0.9em, below: 0.3em, it.body)\n\
        }\n",
    );
    s.push_str("#show raw: set text(font: \"DejaVu Sans Mono\", size: 9pt)\n");
    s.push_str("#show raw.where(block: true): block.with(fill: rgb(\"#f4f4f4\"), inset: 8pt, radius: 3pt, width: 100%)\n");
    // CeTZ for the chirp visualization. Resolved from typst.app/universe
    // on first compile; no manual install required if Typst has network
    // access.
    s.push_str("#import \"@preview/cetz:0.4.2\"\n");
    s.push('\n');
}

fn write_cover(s: &mut String, inputs: &LorawanInputs, output: &PipelineOutput) {
    s.push_str("#v(1fr)\n");
    s.push_str("#align(center)[\n");
    s.push_str("\x20\x20#text(size: 32pt, weight: \"bold\", fill: rgb(\"#1f4e79\"))[LoRaWAN modulation flow]\n");
    s.push_str("\x20\x20#v(0.4em)\n");
    s.push_str("\x20\x20#text(size: 14pt, fill: rgb(\"#555555\"))[Step-by-step trace of message, encryption, framing, modulation]\n");
    s.push_str("\x20\x20#v(2em)\n");
    s.push_str("\x20\x20#text(size: 11pt, fill: rgb(\"#777777\"))[Generated by ");
    s.push_str(&format!(
        "lorawanwiz v{} · #link(\"https://weinzierlweb.com\")\n",
        env!("CARGO_PKG_VERSION")
    ));
    s.push_str("]\n");
    s.push_str("]\n");
    s.push_str("#v(2fr)\n\n");

    // Quick summary box, centered
    s.push_str("#align(center)[\n");
    s.push_str("\x20\x20#box(\n");
    s.push_str("\x20\x20\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20\x20\x20radius: 4pt,\n");
    s.push_str("\x20\x20\x20\x20inset: 14pt,\n");
    s.push_str("\x20\x20\x20\x20width: 80%,\n");
    s.push_str("\x20\x20)[\n");
    s.push_str("\x20\x20\x20\x20#table(\n");
    s.push_str("\x20\x20\x20\x20\x20\x20columns: (auto, 1fr),\n");
    s.push_str("\x20\x20\x20\x20\x20\x20stroke: none,\n");
    s.push_str("\x20\x20\x20\x20\x20\x20align: (right, left),\n");
    s.push_str("\x20\x20\x20\x20\x20\x20column-gutter: 1em,\n");
    s.push_str("\x20\x20\x20\x20\x20\x20row-gutter: 0.4em,\n");
    s.push_str(&format!(
        "\x20\x20\x20\x20\x20\x20[Message:], [`{}`],\n",
        typst_raw_inline(&inputs.message)
    ));
    s.push_str(&format!(
        "\x20\x20\x20\x20\x20\x20[Modulation:], [SF{} · {} kHz · CR {}],\n",
        inputs.sf,
        (inputs.bw_hz / 1000.0) as u32,
        inputs.coding_rate.label(),
    ));
    s.push_str(&format!(
        "\x20\x20\x20\x20\x20\x20[Frame size:], [{} bytes],\n",
        output.frame.len()
    ));
    s.push_str(&format!(
        "\x20\x20\x20\x20\x20\x20[Symbols:], [{} ({:.1} ms each)],\n",
        output.symbols.len(),
        sym_duration_ms(inputs)
    ));
    s.push_str(&format!(
        "\x20\x20\x20\x20\x20\x20[Airtime:], [{:.2} s],\n",
        output.symbols.len() as f32 * sym_duration_ms(inputs) / 1000.0
    ));
    s.push_str("\x20\x20\x20\x20)\n");
    s.push_str("\x20\x20]\n");
    s.push_str("]\n");
    s.push_str("#v(2fr)\n");
    s.push_str("#align(center)[\n");
    s.push_str("\x20\x20#text(size: 9pt, fill: rgb(\"#999999\"))[Copyright L. Weinzierl, 2026]\n");
    s.push_str("]\n");
}

fn write_inputs_section(s: &mut String, inputs: &LorawanInputs) {
    s.push_str("= Inputs\n\n");
    s.push_str("This document traces the full LoRaWAN uplink path for the inputs below: ");
    s.push_str("each section presents one stage of the pipeline, from plaintext bytes ");
    s.push_str("through encryption, MIC, framing, symbol packing, and chirp modulation.\n\n");

    s.push_str("== Message and modulation\n\n");
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, 1fr),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (right, left),\n");
    s.push_str("\x20\x20inset: 6pt,\n");
    s.push_str(&format!(
        "\x20\x20[*Message*], [`{}`],\n",
        typst_raw_inline(&inputs.message)
    ));
    s.push_str(&format!(
        "\x20\x20[*Message length*], [{} bytes (max {} for SF{} / {} kHz, EU868)],\n",
        inputs.message.len(),
        inputs.max_app_payload_bytes(),
        inputs.sf,
        (inputs.bw_hz / 1000.0) as u32
    ));
    s.push_str(&format!(
        "\x20\x20[*Spreading factor*], [SF{}],\n",
        inputs.sf
    ));
    s.push_str(&format!(
        "\x20\x20[*Bandwidth*], [{} kHz],\n",
        (inputs.bw_hz / 1000.0) as u32
    ));
    s.push_str(&format!(
        "\x20\x20[*Coding rate*], [{}],\n",
        inputs.coding_rate.label()
    ));
    s.push_str(&format!(
        "\x20\x20[*Symbol time*], [{:.3} ms ($T_(\"sym\") = 2^(\"SF\") / \"BW\"$)],\n",
        sym_duration_ms(inputs)
    ));
    s.push_str(")\n\n");

    s.push_str("== Frame parameters\n\n");
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, 1fr),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (right, left),\n");
    s.push_str("\x20\x20inset: 6pt,\n");
    s.push_str(&format!(
        "\x20\x20[*DevAddr*], [`{}`],\n",
        format_dev_addr(&inputs.dev_addr)
    ));
    s.push_str(&format!(
        "\x20\x20[*FCnt*], [`0x{:04X}` ({})],\n",
        inputs.f_cnt as u16, inputs.f_cnt
    ));
    s.push_str(&format!(
        "\x20\x20[*FPort*], [{}],\n",
        inputs.f_port
    ));
    s.push_str(")\n\n");

    s.push_str("== Crypto context\n\n");
    s.push_str("Defaults are public test vectors. ");
    s.push_str("Real production keys must never appear in exported documents.\n\n");
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, 1fr),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (right, left),\n");
    s.push_str("\x20\x20inset: 6pt,\n");
    s.push_str(&format!(
        "\x20\x20[*AppSKey*], [`{}`],\n",
        format_hex_compact(&inputs.app_skey)
    ));
    s.push_str(&format!(
        "\x20\x20[*NwkSKey*], [`{}`],\n",
        format_hex_compact(&inputs.nwk_skey)
    ));
    s.push_str(")\n");
}

fn write_plaintext_section(s: &mut String, inputs: &LorawanInputs, output: &PipelineOutput) {
    s.push_str("= Step 1: Plaintext bytes\n\n");
    s.push_str(
        "The message is taken as UTF-8. These bytes form the FRMPayload before \
         encryption.\n\n",
    );

    s.push_str("== ASCII\n\n");
    s.push_str("```\n");
    s.push_str(&typst_safe_for_raw_block(&inputs.message));
    s.push_str("\n```\n\n");

    s.push_str("== Hex\n\n");
    s.push_str("```\n");
    s.push_str(&format_hex_block(&output.plaintext));
    s.push_str("\n```\n");
}

fn write_encryption_section(s: &mut String, output: &PipelineOutput) {
    s.push_str("= Step 2: FRMPayload encryption (AES-128, CTR-style)\n\n");
    s.push_str(
        "LoRaWAN encrypts FRMPayload by XOR-ing the plaintext against a keystream \
         built from $S_i = "
    );
    s.push_str("\"AES\"_\"AppSKey\"(A_i)$, ");
    s.push_str(
        "where $A_i$ is a per-block input that fixes the direction (uplink), \
         DevAddr, FCnt, and the block index $i$. One $A_i$ block produces one \
         16-byte $S_i$, and the message is processed in 16-byte slices.\n\n",
    );

    if output.encrypt_steps.is_empty() {
        s.push_str("_No encryption blocks (empty payload)._\n");
        return;
    }

    s.push_str("== Per-block trace\n\n");
    for step in &output.encrypt_steps {
        s.push_str(&format!("=== Block {}\n\n", step.block_index));
        s.push_str("#table(\n");
        s.push_str("\x20\x20columns: (auto, 1fr),\n");
        s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
        s.push_str("\x20\x20align: (right + horizon, left),\n");
        s.push_str("\x20\x20inset: 6pt,\n");
        s.push_str(&format!(
            "\x20\x20[*$A_{}$*], [`{}`],\n",
            step.block_index,
            format_hex_compact(&step.a_block)
        ));
        s.push_str(&format!(
            "\x20\x20[*$S_{}$*], [`{}`],\n",
            step.block_index,
            format_hex_compact(&step.s_block)
        ));
        s.push_str(&format!(
            "\x20\x20[*PT*], [`{}`],\n",
            format_hex_compact(&step.plaintext)
        ));
        s.push_str(&format!(
            "\x20\x20[*CT*], [`{}`],\n",
            format_hex_compact(&step.ciphertext)
        ));
        s.push_str(")\n\n");
    }

    s.push_str("== Final ciphertext\n\n");
    s.push_str("```\n");
    s.push_str(&format_hex_block(&output.ciphertext));
    s.push_str("\n```\n");
}

fn write_mic_and_frame_section(s: &mut String, output: &PipelineOutput) {
    s.push_str("= Step 3: Message Integrity Code (CMAC-AES-128)\n\n");
    s.push_str(
        "The MIC is the first 4 bytes of $\"CMAC\"_\"NwkSKey\"(B_0 || \"msg\")$, \
         where $B_0$ is a 16-byte authentication block (direction, DevAddr, \
         FCnt, message length) and $\"msg\"$ is the wire-format frame up to \
         (but not including) the MIC itself.\n\n",
    );

    s.push_str("== MIC bytes\n\n");
    s.push_str("```\n");
    s.push_str(&format_hex_block(&output.mic));
    s.push_str("\n```\n\n");

    s.push_str("= Step 4: Complete LoRaWAN frame\n\n");
    s.push_str("The frame on the wire, in transmit order:\n\n");
    s.push_str("```\n");
    s.push_str(
        "MHDR | DevAddr (LE, 4 B) | FCtrl (1 B) | FCnt LE16 (2 B) | FPort (1 B) \
         | FRMPayload | MIC (4 B)\n",
    );
    s.push_str("```\n\n");

    s.push_str(&format!(
        "_Total size:_ *{} bytes*\n\n",
        output.frame.len()
    ));

    s.push_str("== Frame bytes\n\n");
    s.push_str("```\n");
    s.push_str(&format_hex_block(&output.frame));
    s.push_str("\n```\n");
}

fn write_symbols_section(s: &mut String, output: &PipelineOutput) {
    s.push_str("= Step 5: Symbol stream\n\n");
    s.push_str(
        "The frame bytes are packed into SF-bit symbols and Gray-coded, then \
         framed with a preamble (8 downchirps showing 0), sync (2 zeros for \
         this demo), header (2 symbols), and the data symbols.\n\n",
    );

    // Group summary
    s.push_str("== Group summary\n\n");
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, auto, 1fr),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (left, right, left),\n");
    s.push_str("\x20\x20inset: 6pt,\n");
    s.push_str("\x20\x20[*Phase*], [*Count*], [*Indices*],\n");

    let mut last_kind: Option<SymbolKind> = None;
    let mut group_start = 0usize;
    for (i, sym) in output.symbols.iter().enumerate() {
        if last_kind != Some(sym.kind) {
            if let Some(k) = last_kind {
                s.push_str(&format!(
                    "\x20\x20[{}], [{}], [{} to {}],\n",
                    symbol_kind_label(k),
                    i - group_start,
                    group_start,
                    i - 1,
                ));
            }
            last_kind = Some(sym.kind);
            group_start = i;
        }
    }
    if let Some(k) = last_kind {
        let i = output.symbols.len();
        s.push_str(&format!(
            "\x20\x20[{}], [{}], [{} to {}],\n",
            symbol_kind_label(k),
            i - group_start,
            group_start,
            i - 1,
        ));
    }
    s.push_str(")\n\n");

    s.push_str(&format!(
        "_Total symbols:_ *{}*\n\n",
        output.symbols.len()
    ));

    // Detailed symbol listing. Cap at 64 rows to keep the document
    // sensible: long messages can produce hundreds of symbols and we
    // don't want a 30-page export. After the cap, show a note.
    let cap = 64usize;
    s.push_str("== First symbols (detail)\n\n");
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, auto, auto, auto, auto),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (right, left, left, right, right),\n");
    s.push_str("\x20\x20inset: 5pt,\n");
    s.push_str("\x20\x20[*\\#*], [*Phase*], [*Direction*], [*Raw*], [*Gray*],\n");

    for sym in output.symbols.iter().take(cap) {
        let dir = match sym.direction {
            ChirpDirection::Up => "up",
            ChirpDirection::Down => "down",
        };
        s.push_str(&format!(
            "\x20\x20[{}], [{}], [{}], [`0x{:03X}`], [`0x{:03X}`],\n",
            sym.index,
            symbol_kind_label(sym.kind),
            dir,
            sym.raw,
            sym.gray,
        ));
    }
    s.push_str(")\n\n");

    if output.symbols.len() > cap {
        s.push_str(&format!(
            "_… {} additional symbols omitted for brevity._\n",
            output.symbols.len() - cap
        ));
    }
}

/// Number of chirps drawn per horizontal strip in the Typst export's
/// CETZ canvas. Chosen so each strip fits comfortably within A4
/// printable width with the configured margins.
const CHIRPS_PER_STRIP: usize = 12;
/// Horizontal width of one chirp on the page, in centimetres.
const CHIRP_W_CM: f32 = 1.10;
/// Vertical height of one row (signal / reference / product), in cm.
const ROW_H_CM: f32 = 1.4;
/// Vertical gap between rows within a strip, in cm.
const ROW_GAP_CM: f32 = 0.35;
/// Horizontal offset reserved on the left for the y-axis row label.
const LEFT_LABEL_CM: f32 = 1.25;
/// When a chirp's `freq_trace` has more samples than this, downsample
/// before emitting CETZ commands. Keeps the .typ file from blowing up
/// on long messages at high SF without visible loss at this scale.
const MAX_SAMPLES_PER_CHIRP: usize = 40;

fn write_visualization_subsection(
    s: &mut String,
    inputs: &LorawanInputs,
    output: &PipelineOutput,
) {
    s.push_str("== Visualization\n\n");
    s.push_str(
        "Three rows per strip: top is the transmitted signal (per-symbol chirp); \
         middle is the dechirping reference, the conjugate of the basic value-0 \
         upchirp (the same downchirp for every column); bottom is the product, \
         signal times conjugate reference, expressed as instantaneous frequency. \
         For a data upchirp at symbol value $v$, the product is a flat line at \
         height $v dot \"BW\" / 2^\"SF\"$, which is how the receiver reads off \
         the symbol value. Preamble and sync chirps appear chirpy in the product \
         row rather than flat. The plot wraps frequency modulo the bandwidth.\n\n",
    );

    if output.chirps.is_empty() {
        s.push_str("_No chirps to display._\n\n");
        return;
    }

    // Pre-compute the reference downchirp's frequency trace once, since
    // it's identical for every column. The product trace is per-symbol.
    let ref_trace_full = make_reference_trace_for_export(
        output.chirps[0].freq_trace.len(),
        inputs.bw_hz,
    );

    let n_chirps = output.chirps.len();
    let mut start = 0usize;
    while start < n_chirps {
        let end = (start + CHIRPS_PER_STRIP).min(n_chirps);
        emit_strip(s, inputs, output, &ref_trace_full, start, end);
        start = end;
    }
}

fn emit_strip(
    s: &mut String,
    inputs: &LorawanInputs,
    output: &PipelineOutput,
    ref_trace_full: &[f32],
    start: usize,
    end: usize,
) {
    let count = end - start;
    let strip_w = count as f32 * CHIRP_W_CM;

    // Bottom of each row, measured upward from canvas y=0. Y grows up
    // in CETZ, so we lay out from the bottom (product row) toward the
    // top (signal row), leaving room for headers above and a footer
    // label below.
    let footer_h = 0.45;
    let prod_bot = footer_h;
    let prod_top = prod_bot + ROW_H_CM;
    let ref_bot = prod_top + ROW_GAP_CM;
    let ref_top = ref_bot + ROW_H_CM;
    let sig_bot = ref_top + ROW_GAP_CM;
    let sig_top = sig_bot + ROW_H_CM;
    let header_y = sig_top + 0.18;

    // Begin a CETZ canvas. We center it horizontally; left padding is
    // the y-axis-label area, then the chirp strip.
    s.push_str("#align(center)[\n");
    s.push_str("#cetz.canvas(length: 1cm, {\n");
    s.push_str("  import cetz.draw: *\n");

    // Plot region origin (x=0 at the start of the chirp strip, after
    // the left-label column).
    let x0 = LEFT_LABEL_CM;
    let x1 = x0 + strip_w;

    // Row baselines (x-axis of each row).
    for (label, y_bot, y_top, axis_color) in [
        ("signal", sig_bot, sig_top, "#7a7d86"),
        ("reference", ref_bot, ref_top, "#7a7d86"),
        ("product", prod_bot, prod_top, "#7a7d86"),
    ] {
        // Baseline (x-axis at y_bot).
        s.push_str(&format!(
            "  line(({x0:.3}, {y:.3}), ({x1:.3}, {y:.3}), stroke: 0.4pt + rgb(\"{c}\"))\n",
            x0 = x0, x1 = x1, y = y_bot, c = axis_color,
        ));
        // Y-axis (vertical line at x0).
        s.push_str(&format!(
            "  line(({x0:.3}, {y0:.3}), ({x0:.3}, {y1:.3}), stroke: 0.4pt + rgb(\"{c}\"))\n",
            x0 = x0, y0 = y_bot, y1 = y_top, c = axis_color,
        ));
        // Row label on the far left, vertically centered on the row.
        s.push_str(&format!(
            "  content(({xl:.3}, {y:.3}), align(center)[#text(size: 8pt, fill: rgb(\"#3070b0\"))[{label}]])\n",
            xl = LEFT_LABEL_CM * 0.5,
            y = (y_bot + y_top) * 0.5,
            label = label,
        ));
        // BW label at top of row, 0 Hz at bottom.
        let bw_khz = inputs.bw_hz / 1000.0;
        s.push_str(&format!(
            "  content(({xl:.3}, {y:.3}), align(right)[#text(size: 7pt, fill: rgb(\"#7a7d86\"))[{kf:.0} kHz]])\n",
            xl = LEFT_LABEL_CM - 0.05,
            y = y_top - 0.12,
            kf = bw_khz,
        ));
        s.push_str(&format!(
            "  content(({xl:.3}, {y:.3}), align(right)[#text(size: 7pt, fill: rgb(\"#7a7d86\"))[0 Hz]])\n",
            xl = LEFT_LABEL_CM - 0.05,
            y = y_bot + 0.12,
        ));
    }

    // Per-symbol header labels above the signal row.
    for i in start..end {
        let col = (i - start) as f32;
        let cx = x0 + col * CHIRP_W_CM + CHIRP_W_CM * 0.5;
        let sym = &output.symbols[i];
        let kind_letter = symbol_kind_letter(sym.kind);
        // Typst hard line break inside a content block is `\` followed
        // by whitespace. In Rust source one literal backslash is "\\".
        let label = format!("{}{} \\ 0x{:X}", kind_letter, i, sym.raw);
        let color = signal_color_for(sym.kind, output.chirps[i].direction);
        s.push_str(&format!(
            "  content(({cx:.3}, {y:.3}), align(center)[#text(size: 6pt, fill: rgb(\"{c}\"))[{label}]])\n",
            cx = cx, y = header_y, c = color, label = label,
        ));
    }

    // Per-chirp polylines for each row.
    for i in start..end {
        let col = (i - start) as f32;
        let chirp_x_left = x0 + col * CHIRP_W_CM + 0.04 * CHIRP_W_CM;
        let chirp_x_right = x0 + (col + 1.0) * CHIRP_W_CM - 0.04 * CHIRP_W_CM;

        let signal_color =
            signal_color_for(output.symbols[i].kind, output.chirps[i].direction);
        let reference_color = "#7a7d86";
        let product_color = "#dca148";

        let signal_trace = downsample(&output.chirps[i].freq_trace, MAX_SAMPLES_PER_CHIRP);
        let reference_trace = downsample(ref_trace_full, MAX_SAMPLES_PER_CHIRP);
        let product_trace =
            product_trace_for_export(&output.chirps[i].freq_trace, ref_trace_full, inputs.bw_hz);
        let product_trace = downsample(&product_trace, MAX_SAMPLES_PER_CHIRP);

        emit_chirp_polyline(
            s,
            &signal_trace,
            inputs.bw_hz,
            chirp_x_left,
            chirp_x_right,
            sig_bot,
            sig_top,
            signal_color,
        );
        emit_chirp_polyline(
            s,
            &reference_trace,
            inputs.bw_hz,
            chirp_x_left,
            chirp_x_right,
            ref_bot,
            ref_top,
            reference_color,
        );
        emit_chirp_polyline(
            s,
            &product_trace,
            inputs.bw_hz,
            chirp_x_left,
            chirp_x_right,
            prod_bot,
            prod_top,
            product_color,
        );

        // Vertical separator between chirps (skip after the last one).
        if i + 1 < end {
            let sep_x = x0 + (col + 1.0) * CHIRP_W_CM;
            s.push_str(&format!(
                "  line(({x:.3}, {y0:.3}), ({x:.3}, {y1:.3}), stroke: 0.25pt + rgb(\"#cccccc\"))\n",
                x = sep_x, y0 = prod_bot, y1 = sig_top,
            ));
        }
    }

    // Time axis label centred under the product row.
    let t_sym_ms = sym_duration_ms(inputs);
    s.push_str(&format!(
        "  content(({cx:.3}, {y:.3}), align(center)[#text(size: 7pt, fill: rgb(\"#3070b0\"))[symbols {a} to {b} (T_sym = {t:.3} ms)]])\n",
        cx = x0 + strip_w * 0.5,
        y = prod_bot - 0.25,
        a = start, b = end - 1, t = t_sym_ms,
    ));

    s.push_str("})\n");
    s.push_str("]\n\n");
}

/// Emit a single chirp's frequency-trace polyline in CETZ. Breaks the
/// line at any sample-to-sample jump exceeding BW/2 (which corresponds
/// to a frequency wrap), so the visualization doesn't draw a vertical
/// streak across the row.
fn emit_chirp_polyline(
    s: &mut String,
    trace: &[f32],
    bw_hz: f32,
    x_left: f32,
    x_right: f32,
    y_bot: f32,
    y_top: f32,
    color_hex: &str,
) {
    if trace.len() < 2 {
        return;
    }
    let n = trace.len();
    let denom = (n - 1) as f32;
    let xy = |i: usize| -> (f32, f32) {
        let t = i as f32 / denom;
        let x = x_left + t * (x_right - x_left);
        let yn = (trace[i] / bw_hz).clamp(0.0, 1.0);
        let y = y_bot + yn * (y_top - y_bot);
        (x, y)
    };

    // Walk the trace, emitting one `line(...)` per contiguous segment.
    let mut seg_start = 0usize;
    for i in 1..n {
        let dy = (trace[i] - trace[i - 1]).abs();
        if dy > bw_hz * 0.5 {
            if i - seg_start >= 2 {
                emit_line_segment(s, &xy, seg_start, i, color_hex);
            }
            seg_start = i;
        }
    }
    if n - seg_start >= 2 {
        emit_line_segment(s, &xy, seg_start, n, color_hex);
    }
}

fn emit_line_segment<F>(s: &mut String, xy: &F, lo: usize, hi: usize, color_hex: &str)
where
    F: Fn(usize) -> (f32, f32),
{
    s.push_str("  line(");
    let mut first = true;
    for i in lo..hi {
        let (x, y) = xy(i);
        if !first {
            s.push_str(", ");
        }
        first = false;
        s.push_str(&format!("({:.3}, {:.3})", x, y));
    }
    s.push_str(&format!(", stroke: 0.7pt + rgb(\"{}\"))\n", color_hex));
}

/// Reference (conjugate of basic value-0 upchirp) frequency trace,
/// matching how the in-app visualization computes it: a linear sweep
/// from BW down to 0 over `n` samples.
fn make_reference_trace_for_export(n: usize, bw_hz: f32) -> Vec<f32> {
    if n == 0 {
        return Vec::new();
    }
    let denom = (n.saturating_sub(1)) as f32;
    (0..n)
        .map(|i| {
            let t = if denom == 0.0 { 0.0 } else { i as f32 / denom };
            (bw_hz * (1.0 - t)).clamp(0.0, bw_hz)
        })
        .collect()
}

/// Pointwise sum (modulo BW) of a signal frequency trace and the
/// reference. For an upchirp data symbol this collapses to a flat
/// line at the symbol's frequency offset.
fn product_trace_for_export(signal: &[f32], reference: &[f32], bw_hz: f32) -> Vec<f32> {
    let n = signal.len().min(reference.len());
    (0..n)
        .map(|i| (signal[i] + reference[i]).rem_euclid(bw_hz))
        .collect()
}

/// Downsample a freq_trace to at most `max_n` evenly spaced samples,
/// preserving the first and last point. If the trace is already short
/// enough, return a clone.
fn downsample(trace: &[f32], max_n: usize) -> Vec<f32> {
    if trace.len() <= max_n || trace.is_empty() {
        return trace.to_vec();
    }
    let mut out = Vec::with_capacity(max_n);
    let denom = (max_n - 1) as f32;
    let src_last = (trace.len() - 1) as f32;
    for i in 0..max_n {
        let t = i as f32 / denom;
        let idx = (t * src_last).round() as usize;
        out.push(trace[idx.min(trace.len() - 1)]);
    }
    out
}

fn symbol_kind_letter(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Preamble => "P",
        SymbolKind::Sync => "S",
        SymbolKind::Header => "H",
        SymbolKind::Payload => "D",
    }
}

/// Hex color matching the in-app per-symbol-kind palette for the
/// signal row.
fn signal_color_for(kind: SymbolKind, direction: ChirpDirection) -> &'static str {
    if direction == ChirpDirection::Down {
        return "#8a8d96";
    }
    match kind {
        SymbolKind::Preamble => "#8a8d96",
        SymbolKind::Sync => "#d8862c",
        SymbolKind::Header => "#5fa14a",
        SymbolKind::Payload => "#3a8edb",
    }
}

fn write_chirps_section(
    s: &mut String,
    inputs: &LorawanInputs,
    output: &PipelineOutput,
    audio: &AudioSettings,
) {
    s.push_str("= Step 6: Baseband chirps\n\n");
    s.push_str(
        "Each symbol modulates a linear baseband chirp that wraps modulo the \
         bandwidth. Upchirps (preamble end, header, payload) sweep low to high; \
         downchirps (preamble start, optional sync) sweep high to low.\n\n",
    );

    write_visualization_subsection(s, inputs, output);

    s.push_str("== Real-air parameters\n\n");
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, 1fr),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (right, left),\n");
    s.push_str("\x20\x20inset: 6pt,\n");
    s.push_str(&format!(
        "\x20\x20[*Symbol time $T_(\"sym\")$*], [{:.3} ms],\n",
        sym_duration_ms(inputs)
    ));
    s.push_str(&format!(
        "\x20\x20[*Bandwidth*], [{} kHz],\n",
        (inputs.bw_hz / 1000.0) as u32
    ));
    s.push_str(&format!(
        "\x20\x20[*Chirp count*], [{}],\n",
        output.chirps.len()
    ));
    let total_air_ms = output.symbols.len() as f32 * sym_duration_ms(inputs);
    s.push_str(&format!(
        "\x20\x20[*Total airtime*], [{:.1} ms ({:.3} s)],\n",
        total_air_ms,
        total_air_ms / 1000.0
    ));
    s.push_str(")\n\n");

    s.push_str("== Audio playback parameters\n\n");
    s.push_str(
        "Real LoRa chirps span hundreds of kHz, well above hearing. The app's \
         audio button rescales every chirp to a fixed audible target so playback \
         is comfortable regardless of the chosen SF/BW. The visualization keeps \
         the true LoRa numbers above.\n\n",
    );
    s.push_str("#table(\n");
    s.push_str("\x20\x20columns: (auto, 1fr),\n");
    s.push_str("\x20\x20stroke: 0.5pt + rgb(\"#cccccc\"),\n");
    s.push_str("\x20\x20align: (right, left),\n");
    s.push_str("\x20\x20inset: 6pt,\n");
    s.push_str(&format!(
        "\x20\x20[*Audible top frequency*], [{:.0} Hz],\n",
        AUDIO_TARGET_TOP_HZ
    ));
    s.push_str(&format!(
        "\x20\x20[*Audible symbol time*], [{:.0} ms],\n",
        AUDIO_TARGET_T_SYM_S * 1000.0
    ));
    s.push_str(&format!(
        "\x20\x20[*Audible duration*], [{:.2} s],\n",
        output.audio_duration_s
    ));
    s.push_str(&format!(
        "\x20\x20[*Volume*], [{} %{}],\n",
        (audio.volume * 100.0).round() as i32,
        if audio.muted { ", muted" } else { "" }
    ));
    s.push_str(")\n");
}

fn write_about_section(s: &mut String) {
    s.push_str("= About this export\n\n");
    s.push_str(
        "This document was generated by *lorawanwiz*, an interactive tool that \
         visualizes the LoRaWAN v1.0.x uplink path. The exported Typst source \
         can be compiled to PDF with `typst compile lorawanwiz_export.typ` or \
         opened directly in the Typst web app.\n\n",
    );

    s.push_str(
        "The chirp visualization uses the CeTZ drawing package, fetched \
         automatically from the Typst Universe registry on first compile. \
         No manual install is needed if Typst has network access.\n\n",
    );

    s.push_str("== What is real\n\n");
    s.push_str(
        "AES-128 in LoRaWAN's CTR-style construction with the $A_i$ block. \
         CMAC-AES-128 for the MIC over a $B_0$ prefix and the wire-format \
         message. The full frame layout. Gray-coded SF-bit symbols. Linear \
         baseband upchirps that wrap modulo BW.\n\n",
    );

    s.push_str("== What is simplified\n\n");
    s.push_str(
        "No whitening, no Hamming FEC, no interleaving, no CRC. The preamble \
         is shown as 8 downchirps of value 0; real LoRa preamble is 8 upchirps \
         followed by 2 sync symbols and a 2.25-symbol downchirp SFD. \
         LoRaWAN 1.1 splits the MIC across two network keys; this demo uses \
         a single NwkSKey (matching 1.0.x).\n\n",
    );

    s.push_str("== Credits\n\n");
    s.push_str(&format!(
        "lorawanwiz v{} · Copyright L. Weinzierl, 2026 · #link(\"https://weinzierlweb.com\")\n",
        env!("CARGO_PKG_VERSION")
    ));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn sym_duration_ms(inputs: &LorawanInputs) -> f32 {
    (1u32 << inputs.sf) as f32 / inputs.bw_hz * 1000.0
}

fn symbol_kind_label(k: SymbolKind) -> &'static str {
    match k {
        SymbolKind::Preamble => "Preamble",
        SymbolKind::Sync => "Sync",
        SymbolKind::Header => "Header",
        SymbolKind::Payload => "Payload",
    }
}

fn format_dev_addr(b: &[u8; 4]) -> String {
    format!("{:02X}{:02X}{:02X}{:02X}", b[0], b[1], b[2], b[3])
}

fn format_hex_compact(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 3);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&format!("{:02X}", b));
    }
    s
}

/// Multi-line hex block with offsets, 16 bytes per row, suitable for
/// embedding in a Typst raw block.
fn format_hex_block(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".to_string();
    }
    let mut out = String::new();
    for (row_start, chunk) in bytes.chunks(16).enumerate() {
        let offset = row_start * 16;
        out.push_str(&format!("{:04X}: ", offset));
        for (i, b) in chunk.iter().enumerate() {
            if i == 8 {
                out.push(' ');
            }
            out.push_str(&format!(" {:02X}", b));
        }
        // ASCII column
        let pad_count = 16 - chunk.len();
        for _ in 0..pad_count {
            out.push_str("   ");
        }
        if pad_count >= 8 {
            // missed the mid-column space too
            out.push(' ');
        }
        out.push_str("  |");
        for b in chunk {
            let c = *b;
            let printable = if (0x20..=0x7E).contains(&c) {
                c as char
            } else {
                '.'
            };
            out.push(printable);
        }
        out.push('|');
        out.push('\n');
    }
    // Trim final newline so the closing ``` doesn't get an extra blank
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

/// Escape a string for use inside a Typst double-quoted string literal.
fn typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Make a string safe to drop inside a Typst inline raw block:
/// `` `…` `` (single backticks). Backticks inside the content would
/// terminate the raw block, so we map them to a visually similar
/// character. Any other content is preserved verbatim, since raw
/// blocks don't interpret markup.
fn typst_raw_inline(s: &str) -> String {
    s.chars()
        .map(|c| if c == '`' { '\u{2018}' } else { c })
        .collect()
}

/// Make a string safe to drop inside a Typst triple-tick raw block.
/// Triple backticks inside the content would close the block early,
/// so any backtick run of three or more is broken up.
fn typst_safe_for_raw_block(s: &str) -> String {
    // Replace any sequence of three+ backticks by inserting a
    // zero-width space between them. The result still reads as
    // backticks but no longer terminates the raw block.
    let mut out = String::with_capacity(s.len());
    let mut run = 0usize;
    for c in s.chars() {
        if c == '`' {
            run += 1;
            if run >= 3 {
                out.push('\u{200B}'); // zero-width space
                run = 1;
            }
            out.push('`');
        } else {
            run = 0;
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Save dispatch (mirrors persistence.rs but for .typ output)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn save_to_user_chosen_path(text: String) {
    let dialog = rfd::FileDialog::new()
        .set_file_name(DEFAULT_FILENAME)
        .add_filter("Typst document", &[TYP_EXTENSION]);
    if let Some(path) = dialog.save_file() {
        if let Err(e) = std::fs::write(&path, text.as_bytes()) {
            log_warn(&format!("export: write failed: {:?}", e));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn save_to_user_chosen_path(text: String) {
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let parts = js_sys::Array::new();
    parts.push(&wasm_bindgen::JsValue::from_str(&text));

    #[allow(unused_mut)]
    let mut options = BlobPropertyBag::new();
    options.set_type("text/x-typst");

    let blob = match Blob::new_with_str_sequence_and_options(parts.as_ref(), options.as_ref()) {
        Ok(b) => b,
        Err(e) => {
            log_warn(&format!("export: Blob::new failed: {:?}", e));
            return;
        }
    };
    let url = match Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(e) => {
            log_warn(&format!("export: createObjectURL failed: {:?}", e));
            return;
        }
    };
    let document = match web_sys::window().and_then(|w| w.document()) {
        Some(d) => d,
        None => {
            log_warn("export: no document");
            return;
        }
    };
    let anchor = match document.create_element("a") {
        Ok(el) => match el.dyn_into::<HtmlAnchorElement>() {
            Ok(a) => a,
            Err(_) => {
                log_warn("export: not an HtmlAnchorElement");
                return;
            }
        },
        Err(e) => {
            log_warn(&format!("export: create anchor failed: {:?}", e));
            return;
        }
    };
    anchor.set_href(&url);
    anchor.set_download(DEFAULT_FILENAME);
    anchor.click();
    let _ = Url::revoke_object_url(&url);
}

#[cfg(not(target_arch = "wasm32"))]
fn log_warn(msg: &str) {
    eprintln!("{}", msg);
}

#[cfg(target_arch = "wasm32")]
fn log_warn(msg: &str) {
    web_sys::console::warn_1(&msg.into());
}
