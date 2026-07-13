#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analyze;
mod cad;
mod encoding;
mod i18n;
mod style;

use analyze::{apply_fix, detect_issues, preview_fix, Issue, PreviewItem};
use cad::{
    bounding_box, load_dxf_auto, load_dxf_with_encoding, resolve_entity_rgb,
    save_dxf_with_encoding, CadEntity, LayerInfo, LoadOutcome,
};
use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Vec2};
use encoding::TextEncoding;
use i18n::{t, Lang};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const SETTINGS_KEY: &str = "settings";

struct PendingEncoding {
    path: PathBuf,
    candidates: Vec<TextEncoding>,
    selected: TextEncoding,
}

#[derive(Serialize, Deserialize, Default)]
struct Settings {
    lang: Lang,
}

// Основное состояние приложения (модель GUI)
struct App {
    lang: Lang,
    show_license: bool,
    pending_encoding: Option<PendingEncoding>,
    entities: Vec<CadEntity>,
    source_encoding: TextEncoding,
    layers: Vec<LayerInfo>,
    issues: Vec<Issue>,
    file_path: Option<PathBuf>,
    status: Option<String>,
    zoom: f32,
    pan: Vec2,
    undo_stack: Vec<Vec<CadEntity>>,
    redo_stack: Vec<Vec<CadEntity>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            lang: Lang::default(),
            show_license: false,
            pending_encoding: None,
            entities: Vec::new(),
            source_encoding: TextEncoding::Utf8,
            layers: Vec::new(),
            issues: Vec::new(),
            file_path: None,
            status: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        style::apply(&cc.egui_ctx);
        let mut app = App::default();

