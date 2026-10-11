//! The Souls-like window: sidebar of games, per-game page (play, mods, saves, backups),
//! mod installer (drag and drop), shared co-op settings and appearance.

use crate::{
    config::{Config, Mode},
    games::{GAMES, Game},
    ini, launch, modinstall, platform, saves, shared,
};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText, Sense,
    Stroke, StrokeKind, Ui, vec2,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, Sender, channel},
    time::{Duration, Instant},
};

// ---------------------------------------------------------------- palette
#[derive(Clone, Copy)]
struct Pal {
    bg: Color32,
    side: Color32,
    card: Color32,
    line: Color32,
    text: Color32,
    dim: Color32,
    accent: Color32,
    field: Color32,
    ok: Color32,
    warn: Color32,
}

fn pal_for(dark: bool) -> Pal {
    if dark {
        Pal {
            bg: Color32::from_rgb(0x1c, 0x1c, 0x1e),
            side: Color32::from_rgb(0x25, 0x25, 0x28),
            card: Color32::from_rgb(0x2a, 0x2a, 0x2d),
            line: Color32::from_white_alpha(18),
            text: Color32::from_rgb(0xf2, 0xf2, 0xf7),
            dim: Color32::from_rgb(0x8e, 0x8e, 0x93),
            accent: Color32::from_rgb(0x0a, 0x84, 0xff),
            field: Color32::from_rgb(0x3a, 0x3a, 0x3e),
            ok: Color32::from_rgb(0x32, 0xd7, 0x4b),
            warn: Color32::from_rgb(0xff, 0xb3, 0x40),
        }
    } else {
        Pal {
            bg: Color32::from_rgb(0xf6, 0xf6, 0xf7),
            side: Color32::from_rgb(0xe9, 0xe9, 0xec),
            card: Color32::WHITE,
            line: Color32::from_black_alpha(20),
            text: Color32::from_rgb(0x1d, 0x1d, 0x1f),
            dim: Color32::from_rgb(0x6e, 0x6e, 0x73),
            accent: Color32::from_rgb(0x0a, 0x7a, 0xff),
            field: Color32::from_rgb(0xe6, 0xe6, 0xe9),
            ok: Color32::from_rgb(0x30, 0xb8, 0x4a),
            warn: Color32::from_rgb(0xb2, 0x5e, 0x00),
        }
    }
}

fn pal(ui: &Ui) -> Pal {
    pal_for(ui.visuals().dark_mode)
}

fn install_style(ctx: &egui::Context) {
    install_fonts(ctx);
    for (theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        let p = pal_for(dark);
        let mut v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.panel_fill = p.bg;
        v.window_fill = p.card;
        v.extreme_bg_color = p.field;
        v.faint_bg_color = p.card;
        v.override_text_color = Some(p.text);
        v.selection.bg_fill = p.accent;
        v.hyperlink_color = p.accent;
        v.window_corner_radius = CornerRadius::same(14);
        for w in [
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
        ] {
            w.corner_radius = CornerRadius::same(7);
        }
        v.widgets.inactive.weak_bg_fill = p.field;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.hovered.weak_bg_fill = p.field.gamma_multiply(1.15);
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        ctx.set_visuals_of(theme, v);
    }
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 5.0);
    });
}

fn bold() -> egui::FontFamily {
    egui::FontFamily::Name("bold".into())
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSans-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "noto-bold".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/NotoSans-Bold.ttf"
        ))),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "noto".into());
    let mut b = vec!["noto-bold".to_string()];
    b.extend(fonts.families[&egui::FontFamily::Proportional].clone());
    fonts.families.insert(bold(), b);
    ctx.set_fonts(fonts);
}

fn theme_pref(s: &str) -> egui::ThemePreference {
    match s {
        "light" => egui::ThemePreference::Light,
        "dark" => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    }
}

// ---------------------------------------------------------------- widgets
fn card<R>(ui: &mut Ui, p: &Pal, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(p.card)
        .corner_radius(10)
        .stroke(Stroke::new(1.0, p.line))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui)
        })
        .inner
}

fn row(ui: &mut Ui, p: &Pal, first: bool, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    if !first {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(r, 0, p.line);
    }
    Frame::new()
        .inner_margin(Margin::symmetric(16, 11))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.vertical(left);
                ui.with_layout(Layout::right_to_left(Align::Center), right);
            });
        });
}

fn title_sub(ui: &mut Ui, p: &Pal, title: &str, sub: &str) {
    ui.label(RichText::new(title).family(bold()));
    if !sub.is_empty() {
        ui.label(RichText::new(sub).size(12.0).color(p.dim));
    }
}

fn section(ui: &mut Ui, p: &Pal, text: &str) {
    ui.label(RichText::new(text).size(11.0).family(bold()).color(p.dim));
}

fn section_row(ui: &mut Ui, p: &Pal, text: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.add_space(5.0);
            section(ui, p, text);
        });
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

fn pill(ui: &mut Ui, text: &str, color: Color32) {
    Frame::new()
        .fill(color.gamma_multiply(0.18))
        .corner_radius(99)
        .inner_margin(Margin::symmetric(9, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).family(bold()).color(color));
        });
}

fn toggle(ui: &mut Ui, p: &Pal, on: &mut bool) -> egui::Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(40.0, 24.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = p.field.lerp_to_gamma(p.ok, t);
    ui.painter().rect_filled(rect, 12, bg);
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, t);
    ui.painter()
        .circle_filled(egui::pos2(x, rect.center().y), 10.0, Color32::WHITE);
    resp
}

fn segmented(ui: &mut Ui, p: &Pal, labels: &[&str], sel: usize) -> Option<usize> {
    let mut out = None;
    Frame::new()
        .fill(p.field)
        .corner_radius(8)
        .inner_margin(2)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let bw =
                labels.iter().map(|l| l.chars().count()).max().unwrap_or(4) as f32 * 8.2 + 34.0;
            let size = vec2(
                labels.len() as f32 * bw + (labels.len() as f32 - 1.0) * 2.0 + 2.0,
                26.0,
            );
            ui.allocate_ui_with_layout(size, Layout::left_to_right(Align::Center), |ui| {
                for (i, l) in labels.iter().enumerate() {
                    let on = i == sel;
                    let b = egui::Button::new(RichText::new(*l).family(bold()).color(if on {
                        p.text
                    } else {
                        p.dim
                    }))
                    .fill(if on { p.card } else { Color32::TRANSPARENT })
                    .stroke(Stroke::NONE)
                    .corner_radius(6)
                    .min_size(vec2(bw, 26.0));
                    if ui.add(b).clicked() && !on {
                        out = Some(i);
                    }
                }
            });
        });
    out
}

fn ext_color(p: &Pal, ext: &str) -> Color32 {
    match ext {
        "sl2" => Color32::from_rgb(0x6b, 0x82, 0xff),
        "va2" => p.ok,
        "co2" => p.warn,
        _ => p.dim,
    }
}

fn ext_badge(ui: &mut Ui, p: &Pal, ext: &str) {
    let c = ext_color(p, ext);
    Frame::new()
        .fill(c.gamma_multiply(0.18))
        .corner_radius(5)
        .inner_margin(Margin::symmetric(7, 2))
        .show(ui, |ui| {
            ui.label(
                RichText::new(format!(".{ext}"))
                    .monospace()
                    .size(12.0)
                    .color(c),
            );
        });
}

