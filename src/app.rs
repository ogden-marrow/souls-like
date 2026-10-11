use crate::{
    config::{Config, GameState, Mode},
    games::{GAMES, Game},
    ini, launch, modinstall, platform, saves, shared, slots,
};
use eframe::egui::{self, Align, Align2, Color32 as C, CornerRadius, FontId, Frame, Layout, Margin, RichText as T, Sense, Stroke, StrokeKind, Ui, vec2};
use std::{
    collections::{BTreeMap, HashMap},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant, SystemTime},
};

#[derive(Clone, Copy)]
struct Pal {
    bg: C,
    side: C,
    card: C,
    line: C,
    text: C,
    dim: C,
    accent: C,
    field: C,
    ok: C,
    warn: C,
}

fn pal(dark: bool) -> Pal {
    let h = |x: u32| C::from_rgb((x >> 16) as u8, (x >> 8) as u8, x as u8);
    if dark {
        Pal {
            bg: h(0x1c1c1e),
            side: h(0x252528),
            card: h(0x2a2a2d),
            line: C::from_white_alpha(18),
            text: h(0xf2f2f7),
            dim: h(0x8e8e93),
            accent: h(0x0a84ff),
            field: h(0x3a3a3e),
            ok: h(0x32d74b),
            warn: h(0xffb340),
        }
    } else {
        Pal {
            bg: h(0xf6f6f7),
            side: h(0xe9e9ec),
            card: C::WHITE,
            line: C::from_black_alpha(20),
            text: h(0x1d1d1f),
            dim: h(0x6e6e73),
            accent: h(0x0a7aff),
            field: h(0xe6e6e9),
            ok: h(0x30b84a),
            warn: h(0xb25e00),
        }
    }
}

fn bold() -> egui::FontFamily {
    egui::FontFamily::Name("bold".into())
}

fn style(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let faces = [("noto", &include_bytes!("../assets/fonts/NotoSans-Regular.ttf")[..]), ("bold", &include_bytes!("../assets/fonts/NotoSans-Bold.ttf")[..])];
    for (name, bytes) in faces {
        fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_static(bytes)));
    }
    fonts.families.insert(egui::FontFamily::Proportional, vec!["noto".into()]);
    fonts.families.insert(egui::FontFamily::Monospace, vec!["noto".into()]);
    fonts.families.insert(bold(), vec!["bold".into(), "noto".into()]);
    ctx.set_fonts(fonts);
    for (theme, dark) in [(egui::Theme::Dark, true), (egui::Theme::Light, false)] {
        let p = pal(dark);
        let mut v = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        (v.panel_fill, v.window_fill, v.extreme_bg_color, v.faint_bg_color) = (p.bg, p.card, p.field, p.card);
        (v.override_text_color, v.selection.bg_fill, v.hyperlink_color) = (Some(p.text), p.accent, p.accent);
        v.window_corner_radius = CornerRadius::same(14);
        for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active] {
            (w.corner_radius, w.bg_stroke) = (CornerRadius::same(7), Stroke::NONE);
        }
        v.widgets.inactive.weak_bg_fill = p.field;
        v.widgets.hovered.weak_bg_fill = p.field.gamma_multiply(1.15);
        ctx.set_visuals_of(theme, v);
    }
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = vec2(8.0, 8.0);
        s.spacing.button_padding = vec2(12.0, 5.0);
    });
}

fn theme_pref(s: &str) -> egui::ThemePreference {
    match s {
        "light" => egui::ThemePreference::Light,
        "dark" => egui::ThemePreference::Dark,
        _ => egui::ThemePreference::System,
    }
}

fn card(ui: &mut Ui, p: Pal, add: impl FnOnce(&mut Ui)) {
    Frame::new().fill(p.card).corner_radius(10).stroke(Stroke::new(1.0, p.line)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = 0.0;
        add(ui)
    });
}

fn row(ui: &mut Ui, p: Pal, first: bool, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    if !first {
        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().rect_filled(r, 0, p.line);
    }
    Frame::new().inner_margin(Margin::symmetric(16, 11)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.vertical(left);
            ui.with_layout(Layout::right_to_left(Align::Center), right);
        });
    });
}

fn note(ui: &mut Ui, p: Pal, text: &str) {
    ui.label(T::new(text).size(12.0).color(p.dim));
}

fn ts(ui: &mut Ui, p: Pal, title: &str, sub: &str) {
    ui.label(T::new(title).family(bold()));
    if !sub.is_empty() {
        note(ui, p, sub);
    }
}

fn section(ui: &mut Ui, p: Pal, text: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.add_space(5.0);
            ui.label(T::new(text).size(11.0).family(bold()).color(p.dim));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), right);
    });
}

fn title(ui: &mut Ui, p: Pal, text: &str, sub: &str) {
    ui.label(T::new(text).size(30.0).family(bold()));
    if !sub.is_empty() {
        ui.label(T::new(sub).color(p.dim));
    }
    ui.add_space(14.0);
}

fn pill(ui: &mut Ui, text: &str, color: C) {
    Frame::new().fill(color.gamma_multiply(0.18)).corner_radius(99).inner_margin(Margin::symmetric(9, 2)).show(ui, |ui| {
        ui.label(T::new(text).size(11.0).family(bold()).color(color));
    });
}

fn toggle(ui: &mut Ui, p: Pal, on: &mut bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(40.0, 24.0), Sense::click());
    let clicked = resp.clicked();
    *on ^= clicked;
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    ui.painter().rect_filled(rect, 12, p.field.lerp_to_gamma(p.ok, t));
    let x = egui::lerp(rect.left() + 12.0..=rect.right() - 12.0, t);
    ui.painter().circle_filled(egui::pos2(x, rect.center().y), 10.0, C::WHITE);
    clicked
}