        if let Some(storage) = cc.storage {
            if let Some(s) = eframe::get_value::<Settings>(storage, SETTINGS_KEY) {
                app.lang = s.lang;
            }
        }
        app
    }

    fn to_settings(&self) -> Settings {
        Settings { lang: self.lang }
    }

    // Открывает системный диалог выбора файла и загружает выбранный DXF
    fn open_file_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("DXF", &["dxf"])
            .pick_file()
        {
            self.load_path(path);
        }
    }

    // Загружает DXF по указанному пути; если кодировку не удаётся определить,
    // открывает окно выбора кодировки
    fn load_path(&mut self, path: PathBuf) {
        match load_dxf_auto(&path) {
            Ok(LoadOutcome::Loaded(entities, layers, encoding)) => {
                self.apply_loaded(entities, layers, path, encoding);
            }
            Ok(LoadOutcome::NeedsEncoding(candidates)) => {
                let selected = candidates.first().copied().unwrap_or(TextEncoding::Utf8);
                self.pending_encoding = Some(PendingEncoding {
                    path,
                    candidates,
                    selected,
                });
            }
            Err(e) => {
                self.status = Some(format!("{}{}", t(self.lang, "load_error_prefix"), e));
            }
        }
    }

    // Завершает отложенную загрузку файла после того, как пользователь выбрал кодировку
    fn resolve_pending_encoding(&mut self, encoding: TextEncoding) {
        let Some(pending) = self.pending_encoding.take() else {
            return;
        };
        match load_dxf_with_encoding(&pending.path, encoding) {
            Ok((entities, layers)) => {
                self.apply_loaded(entities, layers, pending.path, encoding);
            }
            Err(e) => {
                self.status = Some(format!("{}{}", t(self.lang, "load_error_prefix"), e));
            }
        }
    }

    fn apply_loaded(
        &mut self,
        entities: Vec<CadEntity>,
        layers: Vec<LayerInfo>,
        path: PathBuf,
        encoding: TextEncoding,
    ) {
        self.entities = entities;
        self.layers = layers;
        self.issues = detect_issues(&self.entities);
        self.file_path = Some(path);
        self.source_encoding = encoding;
        self.status = None;
        self.zoom = 1.0;
        self.pan = Vec2::ZERO;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    // Сохраняет текущий чертёж в открытый файл, используя исходную кодировку
    fn save_file(&mut self) {
        let target = match &self.file_path {
            Some(p) => Some(p.clone()),
            None => rfd::FileDialog::new()
                .add_filter("DXF", &["dxf"])
                .set_file_name("drawing.dxf")
                .save_file(),
        };

        let Some(path) = target else { return };

        match save_dxf_with_encoding(&path, &self.entities, &self.layers, self.source_encoding) {
            Ok(()) => {
                self.file_path = Some(path);
                self.status = Some(t(self.lang, "saved_toast").to_string());
            }
            Err(e) => {
                self.status = Some(format!("{}{}", t(self.lang, "save_error_prefix"), e));
            }
        }
    }

    // Применяет автоматическое исправление к найденной проблеме
    fn fix_issue(&mut self, issue_index: usize) {
        if issue_index >= self.issues.len() {
            return;
        }
        let issue = self.issues[issue_index].clone();
        self.push_undo_snapshot();
        apply_fix(&mut self.entities, &issue);
        self.issues = detect_issues(&self.entities);
    }

    // Сохраняет снимок текущего состояния объектов для последующей отмены (undo)
    fn push_undo_snapshot(&mut self) {
        const MAX_HISTORY: usize = 100;
        self.undo_stack.push(self.entities.clone());
        if self.undo_stack.len() > MAX_HISTORY {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    // Откатывает последнее изменение
    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.redo_stack
                .push(std::mem::replace(&mut self.entities, prev));
            self.issues = detect_issues(&self.entities);
            self.status = Some(t(self.lang, "undo_toast").to_string());
        }
    }

    // Повторяет отменённое изменение
    fn redo(&mut self) {
        if let Some(next) = self.redo_stack.pop() {
            self.undo_stack
                .push(std::mem::replace(&mut self.entities, next));
            self.issues = detect_issues(&self.entities);
            self.status = Some(t(self.lang, "redo_toast").to_string());
        }
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        egui::Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let s = self.to_settings();
        eframe::set_value(storage, SETTINGS_KEY, &s);
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.show_license && self.pending_encoding.is_none() {
            handle_resize_edges(ctx);
        }

        self.custom_title_bar(ctx);

        self.license_window(ctx);
        self.encoding_window(ctx);

        if !self.show_license && self.pending_encoding.is_none() {
            let (undo_pressed, redo_pressed) = ctx.input(|i| {
                let ctrl = i.modifiers.ctrl || i.modifiers.command;
                (
                    ctrl && i.key_pressed(egui::Key::Z),
                    ctrl && i.key_pressed(egui::Key::X),
                )
            });
            if undo_pressed {
                self.undo();
            } else if redo_pressed {
                self.redo();
            }
        }

        let toolbar_frame = egui::Frame::none()
            .fill(Color32::from_rgb(250, 250, 251))
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
            .inner_margin(egui::Margin::symmetric(14.0, 8.0));

        egui::TopBottomPanel::top("toolbar")
            .frame(toolbar_frame)
            .show(ctx, |ui| {
                let mut scroll_style = ui.style().spacing.scroll;
                scroll_style.floating = true;
                scroll_style.bar_width = 4.0;
                scroll_style.floating_width = 4.0;
                scroll_style.bar_inner_margin = 2.0;
                scroll_style.bar_outer_margin = 0.0;
                scroll_style.dormant_background_opacity = 0.0;
                scroll_style.dormant_handle_opacity = 0.0;
                scroll_style.active_background_opacity = 1.0;
                scroll_style.active_handle_opacity = 1.0;
                scroll_style.interact_background_opacity = 1.0;
                scroll_style.interact_handle_opacity = 1.0;
                ui.style_mut().spacing.scroll = scroll_style;

                style::apply_scrollbar_colors(ui);

                egui::ScrollArea::horizontal()
                    .id_source("toolbar_scroll")
                    .auto_shrink([false, true])
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| {
                        style::restore_widget_colors(ui);
                        ui.horizontal(|ui| {
                            if ui.button(t(self.lang, "open_button")).clicked() {
                                self.open_file_dialog();
                            }
                            if ui
                                .add_enabled(
                                    !self.entities.is_empty(),
                                    egui::Button::new(t(self.lang, "save_button")),
                                )
                                .clicked()
                            {
                                self.save_file();
                            }

                            ui.separator();

                            let full_name = self
                                .file_path
                                .as_ref()
                                .and_then(|p| p.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_else(|| t(self.lang, "no_file_open").to_string());
                            let name = truncate_file_name(&full_name, 28);
                            let label = ui.label(egui::RichText::new(name).weak());
                            if full_name.chars().count() > 28 {
                                label.on_hover_text(full_name);
                            }

                            if !self.entities.is_empty() {
                                ui.separator();

                                if ui
                                    .add_enabled(
                                        !self.undo_stack.is_empty(),
                                        egui::Button::new(t(self.lang, "undo_button")),
                                    )
                                    .on_hover_text(t(self.lang, "undo_tooltip"))
                                    .clicked()
                                {
                                    self.undo();
                                }
                                if ui
                                    .add_enabled(
                                        !self.redo_stack.is_empty(),
                                        egui::Button::new(t(self.lang, "redo_button")),
                                    )
                                    .on_hover_text(t(self.lang, "redo_tooltip"))
                                    .clicked()
                                {
                                    self.redo();
                                }

                                ui.separator();

                                if ui.button(t(self.lang, "reset_view_button")).clicked() {
                                    self.zoom = 1.0;
                                    self.pan = Vec2::ZERO;
                                }
                                ui.label(
                                    egui::RichText::new(format_zoom_percent(self.zoom))
                                        .weak()
                                        .small(),
                                );

                                ui.label(
                                    egui::RichText::new(t(self.lang, "zoom_hint"))
                                        .weak()
                                        .small(),
                                );

                                ui.separator();
                                if self.issues.is_empty() {
                                    ui.colored_label(
                                        Color32::from_rgb(40, 160, 90),
                                        t(self.lang, "no_issues"),
                                    );
                                } else {
                                    ui.colored_label(
                                        Color32::from_rgb(214, 110, 20),
                                        format!(
                                            "{}{}",
                                            t(self.lang, "issues_found_prefix"),
                                            self.issues.len()
                                        ),
                                    );
                                    ui.label(
                                        egui::RichText::new(t(self.lang, "click_marker_hint"))
                                            .weak()
                                            .small(),
                                    );
                                }
                            }

                            if let Some(msg) = &self.status {
                                ui.separator();
                                ui.label(egui::RichText::new(msg).weak());
                            }
                        });
                    });
            });

        let central_frame = egui::Frame::none()
            .fill(Color32::from_rgb(252, 252, 253))
            .rounding(egui::Rounding {
                nw: 0.0,
                ne: 0.0,
                sw: 14.0,
                se: 14.0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)));

        egui::CentralPanel::default()
            .frame(central_frame)
            .show(ctx, |ui| {
                let rect = ui.available_rect_before_wrap();

                if self.entities.is_empty() {
                    let painter = ui.painter_at(rect);
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        t(self.lang, "empty_state"),
                        egui::FontId::proportional(16.0),
                        Color32::from_rgb(150, 150, 156),
                    );
                    return;
                }

                self.draw_canvas(ui, rect);
            });
    }
}

