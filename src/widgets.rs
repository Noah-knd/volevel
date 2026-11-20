use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Response,
    Sense, Stroke, StrokeKind, Ui,
};

use crate::config::{ControlMode, DuckRole};
use crate::i18n::Strings;

pub struct Palette;

impl Palette {
    pub const BG: Color32 = Color32::from_rgb(0x0E, 0x10, 0x14);
    pub const ROW: Color32 = Color32::from_rgb(0x15, 0x18, 0x20);
    pub const ROW_HOVER: Color32 = Color32::from_rgb(0x1C, 0x20, 0x2B);
    pub const TRACK: Color32 = Color32::from_rgb(0x0B, 0x0D, 0x11);
    pub const BORDER: Color32 = Color32::from_rgb(0x24, 0x29, 0x36);
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(0x1D, 0x22, 0x2E);
    pub const TEXT: Color32 = Color32::from_rgb(0xED, 0xF0, 0xF5);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(0x8B, 0x93, 0xA6);
    pub const TEXT_DIM: Color32 = Color32::from_rgb(0x55, 0x5D, 0x70);
    pub const EMERALD: Color32 = Color32::from_rgb(0x10, 0xB9, 0x81);
    pub const CYAN: Color32 = Color32::from_rgb(0x22, 0xD3, 0xEE);
    pub const AMBER: Color32 = Color32::from_rgb(0xF5, 0x9E, 0x0B);
    pub const AMBER_FILL: Color32 = Color32::from_rgba_premultiplied(0x45, 0x2D, 0x03, 0x48);
    pub const RED_MUTE: Color32 = Color32::from_rgb(0xE0, 0x52, 0x63);
    pub const RED_MUTE_BG: Color32 = Color32::from_rgba_premultiplied(0x21, 0x0C, 0x0F, 0x26);
    pub const CYAN_BG: Color32 = Color32::from_rgba_premultiplied(0x05, 0x1E, 0x22, 0x24);
    pub const SEGMENT_ACTIVE: Color32 = Color32::from_rgb(0x22, 0x27, 0x36);
}

pub fn help_badge(ui: &mut Ui, tooltip: &str) {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() {
            Palette::TEXT
        } else {
            Palette::TEXT_DIM
        };
        ui.painter()
            .circle_stroke(rect.center(), 6.0, Stroke::new(1.0_f32, color));
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            "?",
            FontId::new(9.0, FontFamily::Proportional),
            color,
        );
    }
    resp.on_hover_text(tooltip);
}

pub fn paint_speaker_icon(painter: &Painter, rect: Rect, muted: bool, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.42;
    let stroke = Stroke::new(1.5_f32, color);

    let cone = vec![
        pos2(c.x - s * 0.85, c.y - s * 0.32),
        pos2(c.x - s * 0.38, c.y - s * 0.32),
        pos2(c.x + s * 0.08, c.y - s * 0.72),
        pos2(c.x + s * 0.08, c.y + s * 0.72),
        pos2(c.x - s * 0.38, c.y + s * 0.32),
        pos2(c.x - s * 0.85, c.y + s * 0.32),
    ];
    painter.add(egui::Shape::closed_line(cone, stroke));

    if muted {
        painter.line_segment(
            [pos2(c.x - s * 0.9, c.y - s * 0.85), pos2(c.x + s * 0.9, c.y + s * 0.85)],
            Stroke::new(1.6_f32, Palette::RED_MUTE),
        );
    } else {
        paint_arc(painter, pos2(c.x + s * 0.14, c.y), s * 0.42, -0.75, 0.75, stroke);
        paint_arc(painter, pos2(c.x + s * 0.14, c.y), s * 0.76, -0.82, 0.82, stroke);
    }
}

fn paint_arc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start_rad: f32,
    end_rad: f32,
    stroke: Stroke,
) {
    let steps = 8;
    let pts: Vec<Pos2> = (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let a = start_rad + (end_rad - start_rad) * t;
            pos2(center.x + radius * a.cos(), center.y + radius * a.sin())
        })
        .collect();
    painter.add(egui::Shape::line(pts, stroke));
}

