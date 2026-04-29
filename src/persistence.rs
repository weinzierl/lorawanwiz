//! Save and Load buttons. Serializes/deserializes a `SavedState` to RON.
//!
//! On native, uses the `rfd` crate for native file dialogs. The dialog
//! blocks the Bevy main thread while it is open, which is acceptable for
//! occasional save/load actions. Both save and load complete
//! synchronously inside the click handler.
//!
//! On WASM, save uses Blob + object URL + a synthetic anchor click to
//! trigger a download. Load opens an `<input type=file>` picker; the
//! actual file content arrives asynchronously via FileReader and is
//! parked in a thread-local. A separate system, `poll_pending_load`,
//! runs every frame and applies the loaded state when it shows up.

use bevy::prelude::*;

use crate::state::{
    hex_string, AudioSettings, CryptoEdit, CryptoFocus, InputsDirty, LorawanInputs, SavedState,
};
use crate::ui::{LoadButton, SaveButton};

// Used by the native rfd file-dialog filter; the WASM path uses the
// literal ".ron" via HtmlInputElement::set_accept.
#[cfg(not(target_arch = "wasm32"))]
const FILE_EXTENSION: &str = "ron";
const DEFAULT_FILENAME: &str = "lorawanwiz.ron";

pub fn handle_save_click(
    q: Query<&Interaction, (Changed<Interaction>, With<SaveButton>)>,
    inputs: Res<LorawanInputs>,
    audio: Res<AudioSettings>,
) {
    for i in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        let snapshot = SavedState::from_resources(&inputs, &audio);
        let pretty = ron::ser::PrettyConfig::new()
            .depth_limit(4)
            .indentor("  ".to_string());
        match ron::ser::to_string_pretty(&snapshot, pretty) {
            Ok(text) => save_to_user_chosen_path(text),
            Err(e) => log_warn(&format!("save: serialize failed: {:?}", e)),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn handle_load_click(
    q: Query<&Interaction, (Changed<Interaction>, With<LoadButton>)>,
    mut inputs: ResMut<LorawanInputs>,
    mut audio: ResMut<AudioSettings>,
    mut crypto: ResMut<CryptoEdit>,
    mut dirty: ResMut<InputsDirty>,
) {
    for i in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        if let Some(text) = native_load_dialog() {
            apply_loaded_text(
                &text,
                inputs.as_mut(),
                audio.as_mut(),
                crypto.as_mut(),
                dirty.as_mut(),
            );
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn handle_load_click(q: Query<&Interaction, (Changed<Interaction>, With<LoadButton>)>) {
    for i in &q {
        if *i != Interaction::Pressed {
            continue;
        }
        wasm_load::open_picker();
    }
}

fn apply_loaded_text(
    text: &str,
    inputs: &mut LorawanInputs,
    audio: &mut AudioSettings,
    crypto: &mut CryptoEdit,
    dirty: &mut InputsDirty,
) {
    match ron::from_str::<SavedState>(text) {
        Ok(state) => {
            if state.version > SavedState::CURRENT_VERSION {
                log_warn(&format!(
                    "load: file version {} is newer than supported {}; aborting",
                    state.version,
                    SavedState::CURRENT_VERSION
                ));
                return;
            }
            *inputs = state.inputs;
            *audio = state.audio;
            crypto.dev_addr_text = hex_string(&inputs.dev_addr);
            crypto.app_skey_text = hex_string(&inputs.app_skey);
            crypto.nwk_skey_text = hex_string(&inputs.nwk_skey);
            crypto.focus = CryptoFocus::None;
            dirty.0 = true;
        }
        Err(e) => log_warn(&format!("load: parse failed: {:?}", e)),
    }
}

// ---------------------------------------------------------------------------
// Native paths
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
fn save_to_user_chosen_path(text: String) {
    let dialog = rfd::FileDialog::new()
        .set_file_name(DEFAULT_FILENAME)
        .add_filter("lorawanwiz state", &[FILE_EXTENSION]);
    if let Some(path) = dialog.save_file() {
        if let Err(e) = std::fs::write(&path, text.as_bytes()) {
            log_warn(&format!("save: write failed: {:?}", e));
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn native_load_dialog() -> Option<String> {
    let dialog = rfd::FileDialog::new().add_filter("lorawanwiz state", &[FILE_EXTENSION]);
    let path = dialog.pick_file()?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) => {
            log_warn(&format!("load: read failed: {:?}", e));
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn log_warn(msg: &str) {
    eprintln!("{}", msg);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn poll_pending_load() {
    // Native is synchronous; nothing to poll.
}

// ---------------------------------------------------------------------------
// WASM paths
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
fn save_to_user_chosen_path(text: String) {
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, HtmlAnchorElement, Url};

    let parts = js_sys::Array::new();
    parts.push(&wasm_bindgen::JsValue::from_str(&text));

    // `set_type` is the current name; older web-sys had `type_`, which
    // is now deprecated. `Cargo.toml` pins web-sys to a recent 0.3
    // minor, so `set_type` is available.
    #[allow(unused_mut)]
    let mut options = BlobPropertyBag::new();
    options.set_type("application/ron");

    let blob = match Blob::new_with_str_sequence_and_options(parts.as_ref(), options.as_ref()) {
        Ok(b) => b,
        Err(e) => {
            log_warn(&format!("save: Blob::new failed: {:?}", e));
            return;
        }
    };

    let url = match Url::create_object_url_with_blob(&blob) {
        Ok(u) => u,
        Err(e) => {
            log_warn(&format!("save: createObjectURL failed: {:?}", e));
            return;
        }
    };

    let document = match web_sys::window().and_then(|w| w.document()) {
        Some(d) => d,
        None => {
            log_warn("save: no document");
            return;
        }
    };

    let anchor = match document.create_element("a") {
        Ok(el) => match el.dyn_into::<HtmlAnchorElement>() {
            Ok(a) => a,
            Err(_) => {
                log_warn("save: not an HtmlAnchorElement");
                return;
            }
        },
        Err(e) => {
            log_warn(&format!("save: create anchor failed: {:?}", e));
            return;
        }
    };
    anchor.set_href(&url);
    anchor.set_download(DEFAULT_FILENAME);
    anchor.click();
    let _ = Url::revoke_object_url(&url);
}

#[cfg(target_arch = "wasm32")]
fn log_warn(msg: &str) {
    web_sys::console::warn_1(&msg.into());
}

#[cfg(target_arch = "wasm32")]
pub fn poll_pending_load(
    mut inputs: ResMut<LorawanInputs>,
    mut audio: ResMut<AudioSettings>,
    mut crypto: ResMut<CryptoEdit>,
    mut dirty: ResMut<InputsDirty>,
) {
    if let Some(text) = wasm_load::take_pending() {
        apply_loaded_text(
            &text,
            inputs.as_mut(),
            audio.as_mut(),
            crypto.as_mut(),
            dirty.as_mut(),
        );
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm_load {
    use std::cell::RefCell;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;
    use web_sys::{FileReader, HtmlInputElement};

    thread_local! {
        static PENDING: RefCell<Option<String>> = RefCell::new(None);
        // Closures must outlive the JS callback registration; store them
        // in thread-locals so they aren't dropped when `open_picker`
        // returns.
        static OPEN_CB: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = RefCell::new(None);
        static READ_CB: RefCell<Option<Closure<dyn FnMut(web_sys::Event)>>> = RefCell::new(None);
    }

    pub fn take_pending() -> Option<String> {
        PENDING.with(|p| p.borrow_mut().take())
    }

    pub fn open_picker() {
        let document = match web_sys::window().and_then(|w| w.document()) {
            Some(d) => d,
            None => return,
        };
        let input_el = match document.create_element("input") {
            Ok(el) => el,
            Err(_) => return,
        };
        let input: HtmlInputElement = match input_el.dyn_into() {
            Ok(i) => i,
            Err(_) => return,
        };
        input.set_type("file");
        input.set_accept(".ron");

        let on_change = Closure::wrap(Box::new(move |ev: web_sys::Event| {
            let target = match ev.target() {
                Some(t) => t,
                None => return,
            };
            let input = match target.dyn_into::<HtmlInputElement>() {
                Ok(i) => i,
                Err(_) => return,
            };
            let files = match input.files() {
                Some(f) => f,
                None => return,
            };
            let file = match files.get(0) {
                Some(f) => f,
                None => return,
            };

            let reader = match FileReader::new() {
                Ok(r) => r,
                Err(_) => return,
            };
            let reader_clone = reader.clone();
            let on_read = Closure::wrap(Box::new(move |_: web_sys::Event| {
                if let Ok(result) = reader_clone.result() {
                    if let Some(text) = result.as_string() {
                        PENDING.with(|p| {
                            *p.borrow_mut() = Some(text);
                        });
                    }
                }
            }) as Box<dyn FnMut(_)>);

            reader.set_onload(Some(on_read.as_ref().unchecked_ref()));
            let _ = reader.read_as_text(&file);

            READ_CB.with(|c| {
                *c.borrow_mut() = Some(on_read);
            });
        }) as Box<dyn FnMut(_)>);

        input.set_onchange(Some(on_change.as_ref().unchecked_ref()));
        OPEN_CB.with(|c| *c.borrow_mut() = Some(on_change));
        input.click();
    }
}