fn small_button(ui: &mut Ui, text: &str) -> bool {
    ui.add(egui::Button::new(RichText::new(text).size(12.0)).min_size(vec2(0.0, 24.0)))
        .clicked()
}

fn roman(i: usize) -> &'static str {
    ["I", "II", "III"][i]
}

fn game_color(g: &Game) -> Color32 {
    Color32::from_rgb(g.accent[0], g.accent[1], g.accent[2])
}

fn human_size(b: u64) -> String {
    format!("{:.1} MB", b as f64 / 1_048_576.0)
}

// ---------------------------------------------------------------- state
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Game(usize),
    Mods,
    Shared,
    Settings,
}

#[derive(Default)]
struct View {
    loc: Option<platform::Located>,
    save_base: Option<PathBuf>,
    save_root: Option<PathBuf>,
    saves: Vec<saves::SaveFile>,
    /// One line per save listing its characters (empty when the game's layout isn't supported).
    chars: Vec<String>,
    backups: Vec<saves::Backup>,
    running: bool,
    seamless: bool,
}

enum Dialog {
    CopyOver {
        gi: usize,
        src: PathBuf,
        target: usize,
        new_ext: String,
    },
    Restore {
        gi: usize,
        bi: usize,
        fi: usize,
    },
    Clean {
        gi: usize,
        files: Vec<PathBuf>,
    },
    CopyChar {
        gi: usize,
        src: PathBuf,
        src_ext: String,
        src_slots: Vec<(usize, String)>,
        src_pick: usize,
        dst_idx: usize,
        dst_loaded: Option<usize>,
        dst_slots: Vec<Option<String>>,
        dst_slot: usize,
    },
}

struct Pending {
    path: PathBuf,
    plan: Result<modinstall::Plan, String>,
}