pub fn paint_gear_icon(painter: &Painter, rect: Rect, color: Color32) {
    let c = rect.center();
    let r_outer = rect.width().min(rect.height()) * 0.38;
    let r_inner = r_outer * 0.46;
    let stroke = Stroke::new(1.5_f32, color);
    painter.circle_stroke(c, r_outer * 0.76, stroke);
    painter.circle_stroke(c, r_inner * 0.72, stroke);
    for i in 0..8 {
        let a = (i as f32) * std::f32::consts::FRAC_PI_4;
        let p0 = pos2(c.x + a.cos() * r_outer * 0.76, c.y + a.sin() * r_outer * 0.76);
        let p1 = pos2(c.x + a.cos() * r_outer * 1.08, c.y + a.sin() * r_outer * 1.08);
        painter.line_segment([p0, p1], Stroke::new(1.8_f32, color));
    }
}

pub fn paint_chevron_icon(painter: &Painter, rect: Rect, expanded: bool, color: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height()) * 0.24;
    let pts = if expanded {
        vec![
            pos2(c.x - s, c.y - s * 0.45),
            pos2(c.x, c.y + s * 0.55),
            pos2(c.x + s, c.y - s * 0.45),
        ]
    } else {
        vec![
            pos2(c.x - s * 0.45, c.y - s),
            pos2(c.x + s * 0.55, c.y),
            pos2(c.x - s * 0.45, c.y + s),
        ]
    };
    painter.add(egui::Shape::line(pts, Stroke::new(1.5_f32, color)));
}

pub fn icon_button(
    ui: &mut Ui,
    size: f32,
    active: bool,
    tooltip: &str,
    paint_fn: impl FnOnce(&Painter, Rect, Color32),
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if active {
            Palette::SEGMENT_ACTIVE
        } else if resp.hovered() {
            Palette::ROW_HOVER
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
        }
        let color = if active || resp.hovered() {
            Palette::TEXT
        } else {
            Palette::TEXT_MUTED
        };
        paint_fn(ui.painter(), rect, color);
    }
    resp.on_hover_text(tooltip)
}

pub fn mute_button(ui: &mut Ui, muted: bool, s: &Strings) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if muted {
            Palette::RED_MUTE_BG
        } else if resp.hovered() {
            Palette::ROW_HOVER
        } else {
            Color32::TRANSPARENT
        };
        if bg != Color32::TRANSPARENT {
            ui.painter().rect_filled(rect, CornerRadius::same(6), bg);
        }
        let color = if muted {
            Palette::RED_MUTE
        } else if resp.hovered() {
            Palette::TEXT
        } else {
            Palette::TEXT_MUTED
        };
        paint_speaker_icon(ui.painter(), rect, muted, color);
    }
    resp.on_hover_text(if muted { s.unmute_tip } else { s.mute_tip })
}

pub fn pill_switch(ui: &mut Ui, on: bool, active_color: Color32, label: &str, tip: &str) -> bool {
    let text_galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::new(11.5, FontFamily::Proportional),
        if on { Palette::TEXT } else { Palette::TEXT_MUTED },
    );
    let track_w = 28.0;
    let track_h = 16.0;
    let total_w = track_w + 8.0 + text_galley.size().x + 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(total_w, 26.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if resp.hovered() {
            Palette::ROW_HOVER
        } else {
            Palette::ROW
        };
        ui.painter().rect_filled(rect, CornerRadius::same(13), bg);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(13),
            Stroke::new(1.0_f32, Palette::BORDER),
            StrokeKind::Inside,
        );

        let track = Rect::from_min_size(
            pos2(rect.left() + 6.0, rect.center().y - track_h * 0.5),
            vec2(track_w, track_h),
        );
        let track_fill = if on { active_color } else { Palette::BORDER };
        ui.painter()
            .rect_filled(track, CornerRadius::same(8), track_fill);
        let knob_x = if on {
            track.right() - 8.0
        } else {
            track.left() + 8.0
        };
        ui.painter()
            .circle_filled(pos2(knob_x, track.center().y), 5.5, Palette::TEXT);

        let text_pos = pos2(track.right() + 7.0, rect.center().y - text_galley.size().y * 0.5);
        ui.painter().galley(text_pos, text_galley, Palette::TEXT);
    }

    resp.on_hover_text(tip).clicked()
}

