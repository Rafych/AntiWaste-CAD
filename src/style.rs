use eframe::egui;
use egui::{Color32, Rounding, Stroke, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(0, 122, 255);

pub const WIDGET_BG_INACTIVE: Color32 = Color32::from_rgb(233, 233, 237);
pub const WIDGET_BG_HOVERED: Color32 = Color32::from_rgb(220, 232, 255);
pub const WIDGET_BG_ACTIVE: Color32 = ACCENT;
pub const EXTREME_BG_COLOR: Color32 = Color32::from_rgb(255, 255, 255);

pub struct ScrollbarColors {
    pub track: Color32,
    pub knob_inactive: Color32,
    pub knob_hovered: Color32,
    pub knob_active: Color32,
}

pub const SCROLLBAR_COLORS: ScrollbarColors = ScrollbarColors {
    track: Color32::from_rgba_premultiplied(30, 30, 30, 40),
    knob_inactive: Color32::from_rgba_premultiplied(35, 35, 35, 50),
    knob_hovered: Color32::from_rgba_premultiplied(40, 40, 40, 80),
    knob_active: Color32::from_rgba_premultiplied(45, 45, 45, 110),
};

pub fn apply_scrollbar_colors(ui: &mut egui::Ui) {
    let visuals = ui.visuals_mut();
    visuals.extreme_bg_color = SCROLLBAR_COLORS.track;
    visuals.widgets.inactive.bg_fill = SCROLLBAR_COLORS.knob_inactive;
    visuals.widgets.inactive.weak_bg_fill = SCROLLBAR_COLORS.knob_inactive;
    visuals.widgets.inactive.bg_stroke = Stroke::NONE;
    visuals.widgets.hovered.bg_fill = SCROLLBAR_COLORS.knob_hovered;
    visuals.widgets.hovered.bg_stroke = Stroke::NONE;
    visuals.widgets.active.bg_fill = SCROLLBAR_COLORS.knob_active;
    visuals.widgets.active.bg_stroke = Stroke::NONE;

    ui.style_mut().spacing.scroll.foreground_color = false;
}

pub fn restore_widget_colors(ui: &mut egui::Ui) {
    let defaults = Visuals::light();
    let visuals = ui.visuals_mut();
    visuals.extreme_bg_color = EXTREME_BG_COLOR;
    visuals.widgets.inactive.bg_fill = WIDGET_BG_INACTIVE;
    visuals.widgets.inactive.weak_bg_fill = WIDGET_BG_INACTIVE;
    visuals.widgets.inactive.bg_stroke = defaults.widgets.inactive.bg_stroke;
    visuals.widgets.hovered.bg_fill = WIDGET_BG_HOVERED;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.6));
    visuals.widgets.active.bg_fill = WIDGET_BG_ACTIVE;
    visuals.widgets.active.bg_stroke = defaults.widgets.active.bg_stroke;
}

include!(concat!(env!("OUT_DIR"), "/fonts_generated.rs"));

// Применяет единую светлую тему (закруглённые углы, цвета, тени) ко всему приложению
pub fn apply(ctx: &egui::Context) {
    let mut visuals = Visuals::light();

    let rounding = Rounding::same(10.0);
    visuals.window_rounding = Rounding::same(14.0);
    visuals.menu_rounding = rounding;
    visuals.widgets.noninteractive.rounding = rounding;
    visuals.widgets.inactive.rounding = rounding;
    visuals.widgets.hovered.rounding = rounding;
    visuals.widgets.active.rounding = rounding;
    visuals.widgets.open.rounding = rounding;

    visuals.window_fill = Color32::from_rgb(246, 246, 248);
    visuals.panel_fill = Color32::from_rgb(246, 246, 248);
    visuals.extreme_bg_color = EXTREME_BG_COLOR;
    visuals.faint_bg_color = Color32::from_rgb(238, 238, 241);

    visuals.widgets.inactive.bg_fill = WIDGET_BG_INACTIVE;
    visuals.widgets.inactive.weak_bg_fill = WIDGET_BG_INACTIVE;
    visuals.widgets.hovered.bg_fill = WIDGET_BG_HOVERED;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, ACCENT.linear_multiply(0.6));
    visuals.widgets.active.bg_fill = WIDGET_BG_ACTIVE;

    visuals.selection.bg_fill = ACCENT;
    visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    visuals.hyperlink_color = ACCENT;

    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));
    visuals.window_stroke = Stroke::new(1.0, Color32::from_rgb(228, 228, 232));
    visuals.window_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 10.0),
        blur: 32.0,
        spread: 0.0,
        color: Color32::from_black_alpha(55),
    };
    visuals.popup_shadow = egui::epaint::Shadow {
        offset: egui::vec2(0.0, 6.0),
        blur: 18.0,
        spread: 0.0,
        color: Color32::from_black_alpha(45),
    };

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 7.0);
    style.spacing.window_margin = egui::Margin::same(14.0);
    style.spacing.indent = 16.0;
    style.animation_time = 0.12;

    style.interaction.selectable_labels = false;
    ctx.set_style(style);

    setup_fonts(ctx);
}

// Регистрирует встроенные шрифты и задаёт порядок отображения (запасной шрифт): RU → EN → JA
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts
        .font_data
        .insert("font_ja".to_owned(), egui::FontData::from_static(FONT_JA));
    fonts
        .font_data
        .insert("font_ru".to_owned(), egui::FontData::from_static(FONT_RU));
    fonts
        .font_data
        .insert("font_en".to_owned(), egui::FontData::from_static(FONT_EN));

    let proportional = fonts
        .families
        .get_mut(&egui::FontFamily::Proportional)
        .unwrap();
    proportional.insert(0, "font_ru".to_owned());
    proportional.insert(1, "font_en".to_owned());
    proportional.insert(2, "font_ja".to_owned());

    let monospace = fonts
        .families
        .get_mut(&egui::FontFamily::Monospace)
        .unwrap();
    monospace.push("font_ru".to_owned());
    monospace.push("font_en".to_owned());
    monospace.push("font_ja".to_owned());

    ctx.set_fonts(fonts);
}