pub struct App {
    cfg: Config,
    page: Page,
    views: Vec<View>,
    last_refresh: Instant,
    dialog: Option<Dialog>,
    pending: Vec<Pending>,
    toast: Option<(String, Instant, bool)>,
    tx: Sender<(usize, Result<String, String>)>,
    rx: Receiver<(usize, Result<String, String>)>,
    launching: [bool; 3],
    pw: [String; 3],
    shared_vals: BTreeMap<String, String>,
    ctx: egui::Context,
    icons: [Option<egui::TextureHandle>; 3],
    icon_tried: [bool; 3],
    char_cache: std::collections::HashMap<PathBuf, (std::time::SystemTime, u64, String)>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_style(&cc.egui_ctx);
        Self::with_ctx(&cc.egui_ctx)
    }

    pub fn with_ctx(ctx: &egui::Context) -> Self {
        let cfg = Config::load();
        ctx.set_theme(theme_pref(&cfg.theme));
        let (tx, rx) = channel();
        let page = GAMES
            .iter()
            .position(|g| g.key == cfg.last)
            .map(Page::Game)
            .unwrap_or(Page::Game(1));
        let mut app = App {
            cfg,
            page,
            views: (0..3).map(|_| View::default()).collect(),
            last_refresh: Instant::now(),
            dialog: None,
            pending: vec![],
            toast: None,
            tx,
            rx,
            launching: [false; 3],
            pw: Default::default(),
            shared_vals: BTreeMap::new(),
            ctx: ctx.clone(),
            icons: Default::default(),
            icon_tried: [false; 3],
            char_cache: Default::default(),
        };
        app.refresh_all();
        app.load_texts();
        app
    }

    fn say(&mut self, msg: impl Into<String>, err: bool) {
        self.toast = Some((msg.into(), Instant::now(), err));
    }

    fn refresh(&mut self, gi: usize) {
        let g = &GAMES[gi];
        let st = self.cfg.game(g.key).clone();
        let loc = platform::locate(g, &st.game_dir);
        let base = platform::save_base_dir(g, loc.as_ref(), &st.save_dir);
        let root = base.as_ref().and_then(|b| saves::find_root(g, b));
        let list = root.as_ref().map(|r| saves::list(g, r)).unwrap_or_default();
        let chars: Vec<String> = list.iter().map(|f| self.characters_line(g, f)).collect();
        self.views[gi] = View {
            chars,
            seamless: loc
                .as_ref()
                .map(|l| launch::has_seamless(g, &l.game_dir))
                .unwrap_or(false),
            running: platform::game_running(g),
            backups: saves::list_backups(g, 12),
            saves: list,
            save_root: root,
            save_base: base,
            loc,
        };
    }

    /// "Characters: A · B" for a save, cached until the file changes.
    fn characters_line(&mut self, g: &Game, f: &saves::SaveFile) -> String {
        if g.slots.is_none() {
            return String::new();
        }
        if let Some((m, sz, text)) = self.char_cache.get(&f.path) {
            if *m == f.modified && *sz == f.size {
                return text.clone();
            }
        }
        let text = match crate::slots::read(g, &f.path) {
            Ok(Some(v)) if v.is_empty() => "No characters".to_string(),
            Ok(Some(v)) => format!(
                "Characters: {}",
                v.iter()
                    .map(|s| s.name.clone())
                    .collect::<Vec<_>>()
                    .join(" · ")
            ),
            Ok(None) => String::new(),
            Err(_) => "Characters: couldn't read this save".to_string(),
        };
        self.char_cache
            .insert(f.path.clone(), (f.modified, f.size, text.clone()));
        text
    }

    fn ensure_icon(&mut self, gi: usize) {
        if self.icons[gi].is_some() || self.icon_tried[gi] {
            return;
        }
        let Some(loc) = self.views[gi].loc.as_ref() else {
            return;
        };
        self.icon_tried[gi] = true;
        let g = &GAMES[gi];
        if let Some(img) = crate::gameicon::load(g.key, &loc.game_dir.join(g.proc_name)) {
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [img.width() as usize, img.height() as usize],
                img.as_raw(),
            );
            self.icons[gi] = Some(self.ctx.load_texture(
                format!("icon-{}", g.key),
                ci,
                egui::TextureOptions::LINEAR,
            ));
        }
    }

    fn refresh_all(&mut self) {
        for gi in 0..3 {
            self.refresh(gi);
            self.ensure_icon(gi);
        }
        self.last_refresh = Instant::now();
    }

    fn targets(&self) -> Vec<(usize, PathBuf)> {
        (0..3)
            .filter(|&i| self.views[i].seamless)
            .filter_map(|i| self.views[i].loc.as_ref().map(|l| (i, l.game_dir.clone())))
            .collect()
    }

    /// Load text fields and shared values from the settings files.
    fn load_texts(&mut self) {
        let ts = self.targets();
        let refs: Vec<(&Game, &Path)> = ts.iter().map(|(i, d)| (&GAMES[*i], d.as_path())).collect();
        for gi in 0..3 {
            let g = &GAMES[gi];
            self.pw[gi] = self.views[gi]
                .loc
                .as_ref()
                .and_then(|l| ini::get(&launch::settings_path(g, &l.game_dir), "cooppassword"))
                .unwrap_or_default();
        }
        self.shared_vals.clear();
        for s in shared::SETTINGS {
            let v = self
                .cfg
                .shared
                .get(s.key)
                .cloned()
                .or_else(|| shared::read(s.key, &refs));
            if let Some(v) = v {
                self.shared_vals.insert(s.key.to_string(), v);
            }
        }
    }

    fn set_page(&mut self, page: Page) {
        self.page = page;
        if let Page::Game(i) = page {
            self.cfg.last = GAMES[i].key.to_string();
            self.cfg.save();
        }
        self.refresh_all();
        self.load_texts();
    }

    fn mirrored_for_launch(&self) -> BTreeMap<String, String> {
        if !self.cfg.mirror {
            return BTreeMap::new();
        }
        self.cfg
            .shared
            .iter()
            .filter(|(k, _)| !self.cfg.unsynced.contains(*k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    fn start(&mut self, gi: usize) {
        if self.launching[gi] || self.views[gi].running {
            return;
        }
        let g = &GAMES[gi];
        let Some(loc) = self.views[gi].loc.clone() else {
            return self.say("Game not found in the Steam library.", true);
        };
        let st = self.cfg.game(g.key).clone();
        let base = self.views[gi].save_base.clone();
        let mirrored = self.mirrored_for_launch();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        self.launching[gi] = true;
        std::thread::spawn(move || {
            let r = launch::launch(g, &loc, &st, base.as_deref(), &mirrored);
            let _ = tx.send((gi, r));
            ctx.request_repaint();
        });
    }

    fn restore_root(&self, gi: usize) -> Option<PathBuf> {
        let v = &self.views[gi];
        v.save_root.clone().or_else(|| {
            let base = v.save_base.as_ref()?;
            let mut subs: Vec<PathBuf> = std::fs::read_dir(base)
                .ok()?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            subs.sort();
            subs.into_iter().next()
        })
    }

    // ------------------------------------------------------------ sidebar
    fn nav_game(&mut self, ui: &mut Ui, gi: usize, p: &Pal) {
        let g = &GAMES[gi];
        let sel = self.page == Page::Game(gi);
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click());
        let painter = ui.painter();
        if sel {
            painter.rect_filled(rect, 8, p.accent);
        } else if resp.hovered() {
            painter.rect_filled(rect, 8, p.line);
        }
        let badge = egui::Rect::from_min_size(rect.min + vec2(8.0, 9.0), vec2(28.0, 28.0));
        if let Some(tex) = &self.icons[gi] {
            egui::Image::new(tex)
                .fit_to_exact_size(badge.size())
                .corner_radius(7)
                .paint_at(ui, badge);
        } else {
            painter.rect_filled(
                badge,
                7,
                if sel {
                    Color32::from_white_alpha(56)
                } else {
                    game_color(g)
                },
            );
            painter.text(
                badge.center(),
                Align2::CENTER_CENTER,
                roman(gi),
                FontId::proportional(13.0),
                Color32::WHITE,
            );
        }
        let v = &self.views[gi];
        let sub = if v.running {
            "● Running".to_string()
        } else if v.loc.is_none() {
            "Not installed".to_string()
        } else if g.sub.is_empty() {
            "Ready".to_string()
        } else {
            g.sub.to_string()
        };
        let (tc, sc) = if sel {
            (Color32::WHITE, Color32::from_white_alpha(200))
        } else {
            (p.text, p.dim)
        };
        painter.text(
            rect.min + vec2(46.0, 8.0),
            Align2::LEFT_TOP,
            format!("{} {}", g.name, ""),
            FontId::proportional(13.5),
            tc,
        );
        painter.text(
            rect.min + vec2(46.0, 26.0),
            Align2::LEFT_TOP,
            sub,
            FontId::proportional(11.0),
            sc,
        );
        if resp.clicked() {
            self.set_page(Page::Game(gi));
        }
    }

    fn nav_item(&mut self, ui: &mut Ui, p: &Pal, label: &str, target: Page) {
        let sel = self.page == target;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
        if sel {
            ui.painter().rect_filled(rect, 8, p.accent);
        } else if resp.hovered() {
            ui.painter().rect_filled(rect, 8, p.line);
        }
        ui.painter().text(
            rect.min + vec2(12.0, 8.0),
            Align2::LEFT_TOP,
            label,
            FontId::proportional(13.5),
            if sel { Color32::WHITE } else { p.text },
        );
        if resp.clicked() {
            self.set_page(target);
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let p = pal(ui);
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.add_space(6.0);
        ui.label(RichText::new("Souls-like").size(17.0).family(bold()));
        ui.add_space(10.0);
        section(ui, &p, "GAMES");
        for gi in 0..3 {
            self.nav_game(ui, gi, &p);
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            self.nav_item(ui, &p, "Settings", Page::Settings);
            self.nav_item(ui, &p, "Shared co-op settings", Page::Shared);
            self.nav_item(ui, &p, "Mods", Page::Mods);
        });
    }

    // ------------------------------------------------------------ game page
    fn game_page(&mut self, ui: &mut Ui, gi: usize) {
        let g = &GAMES[gi];
        let p = pal(ui);
        let st = self.cfg.game(g.key).clone();
        let installed = self.views[gi].loc.is_some();
        let running = self.views[gi].running;
        let seamless_ok = self.views[gi].seamless;
        let want_seamless = st.mode == Mode::Seamless;
        let launching = self.launching[gi];

        // header
        ui.horizontal(|ui| {
            if let Some(tex) = &self.icons[gi] {
                ui.add(
                    egui::Image::new(tex)
                        .fit_to_exact_size(vec2(56.0, 56.0))
                        .corner_radius(13),
                );
                ui.add_space(6.0);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(RichText::new(g.name).size(30.0).family(bold()));
                ui.horizontal(|ui| {
                    if !g.sub.is_empty() {
                        ui.label(RichText::new(g.sub).color(p.dim));
                    }
                    if running {
                        pill(ui, "Running", p.ok);
                    } else if !installed {
                        pill(ui, "Not installed", p.warn);
                    } else {
                        pill(ui, "Ready", p.ok);
                    }
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let can = installed && !running && !launching && (!want_seamless || seamless_ok);
                let label = if running {
                    "Running…"
                } else if launching {
                    "Starting…"
                } else {
                    "Play"
                };
                let b = egui::Button::new(
                    RichText::new(label)
                        .size(16.0)
                        .family(bold())
                        .color(Color32::WHITE),
                )
                .fill(p.accent)
                .corner_radius(99)
                .min_size(vec2(130.0, 40.0));
                if ui.add_enabled(can, b).clicked() {
                    self.start(gi);
                }
            });
        });
        ui.add_space(16.0);

        // mode / mods card
        card(ui, &p, |ui| {
            let mode_sub = if want_seamless {
                "Mods on, separate save file"
            } else {
                "Normal online game, every mod switched off"
            };
            let mut chosen = None;
            row(
                ui,
                &p,
                true,
                |ui| title_sub(ui, &p, "Mode", mode_sub),
                |ui| {
                    chosen = segmented(
                        ui,
                        &p,
                        &["Vanilla", "Seamless Co-op"],
                        if want_seamless { 1 } else { 0 },
                    );
                },
            );
            if let Some(i) = chosen {
                let s = self.cfg.game(g.key);
                s.mode = if i == 0 {
                    Mode::Vanilla
                } else {
                    Mode::Seamless
                };
                self.cfg.save();
            }
            if want_seamless && seamless_ok {
                for m in g.mods {
                    let mut on = st.mods.get(m.id).copied().unwrap_or(false);
                    let mut changed = false;
                    row(
                        ui,
                        &p,
                        false,
                        |ui| title_sub(ui, &p, m.title, m.sub),
                        |ui| {
                            changed = toggle(ui, &p, &mut on).changed();
                        },
                    );
                    if changed {
                        self.cfg.game(g.key).mods.insert(m.id.to_string(), on);
                        self.cfg.save();
                    }
                }
                let mut pw_done = false;
                row(
                    ui,
                    &p,
                    false,
                    |ui| {
                        title_sub(
                            ui,
                            &p,
                            "Co-op password",
                            "Friends need the same one to join",
                        )
                    },
                    |ui| {
                        let r = ui
                            .add(egui::TextEdit::singleline(&mut self.pw[gi]).desired_width(190.0));
                        pw_done = r.lost_focus();
                    },
                );
                if pw_done {
                    let v = self.pw[gi].clone();
                    self.write_shared("cooppassword", &v, Some(gi));
                }
                row(
                    ui,
                    &p,
                    false,
                    |ui| {
                        title_sub(
                            ui,
                            &p,
                            "Co-op settings file",
                            "Scaling, invaders, ping display and more",
                        )
                    },
                    |ui| {
                        if small_button(ui, "Open file") {
                            if let Some(l) = &self.views[gi].loc {
                                platform::open_path(&launch::settings_path(g, &l.game_dir));
                            }
                        }
                    },
                );
            }
        });

        ui.add_space(6.0);
        if !installed {
            ui.label(RichText::new("This game isn't in your Steam library. Install it from Steam, or set its folder under Settings.").color(p.warn));
        } else if want_seamless && !seamless_ok {
            card(ui, &p, |ui| {
                row(
                    ui,
                    &p,
                    true,
                    |ui| {
                        ui.label(
                            RichText::new("Seamless Co-op isn't installed for this game")
                                .family(bold())
                                .color(p.warn),
                        );
                        ui.label(RichText::new("Drop the mod's zip anywhere in this window and it installs automatically.").size(12.0).color(p.dim));
                    },
                    |ui| {
                        if small_button(ui, "Open Mods") {
                            self.page = Page::Mods;
                        }
                    },
                );
            });
        } else if want_seamless {
            let ext = launch::expected_ext(g, &self.views[gi].loc.as_ref().unwrap().game_dir, &st);
            ui.label(RichText::new(format!("Starts with Seamless Co-op using save .{ext}. Your normal save is never touched, and every launch backs up first.")).size(12.0).color(p.dim));
        } else {
            ui.label(RichText::new("Starts the plain game through Steam with the mod loader off. Uses your normal .sl2 save.").size(12.0).color(p.dim));
        }
        ui.add_space(18.0);

        // saves
        section_row(ui, &p, "SAVES", |ui| {
            if small_button(ui, "Backups folder") {
                platform::open_path(&saves::backup_root(g));
            }
            if small_button(ui, "Open folder") {
                if let Some(r) = self.views[gi]
                    .save_root
                    .clone()
                    .or(self.views[gi].save_base.clone())
                {
                    platform::open_path(&r);
                }
            }
            if small_button(ui, "Back up now") {
                match self.views[gi].save_root.clone() {
                    Some(r) => match saves::backup(g, &r, "manual") {
                        Ok(Some(n)) => self.say(format!("Backed up to {n}"), false),
                        Ok(None) => self.say("No saves to back up", true),
                        Err(e) => self.say(format!("Backup failed: {e}"), true),
                    },
                    None => self.say("No saves yet", true),
                }
                self.refresh(gi);
            }
        });
        let list: Vec<(PathBuf, String, u64, std::time::SystemTime, String)> = self.views[gi]
            .saves
            .iter()
            .zip(self.views[gi].chars.iter())
            .map(|(s, c)| (s.path.clone(), s.ext.clone(), s.size, s.modified, c.clone()))
            .collect();
        card(ui, &p, |ui| {
            if list.is_empty() {
                row(
                    ui,
                    &p,
                    true,
                    |ui| {
                        title_sub(
                            ui,
                            &p,
                            "No saves yet",
                            "Launch the game once, or restore from a backup below.",
                        )
                    },
                    |_| {},
                );
            }
            for (i, (path, ext, size, modified, chars)) in list.iter().enumerate() {
                row(
                    ui,
                    &p,
                    i == 0,
                    |ui| {
                        ui.horizontal(|ui| {
                            ext_badge(ui, &p, ext);
                            ui.label(RichText::new(g.label(ext)).family(bold()));
                        });
                        ui.label(
                            RichText::new(format!(
                                "{} · last played {}",
                                human_size(*size),
                                saves::fmt_time(*modified)
                            ))
                            .size(12.0)
                            .color(p.dim),
                        );
                        if !chars.is_empty() {
                            ui.label(RichText::new(chars).size(12.0).color(p.text));
                        }
                    },
                    |ui| {
                        if small_button(ui, "Export") {
                            let dest = dirs::document_dir().unwrap_or_default().join(format!(
                                "{}-{}",
                                g.key,
                                path.file_name().unwrap().to_string_lossy()
                            ));
                            match std::fs::copy(path, &dest) {
                                Ok(_) => self.say(format!("Exported to {}", dest.display()), false),
                                Err(e) => self.say(format!("Export failed: {e}"), true),
                            }
                        }
                        if crate::slots::can_copy(g) && small_button(ui, "Copy character…") {
                            match crate::slots::read(g, path) {
                                Ok(Some(chars)) if !chars.is_empty() => {
                                    let dst_idx =
                                        list.iter().position(|l| l.0 != *path).unwrap_or(0);
                                    self.dialog = Some(Dialog::CopyChar {
                                        gi,
                                        src: path.clone(),
                                        src_ext: ext.clone(),
                                        src_slots: chars
                                            .into_iter()
                                            .map(|c| (c.index, c.name))
                                            .collect(),
                                        src_pick: 0,
                                        dst_idx,
                                        dst_loaded: None,
                                        dst_slots: vec![],
                                        dst_slot: 0,
                                    });
                                }
                                Ok(_) => self.say("This save has no characters to copy.", true),
                                Err(e) => self.say(format!("Could not read this save: {e}"), true),
                            }
                        }
                        if small_button(ui, "Copy over…") && list.len() > 0 {
                            self.dialog = Some(Dialog::CopyOver {
                                gi,
                                src: path.clone(),
                                target: 0,
                                new_ext: String::new(),
                            });
                        }
                    },
                );
            }
        });
        ui.add_space(18.0);

        // backups
        section_row(ui, &p, "BACKUPS", |ui| {
            if small_button(ui, "Clean auto-backups") {
                if let Some(r) = self.views[gi].save_root.clone() {
                    let files = saves::auto_backups(&r);
                    if files.len() <= 5 {
                        self.say(
                            format!("Nothing to clean ({} auto-backups)", files.len()),
                            false,
                        );
                    } else {
                        self.dialog = Some(Dialog::Clean { gi, files });
                    }
                }
            }
        });
        let backups: Vec<(String, String, bool)> = self.views[gi]
            .backups
            .iter()
            .map(|b| {
                let exts: Vec<String> = saves::restorable(b)
                    .iter()
                    .map(|f| format!(".{}", f.extension().unwrap_or_default().to_string_lossy()))
                    .collect();
                (b.name.clone(), exts.join(" "), !exts.is_empty())
            })
            .collect();
        card(ui, &p, |ui| {
            if backups.is_empty() {
                row(
                    ui,
                    &p,
                    true,
                    |ui| {
                        title_sub(
                            ui,
                            &p,
                            "No backups yet",
                            "One is made automatically before every launch.",
                        )
                    },
                    |_| {},
                );
            }
            for (i, (name, exts, ok)) in backups.iter().enumerate() {
                row(
                    ui,
                    &p,
                    i == 0,
                    |ui| title_sub(ui, &p, name, exts),
                    |ui| {
                        if ui
                            .add_enabled(
                                *ok,
                                egui::Button::new(RichText::new("Restore…").size(12.0))
                                    .min_size(vec2(0.0, 24.0)),
                            )
                            .clicked()
                        {
                            self.dialog = Some(Dialog::Restore { gi, bi: i, fi: 0 });
                        }
                    },
                );
            }
        });
        ui.add_space(8.0);
        ui.label(
            RichText::new("Restoring and copying always back up the current save first.")
                .size(12.0)
                .color(p.dim),
        );
    }

    /// Set a shared setting. Mirrors to every game, or only `only` when mirroring is off.
    fn write_shared(&mut self, key: &str, value: &str, only: Option<usize>) {
        self.cfg.shared.insert(key.to_string(), value.to_string());
        self.cfg.save();
        let ts = self.targets();
        let mut refs: Vec<(&Game, &Path)> =
            ts.iter().map(|(i, d)| (&GAMES[*i], d.as_path())).collect();
        let n = if self.cfg.mirror && !self.cfg.unsynced.contains(key) {
            shared::apply_key(key, value, &refs)
        } else {
            if let Some(gi) = only {
                refs.retain(|(g, _)| g.key == GAMES[gi].key);
            }
            shared::apply_key(key, value, &refs)
        };
        self.say(
            format!("Saved to {n} game{}", if n == 1 { "" } else { "s" }),
            false,
        );
        self.shared_vals.insert(key.to_string(), value.to_string());
    }

    // ------------------------------------------------------------ mods page
    fn mods_page(&mut self, ui: &mut Ui) {
        let p = pal(ui);
        ui.label(RichText::new("Mods").size(30.0).family(bold()));
        ui.label(RichText::new("Drop a mod zip anywhere in this window. Souls-like works out which game it is for and installs it.").color(p.dim));
        ui.add_space(14.0);

        // drop zone
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 120.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, 14, p.accent.gamma_multiply(0.08));
        ui.painter().rect_stroke(
            rect,
            14,
            Stroke::new(1.5, p.accent.gamma_multiply(0.6)),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center() - vec2(0.0, 12.0),
            Align2::CENTER_CENTER,
            "Drop mod zips here",
            FontId::proportional(17.0),
            p.text,
        );
        ui.painter().text(
            rect.center() + vec2(0.0, 14.0),
            Align2::CENTER_CENTER,
            "Seamless Co-op for DS1, DS2, DS3 · Honest Merchant",
            FontId::proportional(12.0),
            p.dim,
        );
        ui.add_space(14.0);

        // pending
        if !self.pending.is_empty() {
            let mut install = false;
            let mut clear = false;
            card(ui, &p, |ui| {
                for (i, pe) in self.pending.iter().enumerate() {
                    let name = pe
                        .path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    row(
                        ui,
                        &p,
                        i == 0,
                        |ui| match &pe.plan {
                            Ok(plan) => {
                                ui.label(RichText::new(plan.describe()).family(bold()));
                                ui.label(
                                    RichText::new(format!(
                                        "{name} · recognised from {}",
                                        plan.marker
                                    ))
                                    .size(12.0)
                                    .color(p.dim),
                                );
                            }
                            Err(e) => {
                                ui.label(RichText::new(&name).family(bold()));
                                ui.label(RichText::new(e).size(12.0).color(p.warn));
                            }
                        },
                        |ui| {
                            if pe.plan.is_ok() {
                                pill(ui, "Ready", p.ok);
                            }
                        },
                    );
                }
                row(
                    ui,
                    &p,
                    false,
                    |ui| {
                        ui.label(RichText::new("Replaced files are backed up inside the game folder. Your settings files are kept.").size(12.0).color(p.dim));
                    },
                    |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Install")
                                        .family(bold())
                                        .color(Color32::WHITE),
                                )
                                .fill(p.accent),
                            )
                            .clicked()
                        {
                            install = true;
                        }
                        if small_button(ui, "Clear") {
                            clear = true;
                        }
                    },
                );
            });
            ui.add_space(14.0);
            if install {
                self.install_pending();
            } else if clear {
                self.pending.clear();
            }
        }

        // installed
        section(ui, &p, "INSTALLED");
        for gi in 0..3 {
            let g = &GAMES[gi];
            let v = &self.views[gi];
            let dir = v.loc.as_ref().map(|l| l.game_dir.clone());
            ui.add_space(4.0);
            card(ui, &p, |ui| {
                row(
                    ui,
                    &p,
                    true,
                    |ui| {
                        ui.label(RichText::new(g.name).family(bold()));
                    },
                    |ui| {
                        if dir.is_none() {
                            pill(ui, "Not installed", p.warn);
                        }
                    },
                );
                if let Some(dir) = &dir {
                    let mut entries: Vec<(String, bool)> =
                        vec![("Seamless Co-op".into(), v.seamless)];
                    for m in g.mods {
                        let present = match m.id {
                            "rando" => dir.join("randomizer").is_dir(),
                            "honest" => dir.join("honestMerchant.ini").exists(),
                            _ => false,
                        };
                        entries.push((m.title.to_string(), present));
                    }
                    for (name, present) in entries {
                        row(
                            ui,
                            &p,
                            false,
                            |ui| {
                                ui.label(name);
                            },
                            |ui| {
                                if present {
                                    pill(ui, "Installed", p.ok);
                                } else {
                                    pill(ui, "Not installed — drop its zip", p.warn);
                                }
                            },
                        );
                    }
                }
            });
        }
    }

    fn install_pending(&mut self) {
        let pend = std::mem::take(&mut self.pending);
        let mut msgs = vec![];
        let mut failed = false;
        for pe in pend {
            let Ok(plan) = pe.plan else { continue };
            let gi = plan.game;
            let Some(loc) = self.views[gi].loc.clone() else {
                msgs.push(format!("{} is not installed.", GAMES[gi].name));
                failed = true;
                continue;
            };
            match modinstall::install(&plan, &loc.game_dir) {
                Ok(m) => msgs.push(m),
                Err(e) => {
                    msgs.push(format!("Install failed: {e}"));
                    failed = true;
                }
            }
        }
        self.refresh_all();
        self.load_texts();
        self.say(msgs.join("  "), failed);
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        let paths: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if paths.is_empty() {
            return;
        }
        for path in paths {
            let plan = if path
                .extension()
                .map(|e| e.eq_ignore_ascii_case("zip"))
                .unwrap_or(false)
            {
                modinstall::inspect(&path)
            } else {
                Err("Only .zip files can be installed.".to_string())
            };
            self.pending.push(Pending { path, plan });
        }
        self.page = Page::Mods;
    }

    // ------------------------------------------------------------ shared page
    fn shared_page(&mut self, ui: &mut Ui) {
        let p = pal(ui);
        ui.label(
            RichText::new("Shared co-op settings")
                .size(30.0)
                .family(bold()),
        );
        ui.label(RichText::new("One set of co-op settings for every game. Change one here and it is written to each game's Seamless Co-op settings file.").color(p.dim));
        ui.add_space(14.0);
        let n = self.targets().len();
        card(ui, &p, |ui| {
            let mut on = self.cfg.mirror;
            let mut ch = false;
            row(
                ui,
                &p,
                true,
                |ui| {
                    title_sub(
                        ui,
                        &p,
                        "Mirror across all games",
                        &format!("{n} games with Seamless Co-op installed"),
                    )
                },
                |ui| {
                    ch = toggle(ui, &p, &mut on).changed();
                },
            );
            if ch {
                self.cfg.mirror = on;
                self.cfg.save();
            }
        });
        ui.add_space(6.0);
        ui.label(RichText::new("The save-file extension is never mirrored, so a modded game can't end up on your normal save.").size(12.0).color(p.dim));
        ui.add_space(14.0);

        let mirror = self.cfg.mirror;
        let mut edits: Vec<(&'static str, String)> = vec![];
        let mut sync_changes: Vec<(&'static str, bool)> = vec![];
        card(ui, &p, |ui| {
            for (i, s) in shared::SETTINGS.iter().enumerate() {
                let synced = !self.cfg.unsynced.contains(s.key);
                let cur = self.shared_vals.get(s.key).cloned().unwrap_or_default();
                row(
                    ui,
                    &p,
                    i == 0,
                    |ui| title_sub(ui, &p, s.label, s.help),
                    |ui| {
                        ui.add_enabled_ui(mirror, |ui| {
                            let (label, fill, color) = if synced {
                                ("Synced", p.ok.gamma_multiply(0.2), p.ok)
                            } else {
                                ("This game only", p.field, p.dim)
                            };
                            let b = egui::Button::new(
                                RichText::new(label).size(11.0).family(bold()).color(color),
                            )
                            .fill(fill)
                            .corner_radius(99)
                            .min_size(vec2(96.0, 24.0));
                            if ui
                                .add(b)
                                .on_hover_text("Click to change whether this setting is mirrored")
                                .clicked()
                            {
                                sync_changes.push((s.key, !synced));
                            }
                        });
                        ui.add_enabled_ui(mirror && synced, |ui| match s.kind {
                            shared::Kind::Text => {
                                let mut t = cur.clone();
                                let r =
                                    ui.add(egui::TextEdit::singleline(&mut t).desired_width(150.0));
                                if r.changed() {
                                    self.shared_vals.insert(s.key.to_string(), t.clone());
                                }
                                if r.lost_focus() {
                                    edits.push((
                                        s.key,
                                        self.shared_vals.get(s.key).cloned().unwrap_or(t),
                                    ));
                                }
                            }
                            shared::Kind::Bool => {
                                let mut b = cur.trim() == "1";
                                if toggle(ui, &p, &mut b).changed() {
                                    edits.push((s.key, if b { "1" } else { "0" }.to_string()));
                                }
                            }
                            shared::Kind::Int(lo, hi) => {
                                let mut v: i64 = cur.trim().parse().unwrap_or(lo);
                                let before = v;
                                ui.add(egui::DragValue::new(&mut v).range(lo..=hi));
                                if v != before {
                                    edits.push((s.key, v.to_string()));
                                }
                            }
                        });
                    },
                );
            }
        });
        for (k, v) in edits {
            self.write_shared(k, &v, None);
        }
        for (k, on) in sync_changes {
            if on {
                self.cfg.unsynced.remove(k);
            } else {
                self.cfg.unsynced.insert(k.to_string());
            }
            self.cfg.save();
        }
    }

    // ------------------------------------------------------------ settings page
    fn settings_page(&mut self, ui: &mut Ui) {
        let p = pal(ui);
        ui.label(RichText::new("Settings").size(30.0).family(bold()));
        ui.add_space(14.0);
        section(ui, &p, "APPEARANCE");
        card(ui, &p, |ui| {
            let sel = match self.cfg.theme.as_str() {
                "light" => 1,
                "dark" => 2,
                _ => 0,
            };
            let mut ch = None;
            row(
                ui,
                &p,
                true,
                |ui| title_sub(ui, &p, "Theme", "System follows your OS"),
                |ui| {
                    ch = segmented(ui, &p, &["System", "Light", "Dark"], sel);
                },
            );
            if let Some(i) = ch {
                self.cfg.theme = ["system", "light", "dark"][i].to_string();
                self.cfg.save();
                self.ctx.set_theme(theme_pref(&self.cfg.theme));
            }
        });
        ui.add_space(18.0);
        section(ui, &p, "FOLDERS");
        ui.label(
            RichText::new("Leave blank to detect automatically from Steam.")
                .size(12.0)
                .color(p.dim),
        );
        let mut changed = false;
        for gi in 0..3 {
            let g = &GAMES[gi];
            ui.add_space(4.0);
            card(ui, &p, |ui| {
                let st = self.cfg.game(g.key);
                row(ui, &p, true, |ui| title_sub(ui, &p, g.name, ""), |_| {});
                row(
                    ui,
                    &p,
                    false,
                    |ui| title_sub(ui, &p, "Game folder", ""),
                    |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut st.game_dir)
                                .desired_width(380.0)
                                .hint_text("auto"),
                        );
                        changed |= r.lost_focus();
                    },
                );
                row(
                    ui,
                    &p,
                    false,
                    |ui| title_sub(ui, &p, "Save folder", ""),
                    |ui| {
                        let r = ui.add(
                            egui::TextEdit::singleline(&mut st.save_dir)
                                .desired_width(380.0)
                                .hint_text("auto"),
                        );
                        changed |= r.lost_focus();
                    },
                );
            });
        }
        if changed {
            self.cfg.save();
            self.refresh_all();
        }
        ui.add_space(18.0);
        section(ui, &p, "DETECTED");
        card(ui, &p, |ui| {
            let libs = platform::libraries();
            row(
                ui,
                &p,
                true,
                |ui| {
                    ui.label(RichText::new("Steam libraries").family(bold()));
                    for l in &libs {
                        ui.label(
                            RichText::new(l.display().to_string())
                                .size(12.0)
                                .color(p.dim),
                        );
                    }
                    if libs.is_empty() {
                        ui.label(RichText::new("None found").size(12.0).color(p.warn));
                    }
                },
                |_| {},
            );
            row(
                ui,
                &p,
                false,
                |ui| {
                    title_sub(
                        ui,
                        &p,
                        "Version",
                        &format!("Souls-like v{}", env!("CARGO_PKG_VERSION")),
                    )
                },
                |_| {},
            );
            row(
                ui,
                &p,
                false,
                |ui| {
                    title_sub(
                        ui,
                        &p,
                        "Platform",
                        if platform::OS_SUPPORTED {
                            "Games launch through Steam"
                        } else {
                            "Launching isn't supported on this OS yet; saves still work"
                        },
                    )
                },
                |_| {},
            );
            row(
                ui,
                &p,
                false,
                |ui| {
                    title_sub(
                        ui,
                        &p,
                        "Logs",
                        &crate::config::log_dir().display().to_string(),
                    )
                },
                |ui| {
                    if small_button(ui, "Open") {
                        platform::open_path(&crate::config::log_dir());
                    }
                },
            );
        });
    }

    // ------------------------------------------------------------ dialogs
    fn dialogs(&mut self, ctx: &egui::Context) {
        let Some(mut dlg) = self.dialog.take() else {
            return;
        };
        let p = pal_for(ctx.global_style().visuals.dark_mode);
        let mut keep = true;
        let mut confirm = false;
        let modal = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
            ui.set_width(380.0);
            match &mut dlg {
                Dialog::CopyOver { gi, src, target, new_ext } => {
                    let g = &GAMES[*gi];
                    let src_ext = src.extension().unwrap_or_default().to_string_lossy().to_string();
                    ui.label(RichText::new(format!("Copy .{src_ext} over…")).size(16.0).family(bold()));
                    ui.label(RichText::new("The chosen slot is replaced with a copy of this save. A backup is made first.").color(p.dim));
                    let others: Vec<String> = self.views[*gi].saves.iter().filter(|s| s.path != *src).map(|s| s.ext.clone()).collect();
                    let names: Vec<String> = others.iter().map(|e| format!(".{e}  {}", g.label(e))).chain(std::iter::once("New slot…".to_string())).collect();
                    if *target >= names.len() {
                        *target = 0;
                    }
                    egui::ComboBox::from_id_salt("copytarget").width(340.0).selected_text(&names[*target]).show_ui(ui, |ui| {
                        for (i, n) in names.iter().enumerate() {
                            ui.selectable_value(target, i, n);
                        }
                    });
                    if *target == names.len() - 1 {
                        ui.add(egui::TextEdit::singleline(new_ext).hint_text("new extension, e.g. co2"));
                    }
                }
                Dialog::Restore { gi, bi, fi } => {
                    let g = &GAMES[*gi];
                    let b = &self.views[*gi].backups[*bi];
                    ui.label(RichText::new(format!("Restore from {}", b.name)).size(16.0).family(bold()));
                    ui.label(RichText::new("Pick the save to put back. It replaces the current one (a safety backup is made first).").color(p.dim));
                    let files = saves::restorable(b);
                    for (i, f) in files.iter().enumerate() {
                        let ext = f.extension().unwrap_or_default().to_string_lossy().to_string();
                        ui.radio_value(fi, i, format!(".{ext}  {}", g.label(&ext)));
                    }
                }
                Dialog::CopyChar {
                    gi,
                    src_ext,
                    src_slots,
                    src_pick,
                    dst_idx,
                    dst_loaded,
                    dst_slots,
                    dst_slot,
                    ..
                } => {
                    let g = &GAMES[*gi];
                    let saves = &self.views[*gi].saves;
                    ui.label(RichText::new("Copy a character").size(16.0).family(bold()));
                    ui.label(RichText::new(format!("Copies one character from the .{src_ext} save into a slot of any save. A backup is made first.")).color(p.dim));
                    ui.add_space(4.0);
                    ui.label(RichText::new("Character").size(12.0).color(p.dim));
                    egui::ComboBox::from_id_salt("cc_src")
                        .width(340.0)
                        .selected_text(format!("Slot {} · {}", src_slots[*src_pick].0 + 1, src_slots[*src_pick].1))
                        .show_ui(ui, |ui| {
                            for (i, (slot, name)) in src_slots.iter().enumerate() {
                                ui.selectable_value(src_pick, i, format!("Slot {} · {}", slot + 1, name));
                            }
                        });
                    ui.label(RichText::new("Into save").size(12.0).color(p.dim));
                    egui::ComboBox::from_id_salt("cc_dst")
                        .width(340.0)
                        .selected_text(match saves.get(*dst_idx) {
                            Some(s) => format!(".{}  {}", s.ext, g.label(&s.ext)),
                            None => "—".into(),
                        })
                        .show_ui(ui, |ui| {
                            for (i, s) in saves.iter().enumerate() {
                                ui.selectable_value(dst_idx, i, format!(".{}  {}", s.ext, g.label(&s.ext)));
                            }
                        });
                    if *dst_loaded != Some(*dst_idx) {
                        *dst_slots = saves
                            .get(*dst_idx)
                            .and_then(|s| crate::slots::occupants(g, &s.path).ok())
                            .unwrap_or_default();
                        *dst_loaded = Some(*dst_idx);
                        // default to the first empty slot so nothing is replaced by accident
                        *dst_slot = dst_slots.iter().position(|o| o.is_none()).unwrap_or(0);
                    }
                    ui.label(RichText::new("Into slot").size(12.0).color(p.dim));
                    let slot_label = |i: usize| match dst_slots.get(i) {
                        Some(Some(n)) => format!("Slot {} · {}  (will be replaced)", i + 1, n),
                        _ => format!("Slot {} · empty", i + 1),
                    };
                    egui::ComboBox::from_id_salt("cc_slot")
                        .width(340.0)
                        .selected_text(slot_label(*dst_slot))
                        .show_ui(ui, |ui| {
                            for i in 0..dst_slots.len() {
                                ui.selectable_value(dst_slot, i, slot_label(i));
                            }
                        });
                    if matches!(dst_slots.get(*dst_slot), Some(Some(_))) {
                        ui.label(RichText::new("The character in that slot is overwritten. You can restore it from the backup.").size(12.0).color(p.warn));
                    }
                }
                Dialog::Clean { files, .. } => {
                    ui.label(RichText::new("Clean auto-backups").size(16.0).family(bold()));
                    ui.label(RichText::new(format!("Keeps the newest 5 of {} Seamless Co-op auto-backups and deletes the rest.", files.len())).color(p.dim));
                }
            }
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let danger = egui::Button::new(RichText::new("Confirm").family(bold()).color(Color32::WHITE)).fill(Color32::from_rgb(0xff, 0x45, 0x3a));
                if ui.add(danger).clicked() {
                    confirm = true;
                }
                if ui.button("Cancel").clicked() {
                    keep = false;
                }
            });
        });
        if modal.should_close() {
            keep = false;
        }
        if confirm {
            self.run_dialog(dlg);
            return;
        }
        if keep {
            self.dialog = Some(dlg);
        }
    }

    fn run_dialog(&mut self, dlg: Dialog) {
        match dlg {
            Dialog::CopyOver {
                gi,
                src,
                target,
                new_ext,
            } => {
                let g = &GAMES[gi];
                let Some(root) = self.views[gi].save_root.clone() else {
                    return;
                };
                let others: Vec<String> = self.views[gi]
                    .saves
                    .iter()
                    .filter(|s| s.path != src)
                    .map(|s| s.ext.clone())
                    .collect();
                let ext = if target < others.len() {
                    others[target].clone()
                } else {
                    new_ext
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect::<String>()
                };
                if ext.is_empty() || ext == "bak" {
                    return self.say("Enter a valid extension", true);
                }
                if game_running_check(g) {
                    return self.say(format!("Close {} first", g.name), true);
                }
                match saves::copy_over(g, &root, &src, &ext) {
                    Ok(_) => self.say(format!("Copied over .{ext}"), false),
                    Err(e) => self.say(format!("Copy failed: {e}"), true),
                }
                self.refresh(gi);
            }
            Dialog::Restore { gi, bi, fi } => {
                let g = &GAMES[gi];
                let Some(b) = self.views[gi].backups.get(bi) else {
                    return;
                };
                let files = saves::restorable(b);
                let Some(f) = files.get(fi).map(|f| (*f).clone()) else {
                    return;
                };
                let Some(root) = self.restore_root(gi) else {
                    return self.say("Launch the game once first so the save folder exists", true);
                };
                if game_running_check(g) {
                    return self.say(format!("Close {} first", g.name), true);
                }
                match saves::restore(g, &root, &f) {
                    Ok(_) => self.say(
                        format!("Restored {}", f.file_name().unwrap().to_string_lossy()),
                        false,
                    ),
                    Err(e) => self.say(format!("Restore failed: {e}"), true),
                }
                self.refresh(gi);
            }
            Dialog::CopyChar {
                gi,
                src,
                src_slots,
                src_pick,
                dst_idx,
                dst_slot,
                ..
            } => {
                let g = &GAMES[gi];
                let Some(dst) = self.views[gi].saves.get(dst_idx).map(|s| s.path.clone()) else {
                    return;
                };
                let Some(root) = self.views[gi].save_root.clone() else {
                    return;
                };
                let Some((slot, name)) = src_slots.get(src_pick).cloned() else {
                    return;
                };
                if game_running_check(g) {
                    return self.say(format!("Close {} first", g.name), true);
                }
                if dst == src && slot == dst_slot {
                    return self.say("Pick a different slot or save.", true);
                }
                if let Err(e) = saves::backup(g, &root, "before-character-copy") {
                    return self.say(format!("Backup failed, nothing was changed: {e}"), true);
                }
                match crate::slots::copy_slot(g, &src, slot, &dst, dst_slot) {
                    Ok(()) => self.say(format!("Copied {name} to slot {}", dst_slot + 1), false),
                    Err(e) => self.say(format!("Nothing was changed: {e}"), true),
                }
                self.refresh(gi);
            }
            Dialog::Clean { gi, files } => {
                let n = files.len().saturating_sub(5);
                for f in files.into_iter().skip(5) {
                    let _ = std::fs::remove_file(f);
                }
                self.say(format!("Deleted {n} old auto-backups"), false);
                self.refresh(gi);
            }
        }
    }
}

