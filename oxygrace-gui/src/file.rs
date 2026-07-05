//! File open/save plumbing. All dialog calls live here: native builds use
//! rfd's blocking dialogs and real paths; the web build uses the async
//! picker (results arrive through `App::file_tx`) and saves by triggering
//! a browser download.

use std::path::PathBuf;

use crate::app::App;

#[cfg(not(target_arch = "wasm32"))]
pub fn open_dialog(app: &mut App) {
    let dialog = rfd::FileDialog::new()
        .add_filter("Plot projects", &["agr", "oxgr", "xvg", "dat"])
        .add_filter("Grace project", &["agr", "xvg", "dat"])
        .add_filter("Oxygrace project", &["oxgr"])
        .set_title("Open plot project");
    if let Some(path) = dialog.pick_file() {
        open_path(app, path);
    }
}

/// Open a file on the web by driving a hidden `<input type="file">`
/// directly: a single click opens the browser's own file picker and the
/// file loads on the `change` event — no intermediate "Ok" dialog (which
/// is what `rfd`'s web backend would impose).
#[cfg(target_arch = "wasm32")]
pub fn open_dialog(app: &mut App) {
    use eframe::wasm_bindgen::{closure::Closure, JsCast};

    let tx = app.file_tx.clone();
    let ctx = app.egui_ctx.clone();
    let document = web_sys::window().unwrap().document().unwrap();
    let input: web_sys::HtmlInputElement = document
        .create_element("input")
        .unwrap()
        .dyn_into()
        .unwrap();
    input.set_type("file");
    input.set_accept(".agr,.oxgr,.xvg,.dat");

    // The `change` handler keeps the input alive (it captures a clone), and
    // `once_into_js` keeps the closure alive until it fires once.
    let input_for_change = input.clone();
    let onchange = Closure::once_into_js(move || {
        let Some(file) = input_for_change.files().and_then(|f| f.get(0)) else {
            return;
        };
        let name = file.name();
        let buffer_promise = file.array_buffer();
        wasm_bindgen_futures::spawn_local(async move {
            let Ok(buffer) = wasm_bindgen_futures::JsFuture::from(buffer_promise).await else {
                return;
            };
            let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
            // Same lenient decoding as oxygrace::load: UTF-8, else Latin-1.
            let content = match String::from_utf8(bytes) {
                Ok(s) => s,
                Err(e) => e.as_bytes().iter().map(|&b| b as char).collect(),
            };
            let _ = tx.send((name, content));
            ctx.request_repaint();
        });
    });
    input.set_onchange(Some(onchange.unchecked_ref()));
    // A detached input still opens the native picker on click (modern
    // browsers), and stays alive via the closure's captured clone.
    input.click();
}

/// Install a file delivered by the async picker (web), dispatching on the
/// extension like the native `oxygrace::load`.
#[cfg(target_arch = "wasm32")]
pub fn open_loaded(app: &mut App, name: String, content: String) {
    let project = if name.to_ascii_lowercase().ends_with(".oxgr") {
        match oxygrace::format::load_oxgr_str(&content) {
            Ok(p) => p,
            Err(e) => {
                app.status = format!("Failed to open {name}: {e}");
                return;
            }
        }
    } else {
        oxygrace::load_str(&content)
    };
    app.status = format!(
        "Loaded {name} ({} graph{})",
        project.graphs.len(),
        if project.graphs.len() == 1 { "" } else { "s" }
    );
    app.open_project(project, Some(PathBuf::from(name)));
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_path(app: &mut App, path: PathBuf) {
    match oxygrace::load(&path) {
        Ok(project) => {
            app.status = format!(
                "Loaded {} ({} graph{})",
                path.display(),
                project.graphs.len(),
                if project.graphs.len() == 1 { "" } else { "s" }
            );
            app.open_project(project, Some(path));
        }
        Err(e) => {
            app.status = format!("Failed to open {}: {e}", path.display());
        }
    }
}

/// Save to the current path, or fall through to Save As.
#[cfg(not(target_arch = "wasm32"))]
pub fn save(app: &mut App) {
    if app.project.is_none() {
        return;
    }
    match &app.path {
        Some(path) => write_to(app, path.clone()),
        None => save_as(app),
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_as(app: &mut App) {
    if app.project.is_none() {
        return;
    }
    // The extension picks the format (core `save` dispatches): .agr stays
    // the default; choosing .oxgr writes the native format.
    let mut dialog = rfd::FileDialog::new()
        .add_filter("Grace project", &["agr"])
        .add_filter("Oxygrace project", &["oxgr"])
        .set_title("Save plot project");
    if let Some(parent) = app.path.as_ref().and_then(|p| p.parent()) {
        dialog = dialog.set_directory(parent);
    }
    if let Some(name) = app.path.as_ref().and_then(|p| p.file_name()) {
        dialog = dialog.set_file_name(name.to_string_lossy());
    }
    if let Some(path) = dialog.save_file() {
        write_to(app, path);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn write_to(app: &mut App, path: PathBuf) {
    // Free aspect is a view mode: persist the un-stretched geometry.
    let Some(project) = app.project_for_save() else { return };
    let result = oxygrace::save(&project, &path);
    drop(project);
    match result {
        Ok(()) => {
            app.status = format!("Saved {}", path.display());
            app.path = Some(path);
            app.modified = false;
        }
        Err(e) => {
            app.status = format!("Failed to save {}: {e}", path.display());
        }
    }
}

/// The web has no real save dialog: saving downloads the `.agr` file.
#[cfg(target_arch = "wasm32")]
pub fn save(app: &mut App) {
    save_as(app);
}

#[cfg(target_arch = "wasm32")]
pub fn save_as(app: &mut App) {
    // Free aspect is a view mode: persist the un-stretched geometry.
    let Some(project) = app.project_for_save() else { return };
    let content = oxygrace::save_str(&project);
    drop(project);
    let name = app
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled.agr".into());
    match download(&name, &content) {
        Ok(()) => {
            app.status = format!("Downloaded {name}");
            app.modified = false;
        }
        Err(e) => app.status = format!("Download failed: {e:?}"),
    }
}

/// Trigger a browser download of a text file via a temporary object URL.
#[cfg(target_arch = "wasm32")]
fn download(filename: &str, content: &str) -> Result<(), eframe::wasm_bindgen::JsValue> {
    use eframe::wasm_bindgen::JsCast as _;
    let document = web_sys::window()
        .ok_or("no window")?
        .document()
        .ok_or("no document")?;
    let parts = js_sys::Array::of1(&content.into());
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/plain");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)?;
    let anchor: web_sys::HtmlAnchorElement =
        document.create_element("a")?.dyn_into()?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    web_sys::Url::revoke_object_url(&url)?;
    Ok(())
}
