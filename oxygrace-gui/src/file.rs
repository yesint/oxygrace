//! File open/save plumbing. All dialog calls live here so the wasm build
//! (G5) can swap in async dialogs by replacing this module only.

use std::path::PathBuf;

use crate::app::App;

pub fn open_dialog(app: &mut App) {
    let dialog = rfd::FileDialog::new()
        .add_filter("Grace project", &["agr", "xvg", "dat"])
        .set_title("Open Grace project");
    if let Some(path) = dialog.pick_file() {
        open_path(app, path);
    }
}

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
pub fn save(app: &mut App) {
    if app.project.is_none() {
        return;
    }
    match &app.path {
        Some(path) => write_to(app, path.clone()),
        None => save_as(app),
    }
}

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