fn game_running_check(g: &Game) -> bool {
    platform::game_running(g)
}

// ---------------------------------------------------------------- frame loop
impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        self.draw(ui);
    }
}

impl App {
    pub fn draw(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        // background work
        while let Ok((gi, res)) = self.rx.try_recv() {
            self.launching[gi] = false;
            match res {
                Ok(m) => self.say(m, false),
                Err(e) => self.say(e, true),
            }
            self.refresh(gi);
        }
        if self.last_refresh.elapsed() > Duration::from_secs(3) {
            self.refresh_all();
        }
        ctx.request_repaint_after(Duration::from_secs(3));
        self.handle_drops(&ctx);

        let p = pal(ui);
        egui::Panel::left("sidebar")
            .resizable(false)
            .exact_size(236.0)
            .frame(
                Frame::new()
                    .fill(p.side)
                    .inner_margin(Margin::symmetric(12, 14)),
            )
            .show(ui, |ui| self.sidebar(ui));

        egui::CentralPanel::default()
            .frame(
                Frame::new()
                    .fill(p.bg)
                    .inner_margin(Margin::symmetric(40, 30)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let avail = ui.available_width();
                        let w = (avail - 16.0).clamp(300.0, 780.0);
                        let left = ((avail - w) / 2.0).max(0.0) as i8;
                        Frame::new()
                            .inner_margin(Margin {
                                left,
                                right: 0,
                                top: 0,
                                bottom: 0,
                            })
                            .show(ui, |ui| {
                                ui.set_width(w);
                                match self.page {
                                    Page::Game(i) => self.game_page(ui, i),
                                    Page::Mods => self.mods_page(ui),
                                    Page::Shared => self.shared_page(ui),
                                    Page::Settings => self.settings_page(ui),
                                }
                                ui.add_space(20.0);
                            });
                    });
            });

        self.dialogs(&ctx);

        // drag hover overlay
        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("dropover"));
            let painter = ctx.layer_painter(layer);
            let r = ctx.content_rect();
            painter.rect_filled(r, 0, Color32::from_black_alpha(150));
            painter.text(
                r.center(),
                Align2::CENTER_CENTER,
                "Drop to install",
                FontId::proportional(28.0),
                Color32::WHITE,
            );
        }

        // toast
        if let Some((msg, at, err)) = self.toast.clone() {
            if at.elapsed() > Duration::from_secs(5) {
                self.toast = None;
            } else {
                egui::Area::new(egui::Id::new("toast"))
                    .order(egui::Order::Foreground)
                    .anchor(Align2::CENTER_BOTTOM, vec2(0.0, -18.0))
                    .show(&ctx, |ui| {
                        Frame::new()
                            .fill(if err {
                                Color32::from_rgb(0x8f, 0x2a, 0x22)
                            } else {
                                Color32::from_rgb(0x2c, 0x2c, 0x2e)
                            })
                            .corner_radius(12)
                            .inner_margin(Margin::symmetric(16, 10))
                            .show(ui, |ui| ui.label(RichText::new(msg).color(Color32::WHITE)));
                    });
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
    }
}

