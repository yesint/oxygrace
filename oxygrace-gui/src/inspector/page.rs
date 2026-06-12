//! Page-level properties, shown when nothing is selected.

use oxygrace::Project;

use crate::edit::Edit;
use crate::inspector::rows;

pub fn show(ui: &mut egui::Ui, project: &Project, edits: &mut Vec<Edit>) {
    ui.label("Page");
    ui.separator();
    rows::section(ui, "Geometry", true, None, "page_geom", |ui| {
        rows::int(
            ui,
            edits,
            "Width (px)",
            project.page_width as i32,
            16..=20000,
            "page: width",
            |p, v| p.page_width = v.max(16) as u32,
        );
        rows::int(
            ui,
            edits,
            "Height (px)",
            project.page_height as i32,
            16..=20000,
            "page: height",
            |p, v| p.page_height = v.max(16) as u32,
        );
    });
    ui.weak("Select an element in the tree or click the plot to edit it.");
}