impl App {
    // Отрисовывает холст с чертежом: объекты, маркеры проблем, обработку ввода (зум/панорама)
    fn draw_canvas(&mut self, ui: &mut egui::Ui, rect: Rect) {
        if !self.zoom.is_finite()
            || self.zoom <= 0.0
            || !self.pan.x.is_finite()
            || !self.pan.y.is_finite()
        {
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
        }

        let Some(bbox) = bounding_box(&self.entities) else {
            return;
        };
        let (min_x, min_y, max_x, max_y) = bbox;
        let world_w = (max_x - min_x).max(1e-6);
        let world_h = (max_y - min_y).max(1e-6);

        const PADDING: f32 = 24.0;
        let avail_w = (rect.width() - PADDING * 2.0).max(1.0);
        let avail_h = (rect.height() - PADDING * 2.0).max(1.0);

        let base_scale = (avail_w / world_w as f32).min(avail_h / world_h as f32);

        let center_offset = Vec2::new(
            (avail_w - world_w as f32 * base_scale) / 2.0,
            (avail_h - world_h as f32 * base_scale) / 2.0,
        );

        let bg_response = ui.interact(
            rect,
            ui.id().with("canvas_pan_zoom"),
            egui::Sense::click_and_drag(),
        );
        if bg_response.dragged() {
            self.pan += bg_response.drag_delta();
        }
        if bg_response.double_clicked() {
            self.zoom = 1.0;
            self.pan = Vec2::ZERO;
        }

        let modal_open = self.show_license || self.pending_encoding.is_some();

        let touch = ui
            .input(|i| i.multi_touch())
            .filter(|t| rect.contains(t.start_pos));
        let pointer_in_rect = !modal_open
            && ui
                .ctx()
                .pointer_hover_pos()
                .is_some_and(|p| rect.contains(p));
        let touch = if modal_open { None } else { touch };

        if pointer_in_rect || touch.is_some() {
            let zoom_center = ui
                .ctx()
                .pointer_hover_pos()
                .filter(|p| rect.contains(*p))
                .or_else(|| touch.as_ref().map(|t| t.start_pos))
                .unwrap_or_else(|| rect.center());

            let mut zoom_multiplier = 1.0_f32;

            if pointer_in_rect {
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll.is_finite() && scroll.abs() > 0.0 {
                    let m = (scroll * 0.0035).exp();
                    if m.is_finite() {
                        zoom_multiplier *= m;
                    }
                }
            }

            let pinch = ui.input(|i| i.zoom_delta());
            if pinch.is_finite() && (pinch - 1.0).abs() > 0.0001 {
                zoom_multiplier *= pinch;
            }

            if !zoom_multiplier.is_finite() || zoom_multiplier <= 0.0 {
                zoom_multiplier = 1.0;
            }

            if (zoom_multiplier - 1.0).abs() > 0.0001 {
                let old_scale = base_scale * self.zoom;

                const MAX_SCALED_PX: f32 = 100_000.0;
                const MIN_SCALED_PX: f32 = 0.01;
                let max_world_dim = (world_w.max(world_h) as f32).max(1e-6);
                let zoom_upper = (MAX_SCALED_PX / (base_scale * max_world_dim)).max(1.0);
                let zoom_lower = (MIN_SCALED_PX / (base_scale * max_world_dim))
                    .min(zoom_upper)
                    .max(1e-9);

                let new_zoom = (self.zoom * zoom_multiplier).clamp(zoom_lower, zoom_upper);
                let new_scale = base_scale * new_zoom;

                let old_origin = rect.min + Vec2::new(PADDING, PADDING) + center_offset + self.pan;
                let new_origin = zoom_center - (new_scale / old_scale) * (zoom_center - old_origin);
                let new_pan = new_origin - rect.min - Vec2::new(PADDING, PADDING) - center_offset;

                if new_zoom.is_finite() && new_pan.x.is_finite() && new_pan.y.is_finite() {
                    self.pan = new_pan;
                    self.zoom = new_zoom;
                }
            }

            if let Some(t) = touch {
                if t.translation_delta.length() > 0.0 {
                    self.pan += t.translation_delta;
                }
            }
        }

        let scale = base_scale * self.zoom;
        let world_h_scaled = world_h as f32 * scale;
        let origin = rect.min + Vec2::new(PADDING, PADDING) + center_offset + self.pan;

        let to_screen = |p: (f64, f64)| -> Pos2 {
            let x = (p.0 - min_x) as f32 * scale;
            let y = (p.1 - min_y) as f32 * scale;
            Pos2::new(origin.x + x, origin.y + (world_h_scaled - y))
        };

        let painter = ui.painter_at(rect);

        let line_width = 1.4;

        for e in &self.entities {
            let (r, g, b) = resolve_entity_rgb(e.color(), e.layer(), &self.layers);
            let color = Color32::from_rgb(r, g, b);
            draw_entity(&painter, to_screen, e, color, line_width, scale);
        }

        const MARKER_RADIUS: f32 = 9.0;
        let mut clicked_index: Option<usize> = None;
        let mut hovered_index: Option<usize> = None;

        for (i, issue) in self.issues.iter().enumerate() {
            let center = to_screen(issue.center);
            let marker_rect = Rect::from_center_size(center, Vec2::splat(MARKER_RADIUS * 2.6));
            let id = ui.id().with("issue_marker").with(i);
            let resp = ui.interact(marker_rect, id, egui::Sense::click());

            let color = issue.severity().color();
            let fill = if resp.hovered() {
                color
            } else {
                color.linear_multiply(0.85)
            };

            painter.circle_stroke(center, MARKER_RADIUS, Stroke::new(2.2, fill));
            if resp.hovered() {
                painter.circle_filled(center, MARKER_RADIUS - 3.0, fill.linear_multiply(0.35));
                hovered_index = Some(i);
            }

            resp.clone().on_hover_text(issue.detail.clone());
            if resp.clicked() {
                clicked_index = Some(i);
            }
        }

        if let Some(i) = hovered_index {
            if let Some(issue) = self.issues.get(i) {
                let preview = preview_fix(&self.entities, issue);
                for item in &preview {
                    match item {
                        PreviewItem::Delete(idx) => {
                            if let Some(e) = self.entities.get(*idx) {
                                draw_entity(
                                    &painter,
                                    to_screen,
                                    e,
                                    Color32::from_rgb(220, 40, 40),
                                    3.0,
                                    scale,
                                );
                            }
                        }
                        PreviewItem::Change { before, after, .. } => {
                            draw_entity(
                                &painter,
                                to_screen,
                                before,
                                Color32::from_rgb(230, 190, 20),
                                3.0,
                                scale,
                            );
                            draw_entity(
                                &painter,
                                to_screen,
                                after,
                                Color32::from_rgb(40, 180, 90),
                                3.0,
                                scale,
                            );
                        }
                        PreviewItem::Add { p1, p2, .. } => {
                            painter.line_segment(
                                [to_screen(*p1), to_screen(*p2)],
                                Stroke::new(3.0, Color32::from_rgb(0, 122, 255)),
                            );
                        }
                    }
                }
            }
        }

        if let Some(i) = clicked_index {
            self.fix_issue(i);
        }
    }

