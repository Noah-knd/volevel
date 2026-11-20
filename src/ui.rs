use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use egui::{
    pos2, vec2, Align2, ColorImage, Context, CornerRadius, FontData, FontDefinitions, FontFamily,
    FontId, Frame, Margin, ScrollArea, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions,
    ViewportCommand,
};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{
    Icon as TrayIconImage, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

use crate::audio::{AudioCommand, AudioHandle, RgbaIcon, SessionView};
use crate::config::{AppConfig, ControlMode, Language};
use crate::i18n::{strings, Strings};
use crate::startup;
use crate::widgets::{
    duck_role_badge, help_badge, icon_button, mode_segmented, mute_button, paint_chevron_icon,
    paint_gear_icon, param_slider, pill_switch, subtle_button, volume_and_meter, Palette,
};

pub const EMBEDDED_ICON_PNG: &[u8] = include_bytes!("../assets/icon.png");

struct TrayMenuIds {
    open: String,
    bypass: String,
    auto_start: String,
    quit: String,
}

struct TrayController {
    tray: TrayIcon,
    color_icon: TrayIconImage,
    gray_icon: TrayIconImage,
    ids: TrayMenuIds,
}

impl TrayController {
    fn new(config: &AppConfig) -> Option<Self> {
        let (rgba, w, h) = decode_embedded_icon()?;
        let gray = to_grayscale_rgba(&rgba);
        let color_icon = TrayIconImage::from_rgba(rgba, w, h).ok()?;
        let gray_icon = TrayIconImage::from_rgba(gray, w, h).ok()?;
        let (menu, ids) = build_tray_menu(config);
        let s = strings(config.language);
        let icon = if config.bypass {
            gray_icon.clone()
        } else {
            color_icon.clone()
        };
        let tooltip = if config.bypass {
            s.tray_tooltip_bypassed
        } else {
            s.tray_tooltip_active
        };
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip(tooltip)
            .with_icon(icon)
            .build()
            .ok()?;
        Some(Self {
            tray,
            color_icon,
            gray_icon,
            ids,
        })
    }

    fn sync(&mut self, config: &AppConfig) {
        let s = strings(config.language);
        let (menu, ids) = build_tray_menu(config);
        self.ids = ids;
        self.tray.set_menu(Some(Box::new(menu)));
        let icon = if config.bypass {
            self.gray_icon.clone()
        } else {
            self.color_icon.clone()
        };
        let _ = self.tray.set_icon(Some(icon));
        let tip = if config.bypass {
            s.tray_tooltip_bypassed
        } else {
            s.tray_tooltip_active
        };
        let _ = self.tray.set_tooltip(Some(tip));
    }
}

fn build_tray_menu(config: &AppConfig) -> (Menu, TrayMenuIds) {
    let s = strings(config.language);
    let menu = Menu::new();
    let open = MenuItem::new(s.tray_open, true, None);
    let bypass = CheckMenuItem::new(s.tray_bypass, true, config.bypass, None);
    let auto_start = CheckMenuItem::new(s.tray_auto_start, true, config.auto_start, None);
    let quit = MenuItem::new(s.tray_quit, true, None);

    let ids = TrayMenuIds {
        open: open.id().0.clone(),
        bypass: bypass.id().0.clone(),
        auto_start: auto_start.id().0.clone(),
        quit: quit.id().0.clone(),
    };

    let _ = menu.append(&open);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&bypass);
    let _ = menu.append(&auto_start);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);
    (menu, ids)
}


