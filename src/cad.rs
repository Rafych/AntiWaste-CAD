use dxf::entities::{
    Arc, Circle, Ellipse as DxfEllipse, Entity, EntityType, Insert, Line, LwPolyline, MText,
    ModelPoint, Spline as DxfSpline, Text,
};
use dxf::tables::Layer as DxfLayer;
use dxf::{Color, Drawing, Point, Vector};
use std::path::Path;

// Универсальное представление CAD-объекта (независимое от формата DXF-библиотеки)
#[derive(Clone, Debug)]
pub enum CadEntity {
    Line {
        p1: (f64, f64),
        p2: (f64, f64),
        layer: String,
        linetype: String,
        color: i16,
    },
    Circle {
        center: (f64, f64),
        radius: f64,
        layer: String,
        linetype: String,
        color: i16,
    },
    Arc {
        center: (f64, f64),
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        layer: String,
        linetype: String,
        color: i16,
    },

    Polyline {
        points: Vec<(f64, f64)>,
        closed: bool,
        layer: String,
        linetype: String,
        color: i16,
    },
    Point {
        pos: (f64, f64),
        layer: String,
        linetype: String,
        color: i16,
    },
    Text {
        pos: (f64, f64),
        height: f64,
        rotation: f64,
        value: String,
        layer: String,
        linetype: String,
        color: i16,
    },
    MText {
        pos: (f64, f64),
        height: f64,
        value: String,
        layer: String,
        linetype: String,
        color: i16,
    },
    Ellipse {
        center: (f64, f64),
        major_axis: (f64, f64),
        ratio: f64,
        start_param: f64,
        end_param: f64,
        layer: String,
        linetype: String,
        color: i16,
    },

    Spline {
        control_points: Vec<(f64, f64)>,
        layer: String,
        linetype: String,
        color: i16,
    },

    Insert {
        name: String,
        pos: (f64, f64),
        x_scale: f64,
        y_scale: f64,
        rotation: f64,
        layer: String,
        linetype: String,
        color: i16,
    },
}

impl CadEntity {
    pub fn layer(&self) -> &str {
        match self {
            CadEntity::Line { layer, .. }
            | CadEntity::Circle { layer, .. }
            | CadEntity::Arc { layer, .. }
            | CadEntity::Polyline { layer, .. }
            | CadEntity::Point { layer, .. }
            | CadEntity::Text { layer, .. }
            | CadEntity::MText { layer, .. }
            | CadEntity::Ellipse { layer, .. }
            | CadEntity::Spline { layer, .. }
            | CadEntity::Insert { layer, .. } => layer,
        }
    }

    pub fn set_layer(&mut self, new_layer: String) {
        match self {
            CadEntity::Line { layer, .. }
            | CadEntity::Circle { layer, .. }
            | CadEntity::Arc { layer, .. }
            | CadEntity::Polyline { layer, .. }
            | CadEntity::Point { layer, .. }
            | CadEntity::Text { layer, .. }
            | CadEntity::MText { layer, .. }
            | CadEntity::Ellipse { layer, .. }
            | CadEntity::Spline { layer, .. }
            | CadEntity::Insert { layer, .. } => *layer = new_layer,
        }
    }

    pub fn color(&self) -> i16 {
        match self {
            CadEntity::Line { color, .. }
            | CadEntity::Circle { color, .. }
            | CadEntity::Arc { color, .. }
            | CadEntity::Polyline { color, .. }
            | CadEntity::Point { color, .. }
            | CadEntity::Text { color, .. }
            | CadEntity::MText { color, .. }
            | CadEntity::Ellipse { color, .. }
            | CadEntity::Spline { color, .. }
            | CadEntity::Insert { color, .. } => *color,
        }
    }