    // Отрисовывает кастомную (не системную) панель заголовка окна
    fn custom_title_bar(&mut self, ctx: &egui::Context) {
        let bar_height = 40.0;

        let frame = egui::Frame::none()
            .fill(Color32::from_rgb(250, 250, 251))
            .rounding(egui::Rounding {
                nw: 14.0,
                ne: 14.0,
                sw: 0.0,
                se: 0.0,
            })
            .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
            .inner_margin(egui::Margin {
                left: 14.0,
                right: 4.0,
                top: 0.0,
                bottom: 0.0,
            });

        let title_bar_interactive = !self.show_license && self.pending_encoding.is_none();

        egui::TopBottomPanel::top("title_bar")
            .exact_height(bar_height)
            .frame(frame)
            .show(ctx, |ui| {
                ui.add_enabled_ui(title_bar_interactive, |ui| {
                    let bar_rect = ui.max_rect();

                    let bar_drag_resp = ui.interact(
                        bar_rect,
                        ui.id().with("titlebar_drag_full"),
                        egui::Sense::click_and_drag(),
                    );
                    if bar_drag_resp.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if bar_drag_resp.double_clicked() {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }

                    let license_diameter = 20.0;
                    let license_gap = 8.0;
                    let right_margin = 6.0;
                    let combo_width = 96.0;
                    let combo_height = 24.0;
                    let right_group_left = bar_rect.right()
                        - right_margin
                        - license_diameter
                        - license_gap
                        - combo_width;

                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;

                        let close_clicked =
                            traffic_light_button(ui, Color32::from_rgb(255, 95, 87), 0);
                        let min_clicked =
                            traffic_light_button(ui, Color32::from_rgb(255, 189, 46), 1);
                        let max_clicked =
                            traffic_light_button(ui, Color32::from_rgb(40, 201, 64), 2);

                        if close_clicked {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        if min_clicked {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if max_clicked {
                            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                        }

                        ui.add_space(14.0);
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(egui::RichText::new("AntiWaste CAD").size(15.0).strong());

                        let subtitle_text = t(self.lang, "subtitle");
                        let subtitle_galley = ui.fonts(|f| {
                            f.layout_no_wrap(
                                subtitle_text.to_string(),
                                egui::FontId::proportional(12.0),
                                Color32::PLACEHOLDER,
                            )
                        });
                        let would_end = ui.min_rect().right() + 6.0 + subtitle_galley.size().x;
                        if would_end < right_group_left - 12.0 {
                            ui.label(egui::RichText::new(subtitle_text).size(12.0).weak());
                        }
                    });

                    let license_rect = Rect::from_min_size(
                        Pos2::new(
                            bar_rect.right() - right_margin - license_diameter,
                            bar_rect.center().y - license_diameter / 2.0,
                        ),
                        Vec2::splat(license_diameter),
                    );

                    let combo_rect = Rect::from_min_size(
                        Pos2::new(
                            license_rect.left() - license_gap - combo_width,
                            bar_rect.center().y - combo_height / 2.0,
                        ),
                        Vec2::new(combo_width, combo_height),
                    );
                    ui.allocate_ui_at_rect(combo_rect, |ui| {
                        ui.style_mut().spacing.button_padding = egui::vec2(8.0, 4.0);
                        egui::ComboBox::from_id_source("lang_selector")
                            .width(combo_width)
                            .selected_text(self.lang.label())
                            .show_ui(ui, |ui| {
                                for l in Lang::ALL {
                                    ui.selectable_value(&mut self.lang, l, l.label());
                                }
                            });
                    });

                    let license_resp = ui
                        .allocate_ui_at_rect(license_rect, |ui| {
                            license_button(ui, license_diameter)
                        })
                        .inner;
                    if license_resp.clicked() {
                        self.show_license = true;
                    }
                    license_resp.on_hover_text(t(self.lang, "license_button_tooltip"));
                });
            });
    }