pub fn mode_segmented(ui: &mut Ui, mode: &mut ControlMode, s: &Strings) -> bool {
    let options = [
        (ControlMode::Off, s.mode_off, s.mode_off_tip, Palette::TEXT_MUTED),
        (ControlMode::Limiter, s.mode_limiter, s.mode_limiter_tip, Palette::AMBER),
        (ControlMode::Leveler, s.mode_leveler, s.mode_leveler_tip, Palette::CYAN),
    ];
    let seg_w = 54.0;
    let h = 24.0;
    let total = vec2(seg_w * 3.0 + 4.0, h);
    let (outer, _) = ui.allocate_exact_size(total, Sense::hover());
    ui.painter()
        .rect_filled(outer, CornerRadius::same(6), Palette::TRACK);
    ui.painter().rect_stroke(
        outer,
        CornerRadius::same(6),
        Stroke::new(1.0_f32, Palette::BORDER),
        StrokeKind::Inside,
    );

    let mut changed = false;
    for (idx, (val, label, tip, accent)) in options.iter().enumerate() {
        let seg_rect = Rect::from_min_size(
            pos2(outer.left() + 2.0 + idx as f32 * seg_w, outer.top() + 2.0),
            vec2(seg_w, h - 4.0),
        );
        let resp = ui.interact(seg_rect, ui.id().with(("mode_seg", idx)), Sense::click());
        let selected = *mode == *val;
        if selected {
            ui.painter()
                .rect_filled(seg_rect, CornerRadius::same(4), Palette::SEGMENT_ACTIVE);
        } else if resp.hovered() {
            ui.painter()
                .rect_filled(seg_rect, CornerRadius::same(4), Palette::ROW_HOVER);
        }
        let text_color = if selected {
            if *val == ControlMode::Off {
                Palette::TEXT
            } else {
                *accent
            }
        } else if resp.hovered() {
            Palette::TEXT
        } else {
            Palette::TEXT_MUTED
        };
        ui.painter().text(
            seg_rect.center(),
            Align2::CENTER_CENTER,
            *label,
            FontId::new(11.0, FontFamily::Proportional),
            text_color,
        );
        if resp.on_hover_text(*tip).clicked() && !selected {
            *mode = *val;
            changed = true;
        }
    }
    changed
}

pub fn duck_role_badge(ui: &mut Ui, role: &mut DuckRole, active: bool, s: &Strings) -> bool {
    let (label, color, bg) = match *role {
        DuckRole::None => (s.duck_none, Palette::TEXT_DIM, Palette::TRACK),
        DuckRole::Trigger => (s.duck_trigger, Palette::CYAN, Palette::CYAN_BG),
        DuckRole::Target => (
            s.duck_target,
            if active {
                Palette::AMBER
            } else {
                Palette::TEXT_MUTED
            },
            if active {
                Palette::AMBER_FILL
            } else {
                Palette::SEGMENT_ACTIVE
            },
        ),
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(52.0, 24.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let fill = if resp.hovered() && *role == DuckRole::None {
            Palette::ROW_HOVER
        } else {
            bg
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        let border_col = if *role == DuckRole::None {
            Palette::BORDER
        } else {
            color.gamma_multiply(0.45)
        };
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0_f32, border_col),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(10.5, FontFamily::Proportional),
            color,
        );
    }
    if resp.on_hover_text(s.duck_role_tip).clicked() {
        *role = role.next();
        true
    } else {
        false
    }
}

pub struct MeterInteraction {
    pub new_base_volume: Option<f32>,
    pub new_threshold: Option<f32>,
}