fn segmented(ui: &mut Ui, p: Pal, labels: &[&str], sel: usize) -> Option<usize> {
    let mut out = None;
    Frame::new().fill(p.field).corner_radius(8).inner_margin(2).show(ui, |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        let bw = labels.iter().map(|l| l.chars().count()).max().unwrap_or(4) as f32 * 8.2 + 34.0;
        let n = labels.len() as f32;
        ui.allocate_ui_with_layout(vec2(n * bw + (n - 1.0) * 2.0 + 2.0, 26.0), Layout::left_to_right(Align::Center), |ui| {
            for (i, l) in labels.iter().enumerate() {
                let on = i == sel;
                let b = egui::Button::new(T::new(*l).family(bold()).color(if on { p.text } else { p.dim }))
                    .fill(if on { p.card } else { C::TRANSPARENT })
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

fn badge(ui: &mut Ui, p: Pal, ext: &str) {
    let c = match ext {
        "sl2" => C::from_rgb(0x6b, 0x82, 0xff),
        "va2" => p.ok,
        "co2" => p.warn,
        _ => p.dim,
    };
    Frame::new().fill(c.gamma_multiply(0.18)).corner_radius(5).inner_margin(Margin::symmetric(7, 2)).show(ui, |ui| {
        ui.label(T::new(format!(".{ext}")).monospace().size(12.0).color(c));
    });
}

fn small(ui: &mut Ui, text: &str) -> bool {
    ui.add(egui::Button::new(T::new(text).size(12.0)).min_size(vec2(0.0, 24.0))).clicked()
}

fn dropdown(ui: &mut Ui, id: &str, cur: &mut usize, labels: &[String]) {
    egui::ComboBox::from_id_salt(id).width(340.0).selected_text(labels.get(*cur).cloned().unwrap_or_default()).show_ui(ui, |ui| {
        for (i, l) in labels.iter().enumerate() {
            ui.selectable_value(cur, i, l);
        }
    });
}

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
    base: Option<PathBuf>,
    root: Option<PathBuf>,
    saves: Vec<saves::SaveFile>,
    chars: Vec<String>,
    backups: Vec<saves::Backup>,
    running: bool,
    seamless: bool,
}

enum Msg {
    Launched(usize, Result<String, String>),
    Views(Vec<View>),
    Picked(usize, bool, PathBuf),
}

enum Dialog {
    CopyOver { gi: usize, src: PathBuf, target: usize, ext: String },
    Restore { gi: usize, bi: usize, fi: usize },
    Clean { gi: usize, files: Vec<PathBuf> },
    Character { gi: usize, src: PathBuf, from: Vec<(usize, String)>, pick: usize, dst: usize, loaded: Option<usize>, slots: Vec<Option<String>>, slot: usize },
}

type Cache = Arc<Mutex<HashMap<PathBuf, (SystemTime, u64, String)>>>;

fn characters(cache: &Cache, g: &Game, f: &saves::SaveFile) -> String {
    let hit = cache.lock().ok().and_then(|c| c.get(&f.path).cloned()).filter(|(m, sz, _)| *m == f.modified && *sz == f.size);
    if let Some((.., text)) = hit {
        return text;
    }
    let text = match slots::occupants(g, &f.path) {
        Ok(v) if v.iter().all(Option::is_none) => "No characters".into(),
        Ok(v) => format!("Characters: {}", v.into_iter().flatten().collect::<Vec<_>>().join(" · ")),
        Err(_) => "Characters: couldn't read this save".into(),
    };
    cache.lock().into_iter().for_each(|mut c| drop(c.insert(f.path.clone(), (f.modified, f.size, text.clone()))));
    text
}

fn compute_views(states: &[GameState], cache: &Cache) -> Vec<View> {
    let running = platform::running_games();
    states.iter().enumerate().map(|(gi, st)| compute_view(gi, st, running[gi], cache)).collect()
}

fn compute_view(gi: usize, st: &GameState, running: bool, cache: &Cache) -> View {
    let g = &GAMES[gi];
    let loc = platform::locate(g, &st.game_dir);
    let base = platform::save_base_dir(g, loc.as_ref(), &st.save_dir);
    let root = base.as_ref().and_then(|b| saves::find_root(g, b));
    let list = root.as_ref().map(|r| saves::list(g, r)).unwrap_or_default();
    View {
        chars: list.iter().map(|f| characters(cache, g, f)).collect(),
        seamless: loc.as_ref().is_some_and(|l| launch::has_seamless(g, &l.game_dir)),
        running,
        backups: saves::list_backups(g, 12),
        saves: list,
        root,
        base,
        loc,
    }
}

pub struct App {
    cfg: Config,
    page: Page,
    views: Vec<View>,
    last_refresh: Instant,
    refreshing: bool,
    dialog: Option<Dialog>,
    pending: Vec<(PathBuf, Result<modinstall::Plan, String>)>,
    toast: Option<(String, Instant, bool)>,
    tx: mpsc::Sender<Msg>,
    rx: mpsc::Receiver<Msg>,
    launching: [bool; 3],
    pw: [String; 3],
    shared_vals: BTreeMap<String, String>,
    ctx: egui::Context,
    icons: [Option<Option<egui::TextureHandle>>; 3],
    cache: Cache,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style(&cc.egui_ctx);
        let cfg = Config::load();
        cc.egui_ctx.set_theme(theme_pref(&cfg.theme));
        let (tx, rx) = mpsc::channel();
        let mut app = App {
            page: Page::Game(GAMES.iter().position(|g| g.key == cfg.last).unwrap_or(1)),
            cfg,
            views: vec![],
            last_refresh: Instant::now(),
            refreshing: false,
            dialog: None,
            pending: vec![],
            toast: None,
            tx,
            rx,
            launching: [false; 3],
            pw: Default::default(),
            shared_vals: BTreeMap::new(),
            ctx: cc.egui_ctx.clone(),
            icons: Default::default(),
            cache: Default::default(),
        };
        app.views = compute_views(&app.states(), &app.cache);
        app.load_icons();
        app.load_texts();
        app
    }

    fn say(&mut self, msg: impl Into<String>, err: bool) {
        self.toast = Some((msg.into(), Instant::now(), err));
    }

    fn states(&mut self) -> Vec<GameState> {
        GAMES.iter().map(|g| self.cfg.game(g.key).clone()).collect()
    }

    fn refresh(&mut self, gi: usize) {
        let st = self.cfg.game(GAMES[gi].key).clone();
        self.views[gi] = compute_view(gi, &st, self.views[gi].running, &self.cache);
    }

    fn request_refresh(&mut self) {
        if !std::mem::replace(&mut self.refreshing, true) {
            let (states, cache, tx, ctx) = (self.states(), self.cache.clone(), self.tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let _ = tx.send(Msg::Views(compute_views(&states, &cache)));
                ctx.request_repaint();
            });
        }
    }

    fn load_icons(&mut self) {
        for (gi, g) in GAMES.iter().enumerate() {
            let Some(loc) = self.views[gi].loc.as_ref().filter(|_| self.icons[gi].is_none()) else { continue };
            self.icons[gi] = Some(crate::gameicon::load(g.key, &loc.game_dir.join(g.exe)).map(|img| {
                let size = [img.width() as usize, img.height() as usize];
                self.ctx.load_texture(g.key, egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR)
            }));
        }
    }

    fn targets(&self) -> Vec<(usize, PathBuf)> {
        (0..3).filter(|&i| self.views[i].seamless).filter_map(|i| Some((i, self.views[i].loc.as_ref()?.game_dir.clone()))).collect()
    }

    fn load_texts(&mut self) {
        let owned = self.targets();
        let refs: Vec<(&Game, &Path)> = owned.iter().map(|(i, d)| (&GAMES[*i], d.as_path())).collect();
        for (gi, g) in GAMES.iter().enumerate() {
            let dir = self.views[gi].loc.as_ref().map(|l| l.game_dir.clone());
            self.pw[gi] = dir.and_then(|d| ini::get(&launch::settings_path(g, &d), "cooppassword")).unwrap_or_default();
        }
        self.shared_vals = shared::SETTINGS.iter().filter_map(|(key, ..)| Some((key.to_string(), self.cfg.shared.get(*key).cloned().or_else(|| shared::read(key, &refs))?))).collect();
    }

    fn set_page(&mut self, page: Page) {
        self.page = page;
        if let Page::Game(i) = page {
            self.cfg.last = GAMES[i].key.into();
            self.cfg.save();
        }
        self.request_refresh();
        self.load_texts();
    }

    fn start(&mut self, gi: usize) {
        let Some(loc) = self.views[gi].loc.clone().filter(|_| !self.launching[gi] && !self.views[gi].running) else { return };
        let mirrored: BTreeMap<String, String> =
            if self.cfg.mirror { self.cfg.shared.iter().filter(|(k, _)| !self.cfg.unsynced.contains(*k)).map(|(k, v)| (k.clone(), v.clone())).collect() } else { BTreeMap::new() };
        let (st, base, tx, ctx) = (self.cfg.game(GAMES[gi].key).clone(), self.views[gi].base.clone(), self.tx.clone(), self.ctx.clone());
        self.launching[gi] = true;
        std::thread::spawn(move || {
            let _ = tx.send(Msg::Launched(gi, launch::launch(&GAMES[gi], &loc, &st, base.as_deref(), &mirrored)));
            ctx.request_repaint();
        });
    }

    fn write_shared(&mut self, key: &str, value: &str, only: Option<usize>) {
        self.cfg.shared.insert(key.into(), value.into());
        self.cfg.save();
        let owned = self.targets();
        let everywhere = self.cfg.mirror && !self.cfg.unsynced.contains(key);
        let refs: Vec<(&Game, &Path)> = owned.iter().filter(|(i, _)| everywhere || only == Some(*i)).map(|(i, d)| (&GAMES[*i], d.as_path())).collect();
        let n = shared::apply_key(key, value, &refs);
        self.say(format!("Saved to {n} game{}", if n == 1 { "" } else { "s" }), false);
        self.shared_vals.insert(key.into(), value.into());
    }

    fn nav(&mut self, ui: &mut Ui, p: Pal, target: Page, height: f32, paint: impl FnOnce(&Ui, egui::Rect, bool)) {
        let selected = self.page == target;
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
        if selected || resp.hovered() {
            ui.painter().rect_filled(rect, 8, if selected { p.accent } else { p.line });
        }
        paint(ui, rect, selected);
        if resp.clicked() {
            self.set_page(target);
        }
    }

    fn sidebar(&mut self, ui: &mut Ui, p: Pal) {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.add_space(6.0);
        ui.label(T::new("Souls-like").size(17.0).family(bold()));
        ui.add_space(10.0);
        ui.label(T::new("GAMES").size(11.0).family(bold()).color(p.dim));
        for (gi, g) in GAMES.iter().enumerate() {
            let v = &self.views[gi];
            let status = if v.running {
                "Running"
            } else if v.loc.is_none() {
                "Not installed"
            } else if g.sub.is_empty() {
                "Ready"
            } else {
                g.sub
            };
            let icon = self.icons[gi].clone().flatten();
            self.nav(ui, p, Page::Game(gi), 46.0, |ui, rect, on| {
                let badge = egui::Rect::from_min_size(rect.min + vec2(8.0, 9.0), vec2(28.0, 28.0));
                match &icon {
                    Some(tex) => egui::Image::new(tex).fit_to_exact_size(badge.size()).corner_radius(7).paint_at(ui, badge),
                    None => {
                        let [r, g, b] = g.accent;
                        ui.painter().rect_filled(badge, 7, if on { C::from_white_alpha(56) } else { C::from_rgb(r, g, b) });
                        ui.painter().text(badge.center(), Align2::CENTER_CENTER, ["I", "II", "III"][gi], FontId::proportional(13.0), C::WHITE);
                    }
                }
                let (name, sub) = if on { (C::WHITE, C::from_white_alpha(200)) } else { (p.text, p.dim) };
                ui.painter().text(rect.min + vec2(46.0, 8.0), Align2::LEFT_TOP, g.name, FontId::proportional(13.5), name);
                ui.painter().text(rect.min + vec2(46.0, 26.0), Align2::LEFT_TOP, status, FontId::proportional(11.0), sub);
            });
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.add_space(4.0);
            ui.label(T::new(format!("Souls-like v{}", env!("CARGO_PKG_VERSION"))).size(11.0).color(p.dim));
            for (label, page) in [("Settings", Page::Settings), ("Shared co-op settings", Page::Shared), ("Mods", Page::Mods)] {
                self.nav(ui, p, page, 32.0, |ui, rect, on| {
                    ui.painter().text(rect.min + vec2(12.0, 8.0), Align2::LEFT_TOP, label, FontId::proportional(13.5), if on { C::WHITE } else { p.text });
                });
            }
        });
    }

    fn game_page(&mut self, ui: &mut Ui, p: Pal, gi: usize) {
        let g = &GAMES[gi];
        let st = self.cfg.game(g.key).clone();
        let (installed, running, has_mod, launching) = (self.views[gi].loc.is_some(), self.views[gi].running, self.views[gi].seamless, self.launching[gi]);
        let want = st.mode == Mode::Seamless;

        ui.horizontal(|ui| {
            if let Some(Some(tex)) = &self.icons[gi] {
                ui.add(egui::Image::new(tex).fit_to_exact_size(vec2(56.0, 56.0)).corner_radius(13));
                ui.add_space(6.0);
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.label(T::new(g.name).size(30.0).family(bold()));
                ui.horizontal(|ui| {
                    if !g.sub.is_empty() {
                        ui.label(T::new(g.sub).color(p.dim));
                    }
                    let (text, color) = if running {
                        ("Running", p.ok)
                    } else if !installed {
                        ("Not installed", p.warn)
                    } else {
                        ("Ready", p.ok)
                    };
                    pill(ui, text, color);
                });
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let label = if running {
                    "Running…"
                } else if launching {
                    "Starting…"
                } else {
                    "Play"
                };
                let b = egui::Button::new(T::new(label).size(16.0).family(bold()).color(C::WHITE)).fill(p.accent).corner_radius(99).min_size(vec2(130.0, 40.0));
                if ui.add_enabled(installed && !running && !launching && (!want || has_mod), b).clicked() {
                    self.start(gi);
                }
            });
        });
        ui.add_space(16.0);

        card(ui, p, |ui| {
            let sub = if want { "Mods on, separate save file" } else { "Normal online game, every mod switched off" };
            let mut mode = None;
            row(ui, p, true, |ui| ts(ui, p, "Mode", sub), |ui| mode = segmented(ui, p, &["Vanilla", "Seamless Co-op"], want as usize));
            if let Some(i) = mode {
                self.cfg.game(g.key).mode = if i == 0 { Mode::Vanilla } else { Mode::Seamless };
                self.cfg.save();
            }
            if !(want && has_mod) {
                return;
            }
            for m in g.mods {
                let mut on = st.mods.get(m.id).copied().unwrap_or(false);
                let mut changed = false;
                row(ui, p, false, |ui| ts(ui, p, m.title, m.sub), |ui| changed = toggle(ui, p, &mut on));
                if changed {
                    self.cfg.game(g.key).mods.insert(m.id.into(), on);
                    self.cfg.save();
                }
            }
            let mut committed = false;
            row(
                ui,
                p,
                false,
                |ui| ts(ui, p, "Co-op password", "Friends need the same one to join"),
                |ui| {
                    committed = ui.add(egui::TextEdit::singleline(&mut self.pw[gi]).desired_width(190.0)).lost_focus();
                },
            );
            if committed {
                let value = self.pw[gi].clone();
                self.write_shared("cooppassword", &value, Some(gi));
            }
            row(
                ui,
                p,
                false,
                |ui| ts(ui, p, "Co-op settings file", "Scaling, invaders, ping display and more"),
                |ui| {
                    if let (true, Some(l)) = (small(ui, "Open file"), &self.views[gi].loc) {
                        platform::open_path(&launch::settings_path(g, &l.game_dir));
                    }
                },
            );
        });

        ui.add_space(6.0);
        match (installed, want, has_mod) {
            (false, ..) => drop(ui.label(T::new("This game isn't in your Steam library. Install it from Steam, or set its folder under Settings.").color(p.warn))),
            (_, true, false) => card(ui, p, |ui| {
                let left = |ui: &mut Ui| {
                    ui.label(T::new("Seamless Co-op isn't installed for this game").family(bold()).color(p.warn));
                    note(ui, p, "Drop the mod's zip anywhere in this window and it installs automatically.");
                };
                row(ui, p, true, left, |ui| {
                    if small(ui, "Open Mods") {
                        self.page = Page::Mods;
                    }
                });
            }),
            (_, true, true) => {
                let dir = &self.views[gi].loc.as_ref().unwrap().game_dir;
                note(ui, p, &format!("Starts with Seamless Co-op using save .{}. Your normal save is never touched, and every launch backs up first.", launch::expected_ext(g, dir, &st)));
            }
            _ => note(ui, p, "Starts the plain game through Steam with the mod loader off. Uses your normal .sl2 save."),
        }
        ui.add_space(18.0);

        section(ui, p, "SAVES", |ui| {
            if small(ui, "Backups folder") {
                platform::open_path(&saves::backup_root(g));
            }
            if small(ui, "Open folder") {
                self.views[gi].root.as_ref().or(self.views[gi].base.as_ref()).into_iter().for_each(|r| platform::open_path(r));
            }
            if small(ui, "Back up now") {
                let result = self.views[gi].root.as_ref().map(|r| saves::backup(g, r, "manual"));
                match result {
                    Some(Ok(Some(n))) => self.say(format!("Backed up to {n}"), false),
                    Some(Ok(None)) | None => self.say("No saves to back up", true),
                    Some(Err(e)) => self.say(format!("Backup failed: {e}"), true),
                }
                self.refresh(gi);
            }
        });
        let list: Vec<(PathBuf, String, u64, SystemTime, String)> =
            self.views[gi].saves.iter().zip(&self.views[gi].chars).map(|(s, c)| (s.path.clone(), s.ext.clone(), s.size, s.modified, c.clone())).collect();
        card(ui, p, |ui| {
            if list.is_empty() {
                row(ui, p, true, |ui| ts(ui, p, "No saves yet", "Launch the game once, or restore from a backup below."), |_| {});
            }
            for (i, (path, ext, size, modified, chars)) in list.iter().enumerate() {
                let left = |ui: &mut Ui| {
                    ui.horizontal(|ui| {
                        badge(ui, p, ext);
                        ui.label(T::new(g.label(ext)).family(bold()));
                    });
                    note(ui, p, &format!("{:.1} MB · last played {}", *size as f64 / 1_048_576.0, saves::fmt_time(*modified)));
                    if !chars.is_empty() {
                        ui.label(T::new(chars).size(12.0));
                    }
                };
                row(ui, p, i == 0, left, |ui| {
                    if small(ui, "Export") {
                        let dest = dirs::document_dir().unwrap_or_default().join(format!("{}-{}", g.key, path.file_name().unwrap().to_string_lossy()));
                        match std::fs::copy(path, &dest) {
                            Ok(_) => self.say(format!("Exported to {}", dest.display()), false),
                            Err(e) => self.say(format!("Export failed: {e}"), true),
                        }
                    }
                    if chars.starts_with("Characters:") && small(ui, "Copy character…") {
                        let from: Vec<(usize, String)> = slots::occupants(g, path).unwrap_or_default().into_iter().enumerate().filter_map(|(i, n)| Some((i, n?))).collect();
                        let dst = list.iter().position(|l| l.0 != *path).unwrap_or(0);
                        self.dialog = Some(Dialog::Character { gi, src: path.clone(), from, pick: 0, dst, loaded: None, slots: vec![], slot: 0 });
                    }
                    if small(ui, "Copy over…") {
                        self.dialog = Some(Dialog::CopyOver { gi, src: path.clone(), target: 0, ext: String::new() });
                    }
                });
            }
        });
        ui.add_space(18.0);

        section(ui, p, "BACKUPS", |ui| {
            if small(ui, "Clean auto-backups") {
                let files = self.views[gi].root.as_ref().map(|r| saves::auto_backups(r)).unwrap_or_default();
                if files.len() <= 5 {
                    self.say(format!("Nothing to clean ({} auto-backups)", files.len()), false);
                } else {
                    self.dialog = Some(Dialog::Clean { gi, files });
                }
            }
        });
        let backups: Vec<(String, String)> = self.views[gi]
            .backups
            .iter()
            .map(|b| (b.name.clone(), saves::restorable(b).iter().map(|f| format!(".{}", f.extension().unwrap_or_default().to_string_lossy())).collect::<Vec<_>>().join(" ")))
            .collect();
        card(ui, p, |ui| {
            if backups.is_empty() {
                row(ui, p, true, |ui| ts(ui, p, "No backups yet", "One is made automatically before every launch."), |_| {});
            }
            for (i, (name, exts)) in backups.iter().enumerate() {
                row(
                    ui,
                    p,
                    i == 0,
                    |ui| ts(ui, p, name, exts),
                    |ui| {
                        if ui.add_enabled(!exts.is_empty(), egui::Button::new(T::new("Restore…").size(12.0)).min_size(vec2(0.0, 24.0))).clicked() {
                            self.dialog = Some(Dialog::Restore { gi, bi: i, fi: 0 });
                        }
                    },
                );
            }
        });
        ui.add_space(8.0);
        note(ui, p, "Restoring and copying always back up the current save first.");
    }

    fn mods_page(&mut self, ui: &mut Ui, p: Pal) {
        title(ui, p, "Mods", "Drop a mod zip anywhere in this window. Souls-like works out which game it is for and installs it.");
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 120.0), Sense::hover());
        ui.painter().rect_filled(rect, 14, p.accent.gamma_multiply(0.08));
        ui.painter().rect_stroke(rect, 14, Stroke::new(1.5, p.accent.gamma_multiply(0.6)), StrokeKind::Inside);
        ui.painter().text(rect.center() - vec2(0.0, 12.0), Align2::CENTER_CENTER, "Drop mod zips here", FontId::proportional(17.0), p.text);
        ui.painter().text(rect.center() + vec2(0.0, 14.0), Align2::CENTER_CENTER, "Seamless Co-op for DS1, DS2, DS3 · Honest Merchant", FontId::proportional(12.0), p.dim);
        ui.add_space(14.0);

        if !self.pending.is_empty() {
            let (mut install, mut clear) = (false, false);
            card(ui, p, |ui| {
                for (i, (path, plan)) in self.pending.iter().enumerate() {
                    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let left = |ui: &mut Ui| match plan {
                        Ok(plan) => {
                            ui.label(T::new(plan.describe()).family(bold()));
                            note(ui, p, &format!("{name} · recognised from {}", plan.marker));
                        }
                        Err(e) => {
                            ui.label(T::new(&name).family(bold()));
                            ui.label(T::new(e).size(12.0).color(p.warn));
                        }
                    };
                    row(ui, p, i == 0, left, |ui| {
                        if plan.is_ok() {
                            pill(ui, "Ready", p.ok)
                        }
                    });
                }
                row(
                    ui,
                    p,
                    false,
                    |ui| note(ui, p, "Replaced files are backed up inside the game folder. Your settings files are kept."),
                    |ui| {
                        install = ui.add(egui::Button::new(T::new("Install").family(bold()).color(C::WHITE)).fill(p.accent)).clicked();
                        clear = small(ui, "Clear");
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

        ui.label(T::new("INSTALLED").size(11.0).family(bold()).color(p.dim));
        for (gi, g) in GAMES.iter().enumerate() {
            let v = &self.views[gi];
            let mut entries = vec![("Seamless Co-op".to_string(), v.seamless)];
            if let Some(l) = &v.loc {
                let has = |name: &str| l.game_dir.join(name).exists();
                entries.extend(g.mods.iter().map(|m| (m.title.to_string(), if m.id == "rando" { has("randomizer") } else { has("honestMerchant.ini") })));
            }
            ui.add_space(4.0);
            card(ui, p, |ui| {
                row(
                    ui,
                    p,
                    true,
                    |ui| drop(ui.label(T::new(g.name).family(bold()))),
                    |ui| {
                        if v.loc.is_none() {
                            pill(ui, "Not installed", p.warn)
                        }
                    },
                );
                for (name, present) in entries.into_iter().filter(|_| v.loc.is_some()) {
                    row(ui, p, false, |ui| drop(ui.label(name)), |ui| if present { pill(ui, "Installed", p.ok) } else { pill(ui, "Not installed — drop its zip", p.warn) });
                }
            });
        }
    }

    fn install_pending(&mut self) {
        let mut messages = vec![];
        let mut failed = false;
        for (_, plan) in std::mem::take(&mut self.pending).into_iter().filter_map(|(p, r)| Some((p, r.ok()?))) {
            let result = match &self.views[plan.game].loc {
                Some(loc) => modinstall::install(&plan, &loc.game_dir).map_err(|e| format!("Install failed: {e}")),
                None => Err(format!("{} is not installed.", GAMES[plan.game].name)),
            };
            failed |= result.is_err();
            messages.push(result.unwrap_or_else(|e| e));
        }
        self.request_refresh();
        self.load_texts();
        self.say(messages.join("  "), failed);
    }

    fn shared_page(&mut self, ui: &mut Ui, p: Pal) {
        title(ui, p, "Shared co-op settings", "One set of co-op settings for every game. Change one here and it is written to each game's Seamless Co-op settings file.");
        let n = self.targets().len();
        let mut on = self.cfg.mirror;
        card(ui, p, |ui| {
            row(
                ui,
                p,
                true,
                |ui| ts(ui, p, "Mirror across all games", &format!("{n} games with Seamless Co-op installed")),
                |ui| {
                    if toggle(ui, p, &mut on) {
                        self.cfg.mirror = on;
                        self.cfg.save();
                    }
                },
            );
        });
        ui.add_space(6.0);
        note(ui, p, "The save-file extension is never mirrored, so a modded game can't end up on your normal save.");
        ui.add_space(14.0);

        let mirror = self.cfg.mirror;
        let (mut edits, mut flips) = (vec![], vec![]);
        card(ui, p, |ui| {
            for (i, (key, label, help, kind)) in shared::SETTINGS.iter().enumerate() {
                let synced = !self.cfg.unsynced.contains(*key);
                let current = self.shared_vals.get(*key).cloned().unwrap_or_default();
                row(
                    ui,
                    p,
                    i == 0,
                    |ui| ts(ui, p, label, help),
                    |ui| {
                        ui.add_enabled_ui(mirror, |ui| {
                            let (text, color) = if synced { ("Synced", p.ok) } else { ("This game only", p.dim) };
                            let b = egui::Button::new(T::new(text).size(11.0).family(bold()).color(color))
                                .fill(if synced { p.ok.gamma_multiply(0.2) } else { p.field })
                                .corner_radius(99)
                                .min_size(vec2(96.0, 24.0));
                            if ui.add(b).on_hover_text("Click to change whether this setting is mirrored").clicked() {
                                flips.push((*key, !synced));
                            }
                        });
                        ui.add_enabled_ui(mirror && synced, |ui| match kind {
                            shared::Kind::Text => {
                                let mut text = current.clone();
                                let r = ui.add(egui::TextEdit::singleline(&mut text).desired_width(150.0));
                                if r.changed() {
                                    self.shared_vals.insert(key.to_string(), text.clone());
                                }
                                if r.lost_focus() {
                                    edits.push((*key, text));
                                }
                            }
                            shared::Kind::Bool => {
                                let mut b = current.trim() == "1";
                                if toggle(ui, p, &mut b) {
                                    edits.push((*key, (b as u8).to_string()));
                                }
                            }
                            shared::Kind::Int(lo, hi) => {
                                let before: i64 = current.trim().parse().unwrap_or(*lo);
                                let mut v = before;
                                ui.add(egui::DragValue::new(&mut v).range(*lo..=*hi));
                                if v != before {
                                    edits.push((*key, v.to_string()));
                                }
                            }
                        });
                    },
                );
            }
        });
        edits.into_iter().for_each(|(k, v)| self.write_shared(k, &v, None));
        for (key, on) in flips {
            if on {
                self.cfg.unsynced.remove(key);
            } else {
                self.cfg.unsynced.insert(key.into());
            }
            self.cfg.save();
        }
    }

    fn settings_page(&mut self, ui: &mut Ui, p: Pal) {
        title(ui, p, "Settings", "");
        ui.label(T::new("APPEARANCE").size(11.0).family(bold()).color(p.dim));
        card(ui, p, |ui| {
            let sel = ["system", "light", "dark"].iter().position(|t| *t == self.cfg.theme).unwrap_or(0);
            let mut chosen = None;
            row(ui, p, true, |ui| ts(ui, p, "Theme", "System follows your OS"), |ui| chosen = segmented(ui, p, &["System", "Light", "Dark"], sel));
            if let Some(i) = chosen {
                self.cfg.theme = ["system", "light", "dark"][i].into();
                self.cfg.save();
                self.ctx.set_theme(theme_pref(&self.cfg.theme));
            }
        });
        ui.add_space(18.0);
        ui.label(T::new("FOLDERS").size(11.0).family(bold()).color(p.dim));
        note(ui, p, "Leave blank to detect automatically from Steam.");
        let (mut changed, mut picks) = (false, vec![]);
        for (gi, g) in GAMES.iter().enumerate() {
            ui.add_space(4.0);
            card(ui, p, |ui| {
                let st = self.cfg.game(g.key);
                row(ui, p, true, |ui| ts(ui, p, g.name, ""), |_| {});
                for (is_game, label, value) in [(true, "Game folder", &mut st.game_dir), (false, "Save folder", &mut st.save_dir)] {
                    row(
                        ui,
                        p,
                        false,
                        |ui| ts(ui, p, label, ""),
                        |ui| {
                            if small(ui, "Browse…") {
                                picks.push((gi, is_game, value.clone()));
                            }
                            changed |= ui.add(egui::TextEdit::singleline(value).desired_width(300.0).hint_text("auto")).lost_focus();
                        },
                    );
                }
            });
        }
        if changed {
            self.cfg.save();
            self.request_refresh();
        }
        for (gi, is_game, current) in picks {
            let (tx, ctx) = (self.tx.clone(), self.ctx.clone());
            std::thread::spawn(move || {
                let mut dialog = rfd::FileDialog::new().set_title(if is_game { "Choose the game folder" } else { "Choose the save folder" });
                if Path::new(current.trim()).is_dir() {
                    dialog = dialog.set_directory(current.trim());
                }
                if let Some(path) = dialog.pick_folder() {
                    let _ = tx.send(Msg::Picked(gi, is_game, path));
                    ctx.request_repaint();
                }
            });
        }
        ui.add_space(18.0);
        ui.label(T::new("DETECTED").size(11.0).family(bold()).color(p.dim));
        card(ui, p, |ui| {
            let libs = platform::libraries();
            row(
                ui,
                p,
                true,
                |ui| {
                    ui.label(T::new("Steam libraries").family(bold()));
                    libs.iter().for_each(|l| note(ui, p, &l.display().to_string()));
                    if libs.is_empty() {
                        ui.label(T::new("None found").size(12.0).color(p.warn));
                    }
                },
                |_| {},
            );
            let platform_note = if platform::CAN_LAUNCH { "Games launch through Steam" } else { "Launching isn't supported on this OS yet; saves still work" };
            row(ui, p, false, |ui| ts(ui, p, "Version", &format!("Souls-like v{}", env!("CARGO_PKG_VERSION"))), |_| {});
            row(ui, p, false, |ui| ts(ui, p, "Platform", platform_note), |_| {});
            row(
                ui,
                p,
                false,
                |ui| ts(ui, p, "Logs", &crate::config::log_dir().display().to_string()),
                |ui| {
                    if small(ui, "Open") {
                        platform::open_path(&crate::config::log_dir());
                    }
                },
            );
        });
    }

    fn dialogs(&mut self, ctx: &egui::Context, p: Pal) {
        let Some(mut dialog) = self.dialog.take() else { return };
        let (mut keep, mut confirm) = (true, false);
        let modal = egui::Modal::new(egui::Id::new("dialog")).show(ctx, |ui| {
            ui.set_width(380.0);
            let heading = |ui: &mut Ui, h: &str, body: &str| {
                ui.label(T::new(h).size(16.0).family(bold()));
                ui.label(T::new(body).color(p.dim));
            };
            let caption = |ui: &mut Ui, text: &str| ui.label(T::new(text).size(12.0).color(p.dim));
            match &mut dialog {
                Dialog::CopyOver { gi, src, target, ext } => {
                    let g = &GAMES[*gi];
                    heading(ui, &format!("Copy .{} over…", src.extension().unwrap_or_default().to_string_lossy()), "The chosen slot is replaced with a copy of this save. A backup is made first.");
                    let others: Vec<&saves::SaveFile> = self.views[*gi].saves.iter().filter(|s| s.path != *src).collect();
                    let labels: Vec<String> = others.iter().map(|s| format!(".{}  {}", s.ext, g.label(&s.ext))).chain(["New slot…".to_string()]).collect();
                    *target = (*target).min(labels.len() - 1);
                    dropdown(ui, "target", target, &labels);
                    if *target == others.len() {
                        ui.add(egui::TextEdit::singleline(ext).hint_text("new extension, e.g. co2"));
                    }
                }
                Dialog::Restore { gi, bi, fi } => {
                    let g = &GAMES[*gi];
                    let backup = &self.views[*gi].backups[*bi];
                    heading(ui, &format!("Restore from {}", backup.name), "Pick the save to put back. It replaces the current one (a safety backup is made first).");
                    for (i, f) in saves::restorable(backup).iter().enumerate() {
                        let ext = f.extension().unwrap_or_default().to_string_lossy();
                        ui.radio_value(fi, i, format!(".{ext}  {}", g.label(&ext)));
                    }
                }
                Dialog::Character { gi, src, from, pick, dst, loaded, slots: occupied, slot } => {
                    let g = &GAMES[*gi];
                    let saves = &self.views[*gi].saves;
                    let ext = src.extension().unwrap_or_default().to_string_lossy();
                    heading(ui, "Copy a character", &format!("Copies one character from the .{ext} save into a slot of any save. A backup is made first."));
                    caption(ui, "Character");
                    dropdown(ui, "from", pick, &from.iter().map(|(i, n)| format!("Slot {} · {n}", i + 1)).collect::<Vec<_>>());
                    caption(ui, "Into save");
                    dropdown(ui, "dst", dst, &saves.iter().map(|s| format!(".{}  {}", s.ext, g.label(&s.ext))).collect::<Vec<_>>());
                    if *loaded != Some(*dst) {
                        *occupied = saves.get(*dst).and_then(|s| slots::occupants(g, &s.path).ok()).unwrap_or_default();
                        *slot = occupied.iter().position(Option::is_none).unwrap_or(0);
                        *loaded = Some(*dst);
                    }
                    caption(ui, "Into slot");
                    let labels: Vec<String> = occupied
                        .iter()
                        .enumerate()
                        .map(|(i, o)| match o {
                            Some(n) => format!("Slot {} · {n}  (will be replaced)", i + 1),
                            None => format!("Slot {} · empty", i + 1),
                        })
                        .collect();
                    dropdown(ui, "slot", slot, &labels);
                    if matches!(occupied.get(*slot), Some(Some(_))) {
                        ui.label(T::new("The character in that slot is overwritten. You can restore it from the backup.").size(12.0).color(p.warn));
                    }
                }
                Dialog::Clean { files, .. } => heading(ui, "Clean auto-backups", &format!("Keeps the newest 5 of {} Seamless Co-op auto-backups and deletes the rest.", files.len())),
            }
            ui.add_space(6.0);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                confirm = ui.add(egui::Button::new(T::new("Confirm").family(bold()).color(C::WHITE)).fill(C::from_rgb(0xff, 0x45, 0x3a))).clicked();
                keep = !ui.button("Cancel").clicked();
            });
        });
        match (confirm, keep && !modal.should_close()) {
            (true, _) => self.run_dialog(dialog),
            (_, true) => self.dialog = Some(dialog),
            _ => {}
        }
    }

    fn run_dialog(&mut self, dialog: Dialog) {
        let (gi, result): (usize, Result<String, String>) = match dialog {
            Dialog::CopyOver { gi, src, target, ext } => {
                let g = &GAMES[gi];
                let others: Vec<String> = self.views[gi].saves.iter().filter(|s| s.path != src).map(|s| s.ext.clone()).collect();
                let ext = others.get(target).cloned().unwrap_or_else(|| ext.chars().filter(char::is_ascii_alphanumeric).collect());
                let root = self.views[gi].root.clone();
                let result = match root {
                    _ if ext.is_empty() || ext == "bak" => Err("Enter a valid extension".into()),
                    _ if self.views[gi].running => Err(format!("Close {} first", g.name)),
                    Some(root) => saves::copy_over(g, &root, &src, &ext).map(|_| format!("Copied over .{ext}")).map_err(|e| format!("Copy failed: {e}")),
                    None => Err("No save folder yet".into()),
                };
                (gi, result)
            }
            Dialog::Restore { gi, bi, fi } => {
                let g = &GAMES[gi];
                let file = self.views[gi].backups.get(bi).and_then(|b| saves::restorable(b).get(fi).map(|f| (*f).clone()));
                let v = &self.views[gi];
                let root = v.root.clone().or_else(|| saves::children_dirs(v.base.as_deref()?).into_iter().next());
                let result = match (file, root) {
                    _ if v.running => Err(format!("Close {} first", g.name)),
                    (Some(f), Some(root)) => saves::restore(g, &root, &f).map(|_| format!("Restored {}", f.file_name().unwrap().to_string_lossy())).map_err(|e| format!("Restore failed: {e}")),
                    _ => Err("Launch the game once first so the save folder exists".into()),
                };
                (gi, result)
            }
            Dialog::Character { gi, src, from, pick, dst, slot, .. } => {
                let g = &GAMES[gi];
                let v = &self.views[gi];
                let (target, root, chosen) = (v.saves.get(dst).map(|s| s.path.clone()), v.root.clone(), from.get(pick).cloned());
                let result = match (target, root, chosen) {
                    _ if v.running => Err(format!("Close {} first", g.name)),
                    (Some(t), Some(_), Some((from_slot, _))) if t == src && from_slot == slot => Err("Pick a different slot or save.".into()),
                    (Some(t), Some(r), Some((from_slot, name))) => saves::backup(g, &r, "before-character-copy")
                        .map_err(|e| format!("Backup failed, nothing was changed: {e}"))
                        .and_then(|_| slots::copy_slot(g, &src, from_slot, &t, slot).map_err(|e| format!("Nothing was changed: {e}")))
                        .map(|_| format!("Copied {name} to slot {}", slot + 1)),
                    _ => Err("Nothing to copy.".into()),
                };
                (gi, result)
            }
            Dialog::Clean { gi, files } => {
                files.iter().skip(5).for_each(|f| drop(std::fs::remove_file(f)));
                (gi, Ok(format!("Deleted {} old auto-backups", files.len().saturating_sub(5))))
            }
        };
        self.say(result.clone().unwrap_or_else(|e| e), result.is_err());
        self.refresh(gi);
    }

    fn take_drops(&mut self, ctx: &egui::Context) {
        let paths: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        for path in &paths {
            let is_zip = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("zip"));
            let plan = if is_zip { modinstall::inspect(path) } else { Err("Only .zip files can be installed.".into()) };
            self.pending.push((path.clone(), plan));
        }
        if !paths.is_empty() {
            self.page = Page::Mods;
        }
    }

    fn take_messages(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                Msg::Launched(gi, result) => {
                    self.launching[gi] = false;
                    self.say(result.clone().unwrap_or_else(|e| e), result.is_err());
                    self.refresh(gi);
                }
                Msg::Views(views) => {
                    (self.views, self.refreshing, self.last_refresh) = (views, false, Instant::now());
                    self.load_icons();
                }
                Msg::Picked(gi, is_game, path) => {
                    let st = self.cfg.game(GAMES[gi].key);
                    *(if is_game { &mut st.game_dir } else { &mut st.save_dir }) = path.display().to_string();
                    self.cfg.save();
                    self.request_refresh();
                    self.load_texts();
                    self.say(format!("{} folder set for {}", if is_game { "Game" } else { "Save" }, GAMES[gi].name), false);
                }
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let p = pal(ui.visuals().dark_mode);
        self.take_messages();
        if self.last_refresh.elapsed() > Duration::from_secs(3) {
            self.request_refresh();
        }
        ctx.request_repaint_after(Duration::from_secs(3));
        self.take_drops(&ctx);

        egui::Panel::left("sidebar").resizable(false).exact_size(236.0).frame(Frame::new().fill(p.side).inner_margin(Margin::symmetric(12, 14))).show(ui, |ui| self.sidebar(ui, p));
        egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(40, 30))).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let avail = ui.available_width();
                let width = (avail - 16.0).clamp(300.0, 780.0);
                Frame::new().inner_margin(Margin { left: ((avail - width) / 2.0).max(0.0) as i8, ..Default::default() }).show(ui, |ui| {
                    ui.set_width(width);
                    match self.page {
                        Page::Game(i) => self.game_page(ui, p, i),
                        Page::Mods => self.mods_page(ui, p),
                        Page::Shared => self.shared_page(ui, p),
                        Page::Settings => self.settings_page(ui, p),
                    }
                    ui.add_space(20.0);
                });
            });
        });
        self.dialogs(&ctx, p);

        if ctx.input(|i| !i.raw.hovered_files.is_empty()) {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop")));
            painter.rect_filled(ctx.content_rect(), 0, C::from_black_alpha(150));
            painter.text(ctx.content_rect().center(), Align2::CENTER_CENTER, "Drop to install", FontId::proportional(28.0), C::WHITE);
        }
        if let Some((msg, at, err)) = self.toast.clone() {
            if at.elapsed() > Duration::from_secs(5) {
                self.toast = None;
            } else {
                egui::Area::new(egui::Id::new("toast")).order(egui::Order::Foreground).anchor(Align2::CENTER_BOTTOM, vec2(0.0, -18.0)).show(&ctx, |ui| {
                    Frame::new()
                        .fill(if err { C::from_rgb(0x8f, 0x2a, 0x22) } else { C::from_rgb(0x2c, 0x2c, 0x2e) })
                        .corner_radius(12)
                        .inner_margin(Margin::symmetric(16, 10))
                        .show(ui, |ui| ui.label(T::new(msg).color(C::WHITE)));
                });
                ctx.request_repaint_after(Duration::from_millis(500));
            }
        }
    }
}