    // Окно с информацией о лицензиях используемых библиотек и шрифтов
    fn license_window(&mut self, ctx: &egui::Context) {
        if !self.show_license {
            return;
        }

        let lang = self.lang;
        let screen = ctx.screen_rect();

        const MARGIN: f32 = 24.0;
        let window_size = Vec2::new(
            460.0_f32.min((screen.width() - MARGIN * 2.0).max(240.0)),
            460.0_f32.min((screen.height() - MARGIN * 2.0).max(260.0)),
        );

        let win_rect = Rect::from_center_size(screen.center(), window_size);
        let mut close_clicked = false;

        egui::Area::new(egui::Id::new("license_modal_layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .interactable(true)
            .show(ctx, |ui| {
                ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

                ui.allocate_ui_at_rect(win_rect, |ui| {
                    ui.set_width(window_size.x);
                    ui.set_height(window_size.y);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(246, 246, 248))
                        .rounding(egui::Rounding::same(14.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
                        .show(ui, |ui| {
                            ui.set_width(window_size.x);
                            ui.set_height(window_size.y);

                            let bar_height = 36.0;
                            let bar_frame = egui::Frame::none()
                                .fill(Color32::from_rgb(250, 250, 251))
                                .rounding(egui::Rounding {
                                    nw: 14.0,
                                    ne: 14.0,
                                    sw: 0.0,
                                    se: 0.0,
                                });

                            egui::TopBottomPanel::top("license_title_bar")
                                .exact_height(bar_height)
                                .frame(bar_frame)
                                .show_inside(ui, |ui| {
                                    let bar_rect = ui.max_rect();

                                    ui.painter().text(
                                        bar_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        t(lang, "license_title"),
                                        egui::FontId::proportional(14.0),
                                        Color32::from_rgb(40, 40, 44),
                                    );

                                    let close_diameter = 22.0;
                                    let right_margin = 8.0;
                                    let close_rect = Rect::from_min_size(
                                        Pos2::new(
                                            bar_rect.right() - right_margin - close_diameter,
                                            bar_rect.center().y - close_diameter / 2.0,
                                        ),
                                        Vec2::splat(close_diameter),
                                    );
                                    let resp = ui
                                        .allocate_ui_at_rect(close_rect, |ui| {
                                            close_x_button(ui, close_diameter)
                                        })
                                        .inner;
                                    if resp.clicked() {
                                        close_clicked = true;
                                    }
                                });

                            let body_rounding = egui::Rounding {
                                nw: 0.0,
                                ne: 0.0,
                                sw: 14.0,
                                se: 14.0,
                            };
                            let body_frame = egui::Frame::none()
                                .fill(Color32::from_rgb(246, 246, 248))
                                .rounding(body_rounding)
                                .inner_margin(egui::Margin::same(16.0));

                            egui::CentralPanel::default()
                                .frame(body_frame)
                                .show_inside(ui, |ui| {
                                    style::apply_scrollbar_colors(ui);
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false, false])
                                        .scroll_bar_visibility(
                                            egui::scroll_area::ScrollBarVisibility::AlwaysHidden,
                                        )
                                        .show(ui, |ui| {
                                            style::restore_widget_colors(ui);
                                            ui.label(
                                                egui::RichText::new(t(lang, "license_libraries"))
                                                    .strong(),
                                            );
                                            ui.add_space(4.0);
                                            for (name, version, license) in LIBRARY_LICENSES {
                                                ui.horizontal(|ui| {
                                                    ui.label(egui::RichText::new(*name).strong());
                                                    ui.weak(format!("v{}", version));
                                                    ui.label(format!("— {}", license));
                                                });
                                            }

                                            ui.add_space(14.0);
                                            ui.separator();
                                            ui.add_space(10.0);

                                            ui.label(
                                                egui::RichText::new(t(lang, "license_fonts"))
                                                    .strong(),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(t(lang, "license_fonts_body"));
                                            ui.add_space(6.0);
                                            ui.hyperlink_to(
                                                "openfontlicense.org",
                                                "https://openfontlicense.org",
                                            );

                                            ui.add_space(14.0);
                                            ui.separator();
                                            ui.add_space(10.0);

                                            ui.label(
                                                egui::RichText::new(t(lang, "license_software"))
                                                    .strong(),
                                            );
                                            ui.add_space(4.0);
                                            ui.label(t(lang, "license_software_body"));
                                            ui.add_space(6.0);
                                            ui.hyperlink_to(
                                                "github.com/Rafych/AntiWaste-CAD",
                                                "https://github.com/Rafych/AntiWaste-CAD",
                                            );
                                            ui.add_space(14.0);
                                        });
                                });
                        });
                });
            });

        if close_clicked || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_license = false;
        }
    }

    // Окно выбора кодировки, когда автоопределение не дало однозначного результата
    fn encoding_window(&mut self, ctx: &egui::Context) {
        if self.pending_encoding.is_none() {
            return;
        }

        let lang = self.lang;
        let screen = ctx.screen_rect();

        const MARGIN: f32 = 24.0;
        let window_size = Vec2::new(
            420.0_f32.min((screen.width() - MARGIN * 2.0).max(240.0)),
            380.0_f32.min((screen.height() - MARGIN * 2.0).max(260.0)),
        );

        let win_rect = Rect::from_center_size(screen.center(), window_size);
        let mut close_clicked = false;
        let mut confirmed: Option<TextEncoding> = None;

        egui::Area::new(egui::Id::new("encoding_modal_layer"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .interactable(true)
            .show(ctx, |ui| {
                ui.allocate_response(screen.size(), egui::Sense::click_and_drag());
                ui.painter()
                    .rect_filled(screen, 0.0, Color32::from_black_alpha(90));

                ui.allocate_ui_at_rect(win_rect, |ui| {
                    ui.set_width(window_size.x);
                    ui.set_height(window_size.y);

                    egui::Frame::none()
                        .fill(Color32::from_rgb(246, 246, 248))
                        .rounding(egui::Rounding::same(14.0))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(228, 228, 232)))
                        .show(ui, |ui| {
                            ui.set_width(window_size.x);
                            ui.set_height(window_size.y);

                            let bar_height = 36.0;
                            let bar_frame = egui::Frame::none()
                                .fill(Color32::from_rgb(250, 250, 251))
                                .rounding(egui::Rounding {
                                    nw: 14.0,
                                    ne: 14.0,
                                    sw: 0.0,
                                    se: 0.0,
                                });

                            egui::TopBottomPanel::top("encoding_title_bar")
                                .exact_height(bar_height)
                                .frame(bar_frame)
                                .show_inside(ui, |ui| {
                                    let bar_rect = ui.max_rect();

                                    ui.painter().text(
                                        bar_rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        t(lang, "encoding_title"),
                                        egui::FontId::proportional(14.0),
                                        Color32::from_rgb(40, 40, 44),
                                    );

                                    let close_diameter = 22.0;
                                    let right_margin = 8.0;
                                    let close_rect = Rect::from_min_size(
                                        Pos2::new(
                                            bar_rect.right() - right_margin - close_diameter,
                                            bar_rect.center().y - close_diameter / 2.0,
                                        ),
                                        Vec2::splat(close_diameter),
                                    );
                                    let resp = ui
                                        .allocate_ui_at_rect(close_rect, |ui| {
                                            close_x_button(ui, close_diameter)
                                        })
                                        .inner;
                                    if resp.clicked() {
                                        close_clicked = true;
                                    }
                                });

                            let body_rounding = egui::Rounding {
                                nw: 0.0,
                                ne: 0.0,
                                sw: 14.0,
                                se: 14.0,
                            };
                            let body_frame = egui::Frame::none()
                                .fill(Color32::from_rgb(246, 246, 248))
                                .rounding(body_rounding)
                                .inner_margin(egui::Margin::same(16.0));

                            egui::CentralPanel::default()
                                .frame(body_frame)
                                .show_inside(ui, |ui| {
                                    let Some(pending) = self.pending_encoding.as_mut() else {
                                        return;
                                    };

                                    ui.label(t(lang, "encoding_body"));
                                    ui.add_space(4.0);
                                    ui.weak(
                                        pending
                                            .path
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_default(),
                                    );
                                    ui.add_space(14.0);

                                    style::apply_scrollbar_colors(ui);
                                    egui::ScrollArea::vertical()
                                        .auto_shrink([false, true])
                                        .max_height(window_size.y - 190.0)
                                        .show(ui, |ui| {
                                            style::restore_widget_colors(ui);
                                            for enc in pending.candidates.clone() {
                                                ui.radio_value(
                                                    &mut pending.selected,
                                                    enc,
                                                    enc.label(lang),
                                                );
                                            }
                                        });

                                    ui.add_space(14.0);
                                    ui.separator();
                                    ui.add_space(10.0);

                                    ui.horizontal(|ui| {
                                        if ui.button(t(lang, "encoding_cancel")).clicked() {
                                            close_clicked = true;
                                        }
                                        if ui.button(t(lang, "encoding_confirm")).clicked() {
                                            confirmed = Some(pending.selected);
                                        }
                                    });
                                });
                        });
                });
            });

        if let Some(enc) = confirmed {
            self.resolve_pending_encoding(enc);
            return;
        }

        if close_clicked || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.pending_encoding = None;
        }
    }
}