pub fn volume_and_meter(
    ui: &mut Ui,
    width: f32,
    base_volume: f32,
    effective_volume: f32,
    peak: f32,
    gain_reduction: f32,
    muted: bool,
    bypassed: bool,
    mode: ControlMode,
    threshold: f32,
    s: &Strings,
) -> MeterInteraction {
    let mut out = MeterInteraction {
        new_base_volume: None,
        new_threshold: None,
    };

    let pct_w = 40.0;
    let bar_w = (width - pct_w - 8.0).max(80.0);

    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let (vol_rect, vol_resp) =
                ui.allocate_exact_size(vec2(bar_w, 16.0), Sense::click_and_drag());

            let mut displayed_vol = base_volume.clamp(0.0, 1.0);
            if vol_resp.dragged() || vol_resp.clicked() {
                if let Some(pos) = vol_resp.interact_pointer_pos() {
                    let v = ((pos.x - vol_rect.left()) / vol_rect.width()).clamp(0.0, 1.0);
                    displayed_vol = v;
                    out.new_base_volume = Some(v);
                }
            } else if vol_resp.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll.abs() > 0.5 {
                    let step = if scroll > 0.0 { 0.02 } else { -0.02 };
                    let v = (base_volume + step).clamp(0.0, 1.0);
                    displayed_vol = v;
                    out.new_base_volume = Some(v);
                }
            }

            let track = Rect::from_min_size(
                pos2(vol_rect.left(), vol_rect.center().y - 4.0),
                vec2(vol_rect.width(), 8.0),
            );
            ui.painter()
                .rect_filled(track, CornerRadius::same(4), Palette::TRACK);
            ui.painter().rect_stroke(
                track,
                CornerRadius::same(4),
                Stroke::new(1.0_f32, Palette::BORDER),
                StrokeKind::Inside,
            );

            let fill_w = track.width() * displayed_vol;
            if fill_w > 1.0 {
                let fill_rect = Rect::from_min_size(track.min, vec2(fill_w, track.height()));
                let fill_color = if muted || bypassed {
                    Palette::TEXT_DIM
                } else {
                    Color32::from_rgb(0xD8, 0xDE, 0xEA)
                };
                ui.painter()
                    .rect_filled(fill_rect, CornerRadius::same(4), fill_color);
            }

            if vol_resp.hovered() || vol_resp.dragged() {
                let kx = track.left() + fill_w;
                ui.painter().circle_filled(
                    pos2(
                        kx.clamp(track.left() + 4.0, track.right() - 4.0),
                        track.center().y,
                    ),
                    5.5,
                    Palette::TEXT,
                );
            }

            vol_resp.on_hover_text(s.volume_bar_tip);

            let pct_rect = ui.allocate_exact_size(vec2(pct_w, 16.0), Sense::hover()).0;
            let pct_text = format!("{:>3}%", (displayed_vol * 100.0).round() as i32);
            ui.painter().text(
                pos2(pct_rect.right(), pct_rect.center().y),
                Align2::RIGHT_CENTER,
                pct_text,
                FontId::new(11.5, FontFamily::Monospace),
                if muted { Palette::TEXT_DIM } else { Palette::TEXT },
            );
        });

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 8.0;
            let sense = if mode == ControlMode::Off {
                Sense::hover()
            } else {
                Sense::click_and_drag()
            };
            let (meter_rect, meter_resp) = ui.allocate_exact_size(vec2(bar_w, 10.0), sense);

            let mut cur_thresh = threshold.clamp(0.05, 1.0);
            if mode != ControlMode::Off && (meter_resp.dragged() || meter_resp.clicked()) {
                if let Some(pos) = meter_resp.interact_pointer_pos() {
                    let t = ((pos.x - meter_rect.left()) / meter_rect.width()).clamp(0.05, 1.0);
                    cur_thresh = t;
                    out.new_threshold = Some(t);
                }
            }

            ui.painter()
                .rect_filled(meter_rect, CornerRadius::same(3), Palette::TRACK);
            ui.painter().rect_stroke(
                meter_rect,
                CornerRadius::same(3),
                Stroke::new(1.0_f32, Palette::BORDER_SUBTLE),
                StrokeKind::Inside,
            );

            let pk = peak.clamp(0.0, 1.0);
            if pk > 0.004 {
                let pk_w = meter_rect.width() * pk;
                let pk_rect = Rect::from_min_size(meter_rect.min, vec2(pk_w, meter_rect.height()));
                let pk_color = if muted {
                    Palette::TEXT_DIM
                } else if pk > 0.72 {
                    Palette::CYAN
                } else {
                    Palette::EMERALD
                };
                ui.painter()
                    .rect_filled(pk_rect, CornerRadius::same(3), pk_color);
            }

            if !muted && !bypassed && gain_reduction > 0.008 {
                let gr_start =
                    meter_rect.left() + meter_rect.width() * effective_volume.clamp(0.0, 1.0);
                let gr_end = meter_rect.left() + meter_rect.width() * base_volume.clamp(0.0, 1.0);
                if gr_end > gr_start + 1.0 {
                    let gr_rect = Rect::from_min_max(
                        pos2(gr_start, meter_rect.top() + 1.0),
                        pos2(gr_end, meter_rect.bottom() - 1.0),
                    );
                    ui.painter()
                        .rect_filled(gr_rect, CornerRadius::same(2), Palette::AMBER_FILL);
                    ui.painter().line_segment(
                        [pos2(gr_start, meter_rect.top()), pos2(gr_end, meter_rect.top())],
                        Stroke::new(1.5_f32, Palette::AMBER),
                    );
                }
            }

            if mode != ControlMode::Off {
                let mx = meter_rect.left() + meter_rect.width() * cur_thresh;
                let marker_color = match mode {
                    ControlMode::Limiter => Palette::AMBER,
                    ControlMode::Leveler => Palette::CYAN,
                    ControlMode::Off => Palette::TEXT_DIM,
                };
                ui.painter().line_segment(
                    [
                        pos2(mx, meter_rect.top() - 1.5),
                        pos2(mx, meter_rect.bottom() + 1.5),
                    ],
                    Stroke::new(2.0_f32, marker_color),
                );
            }

            let meter_tip = match mode {
                ControlMode::Limiter => s.meter_ceiling_tip,
                ControlMode::Leveler => s.meter_target_tip,
                ControlMode::Off => s.meter_monitor_tip,
            };
            meter_resp.on_hover_text(meter_tip);

            let gr_rect = ui.allocate_exact_size(vec2(pct_w, 10.0), Sense::hover()).0;
            if !muted && !bypassed && gain_reduction > 0.008 {
                let gr_text = format!("-{:>2}%", (gain_reduction * 100.0).round() as i32);
                ui.painter().text(
                    pos2(gr_rect.right(), gr_rect.center().y),
                    Align2::RIGHT_CENTER,
                    gr_text,
                    FontId::new(10.0, FontFamily::Monospace),
                    Palette::AMBER,
                );
            }
        });
    });

    out
}

