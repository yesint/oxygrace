//! The model → texture loop: re-render the plot only when the model changed
//! (egui's reactive repaint keeps idle frames free of any work).

use crate::app::App;

pub fn refresh_texture(app: &mut App, ctx: &egui::Context) {
    if !app.dirty {
        return;
    }
    app.dirty = false;
    let Some(project) = &app.project else { return };
    let res = oxygrace::render_pixmap(project, &app.fonts);
    let size = [res.pixmap.width() as usize, res.pixmap.height() as usize];
    // tiny-skia pixmaps are premultiplied RGBA — use the matching constructor.
    let image = egui::ColorImage::from_rgba_premultiplied(size, res.pixmap.data());
    match &mut app.texture {
        Some(t) => t.set(image, egui::TextureOptions::LINEAR),
        None => app.texture = Some(ctx.load_texture("plot", image, egui::TextureOptions::LINEAR)),
    }
    app.page_size = (res.pixmap.width(), res.pixmap.height());
    app.render_info = Some(res.info);
}