fn format_zoom_percent(zoom: f32) -> String {
    let percent = zoom as f64 * 100.0;
    let abs = percent.abs();
    if abs != 0.0 && (abs < 0.001 || abs >= 1_000_000.0) {
        format!("{:.2e}%", percent)
    } else if abs < 1.0 {
        format!("{:.4}%", percent)
    } else if abs < 100.0 {
        format!("{:.2}%", percent)
    } else {
        format!("{:.0}%", percent)
    }
}

const MAX_FONT_SIZE: f32 = 256.0;

fn quantize_font_size(size: f32) -> f32 {
    let size = size.max(1.0);

    const STEPS_PER_OCTAVE: f32 = 24.0;
    let log2 = size.log2();
    let quantized_log2 = (log2 * STEPS_PER_OCTAVE).round() / STEPS_PER_OCTAVE;
    2f32.powf(quantized_log2).max(1.0)
}

// Отрисовывает один CAD-объект на холсте с учётом текущего масштаба/сдвига
fn draw_entity(
    painter: &egui::Painter,
    to_screen: impl Fn((f64, f64)) -> Pos2,
    e: &CadEntity,
    color: Color32,
    width: f32,
    scale: f32,
) {
    let stroke = Stroke::new(width, color);
    match e {
        CadEntity::Line { p1, p2, .. } => {
            painter.line_segment([to_screen(*p1), to_screen(*p2)], stroke);
        }
        CadEntity::Circle { center, radius, .. } => {
            let pts = arc_points(*center, *radius, 0.0, std::f64::consts::TAU, 64);
            painter.add(egui::Shape::line(
                pts.into_iter().map(to_screen).collect(),
                stroke,
            ));
        }
        CadEntity::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => {
            let a0 = start_angle.to_radians();
            let mut a1 = end_angle.to_radians();
            if a1 < a0 {
                a1 += std::f64::consts::TAU;
            }
            let pts = arc_points(*center, *radius, a0, a1, 48);
            painter.add(egui::Shape::line(
                pts.into_iter().map(to_screen).collect(),
                stroke,
            ));
        }
        CadEntity::Polyline { points, closed, .. } => {
            if points.len() >= 2 {
                let mut pts: Vec<Pos2> = points.iter().map(|p| to_screen(*p)).collect();
                if *closed {
                    pts.push(to_screen(points[0]));
                }
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        CadEntity::Point { pos, .. } => {
            painter.circle_filled(to_screen(*pos), 2.2, color);
        }
        CadEntity::Text {
            pos, value, height, ..
        }
        | CadEntity::MText {
            pos, value, height, ..
        } => {
            let font_size =
                quantize_font_size(((*height as f32) * scale).clamp(1.0, MAX_FONT_SIZE));
            painter.text(
                to_screen(*pos),
                egui::Align2::LEFT_BOTTOM,
                value,
                egui::FontId::proportional(font_size),
                color,
            );
        }
        CadEntity::Ellipse {
            center,
            major_axis,
            ratio,
            start_param,
            end_param,
            ..
        } => {
            let a = (major_axis.0.powi(2) + major_axis.1.powi(2)).sqrt();
            let b = a * ratio;
            let rot = major_axis.1.atan2(major_axis.0);
            let mut t1 = *end_param;
            if t1 < *start_param {
                t1 += std::f64::consts::TAU;
            }
            let steps = 64;
            let pts: Vec<Pos2> = (0..=steps)
                .map(|i| {
                    let t = start_param + (t1 - start_param) * (i as f64 / steps as f64);
                    let ex = a * t.cos();
                    let ey = b * t.sin();
                    let x = center.0 + ex * rot.cos() - ey * rot.sin();
                    let y = center.1 + ex * rot.sin() + ey * rot.cos();
                    to_screen((x, y))
                })
                .collect();
            painter.add(egui::Shape::line(pts, stroke));
        }
        CadEntity::Spline { control_points, .. } => {
            if control_points.len() >= 2 {
                let pts: Vec<Pos2> = control_points.iter().map(|p| to_screen(*p)).collect();
                painter.add(egui::Shape::line(pts, stroke));
            }
        }
        CadEntity::Insert { pos, name, .. } => {
            let c = to_screen(*pos);
            let r = 5.0;
            painter.line_segment([c + Vec2::new(-r, 0.0), c + Vec2::new(r, 0.0)], stroke);
            painter.line_segment([c + Vec2::new(0.0, -r), c + Vec2::new(0.0, r)], stroke);

            let font_size = quantize_font_size((0.8_f32 * scale).clamp(1.0, MAX_FONT_SIZE));
            painter.text(
                c + Vec2::new(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                name,
                egui::FontId::proportional(font_size),
                color,
            );
        }
    }
}

// Обрезает длинное имя файла с многоточием посередине для отображения в UI
fn truncate_file_name(name: &str, max_len: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_len {
        return name.to_string();
    }

    let (stem, ext) = match name.rfind('.') {
        Some(pos) if pos > 0 => (&name[..pos], &name[pos..]),
        _ => (name, ""),
    };

    const ELLIPSIS: &str = "...";
    let ellipsis_len = ELLIPSIS.chars().count();
    let ext_len = ext.chars().count();

    let keep = max_len.saturating_sub(ellipsis_len + ext_len).max(1);
    let head: String = stem.chars().take(keep).collect();

    format!("{head}{ELLIPSIS}{ext}")
}

fn arc_points(center: (f64, f64), radius: f64, a0: f64, a1: f64, steps: usize) -> Vec<(f64, f64)> {
    (0..=steps)
        .map(|i| {
            let t = a0 + (a1 - a0) * (i as f64 / steps as f64);
            (center.0 + radius * t.cos(), center.1 + radius * t.sin())
        })
        .collect()
}

// Обрабатывает изменение размера окна без системной рамки (перетаскивание краёв)
fn handle_resize_edges(ctx: &egui::Context) {
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if maximized {
        return;
    }

    let screen = ctx.screen_rect();
    const BORDER: f32 = 8.0;
    const MIN_SIZE: Vec2 = Vec2::new(420.0, 480.0);

    let area = egui::Area::new(egui::Id::new("resize_overlay"))
        .order(egui::Order::Foreground)
        .fixed_pos(Pos2::ZERO)
        .interactable(true);

    area.show(ctx, |ui| {
        let east_rect = Rect::from_min_max(
            Pos2::new(screen.right() - BORDER, screen.top()),
            Pos2::new(screen.right(), screen.bottom() - BORDER),
        );
        let south_rect = Rect::from_min_max(
            Pos2::new(screen.left(), screen.bottom() - BORDER),
            Pos2::new(screen.right() - BORDER, screen.bottom()),
        );
        let corner_rect = Rect::from_min_max(
            Pos2::new(screen.right() - BORDER, screen.bottom() - BORDER),
            Pos2::new(screen.right(), screen.bottom()),
        );

        let east = ui.interact(east_rect, ui.id().with("resize_east"), egui::Sense::drag());
        let south = ui.interact(
            south_rect,
            ui.id().with("resize_south"),
            egui::Sense::drag(),
        );
        let corner = ui.interact(
            corner_rect,
            ui.id().with("resize_corner"),
            egui::Sense::drag(),
        );

        if east.hovered() || east.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeEast);
        }
        if south.hovered() || south.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouth);
        }
        if corner.hovered() || corner.dragged() {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeSouthEast);
        }

        let mut new_size = screen.size();
        let mut changed = false;

        if corner.dragged() {
            new_size += corner.drag_delta();
            changed = true;
        } else {
            if east.dragged() {
                new_size.x += east.drag_delta().x;
                changed = true;
            }
            if south.dragged() {
                new_size.y += south.drag_delta().y;
                changed = true;
            }
        }

        if changed {
            new_size = new_size.max(MIN_SIZE);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(new_size));
        }
    });
}