#[cfg(windows)]
fn wake_native_window() {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowTextW, GetWindowThreadProcessId, SetForegroundWindow, ShowWindow,
        SW_RESTORE, SW_SHOW,
    };

    struct Search {
        pid: u32,
        found: HWND,
    }

    unsafe extern "system" fn enum_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut Search);
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == state.pid {
            let mut buf = [0u16; 64];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 && String::from_utf16_lossy(&buf[..len as usize]) == "Volevel" {
                state.found = hwnd;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    unsafe {
        let mut search = Search {
            pid: GetCurrentProcessId(),
            found: HWND::default(),
        };
        let _ = EnumWindows(Some(enum_proc), LPARAM(&mut search as *mut _ as isize));
        if !search.found.0.is_null() {
            let _ = ShowWindow(search.found, SW_SHOW);
            let _ = ShowWindow(search.found, SW_RESTORE);
            let _ = SetForegroundWindow(search.found);
        }
    }
}

#[cfg(not(windows))]
fn wake_native_window() {}

pub fn decode_embedded_icon() -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(EMBEDDED_ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

fn to_grayscale_rgba(rgba: &[u8]) -> Vec<u8> {
    let mut out = rgba.to_vec();
    for px in out.chunks_exact_mut(4) {
        let lum = (0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32) as u8;
        let muted = ((lum as u16 * 85) / 100) as u8;
        px[0] = muted;
        px[1] = muted;
        px[2] = muted;
    }
    out
}

pub fn install_windows_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    // Use Yu Gothic UI / Meiryo as primary proportional font so Latin and Japanese share one baseline.
    let ui_candidates = [
        r"C:\Windows\Fonts\YuGothM.ttc",
        r"C:\Windows\Fonts\YuGothR.ttc",
        r"C:\Windows\Fonts\meiryo.ttc",
        r"C:\Windows\Fonts\segoeui.ttf",
    ];
    let mono_candidates = [r"C:\Windows\Fonts\consola.ttf"];

    if let Some(bytes) = load_first_existing(&ui_candidates) {
        fonts
            .font_data
            .insert("win_ui".into(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "win_ui".into());
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .push("win_ui".into());
    }
    if let Some(bytes) = load_first_existing(&mono_candidates) {
        fonts
            .font_data
            .insert("win_mono".into(), Arc::new(FontData::from_owned(bytes)));
        fonts
            .families
            .entry(FontFamily::Monospace)
            .or_default()
            .insert(0, "win_mono".into());
    }
    ctx.set_fonts(fonts);
}

fn load_first_existing(paths: &[&str]) -> Option<Vec<u8>> {
    for p in paths {
        if let Ok(bytes) = std::fs::read(p) {
            return Some(bytes);
        }
    }
    None
}

pub fn apply_theme(ctx: &Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = Palette::BG;
    visuals.panel_fill = Palette::BG;
    visuals.extreme_bg_color = Palette::TRACK;
    visuals.faint_bg_color = Palette::ROW;
    visuals.override_text_color = Some(Palette::TEXT);
    visuals.window_stroke = Stroke::new(1.0_f32, Palette::BORDER);
    visuals.window_corner_radius = CornerRadius::same(10);
    ctx.set_visuals(visuals);
}

pub struct VolevelApp {
    config: AppConfig,
    audio: AudioHandle,
    tray: Option<TrayController>,
    logo_tex: Option<TextureHandle>,
    app_textures: HashMap<String, TextureHandle>,
    expanded: HashSet<String>,
    settings_open: bool,
    window_visible: Arc<AtomicBool>,
    pending_show: Arc<AtomicBool>,
    quitting: bool,
}

impl VolevelApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        mut config: AppConfig,
        visible: bool,
        show_event: usize,
    ) -> Self {
        install_windows_fonts(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx);

        config.auto_start = startup::is_enabled();
        let logo_tex = decode_embedded_icon().map(|(rgba, w, h)| {
            let img = ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            cc.egui_ctx
                .load_texture("volevel_logo", img, TextureOptions::LINEAR)
        });
        let tray = TrayController::new(&config);
        let audio = AudioHandle::start(config.clone());

        let window_visible = Arc::new(AtomicBool::new(visible));
        let pending_show = Arc::new(AtomicBool::new(false));
        let ctx_bg = cc.egui_ctx.clone();
        let vis_bg = Arc::clone(&window_visible);
        let show_bg = Arc::clone(&pending_show);
        std::thread::spawn(move || {
            loop {
                let mut request_show = false;
                #[cfg(windows)]
                if show_event != 0 {
                    use windows::Win32::Foundation::{HANDLE, WAIT_OBJECT_0};
                    use windows::Win32::System::Threading::WaitForSingleObject;
                    if unsafe {
                        WaitForSingleObject(HANDLE(show_event as *mut _), 100) == WAIT_OBJECT_0
                    } {
                        request_show = true;
                    }
                } else {
                    std::thread::sleep(Duration::from_millis(100));
                }
                #[cfg(not(windows))]
                std::thread::sleep(Duration::from_millis(100));

                // Drain tray icon events here so hover/move events never trigger a window wake.
                while let Ok(ev) = TrayIconEvent::receiver().try_recv() {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        request_show = true;
                    }
                }

                if request_show {
                    show_bg.store(true, Ordering::SeqCst);
                    vis_bg.store(true, Ordering::SeqCst);
                    wake_native_window();
                    ctx_bg.send_viewport_cmd(ViewportCommand::Visible(true));
                    ctx_bg.send_viewport_cmd(ViewportCommand::Minimized(false));
                    ctx_bg.send_viewport_cmd(ViewportCommand::Focus);
                    ctx_bg.request_repaint();
                } else if !MenuEvent::receiver().is_empty() {
                    ctx_bg.request_repaint();
                }
            }
        });

        Self {
            config,
            audio,
            tray,
            logo_tex,
            app_textures: HashMap::new(),
            expanded: HashSet::new(),
            settings_open: false,
            window_visible,
            pending_show,
            quitting: false,
        }
    }

    fn set_bypass(&mut self, bypass: bool) {
        self.config.bypass = bypass;
        let _ = self.audio.tx.send(AudioCommand::SetBypass(bypass));
        if let Some(tray) = &mut self.tray {
            tray.sync(&self.config);
        }
    }

    fn set_auto_start(&mut self, enable: bool) {
        if startup::set_enabled(enable).is_ok() {
            self.config.auto_start = enable;
            let _ = self.audio.tx.send(AudioCommand::SetAutoStart(enable));
            if let Some(tray) = &mut self.tray {
                tray.sync(&self.config);
            }
        }
    }

    fn set_language(&mut self, lang: Language) {
        if self.config.language != lang {
            self.config.language = lang;
            let _ = self.audio.tx.send(AudioCommand::SetLanguage(lang));
            if let Some(tray) = &mut self.tray {
                tray.sync(&self.config);
            }
        }
    }

    fn show_window(&self, ctx: &Context) {
        self.window_visible.store(true, Ordering::SeqCst);
        wake_native_window();
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
    }

    fn poll_tray_events(&mut self, ctx: &Context) {
        if self.pending_show.swap(false, Ordering::SeqCst) {
            self.show_window(ctx);
        }


        let mut actions = Vec::new();
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            if let Some(tray) = &self.tray {
                let id = ev.id.0.as_str();
                if id == tray.ids.open {
                    actions.push(0u8);
                } else if id == tray.ids.bypass {
                    actions.push(1u8);
                } else if id == tray.ids.auto_start {
                    actions.push(2u8);
                } else if id == tray.ids.quit {
                    actions.push(3u8);
                }
            }
        }

        for a in actions {
            match a {
                0 => self.show_window(ctx),
                1 => self.set_bypass(!self.config.bypass),
                2 => self.set_auto_start(!self.config.auto_start),
                3 => {
                    self.quitting = true;
                    self.audio.shutdown();
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
                _ => {}
            }
        }
    }

    fn render_header(&mut self, ui: &mut egui::Ui, s: &Strings) {
        Frame::new()
            .fill(Palette::BG)
            .inner_margin(Margin::symmetric(16, 12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    if let Some(logo) = &self.logo_tex {
                        ui.add(egui::Image::new(logo).fit_to_exact_size(vec2(22.0, 22.0)));
                    }
                    ui.label(
                        egui::RichText::new("Volevel")
                            .size(15.0)
                            .strong()
                            .color(Palette::TEXT),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        if icon_button(
                            ui,
                            28.0,
                            self.settings_open,
                            s.settings_tooltip,
                            paint_gear_icon,
                        )
                        .clicked()
                        {
                            self.settings_open = !self.settings_open;
                        }

                        let active = !self.config.bypass;
                        let status_label = if active {
                            s.status_active
                        } else {
                            s.status_bypassed
                        };
                        if pill_switch(ui, active, Palette::EMERALD, status_label, s.bypass_tooltip)
                        {
                            self.set_bypass(!self.config.bypass);
                        }
                    });
                });
            });

        let rect = ui.min_rect();
        ui.painter().line_segment(
            [pos2(rect.left(), rect.bottom()), pos2(rect.right(), rect.bottom())],
            Stroke::new(1.0_f32, Palette::BORDER),
        );
    }

    fn render_column_headers(&self, ui: &mut egui::Ui, s: &Strings) {
        ui.horizontal(|ui| {
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 10.0;

            let app_w = 24.0 + 10.0 + 112.0 + 10.0 + 28.0;
            let (r_app, _) = ui.allocate_exact_size(vec2(app_w, 16.0), Sense::hover());
            ui.painter().text(
                pos2(r_app.left(), r_app.center().y),
                Align2::LEFT_CENTER,
                s.col_app,
                FontId::new(10.5, FontFamily::Proportional),
                Palette::TEXT_DIM,
            );

            let right_controls_w = 166.0 + 52.0 + 24.0 + 24.0 + 12.0;
            let meter_w = (ui.available_width() - right_controls_w).max(150.0);

            for (w, label, tip) in [
                (meter_w, s.col_volume_meter, s.col_volume_meter_tip),
                (166.0, s.col_mode, s.col_mode_tip),
                (76.0, s.col_ducking, s.col_ducking_tip),
            ] {
                ui.allocate_ui_with_layout(
                    vec2(w, 16.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_width(w);
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.label(
                            egui::RichText::new(label)
                                .size(10.5)
                                .color(Palette::TEXT_DIM),
                        );
                        help_badge(ui, tip);
                    },
                );
            }
        });
    }

    fn render_settings_popover(&mut self, ctx: &Context, s: &Strings) {
        if !self.settings_open {
            return;
        }
        let popover_w = 328.0;
        let anchor = pos2((ctx.screen_rect().right() - popover_w - 16.0).max(12.0), 54.0);
        egui::Area::new(egui::Id::new("volevel_settings_popover"))
            .fixed_pos(anchor)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                Frame::new()
                    .fill(Palette::ROW)
                    .stroke(Stroke::new(1.0_f32, Palette::BORDER))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(Margin::same(14))
                    .show(ui, |ui| {
                        ui.set_width(popover_w - 28.0);
                        ui.spacing_mut().item_spacing.y = 10.0;

                        ui.label(
                            egui::RichText::new(s.settings_title)
                                .size(13.0)
                                .strong()
                                .color(Palette::TEXT),
                        );

                        ui.label(
                            egui::RichText::new(s.settings_general)
                                .size(10.5)
                                .color(Palette::TEXT_DIM),
                        );

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(
                                egui::RichText::new(s.language_label)
                                    .size(12.0)
                                    .color(Palette::TEXT_MUTED),
                            );
                            help_badge(ui, s.language_tip);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    for (lang, label) in
                                        [(Language::Japanese, "日本語"), (Language::English, "EN")]
                                    {
                                        let selected = self.config.language == lang;
                                        let (rect, resp) = ui.allocate_exact_size(
                                            vec2(56.0, 22.0),
                                            Sense::click(),
                                        );
                                        ui.painter().rect_filled(
                                            rect,
                                            CornerRadius::same(5),
                                            if selected {
                                                Palette::SEGMENT_ACTIVE
                                            } else if resp.hovered() {
                                                Palette::ROW_HOVER
                                            } else {
                                                Palette::TRACK
                                            },
                                        );
                                        ui.painter().rect_stroke(
                                            rect,
                                            CornerRadius::same(5),
                                            Stroke::new(1.0_f32, Palette::BORDER),
                                            StrokeKind::Inside,
                                        );
                                        ui.painter().text(
                                            rect.center(),
                                            Align2::CENTER_CENTER,
                                            label,
                                            FontId::new(11.0, FontFamily::Proportional),
                                            if selected {
                                                Palette::TEXT
                                            } else {
                                                Palette::TEXT_MUTED
                                            },
                                        );
                                        if resp.clicked() {
                                            self.set_language(lang);
                                        }
                                    }
                                },
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(
                                egui::RichText::new(s.auto_start_label)
                                    .size(12.0)
                                    .color(Palette::TEXT_MUTED),
                            );
                            help_badge(ui, s.auto_start_tip);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let on = self.config.auto_start;
                                    let (rect, resp) =
                                        ui.allocate_exact_size(vec2(32.0, 18.0), Sense::click());
                                    ui.painter().rect_filled(
                                        rect,
                                        CornerRadius::same(9),
                                        if on { Palette::EMERALD } else { Palette::BORDER },
                                    );
                                    let kx = if on {
                                        rect.right() - 9.0
                                    } else {
                                        rect.left() + 9.0
                                    };
                                    ui.painter().circle_filled(
                                        pos2(kx, rect.center().y),
                                        6.0,
                                        Palette::TEXT,
                                    );
                                    if resp.on_hover_text(s.auto_start_tip).clicked() {
                                        self.set_auto_start(!on);
                                    }
                                },
                            );
                        });

                        let sep = ui
                            .allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover())
                            .0;
                        ui.painter()
                            .rect_filled(sep, CornerRadius::ZERO, Palette::BORDER);

                        ui.label(
                            egui::RichText::new(s.settings_ducking)
                                .size(11.0)
                                .color(Palette::TEXT),
                        );
                        ui.label(
                            egui::RichText::new(s.settings_ducking_desc)
                                .size(10.5)
                                .color(Palette::TEXT_DIM),
                        );

                        let mut duck_changed = false;
                        let d = &mut self.config.ducking;
                        duck_changed |= param_slider(
                            ui,
                            s.duck_threshold_label,
                            s.duck_threshold_tip,
                            &mut d.trigger_threshold,
                            0.01..=0.30,
                            Palette::CYAN,
                            |v| format!("{:>3}%", (v * 100.0).round() as i32),
                        );
                        duck_changed |= param_slider(
                            ui,
                            s.duck_reduction_label,
                            s.duck_reduction_tip,
                            &mut d.reduction,
                            0.10..=0.90,
                            Palette::AMBER,
                            |v| format!("-{:>2}%", (v * 100.0).round() as i32),
                        );
                        duck_changed |= param_slider(
                            ui,
                            s.duck_hold_label,
                            s.duck_hold_tip,
                            &mut d.hold_ms,
                            100.0..=2000.0,
                            Palette::TEXT_MUTED,
                            |v| format!("{:>4}ms", v.round() as i32),
                        );
                        duck_changed |= param_slider(
                            ui,
                            s.duck_release_label,
                            s.duck_release_tip,
                            &mut d.release_ms,
                            100.0..=2000.0,
                            Palette::TEXT_MUTED,
                            |v| format!("{:>4}ms", v.round() as i32),
                        );
                        if duck_changed {
                            let _ = self
                                .audio
                                .tx
                                .send(AudioCommand::UpdateDucking(self.config.ducking.clone()));
                        }

                        if !self.config.hidden_apps.is_empty() {
                            let sep_h = ui
                                .allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover())
                                .0;
                            ui.painter()
                                .rect_filled(sep_h, CornerRadius::ZERO, Palette::BORDER);

                            ui.label(
                                egui::RichText::new(s.settings_hidden)
                                    .size(11.0)
                                    .color(Palette::TEXT),
                            );

                            let hidden_list: Vec<String> =
                                self.config.hidden_apps.iter().cloned().collect();
                            for key in hidden_list {
                                ui.horizontal(|ui| {
                                    let label = key.strip_suffix(".exe").unwrap_or(&key);
                                    ui.label(
                                        egui::RichText::new(label)
                                            .size(11.5)
                                            .color(Palette::TEXT_MUTED),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if subtle_button(
                                                ui,
                                                s.unhide_label,
                                                s.unhide_tip,
                                                false,
                                                false,
                                            ) {
                                                self.config.hidden_apps.remove(&key);
                                                let _ = self.audio.tx.send(
                                                    AudioCommand::SetHidden {
                                                        key: key.clone(),
                                                        hidden: false,
                                                    },
                                                );
                                            }
                                        },
                                    );
                                });
                            }
                        }

                        let sep2 = ui
                            .allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover())
                            .0;
                        ui.painter()
                            .rect_filled(sep2, CornerRadius::ZERO, Palette::BORDER);

                        if subtle_button(ui, s.reset_all_label, s.reset_all_tip, true, true) {
                            self.config.ducking = crate::config::DuckingConfig::default();
                            let _ = self.audio.tx.send(AudioCommand::ResetAll);
                        }
                    });
            });
    }

    fn render_session_row(&mut self, ui: &mut egui::Ui, session: &SessionView, s: &Strings) {
        let is_expanded = self.expanded.contains(&session.key);
        let mut rule = session.rule.clone();
        let mut rule_changed = false;

        ui.push_id(&session.key, |ui| {
            Frame::new()
                .fill(Palette::ROW)
                .stroke(Stroke::new(1.0_f32, Palette::BORDER))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::symmetric(12, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = 8.0;

                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;

                        if let Some(tex) = self.app_textures.get(&session.key) {
                            ui.add(egui::Image::new(tex).fit_to_exact_size(vec2(24.0, 24.0)));
                        } else {
                            let (badge, _) =
                                ui.allocate_exact_size(vec2(24.0, 24.0), Sense::hover());
                            ui.painter().rect_filled(
                                badge,
                                CornerRadius::same(6),
                                Palette::SEGMENT_ACTIVE,
                            );
                            let initial = session
                                .display_name
                                .chars()
                                .next()
                                .unwrap_or('?')
                                .to_uppercase()
                                .to_string();
                            ui.painter().text(
                                badge.center(),
                                Align2::CENTER_CENTER,
                                initial,
                                FontId::new(12.0, FontFamily::Proportional),
                                Palette::TEXT_MUTED,
                            );
                        }

                        let name_w = 112.0;
                        let (name_rect, _) =
                            ui.allocate_exact_size(vec2(name_w, 28.0), Sense::hover());
                        let clipped = ui.painter().with_clip_rect(name_rect);
                        clipped.text(
                            pos2(name_rect.left(), name_rect.center().y),
                            Align2::LEFT_CENTER,
                            &session.display_name,
                            FontId::new(12.5, FontFamily::Proportional),
                            if session.muted {
                                Palette::TEXT_DIM
                            } else {
                                Palette::TEXT
                            },
                        );

                        if mute_button(ui, session.muted, s).clicked() {
                            let _ = self.audio.tx.send(AudioCommand::ToggleMute {
                                key: session.key.clone(),
                            });
                        }

                        let right_controls_w = 166.0 + 52.0 + 24.0 + 24.0;
                        let meter_w = (ui.available_width() - right_controls_w).max(150.0);

                        let threshold = match rule.mode {
                            ControlMode::Limiter => rule.ceiling,
                            ControlMode::Leveler => rule.target,
                            ControlMode::Off => rule.ceiling,
                        };

                        let interaction = volume_and_meter(
                            ui,
                            meter_w,
                            session.base_volume,
                            session.effective_volume,
                            session.peak,
                            session.gain_reduction,
                            session.muted,
                            self.config.bypass,
                            rule.mode,
                            threshold,
                            s,
                        );

                        if let Some(v) = interaction.new_base_volume {
                            let _ = self.audio.tx.send(AudioCommand::SetBaseVolume {
                                key: session.key.clone(),
                                volume: v,
                            });
                        }
                        if let Some(t) = interaction.new_threshold {
                            match rule.mode {
                                ControlMode::Limiter => rule.ceiling = t,
                                ControlMode::Leveler => rule.target = t,
                                ControlMode::Off => {}
                            }
                            rule_changed = true;
                        }

                        if mode_segmented(ui, &mut rule.mode, s) {
                            rule_changed = true;
                        }

                        if duck_role_badge(ui, &mut rule.duck_role, session.ducking_active, s) {
                            rule_changed = true;
                        }

                        if icon_button(ui, 24.0, is_expanded, s.details_tip, |p, r, c| {
                            paint_chevron_icon(p, r, is_expanded, c);
                        })
                        .clicked()
                        {
                            if is_expanded {
                                self.expanded.remove(&session.key);
                            } else {
                                self.expanded.insert(session.key.clone());
                            }
                        }
                    });

                    if is_expanded {
                        let sep = ui
                            .allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover())
                            .0;
                        ui.painter()
                            .rect_filled(sep, CornerRadius::ZERO, Palette::BORDER_SUBTLE);

                        Frame::NONE
                            .inner_margin(Margin {
                                left: 8,
                                right: 0,
                                top: 2,
                                bottom: 0,
                            })
                            .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 6.0;
                            match rule.mode {
                                ControlMode::Limiter => {
                                    rule_changed |= param_slider(
                                        ui,
                                        s.ceiling_label,
                                        s.ceiling_tip,
                                        &mut rule.ceiling,
                                        0.05..=1.0,
                                        Palette::AMBER,
                                        |v| format!("{:>3}%", (v * 100.0).round() as i32),
                                    );
                                    rule_changed |= param_slider(
                                        ui,
                                        s.release_label,
                                        s.release_tip,
                                        &mut rule.release_ms,
                                        50.0..=2000.0,
                                        Palette::TEXT_MUTED,
                                        |v| format!("{:>4}ms", v.round() as i32),
                                    );
                                }
                                ControlMode::Leveler => {
                                    rule_changed |= param_slider(
                                        ui,
                                        s.target_label,
                                        s.target_tip,
                                        &mut rule.target,
                                        0.05..=1.0,
                                        Palette::CYAN,
                                        |v| format!("{:>3}%", (v * 100.0).round() as i32),
                                    );
                                    rule_changed |= param_slider(
                                        ui,
                                        s.noise_gate_label,
                                        s.noise_gate_tip,
                                        &mut rule.noise_gate,
                                        0.005..=0.20,
                                        Palette::EMERALD,
                                        |v| format!("{:>3}%", (v * 100.0).round() as i32),
                                    );
                                    rule_changed |= param_slider(
                                        ui,
                                        s.max_gain_label,
                                        s.max_gain_tip,
                                        &mut rule.max_gain,
                                        0.10..=1.0,
                                        Palette::AMBER,
                                        |v| format!("{:>3}%", (v * 100.0).round() as i32),
                                    );
                                    rule_changed |= param_slider(
                                        ui,
                                        s.release_label,
                                        s.release_tip,
                                        &mut rule.release_ms,
                                        100.0..=2000.0,
                                        Palette::TEXT_MUTED,
                                        |v| format!("{:>4}ms", v.round() as i32),
                                    );
                                }
                                ControlMode::Off => {
                                    rule_changed |= param_slider(
                                        ui,
                                        s.ceiling_label,
                                        s.ceiling_tip,
                                        &mut rule.ceiling,
                                        0.05..=1.0,
                                        Palette::AMBER,
                                        |v| format!("{:>3}%", (v * 100.0).round() as i32),
                                    );
                                    rule_changed |= param_slider(
                                        ui,
                                        s.release_label,
                                        s.release_tip,
                                        &mut rule.release_ms,
                                        50.0..=2000.0,
                                        Palette::TEXT_MUTED,
                                        |v| format!("{:>4}ms", v.round() as i32),
                                    );
                                }
                            }
                            ui.add_space(4.0);
                            ui.allocate_ui_with_layout(
                                vec2(ui.available_width(), 24.0),
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().item_spacing.x = 8.0;
                                    if subtle_button(
                                        ui,
                                        s.reset_app_label,
                                        s.reset_app_tip,
                                        false,
                                        false,
                                    ) {
                                        let _ = self.audio.tx.send(AudioCommand::ResetApp {
                                            key: session.key.clone(),
                                        });
                                        rule_changed = false;
                                    }
                                    if subtle_button(
                                        ui,
                                        s.hide_app_label,
                                        s.hide_app_tip,
                                        false,
                                        false,
                                    ) {
                                        self.config.hidden_apps.insert(session.key.clone());
                                        self.expanded.remove(&session.key);
                                        let _ = self.audio.tx.send(AudioCommand::SetHidden {
                                            key: session.key.clone(),
                                            hidden: true,
                                        });
                                        rule_changed = false;
                                    }
                                },
                            );
                        });
                    }
                });
        });

        if rule_changed {
            let _ = self.audio.tx.send(AudioCommand::UpdateRule {
                key: session.key.clone(),
                rule,
            });
        }
    }
}