#[cfg(test)]
mod shots {
    use super::*;
    use egui_kittest::Harness;

    /// Renders every page to PNGs: `SHOTS_DIR=/tmp/x cargo test render_pages -- --ignored`.
    #[test]
    #[ignore]
    fn render_pages() {
        let dir = std::env::var("SHOTS_DIR").unwrap_or_else(|_| "/tmp/souls-like-shots".into());
        std::fs::create_dir_all(&dir).unwrap();
        for dark in [true, false] {
            let app = App::with_ctx(&egui::Context::default());
            let mut first = true;
            let mut h = Harness::builder()
                .with_size(vec2(1060.0, 740.0))
                .build_ui_state(
                    move |ui, app: &mut App| {
                        if first {
                            // fonts take effect on the next frame
                            install_style(ui.ctx());
                            app.ctx = ui.ctx().clone();
                            app.icons = Default::default();
                            app.icon_tried = [false; 3];
                            for gi in 0..3 {
                                app.ensure_icon(gi);
                            }
                            first = false;
                            return;
                        }
                        app.draw(ui)
                    },
                    app,
                );
            h.ctx.set_theme(if dark {
                egui::ThemePreference::Dark
            } else {
                egui::ThemePreference::Light
            });
            let pages = [
                ("ds1", Page::Game(0)),
                ("ds2", Page::Game(1)),
                ("ds3", Page::Game(2)),
                ("mods", Page::Mods),
                ("shared", Page::Shared),
                ("settings", Page::Settings),
            ];
            // character-copy dialog on the DS3 page
            {
                let st = h.state_mut();
                if let Some(first) = st.views[2].saves.first() {
                    let src = first.path.clone();
                    let chars = crate::slots::read(&GAMES[2], &src)
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    st.dialog = Some(Dialog::CopyChar {
                        gi: 2,
                        src_ext: first.ext.clone(),
                        src,
                        src_slots: chars.into_iter().map(|c| (c.index, c.name)).collect(),
                        src_pick: 0,
                        dst_idx: 1,
                        dst_loaded: None,
                        dst_slots: vec![],
                        dst_slot: 0,
                    });
                    st.page = Page::Game(2);
                }
                h.run();
                let img = h.render().expect("render");
                img.save(format!(
                    "{dir}/{}-copychar.png",
                    if dark { "dark" } else { "light" }
                ))
                .unwrap();
                h.state_mut().dialog = None;
            }
            for (name, page) in pages {
                h.state_mut().page = page;
                h.run();
                let img = h.render().expect("render");
                img.save(format!(
                    "{dir}/{}-{name}.png",
                    if dark { "dark" } else { "light" }
                ))
                .unwrap();
            }
        }
    }
}