    // Возвращает ограничивающий прямоугольник (bbox) объекта: (min_x, min_y, max_x, max_y)
    pub fn extent(&self) -> (f64, f64, f64, f64) {
        match self {
            CadEntity::Line { p1, p2, .. } => {
                bbox_of_points(&[*p1, *p2]).unwrap_or((p1.0, p1.1, p1.0, p1.1))
            }
            CadEntity::Circle { center, radius, .. } => (
                center.0 - radius,
                center.1 - radius,
                center.0 + radius,
                center.1 + radius,
            ),
            CadEntity::Arc { center, radius, .. } => (
                center.0 - radius,
                center.1 - radius,
                center.0 + radius,
                center.1 + radius,
            ),
            CadEntity::Polyline { points, .. } => {
                bbox_of_points(points).unwrap_or((0.0, 0.0, 0.0, 0.0))
            }
            CadEntity::Point { pos, .. } => (pos.0, pos.1, pos.0, pos.1),
            CadEntity::Text { pos, height, .. } => {
                (pos.0, pos.1, pos.0 + height * 3.0, pos.1 + height)
            }
            CadEntity::MText { pos, height, .. } => {
                (pos.0, pos.1, pos.0 + height * 3.0, pos.1 + height)
            }
            CadEntity::Ellipse {
                center, major_axis, ..
            } => {
                let r = (major_axis.0.powi(2) + major_axis.1.powi(2)).sqrt();
                (center.0 - r, center.1 - r, center.0 + r, center.1 + r)
            }
            CadEntity::Spline { control_points, .. } => {
                bbox_of_points(control_points).unwrap_or((0.0, 0.0, 0.0, 0.0))
            }
            CadEntity::Insert { pos, .. } => (pos.0, pos.1, pos.0, pos.1),
        }
    }
}

#[derive(Clone, Debug)]
pub struct LayerInfo {
    pub name: String,

    pub color: i16,
    pub linetype: String,
}

// Определяет, является ли тип линии штриховым (по имени, без учёта регистра)
pub fn is_dashed_linetype(name: &str) -> bool {
    let n = name.trim().to_uppercase();
    if n.is_empty() || n == "CONTINUOUS" || n == "BYLAYER" || n == "BYBLOCK" {
        return false;
    }
    const DASH_HINTS: [&str; 7] = [
        "DASH", "DOT", "HIDDEN", "CENTER", "PHANTOM", "DIVIDE", "BORDER",
    ];
    DASH_HINTS.iter().any(|h| n.contains(h))
}

fn bbox_of_points(pts: &[(f64, f64)]) -> Option<(f64, f64, f64, f64)> {
    let mut it = pts.iter();
    let first = *it.next()?;
    let mut b = (first.0, first.1, first.0, first.1);
    for p in it {
        b.0 = b.0.min(p.0);
        b.1 = b.1.min(p.1);
        b.2 = b.2.max(p.0);
        b.3 = b.3.max(p.1);
    }
    Some(b)
}

fn solid_trace_outline(p1: Point, p2: Point, p3: Point, p4: Point) -> Vec<(f64, f64)> {
    vec![(p1.x, p1.y), (p2.x, p2.y), (p4.x, p4.y), (p3.x, p3.y)]
}

const INFINITE_LINE_DISPLAY_LENGTH: f64 = 1.0e6;

fn color_to_aci(c: &Color) -> i16 {
    if c.is_by_layer() {
        256
    } else if c.is_by_block() {
        0
    } else if let Some(idx) = c.index() {
        idx as i16
    } else {
        256
    }
}

// Преобразует индекс цвета AutoCAD (ACI, 0-255) в RGB
pub fn aci_to_rgb(index: i16) -> (u8, u8, u8) {
    const BASE: [(u8, u8, u8); 10] = [
        (0, 0, 0),
        (255, 0, 0),
        (255, 255, 0),
        (0, 255, 0),
        (0, 255, 255),
        (0, 0, 255),
        (255, 0, 255),
        (255, 255, 255),
        (128, 128, 128),
        (192, 192, 192),
    ];

    if (0..=9).contains(&index) {
        return BASE[index as usize];
    }
    if !(1..=255).contains(&index) {
        return (191, 191, 191);
    }

    if index >= 10 && index <= 249 {
        let group = (index - 10) / 10;
        let shade = (index - 10) % 10;
        let hue = (group as f32) / 24.0;
        let lightness = match shade {
            0 => 0.50,
            1 => 0.65,
            2 => 0.35,
            3 => 0.72,
            4 => 0.28,
            5 => 0.78,
            6 => 0.22,
            7 => 0.85,
            8 => 0.18,
            _ => 0.90,
        };
        return hsl_to_rgb(hue, 1.0, lightness);
    }

    let t = (index - 250) as f32 / 5.0;
    let v = (51.0 + t * (204.0 - 51.0)).round().clamp(0.0, 255.0) as u8;
    (v, v, v)
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    if s <= 0.0 {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let hue_to_rgb = |p: f32, q: f32, mut t: f32| -> f32 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    };
    let r = hue_to_rgb(p, q, h + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h);
    let b = hue_to_rgb(p, q, h - 1.0 / 3.0);
    (
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    )
}