fn license_button(ui: &mut egui::Ui, diameter: f32) -> egui::Response {
    let size = Vec2::splat(diameter);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();

    let base = Color32::from_rgb(120, 120, 128);
    let hover = Color32::from_rgb(60, 60, 66);
    let color = if response.hovered() { hover } else { base };

    let center = rect.center();
    let r = diameter / 2.0;
    painter.circle_stroke(center, r - 1.0, Stroke::new(1.4, color));

    let dot_r = 1.3;
    let bar_top = center.y - r * 0.45;
    let bar_bottom = center.y + r * 0.12;
    painter.line_segment(
        [
            Pos2::new(center.x, bar_top),
            Pos2::new(center.x, bar_bottom),
        ],
        Stroke::new(1.6, color),
    );
    painter.circle_filled(Pos2::new(center.x, center.y + r * 0.45), dot_r, color);

    response
}

fn close_x_button(ui: &mut egui::Ui, diameter: f32) -> egui::Response {
    let size = Vec2::splat(diameter);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();

    let hover = response.hovered();
    if hover {
        painter.circle_filled(
            rect.center(),
            diameter / 2.0,
            Color32::from_rgb(232, 232, 236),
        );
    }

    let color = if hover {
        Color32::from_rgb(40, 40, 44)
    } else {
        Color32::from_rgb(120, 120, 128)
    };
    let r = diameter * 0.22;
    let c = rect.center();
    let stroke = Stroke::new(1.5, color);
    painter.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
    painter.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], stroke);

    response
}