impl eframe::App for VolevelApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        if !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            self.window_visible.store(false, Ordering::SeqCst);
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(ViewportCommand::Visible(false));
        }

        self.poll_tray_events(ctx);

        if !self.window_visible.load(Ordering::SeqCst) {
            return;
        }

        let (sessions, new_icons): (Vec<SessionView>, Vec<(String, RgbaIcon)>) = {
            let snap = self.audio.snapshot.lock().unwrap();
            let missing = snap
                .icons
                .iter()
                .filter(|(k, _)| !self.app_textures.contains_key(*k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            (snap.sessions.clone(), missing)
        };
        for (k, icon) in new_icons {
            let img = ColorImage::from_rgba_unmultiplied([icon.width, icon.height], &icon.rgba);
            let tex = ctx.load_texture(format!("app_icon_{k}"), img, TextureOptions::LINEAR);
            self.app_textures.insert(k, tex);
        }

        let s = strings(self.config.language);

        egui::TopBottomPanel::top("header")
            .frame(Frame::NONE.fill(Palette::BG))
            .show(ctx, |ui| {
                self.render_header(ui, s);
            });

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(Palette::BG).inner_margin(Margin::same(16)))
            .show(ctx, |ui| {
                if sessions.is_empty() {
                    let avail = ui.available_rect_before_wrap();
                    ui.painter().text(
                        pos2(avail.center().x, avail.center().y - 12.0),
                        Align2::CENTER_CENTER,
                        s.empty_title,
                        FontId::new(14.0, FontFamily::Proportional),
                        Palette::TEXT_MUTED,
                    );
                    ui.painter().text(
                        pos2(avail.center().x, avail.center().y + 12.0),
                        Align2::CENTER_CENTER,
                        s.empty_subtitle,
                        FontId::new(11.5, FontFamily::Proportional),
                        Palette::TEXT_DIM,
                    );
                } else {
                    self.render_column_headers(ui, s);
                    ui.add_space(6.0);
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        for session in &sessions {
                            if self.config.hidden_apps.contains(&session.key) {
                                continue;
                            }
                            self.render_session_row(ui, session, s);
                        }
                    });
                }
            });

        self.render_settings_popover(ctx, s);

        ctx.request_repaint_after(Duration::from_millis(33));
    }
}
