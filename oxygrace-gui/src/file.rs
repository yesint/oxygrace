//! File open/save plumbing. All dialog calls live here: native builds use
//! rfd's blocking dialogs and real paths; the web build uses the async
//! picker (results arrive through `App::file_tx`) and saves by triggering
//! a browser download.

use std::path::PathBuf;

use crate::app::App;

#[cfg(not(target_arch = "wasm32"))]
pub fn open_dialog(app: &mut App) {
    let dialog = rfd::FileDialog::new()
        .add_filter("Grace project", &["agr", "xvg", "dat"])
        .set_title("Open Grace project");
    if let Some(path) = dialog.pick_file() {
        open_path(app, path);
    }
}

#[cfg(target_arch = "wasm32")]
pub fn open_dialog(app: &mut App) {
    let tx = app.file_tx.clone();
    let ctx = app.egui_ctx.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let Some(file) = rfd::AsyncFileDialog::new()
            .add_filter("Grace project", &["agr", "xvg", "dat"])
            .pick_file()
            .await
        else {
            return;
        };
        let bytes = file.read().await;
        // Same lenient decoding as oxygrace::load: UTF-8, else Latin-1.
        let content = match String::from_utf8(bytes) {
            Ok(s) => s,
            Err(e) => e.as_bytes().iter().map(|&b| b as char).collect(),
        };
        let _ = tx.send((file.file_name(), content));
        ctx.request_repaint();
    });
}

/// Install a file delivered by the async picker (web).
#[cfg(target_arch = "wasm32")]
pub fn open_loaded(app: &mut App, name: String, content: String) {
    let project = oxygrace::load_str(&content);
    app.status = format!(
        "Loaded {name} ({} graph{})",
        project.graphs.len(),
        if project.graphs.len() == 1 { "" } else { "s" }
    );
    app.project = Some(project);
    app.path = Some(PathBuf::from(name));
    app.selection = None;
    app.dirty = true;
    app.modified = false;
    app.undo.clear();
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
            app.project = Some(project);
            app.path = Some(path);
            app.selection = None;
            app.dirty = true;
            app.modified = false;
            app.undo.clear();
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
    let mut dialog = rfd::FileDialog::new()
        .add_filter("Grace project", &["agr"])
        .set_title("Save Grace project");
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
    let Some(project) = &app.project else { return };
    match oxygrace::save(project, &path) {
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
    let Some(project) = &app.project else { return };
    let name = app
        .path
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "untitled.agr".into());
    let content = oxygrace::save_str(project);
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