fn traffic_light_button(ui: &mut egui::Ui, color: Color32, kind: u8) -> bool {
    let size = Vec2::splat(14.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size.x / 2.0, color);

    if response.hovered() {
        let c = rect.center();
        let stroke = Stroke::new(1.4, Color32::from_black_alpha(170));
        match kind {
            0 => {
                let r = 3.2;
                painter.line_segment([c + Vec2::new(-r, -r), c + Vec2::new(r, r)], stroke);
                painter.line_segment([c + Vec2::new(-r, r), c + Vec2::new(r, -r)], stroke);
            }
            1 => {
                let r = 3.4;
                painter.line_segment([c + Vec2::new(-r, 0.0), c + Vec2::new(r, 0.0)], stroke);
            }
            _ => {
                let r = 2.6;
                painter.rect_stroke(Rect::from_center_size(c, Vec2::splat(r * 2.0)), 1.0, stroke);
            }
        }
    }
    response.clicked()
}

const LIBRARY_LICENSES: &[(&str, &str, &str)] = &[
    ("eframe", "0.27", "MIT OR Apache-2.0"),
    ("egui", "0.27", "MIT OR Apache-2.0"),
    ("serde", "1", "MIT OR Apache-2.0"),
    ("image", "0.24", "MIT"),
    ("dxf", "0.5", "MIT"),
    ("rfd", "0.14", "MIT"),
    ("winres", "0.1", "MIT (только для Windows)"),
];

include!(concat!(env!("OUT_DIR"), "/icon_info.rs"));
const ICON_BYTES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon.png"));

// Загружает иконку приложения, встроенную в бинарник во время сборки
fn load_app_icon() -> Option<egui::IconData> {
    if !HAS_ICON {
        return None;
    }
    let img = image::load_from_memory(ICON_BYTES).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

// Точка входа: настраивает окно и запускает цикл eframe
fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_inner_size([1120.0, 720.0])
        .with_min_inner_size([420.0, 480.0])
        .with_title("AntiWaste CAD")
        .with_decorations(false)
        .with_transparent(true);

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        persist_window: false,
        ..Default::default()
    };

    eframe::run_native(
        "AntiWaste CAD",
        options,
        Box::new(|cc| Box::new(App::new(cc))),
    )
}