pub fn param_slider(
    ui: &mut Ui,
    label: &str,
    tooltip: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    accent: Color32,
    format_val: impl Fn(f32) -> String,
) -> bool {
    let mut changed = false;
    let min = *range.start();
    let max = *range.end();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let (label_rect, label_resp) = ui.allocate_exact_size(vec2(106.0, 18.0), Sense::hover());
        ui.painter().text(
            pos2(label_rect.left(), label_rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::new(11.0, FontFamily::Proportional),
            Palette::TEXT_MUTED,
        );
        label_resp.on_hover_text(tooltip);

        help_badge(ui, tooltip);

        let slider_w = (ui.available_width() - 58.0).max(72.0);
        let (rect, resp) = ui.allocate_exact_size(vec2(slider_w, 18.0), Sense::click_and_drag());
        if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let t = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 1.0);
                *value = min + t * (max - min);
                changed = true;
            }
        }
        let t = ((*value - min) / (max - min)).clamp(0.0, 1.0);
        let track = Rect::from_min_size(
            pos2(rect.left(), rect.center().y - 2.0),
            vec2(rect.width(), 4.0),
        );
        ui.painter()
            .rect_filled(track, CornerRadius::same(2), Palette::TRACK);
        let fill = Rect::from_min_size(track.min, vec2(track.width() * t, 4.0));
        ui.painter().rect_filled(fill, CornerRadius::same(2), accent);
        let kx = (track.left() + track.width() * t).clamp(track.left() + 4.0, track.right() - 4.0);
        ui.painter()
            .circle_filled(pos2(kx, track.center().y), 4.5, Palette::TEXT);
        resp.on_hover_text(tooltip);

        let val_rect = ui.allocate_exact_size(vec2(50.0, 18.0), Sense::hover()).0;
        ui.painter().text(
            pos2(val_rect.right(), val_rect.center().y),
            Align2::RIGHT_CENTER,
            format_val(*value),
            FontId::new(11.0, FontFamily::Monospace),
            Palette::TEXT,
        );
    });
    changed
}

pub fn subtle_button(ui: &mut Ui, label: &str, tooltip: &str, danger: bool, full_width: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        FontId::new(11.0, FontFamily::Proportional),
        Palette::TEXT_MUTED,
    );
    let w = if full_width {
        ui.available_width()
    } else {
        galley.size().x + 20.0
    };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    if ui.is_rect_visible(rect) {
        let hovered = resp.hovered();
        let fill = if hovered && danger {
            Palette::RED_MUTE_BG
        } else if hovered {
            Palette::ROW_HOVER
        } else {
            Palette::TRACK
        };
        let border = if hovered && danger {
            Palette::RED_MUTE.gamma_multiply(0.6)
        } else {
            Palette::BORDER
        };
        let text_col = if hovered && danger {
            Palette::RED_MUTE
        } else if hovered {
            Palette::TEXT
        } else {
            Palette::TEXT_MUTED
        };
        ui.painter().rect_filled(rect, CornerRadius::same(6), fill);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(6),
            Stroke::new(1.0_f32, border),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(11.0, FontFamily::Proportional),
            text_col,
        );
    }
    resp.on_hover_text(tooltip).clicked()
}
