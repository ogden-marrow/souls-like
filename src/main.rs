mod app;
mod config;
mod gameicon;
mod games;
mod ini;
mod launch;
mod modinstall;
mod platform;
mod saves;
mod shared;

fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("Souls-like")
            .with_app_id("souls-like")
            .with_inner_size([1060.0, 740.0])
            .with_min_inner_size([860.0, 560.0])
            .with_drag_and_drop(true)
            .with_icon(window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Souls-like",
        opts,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

fn window_icon() -> eframe::egui::IconData {
    let img = image::load_from_memory(include_bytes!("../assets/icon.png"))
        .expect("icon")
        .to_rgba8();
    eframe::egui::IconData {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    }
}