pub fn resolve_entity_rgb(
    entity_color: i16,
    layer_name: &str,
    layers: &[LayerInfo],
) -> (u8, u8, u8) {
    const FALLBACK: (u8, u8, u8) = (70, 70, 78);

    let resolve_layer_color = || -> (u8, u8, u8) {
        layers
            .iter()
            .find(|l| l.name == layer_name)
            .map(|l| {
                if l.color == 7 {
                    FALLBACK
                } else {
                    aci_to_rgb(l.color)
                }
            })
            .unwrap_or(FALLBACK)
    };

    match entity_color {
        256 => resolve_layer_color(),
        0 => resolve_layer_color(),
        7 => FALLBACK,
        c if (1..=255).contains(&c) => aci_to_rgb(c),
        _ => FALLBACK,
    }
}

fn aci_to_color(v: i16) -> Color {
    if v == 256 {
        Color::by_layer()
    } else if v == 0 {
        Color::by_block()
    } else if (1..=255).contains(&v) {
        Color::from_index(v as u8)
    } else {
        Color::by_layer()
    }
}

// Результат попытки загрузки DXF: либо файл уже загружен, либо кодировку
// не удалось определить однозначно и нужно спросить пользователя
pub enum LoadOutcome {
    Loaded(
        Vec<CadEntity>,
        Vec<LayerInfo>,
        crate::encoding::TextEncoding,
    ),

    NeedsEncoding(Vec<crate::encoding::TextEncoding>),
}

// Загружает DXF с автоматическим определением кодировки текста
pub fn load_dxf_auto(path: &Path) -> Result<LoadOutcome, String> {
    let raw = std::fs::read(path).map_err(|e| format!("Не удалось прочитать файл: {}", e))?;

    match crate::encoding::detect_encoding(&raw) {
        crate::encoding::EncodingDetection::Confident(enc) => {
            let (entities, layers) = load_dxf_bytes_with_encoding(&raw, enc)?;
            Ok(LoadOutcome::Loaded(entities, layers, enc))
        }
        crate::encoding::EncodingDetection::Ambiguous(candidates) => {
            Ok(LoadOutcome::NeedsEncoding(candidates))
        }
    }
}

pub fn load_dxf_with_encoding(
    path: &Path,
    encoding: crate::encoding::TextEncoding,
) -> Result<(Vec<CadEntity>, Vec<LayerInfo>), String> {
    let raw = std::fs::read(path).map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
    load_dxf_bytes_with_encoding(&raw, encoding)
}

fn load_dxf_bytes_with_encoding(
    raw: &[u8],
    encoding: crate::encoding::TextEncoding,
) -> Result<(Vec<CadEntity>, Vec<LayerInfo>), String> {
    let decoded = encoding.decode_with(raw);
    let mut cursor = std::io::Cursor::new(decoded.into_bytes());

    let drawing = Drawing::load_with_encoding(&mut cursor, encoding_rs::UTF_8)
        .map_err(|e| format!("Не удалось загрузить DXF: {}", e))?;
    build_from_drawing(drawing)
}

// Преобразует объекты из библиотеки `dxf` во внутренний формат CadEntity
fn build_from_drawing(drawing: Drawing) -> Result<(Vec<CadEntity>, Vec<LayerInfo>), String> {
    let mut out = Vec::new();
    for e in drawing.entities() {
        let layer = e.common.layer.clone();
        let linetype = e.common.line_type_name.clone();
        let color = color_to_aci(&e.common.color);
        match &e.specific {
            EntityType::Line(l) => {
                out.push(CadEntity::Line {
                    p1: (l.p1.x, l.p1.y),
                    p2: (l.p2.x, l.p2.y),
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Circle(c) => {
                out.push(CadEntity::Circle {
                    center: (c.center.x, c.center.y),
                    radius: c.radius,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Arc(a) => {
                out.push(CadEntity::Arc {
                    center: (a.center.x, a.center.y),
                    radius: a.radius,
                    start_angle: a.start_angle,
                    end_angle: a.end_angle,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::LwPolyline(p) => {
                let points = p.vertices.iter().map(|v| (v.x, v.y)).collect();
                out.push(CadEntity::Polyline {
                    points,
                    closed: p.get_is_closed(),
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Polyline(p) => {
                let points = p.vertices().map(|v| (v.location.x, v.location.y)).collect();
                out.push(CadEntity::Polyline {
                    points,
                    closed: p.get_is_closed(),
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::ModelPoint(pt) => {
                out.push(CadEntity::Point {
                    pos: (pt.location.x, pt.location.y),
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Text(t) => {
                out.push(CadEntity::Text {
                    pos: (t.location.x, t.location.y),
                    height: t.text_height,
                    rotation: t.rotation,
                    value: t.value.clone(),
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::MText(m) => {
                out.push(CadEntity::MText {
                    pos: (m.insertion_point.x, m.insertion_point.y),
                    height: m.initial_text_height,
                    value: m.text.clone(),
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Ellipse(el) => {
                out.push(CadEntity::Ellipse {
                    center: (el.center.x, el.center.y),
                    major_axis: (el.major_axis.x, el.major_axis.y),
                    ratio: el.minor_axis_ratio,
                    start_param: el.start_parameter,
                    end_param: el.end_parameter,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Spline(sp) => {
                let control_points = sp.control_points.iter().map(|p| (p.x, p.y)).collect();
                out.push(CadEntity::Spline {
                    control_points,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Insert(ins) => {
                out.push(CadEntity::Insert {
                    name: ins.name.clone(),
                    pos: (ins.location.x, ins.location.y),
                    x_scale: ins.x_scale_factor,
                    y_scale: ins.y_scale_factor,
                    rotation: ins.rotation,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Solid(s) => {
                let points = solid_trace_outline(
                    s.first_corner.clone(),
                    s.second_corner.clone(),
                    s.third_corner.clone(),
                    s.fourth_corner.clone(),
                );
                out.push(CadEntity::Polyline {
                    points,
                    closed: true,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Trace(t) => {
                let points = solid_trace_outline(
                    t.first_corner.clone(),
                    t.second_corner.clone(),
                    t.third_corner.clone(),
                    t.fourth_corner.clone(),
                );
                out.push(CadEntity::Polyline {
                    points,
                    closed: true,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Leader(l) => {
                let points = l.vertices.iter().map(|p| (p.x, p.y)).collect();
                out.push(CadEntity::Polyline {
                    points,
                    closed: false,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::Ray(r) => {
                let len = INFINITE_LINE_DISPLAY_LENGTH;
                let p1 = (r.start_point.x, r.start_point.y);
                let p2 = (
                    r.start_point.x + r.unit_direction_vector.x * len,
                    r.start_point.y + r.unit_direction_vector.y * len,
                );
                out.push(CadEntity::Line {
                    p1,
                    p2,
                    layer,
                    linetype,
                    color,
                });
            }
            EntityType::XLine(x) => {
                let len = INFINITE_LINE_DISPLAY_LENGTH;
                let p1 = (
                    x.first_point.x - x.unit_direction_vector.x * len,
                    x.first_point.y - x.unit_direction_vector.y * len,
                );
                let p2 = (
                    x.first_point.x + x.unit_direction_vector.x * len,
                    x.first_point.y + x.unit_direction_vector.y * len,
                );
                out.push(CadEntity::Line {
                    p1,
                    p2,
                    layer,
                    linetype,
                    color,
                });
            }
            _ => {}
        }
    }

    let mut layers = Vec::new();
    for l in drawing.layers() {
        layers.push(LayerInfo {
            name: l.name.clone(),
            color: color_to_aci(&l.color),
            linetype: l.line_type_name.clone(),
        });
    }

    Ok((out, layers))
}

// Обратное преобразование: собирает объект Drawing из внутреннего представления
fn build_drawing(entities: &[CadEntity], layers: &[LayerInfo]) -> Drawing {
    let mut drawing = Drawing::new();

    let existing_layer_names: std::collections::HashSet<String> =
        drawing.layers().map(|l| l.name.clone()).collect();
    for li in layers {
        if existing_layer_names.contains(&li.name) {
            continue;
        }
        let mut layer = DxfLayer::default();
        layer.name = li.name.clone();
        layer.color = aci_to_color(li.color);
        layer.line_type_name = li.linetype.clone();
        drawing.add_layer(layer);
    }

    for ent in entities {
        let entity = match ent {
            CadEntity::Line {
                p1,
                p2,
                layer,
                linetype,
                color,
            } => {
                let mut line = Line::default();
                line.p1 = Point::new(p1.0, p1.1, 0.0);
                line.p2 = Point::new(p2.0, p2.1, 0.0);
                let mut e = Entity::new(EntityType::Line(line));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Circle {
                center,
                radius,
                layer,
                linetype,
                color,
            } => {
                let mut c = Circle::default();
                c.center = Point::new(center.0, center.1, 0.0);
                c.radius = *radius;
                let mut e = Entity::new(EntityType::Circle(c));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Arc {
                center,
                radius,
                start_angle,
                end_angle,
                layer,
                linetype,
                color,
            } => {
                let mut arc = Arc::default();
                arc.center = Point::new(center.0, center.1, 0.0);
                arc.radius = *radius;
                arc.start_angle = *start_angle;
                arc.end_angle = *end_angle;
                let mut e = Entity::new(EntityType::Arc(arc));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Polyline {
                points,
                closed,
                layer,
                linetype,
                color,
            } => {
                let mut poly = LwPolyline::default();
                poly.set_is_closed(*closed);
                poly.vertices = points
                    .iter()
                    .map(|(x, y)| {
                        let mut v = dxf::LwPolylineVertex::default();
                        v.x = *x;
                        v.y = *y;
                        v
                    })
                    .collect();
                let mut e = Entity::new(EntityType::LwPolyline(poly));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Point {
                pos,
                layer,
                linetype,
                color,
            } => {
                let mut p = ModelPoint::default();
                p.location = Point::new(pos.0, pos.1, 0.0);
                let mut e = Entity::new(EntityType::ModelPoint(p));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Text {
                pos,
                height,
                rotation,
                value,
                layer,
                linetype,
                color,
            } => {
                let mut t = Text::default();
                t.location = Point::new(pos.0, pos.1, 0.0);
                t.text_height = *height;
                t.rotation = *rotation;
                t.value = value.clone();
                let mut e = Entity::new(EntityType::Text(t));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::MText {
                pos,
                height,
                value,
                layer,
                linetype,
                color,
            } => {
                let mut m = MText::default();
                m.insertion_point = Point::new(pos.0, pos.1, 0.0);
                m.initial_text_height = *height;
                m.text = value.clone();
                let mut e = Entity::new(EntityType::MText(m));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Ellipse {
                center,
                major_axis,
                ratio,
                start_param,
                end_param,
                layer,
                linetype,
                color,
            } => {
                let mut el = DxfEllipse::default();
                el.center = Point::new(center.0, center.1, 0.0);
                el.major_axis = Vector::new(major_axis.0, major_axis.1, 0.0);
                el.minor_axis_ratio = *ratio;
                el.start_parameter = *start_param;
                el.end_parameter = *end_param;
                let mut e = Entity::new(EntityType::Ellipse(el));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Spline {
                control_points,
                layer,
                linetype,
                color,
            } => {
                let mut sp = DxfSpline::default();
                sp.control_points = control_points
                    .iter()
                    .map(|(x, y)| Point::new(*x, *y, 0.0))
                    .collect();
                let mut e = Entity::new(EntityType::Spline(sp));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
            CadEntity::Insert {
                name,
                pos,
                x_scale,
                y_scale,
                rotation,
                layer,
                linetype,
                color,
            } => {
                let mut ins = Insert::default();
                ins.name = name.clone();
                ins.location = Point::new(pos.0, pos.1, 0.0);
                ins.x_scale_factor = *x_scale;
                ins.y_scale_factor = *y_scale;
                ins.rotation = *rotation;
                let mut e = Entity::new(EntityType::Insert(ins));
                e.common.layer = layer.clone();
                e.common.line_type_name = linetype.clone();
                e.common.color = aci_to_color(*color);
                e
            }
        };
        drawing.add_entity(entity);
    }

    drawing
}

// Сохраняет чертёж в DXF, при необходимости перекодируя текст в исходную кодировку
pub fn save_dxf_with_encoding(
    path: &Path,
    entities: &[CadEntity],
    layers: &[LayerInfo],
    encoding: crate::encoding::TextEncoding,
) -> Result<(), String> {
    let drawing = build_drawing(entities, layers);

    if encoding == crate::encoding::TextEncoding::Utf8 {
        return drawing
            .save_file(path)
            .map_err(|e| format!("Не удалось сохранить DXF: {}", e));
    }

    let mut buf: Vec<u8> = Vec::new();
    drawing
        .save(&mut buf)
        .map_err(|e| format!("Не удалось сохранить DXF: {}", e))?;
    let text = String::from_utf8(buf).map_err(|e| {
        format!(
            "Не удалось сохранить DXF (внутренняя ошибка кодировки): {}",
            e
        )
    })?;
    let bytes = encoding.encode(&text);
    std::fs::write(path, bytes).map_err(|e| format!("Не удалось сохранить DXF: {}", e))
}

pub fn bounding_box(entities: &[CadEntity]) -> Option<(f64, f64, f64, f64)> {
    let mut it = entities.iter();
    let first = it.next()?;
    let mut b = first.extent();
    for e in it {
        let x = e.extent();
        b.0 = b.0.min(x.0);
        b.1 = b.1.min(x.1);
        b.2 = b.2.max(x.2);
        b.3 = b.3.max(x.3);
    }
    Some(b)
}
