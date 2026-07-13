use crate::cad::{bounding_box, is_dashed_linetype, CadEntity};
use egui::Color32;

// Виды проблем чертежа, которые умеет находить и исправлять анализатор
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    NearDuplicateLine,
    NearDuplicateArc,
    NearDuplicateCircle,
    NearDuplicatePoint,
    NearDuplicateText,
    NearDuplicatePolyline,
    DuplicateLayer,

    LineOvershoot,

    LineGap,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    High,

    Medium,

    Low,
}

impl Severity {
    pub fn color(self) -> Color32 {
        match self {
            Severity::High => Color32::from_rgb(220, 53, 53),
            Severity::Medium => Color32::from_rgb(255, 149, 0),
            Severity::Low => Color32::from_rgb(0, 122, 255),
        }
    }
}

impl IssueKind {
    pub fn severity(self) -> Severity {
        match self {
            IssueKind::LineGap | IssueKind::LineOvershoot => Severity::High,
            IssueKind::NearDuplicateLine
            | IssueKind::NearDuplicateArc
            | IssueKind::NearDuplicateCircle
            | IssueKind::NearDuplicatePoint
            | IssueKind::NearDuplicateText
            | IssueKind::NearDuplicatePolyline => Severity::Medium,
            IssueKind::DuplicateLayer => Severity::Low,
        }
    }
}

// Найденная проблема: тип, положение, индексы затронутых объектов и данные для исправления
#[derive(Clone, Debug)]
pub struct Issue {
    pub kind: IssueKind,

    pub center: (f64, f64),

    pub primary: usize,

    pub secondary: usize,

    pub detail: String,

    pub endpoint_a: Option<u8>,

    pub endpoint_b: Option<u8>,

    pub line_overlap: Option<((f64, f64), (f64, f64))>,
}

impl Issue {
    pub fn severity(&self) -> Severity {
        self.kind.severity()
    }
}

// Одно элементарное изменение, которое будет применено при исправлении проблемы
// (используется и для предварительного просмотра, и для самого исправления)
pub enum PreviewItem {
    Delete(usize),

    Change {
        before: CadEntity,
        after: CadEntity,
    },

    Add {
        p1: (f64, f64),
        p2: (f64, f64),
    },
}

// Строит список изменений, которые потребуются для исправления данной проблемы,
// не применяя их — используется как для предпросмотра, так и внутри apply_fix
pub fn preview_fix(entities: &[CadEntity], issue: &Issue) -> Vec<PreviewItem> {
    let mut items = Vec::new();
    match issue.kind {
        IssueKind::DuplicateLayer => {
            if issue.secondary < entities.len() {
                let target_raw = entities[issue.secondary].layer().to_string();
                let normalized = target_raw.trim().to_lowercase();
                for e in entities.iter() {
                    if e.layer().trim().to_lowercase() == normalized {
                        items.push(PreviewItem::Change {
                            before: e.clone(),
                            after: e.clone(),
                        });
                    }
                }
            }
        }
        IssueKind::LineOvershoot => {
            if let Some(CadEntity::Line {
                p1,
                p2,
                layer,
                linetype,
                color,
            }) = entities.get(issue.secondary)
            {
                let mut np1 = *p1;
                let mut np2 = *p2;
                match issue.endpoint_b {
                    Some(0) => np1 = issue.center,
                    Some(1) => np2 = issue.center,
                    _ => {}
                }
                items.push(PreviewItem::Change {
                    before: entities[issue.secondary].clone(),
                    after: CadEntity::Line {
                        p1: np1,
                        p2: np2,
                        layer: layer.clone(),
                        linetype: linetype.clone(),
                        color: *color,
                    },
                });
            }
        }
        IssueKind::LineGap => {
            if let (
                Some(CadEntity::Line {
                    p1: ap1,
                    p2: ap2,
                    ..
                }),
                Some(CadEntity::Line {
                    p1: bp1, p2: bp2, ..
                }),
            ) = (entities.get(issue.primary), entities.get(issue.secondary))
            {
                let a_end = if issue.endpoint_a == Some(1) {
                    *ap2
                } else {
                    *ap1
                };
                let b_end = if issue.endpoint_b == Some(1) {
                    *bp2
                } else {
                    *bp1
                };
                items.push(PreviewItem::Add {
                    p1: a_end,
                    p2: b_end,
                });
            }
        }
        IssueKind::NearDuplicateLine => {
            if issue.secondary < entities.len() {
                items.push(PreviewItem::Delete(issue.secondary));
                for remaining in trim_secondary_line(entities, issue) {
                    if let CadEntity::Line { p1, p2, .. } = remaining {
                        items.push(PreviewItem::Add { p1, p2 });
                    }
                }
            }
        }
        _ => {
            if issue.secondary < entities.len() {
                items.push(PreviewItem::Delete(issue.secondary));
            }
        }
    }
    items
}

const ANGLE_THRESHOLD_DEG: f64 = 3.0;
const REL_DIST_THRESHOLD: f64 = 0.004;
const REL_RADIUS_THRESHOLD: f64 = 0.01;

const MIN_OVERLAP_FRACTION: f64 = 0.2;

const STRONG_OVERLAP_FRACTION: f64 = 0.6;

const ENDPOINT_ANCHOR_MULT: f64 = 4.0;

const EXTREME_LENGTH_RATIO: f64 = 0.1;
const EXTREME_LENGTH_MIN_OVERLAP_FRACTION: f64 = 0.8;

const HATCH_NEIGHBOR_MIN: usize = 3;

const HATCH_SEARCH_REL: f64 = 0.05;

// Главная функция анализа: обходит все объекты и находит проблемы
// (дубликаты, разрывы, нахлёсты и т.д.)
pub fn detect_issues(entities: &[CadEntity]) -> Vec<Issue> {
    let mut issues = Vec::new();

    let diag = bounding_box(entities)
        .map(|(x0, y0, x1, y1)| ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt())
        .unwrap_or(1.0)
        .max(1e-6);

    let dist_threshold = diag * REL_DIST_THRESHOLD;
    let radius_threshold = diag * REL_RADIUS_THRESHOLD;
    let hatch_like = compute_hatch_like_lines(entities, diag);

    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            if let Some(issue) = compare_pair(
                entities,
                i,
                j,
                dist_threshold,
                radius_threshold,
                &hatch_like,
            ) {
                issues.push(issue);
            }
        }
    }

    issues.extend(detect_line_connectivity_issues(
        entities,
        diag,
        dist_threshold,
        &hatch_like,
    ));

    let mut seen: Vec<(String, String)> = Vec::new();
    for (idx, e) in entities.iter().enumerate() {
        let raw = e.layer().to_string();
        let normalized = raw.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if let Some((_, canonical_raw)) = seen.iter().find(|(n, _)| *n == normalized) {
            if *canonical_raw != raw {
                let center = entity_marker_point(e);
                issues.push(Issue {
                    kind: IssueKind::DuplicateLayer,
                    center,
                    primary: usize::MAX,
                    secondary: idx,
                    detail: format!(
                        "Слой с расхождением в написании «{}» → «{}»",
                        raw, canonical_raw
                    ),
                    endpoint_a: None,
                    endpoint_b: None,
                    line_overlap: None,
                });
            }
        } else {
            seen.push((normalized, raw));
        }
    }

    issues
}

// Штраф/бонус к порогу схожести в зависимости от того, на одном ли слое объекты
fn layer_tier_factor(layer_a: &str, layer_b: &str) -> f64 {
    if layer_a.trim().to_lowercase() == layer_b.trim().to_lowercase() {
        1.0
    } else {
        0.5
    }
}

// Сравнивает пару объектов и решает, являются ли они почти дублирующимися
fn compare_pair(
    entities: &[CadEntity],
    i: usize,
    j: usize,
    dist_threshold: f64,
    radius_threshold: f64,
    hatch_like: &[bool],
) -> Option<Issue> {
    match (&entities[i], &entities[j]) {
        (
            CadEntity::Line {
                p1: a1,
                p2: a2,
                layer: la,
                ..
            },
            CadEntity::Line {
                p1: b1,
                p2: b2,
                layer: lb,
                ..
            },
        ) => {
            if hatch_like[i] || hatch_like[j] {
                return None;
            }
            let eff_dist = dist_threshold * layer_tier_factor(la, lb);
            let overlap = near_duplicate_line_check(*a1, *a2, *b1, *b2, eff_dist)?;
            Some(Issue {
                kind: IssueKind::NearDuplicateLine,
                center: overlap.mid,
                primary: i,
                secondary: j,
                detail: format!(
                    "Почти дублирующийся отрезок (промежуток ≈{:.3})",
                    overlap.gap
                ),
                endpoint_a: None,
                endpoint_b: None,
                line_overlap: Some((overlap.p1, overlap.p2)),
            })
        }
        (
            CadEntity::Arc {
                center: ac,
                radius: ar,
                start_angle: a_s,
                end_angle: a_e,
                layer: la,
                ..
            },
            CadEntity::Arc {
                center: bc,
                radius: br,
                start_angle: b_s,
                end_angle: b_e,
                layer: lb,
                ..
            },
        ) => {
            let tier = layer_tier_factor(la, lb);
            let center_dist = dist(*ac, *bc);
            let radius_diff = (ar - br).abs();
            if center_dist >= dist_threshold * tier || radius_diff >= radius_threshold * tier {
                return None;
            }

            let overlap = arc_angle_overlap(*a_s, *a_e, *b_s, *b_e)?;

            let marker = (
                ac.0 + ar * overlap.mid_angle.cos(),
                ac.1 + ar * overlap.mid_angle.sin(),
            );
            Some(Issue {
                kind: IssueKind::NearDuplicateArc,
                center: marker,
                primary: i,
                secondary: j,
                detail: format!(
                    "Почти дублирующаяся дуга (разница центров ≈{:.3} / разница радиусов ≈{:.3})",
                    center_dist, radius_diff
                ),
                endpoint_a: None,
                endpoint_b: None,
                line_overlap: None,
            })
        }
        (
            CadEntity::Circle {
                center: ac,
                radius: ar,
                layer: la,
                ..
            },
            CadEntity::Circle {
                center: bc,
                radius: br,
                layer: lb,
                ..
            },
        ) => {
            let tier = layer_tier_factor(la, lb);
            let center_dist = dist(*ac, *bc);
            let radius_diff = (ar - br).abs();
            if center_dist < dist_threshold * tier && radius_diff < radius_threshold * tier {
                let angle = if center_dist > 1e-9 {
                    (bc.1 - ac.1).atan2(bc.0 - ac.0)
                } else {
                    std::f64::consts::FRAC_PI_2
                };
                let marker = (ac.0 + ar * angle.cos(), ac.1 + ar * angle.sin());
                Some(Issue {
                    kind: IssueKind::NearDuplicateCircle,
                    center: marker,
                    primary: i,
                    secondary: j,
                    detail: format!(
                        "Почти дублирующаяся окружность (разница центров ≈{:.3} / разница радиусов ≈{:.3})",
                        center_dist, radius_diff
                    ),
                    endpoint_a: None,
                    endpoint_b: None,
                    line_overlap: None,
                })
            } else {
                None
            }
        }
        (
            CadEntity::Point {
                pos: a, layer: la, ..
            },
            CadEntity::Point {
                pos: b, layer: lb, ..
            },
        ) => {
            let eff_dist = dist_threshold * layer_tier_factor(la, lb);
            let d = dist(*a, *b);
            if d < eff_dist {
                Some(Issue {
                    kind: IssueKind::NearDuplicatePoint,
                    center: *a,
                    primary: i,
                    secondary: j,
                    detail: format!("Точки почти в одних координатах (расстояние ≈{:.3})", d),
                    endpoint_a: None,
                    endpoint_b: None,
                    line_overlap: None,
                })
            } else {
                None
            }
        }
        (
            CadEntity::Text {
                pos: a,
                value: av,
                layer: la,
                ..
            },
            CadEntity::Text {
                pos: b,
                value: bv,
                layer: lb,
                ..
            },
        )
        | (
            CadEntity::MText {
                pos: a,
                value: av,
                layer: la,
                ..
            },
            CadEntity::MText {
                pos: b,
                value: bv,
                layer: lb,
                ..
            },
        ) => {
            let eff_dist = dist_threshold * layer_tier_factor(la, lb);
            let d = dist(*a, *b);
            if d < eff_dist && av.trim() == bv.trim() {
                Some(Issue {
                    kind: IssueKind::NearDuplicateText,
                    center: *a,
                    primary: i,
                    secondary: j,
                    detail: format!(
                        "Текст с почти одинаковым положением и содержимым (расстояние ≈{:.3})",
                        d
                    ),
                    endpoint_a: None,
                    endpoint_b: None,
                    line_overlap: None,
                })
            } else {
                None
            }
        }
        (
            CadEntity::Polyline {
                points: ap,
                layer: la,
                ..
            },
            CadEntity::Polyline {
                points: bp,
                layer: lb,
                ..
            },
        ) => {
            if ap.len() != bp.len() || ap.is_empty() {
                return None;
            }
            let eff_dist = dist_threshold * layer_tier_factor(la, lb);

            let max_d_forward = ap
                .iter()
                .zip(bp.iter())
                .map(|(a, b)| dist(*a, *b))
                .fold(0.0_f64, f64::max);
            let max_d_reversed = ap
                .iter()
                .zip(bp.iter().rev())
                .map(|(a, b)| dist(*a, *b))
                .fold(0.0_f64, f64::max);
            let max_d = max_d_forward.min(max_d_reversed);
            if max_d < eff_dist {
                let mid = entities[i].extent();
                let center = ((mid.0 + mid.2) / 2.0, (mid.1 + mid.3) / 2.0);
                Some(Issue {
                    kind: IssueKind::NearDuplicatePolyline,
                    center,
                    primary: i,
                    secondary: j,
                    detail: format!(
                        "Почти дублирующаяся полилиния (макс. разница вершин ≈{:.3})",
                        max_d
                    ),
                    endpoint_a: None,
                    endpoint_b: None,
                    line_overlap: None,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

// Обрезает вторую линию так, чтобы устранить нахлёст (overshoot)
fn trim_secondary_line(entities: &[CadEntity], issue: &Issue) -> Vec<CadEntity> {
    let mut result = Vec::new();
    let (
        Some((ov1, ov2)),
        Some(CadEntity::Line {
            p1: s1,
            p2: s2,
            layer,
            linetype,
            color,
        }),
    ) = (issue.line_overlap, entities.get(issue.secondary))
    else {
        return result;
    };

    let dx = s2.0 - s1.0;
    let dy = s2.1 - s1.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return result;
    }
    let ux = dx / len;
    let uy = dy / len;
    let proj = |p: (f64, f64)| -> f64 { (p.0 - s1.0) * ux + (p.1 - s1.1) * uy };

    let t1 = proj(ov1);
    let t2 = proj(ov2);
    let (near_s1, t_lo, near_s2, t_hi) = if t1 <= t2 {
        (ov1, t1, ov2, t2)
    } else {
        (ov2, t2, ov1, t1)
    };

    let diag = bounding_box(entities)
        .map(|(x0, y0, x1, y1)| ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt())
        .unwrap_or(1.0)
        .max(1e-6);
    let noise_floor = (diag * REL_DIST_THRESHOLD * 2.0).max(len * 1e-6).max(1e-9);

    if t_lo > noise_floor {
        result.push(CadEntity::Line {
            p1: *s1,
            p2: near_s1,
            layer: layer.clone(),
            linetype: linetype.clone(),
            color: *color,
        });
    }
    if t_hi < len - noise_floor {
        result.push(CadEntity::Line {
            p1: near_s2,
            p2: *s2,
            layer: layer.clone(),
            linetype: linetype.clone(),
            color: *color,
        });
    }
    result
}

// Евклидово расстояние между двумя точками
fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

const CONNECT_MIN_CORNER_ANGLE_DEG: f64 = 8.0;
const OVERSHOOT_MAX_REL: f64 = 0.02;
const GAP_MAX_REL: f64 = 0.02;

// Находит точку пересечения двух отрезков (если она есть)
fn line_intersect(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> Option<((f64, f64), f64, f64)> {
    let dx1 = a2.0 - a1.0;
    let dy1 = a2.1 - a1.1;
    let dx2 = b2.0 - b1.0;
    let dy2 = b2.1 - b1.1;
    let denom = dx1 * dy2 - dy1 * dx2;
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = ((b1.0 - a1.0) * dy2 - (b1.1 - a1.1) * dx2) / denom;
    let s = ((b1.0 - a1.0) * dy1 - (b1.1 - a1.1) * dx1) / denom;
    let px = a1.0 + t * dx1;
    let py = a1.1 + t * dy1;
    Some(((px, py), t, s))
}

// Вычисляет длину выступа (overshoot) линии за пределы точки пересечения
fn overshoot_len(t: f64, len: f64) -> f64 {
    if t > 1.0 {
        (t - 1.0) * len
    } else if t < 0.0 {
        -t * len
    } else {
        0.0
    }
}

// Ищет разрывы (gap) и выступы (overshoot) в местах, где линии должны соединяться
fn detect_line_connectivity_issues(
    entities: &[CadEntity],
    diag: f64,
    connected_threshold: f64,
    hatch_like: &[bool],
) -> Vec<Issue> {
    let mut issues = Vec::new();
    let overshoot_max = diag * OVERSHOOT_MAX_REL;
    let gap_max = diag * GAP_MAX_REL;

    let lines: Vec<usize> = entities
        .iter()
        .enumerate()
        .filter_map(|(idx, e)| match e {
            CadEntity::Line { linetype, .. }
                if !hatch_like[idx] && !is_dashed_linetype(linetype) =>
            {
                Some(idx)
            }
            _ => None,
        })
        .collect();

    for (ia, &i) in lines.iter().enumerate() {
        let CadEntity::Line { p1: a1, p2: a2, .. } = &entities[i] else {
            continue;
        };
        let len_a = dist(*a1, *a2);
        if len_a < 1e-9 {
            continue;
        }
        for &j in lines.iter().skip(ia + 1) {
            let CadEntity::Line { p1: b1, p2: b2, .. } = &entities[j] else {
                continue;
            };
            let len_b = dist(*b1, *b2);
            if len_b < 1e-9 {
                continue;
            }

            let angle_a = (a2.1 - a1.1).atan2(a2.0 - a1.0);
            let angle_b = (b2.1 - b1.1).atan2(b2.0 - b1.0);
            let mut adiff = (angle_a - angle_b).to_degrees().abs() % 180.0;
            if adiff > 90.0 {
                adiff = 180.0 - adiff;
            }
            if adiff < CONNECT_MIN_CORNER_ANGLE_DEG {
                continue;
            }

            let Some((corner, t, s)) = line_intersect(*a1, *a2, *b1, *b2) else {
                continue;
            };
            let extra_a = overshoot_len(t, len_a);
            let extra_b = overshoot_len(s, len_b);

            let a_end: u8 = if t > 1.0 { 1 } else { 0 };
            let b_end: u8 = if s > 1.0 { 1 } else { 0 };

            let a_within = extra_a <= connected_threshold;
            let b_within = extra_b <= connected_threshold;

            if a_within && b_within {
                continue;
            }

            let a_overshoots = extra_a > connected_threshold && extra_a <= overshoot_max;
            let b_overshoots = extra_b > connected_threshold && extra_b <= overshoot_max;
            let a_short = extra_a > connected_threshold && extra_a <= gap_max;
            let b_short = extra_b > connected_threshold && extra_b <= gap_max;

            if a_overshoots && b_within {
                issues.push(Issue {
                    kind: IssueKind::LineOvershoot,
                    center: corner,
                    primary: j,
                    secondary: i,
                    detail: format!(
                        "Линия выступает за пределы (величина выступа ≈{:.3})",
                        extra_a
                    ),
                    endpoint_a: None,
                    endpoint_b: Some(a_end),
                    line_overlap: None,
                });
            } else if b_overshoots && a_within {
                issues.push(Issue {
                    kind: IssueKind::LineOvershoot,
                    center: corner,
                    primary: i,
                    secondary: j,
                    detail: format!(
                        "Линия выступает за пределы (величина выступа ≈{:.3})",
                        extra_b
                    ),
                    endpoint_a: None,
                    endpoint_b: Some(b_end),
                    line_overlap: None,
                });
            } else if a_short && b_short {
                let a_pt = if a_end == 1 { *a2 } else { *a1 };
                let b_pt = if b_end == 1 { *b2 } else { *b1 };
                let gap_dist = dist(a_pt, b_pt);
                issues.push(Issue {
                    kind: IssueKind::LineGap,
                    center: corner,
                    primary: i,
                    secondary: j,
                    detail: format!(
                        "Линии не соединены между собой (промежуток ≈{:.3})",
                        gap_dist
                    ),
                    endpoint_a: Some(a_end),
                    endpoint_b: Some(b_end),
                    line_overlap: None,
                });
            }
        }
    }

    issues
}

// Помечает линии, которые похожи на штриховку (hatch), чтобы исключить их из проверок
fn compute_hatch_like_lines(entities: &[CadEntity], diag: f64) -> Vec<bool> {
    let mut flags = vec![false; entities.len()];
    let search_r = diag * HATCH_SEARCH_REL;

    struct LineInfo {
        idx: usize,
        mid: (f64, f64),
        angle: f64,
    }

    let lines: Vec<LineInfo> = entities
        .iter()
        .enumerate()
        .filter_map(|(idx, e)| {
            if let CadEntity::Line { p1, p2, .. } = e {
                let dx = p2.0 - p1.0;
                let dy = p2.1 - p1.1;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 1e-9 {
                    return None;
                }
                Some(LineInfo {
                    idx,
                    mid: ((p1.0 + p2.0) / 2.0, (p1.1 + p2.1) / 2.0),
                    angle: dy.atan2(dx),
                })
            } else {
                None
            }
        })
        .collect();

    for a in &lines {
        let mut neighbor_count = 0usize;
        for b in &lines {
            if a.idx == b.idx {
                continue;
            }
            let mut diff = (a.angle - b.angle).to_degrees().abs() % 180.0;
            if diff > 90.0 {
                diff = 180.0 - diff;
            }
            if diff > ANGLE_THRESHOLD_DEG {
                continue;
            }
            if dist(a.mid, b.mid) < search_r {
                neighbor_count += 1;
                if neighbor_count >= HATCH_NEIGHBOR_MIN {
                    break;
                }
            }
        }
        if neighbor_count >= HATCH_NEIGHBOR_MIN {
            flags[a.idx] = true;
        }
    }

    flags
}

// Возвращает точку на объекте, в которой стоит показать маркер проблемы
fn entity_marker_point(e: &CadEntity) -> (f64, f64) {
    let (x0, y0, x1, y1) = e.extent();
    ((x0 + x1) / 2.0, (y0 + y1) / 2.0)
}

struct LineOverlap {
    mid: (f64, f64),
    gap: f64,
    p1: (f64, f64),
    p2: (f64, f64),
}

// Проверяет, являются ли две линии почти дублирующимися (по расстоянию и углу)
fn near_duplicate_line_check(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
    dist_threshold: f64,
) -> Option<LineOverlap> {
    let dir_a = (a2.0 - a1.0, a2.1 - a1.1);
    let dir_b = (b2.0 - b1.0, b2.1 - b1.1);
    let len_a = (dir_a.0.powi(2) + dir_a.1.powi(2)).sqrt();
    let len_b = (dir_b.0.powi(2) + dir_b.1.powi(2)).sqrt();
    if len_a < 1e-9 || len_b < 1e-9 {
        return None;
    }

    let angle_a = dir_a.1.atan2(dir_a.0);
    let angle_b = dir_b.1.atan2(dir_b.0);
    let mut diff = (angle_a - angle_b).to_degrees().abs() % 180.0;
    if diff > 90.0 {
        diff = 180.0 - diff;
    }
    if diff > ANGLE_THRESHOLD_DEG {
        return None;
    }

    let ux = dir_a.0 / len_a;
    let uy = dir_a.1 / len_a;

    let perp = |p: (f64, f64)| -> f64 {
        let vx = p.0 - a1.0;
        let vy = p.1 - a1.1;
        (vx * uy - vy * ux).abs()
    };
    let d = (perp(b1) + perp(b2)) / 2.0;
    if d > dist_threshold {
        return None;
    }

    let proj = |p: (f64, f64)| -> f64 { (p.0 - a1.0) * ux + (p.1 - a1.1) * uy };
    let (a_lo, a_hi) = (0.0_f64, len_a);
    let (mut b_lo, mut b_hi) = (proj(b1), proj(b2));
    if b_lo > b_hi {
        std::mem::swap(&mut b_lo, &mut b_hi);
    }

    let overlap_lo = a_lo.max(b_lo);
    let overlap_hi = a_hi.min(b_hi);
    let overlap_len = overlap_hi - overlap_lo;
    if overlap_len <= 0.0 {
        return None;
    }

    let min_overlap_abs = dist_threshold * 3.0;
    if overlap_len < min_overlap_abs {
        return None;
    }

    let shorter = len_a.min(len_b);
    let longer = len_a.max(len_b);
    let overlap_frac = overlap_len / shorter;
    let length_ratio = shorter / longer;

    let required_frac = if length_ratio < EXTREME_LENGTH_RATIO {
        EXTREME_LENGTH_MIN_OVERLAP_FRACTION
    } else {
        MIN_OVERLAP_FRACTION
    };
    if overlap_frac < required_frac {
        return None;
    }

    if overlap_frac < STRONG_OVERLAP_FRACTION {
        let endpoint_gap = dist(a1, b1)
            .min(dist(a1, b2))
            .min(dist(a2, b1))
            .min(dist(a2, b2));
        if endpoint_gap > dist_threshold * ENDPOINT_ANCHOR_MULT {
            return None;
        }
    }

    let mid_t = (overlap_lo + overlap_hi) / 2.0;
    let mid = (a1.0 + ux * mid_t, a1.1 + uy * mid_t);
    let seg_p1 = (a1.0 + ux * overlap_lo, a1.1 + uy * overlap_lo);
    let seg_p2 = (a1.0 + ux * overlap_hi, a1.1 + uy * overlap_hi);
    Some(LineOverlap {
        mid,
        gap: d,
        p1: seg_p1,
        p2: seg_p2,
    })
}

struct ArcOverlap {
    mid_angle: f64,
}

const MIN_ARC_OVERLAP_DEG: f64 = 2.0;

// Вычисляет пересечение угловых диапазонов двух дуг
fn arc_angle_overlap(a_s: f64, a_e: f64, b_s: f64, b_e: f64) -> Option<ArcOverlap> {
    let norm_span = |s: f64, e: f64| -> (f64, f64) {
        let mut span = (e - s) % 360.0;
        if span <= 0.0 {
            span += 360.0;
        }
        (s.rem_euclid(360.0), span)
    };
    let (a_start, a_span) = norm_span(a_s, a_e);
    let (b_start, b_span) = norm_span(b_s, b_e);

    let mut best_overlap = 0.0_f64;
    let mut best_mid_deg = 0.0_f64;

    for shift in [-360.0, 0.0, 360.0] {
        let b_lo = b_start + shift;
        let b_hi = b_lo + b_span;
        let lo = a_start.max(b_lo);
        let hi = (a_start + a_span).min(b_hi);
        let ov = hi - lo;
        if ov > best_overlap {
            best_overlap = ov;
            best_mid_deg = (lo + hi) / 2.0;
        }
    }

    if best_overlap < MIN_ARC_OVERLAP_DEG {
        return None;
    }

    Some(ArcOverlap {
        mid_angle: best_mid_deg.to_radians(),
    })
}

// Применяет исправление к списку объектов на основе preview_fix
pub fn apply_fix(entities: &mut Vec<CadEntity>, issue: &Issue) {
    match issue.kind {
        IssueKind::DuplicateLayer => {
            let target_raw = entities[issue.secondary].layer().to_string();
            let normalized = target_raw.trim().to_lowercase();

            let canonical = entities
                .iter()
                .map(|e| e.layer().to_string())
                .find(|raw| raw.trim().to_lowercase() == normalized && *raw != target_raw);

            if let Some(canonical_name) = canonical {
                for e in entities.iter_mut() {
                    if e.layer().trim().to_lowercase() == normalized {
                        e.set_layer(canonical_name.clone());
                    }
                }
            }
        }
        IssueKind::LineOvershoot => {
            if let Some(CadEntity::Line { p1, p2, .. }) = entities.get_mut(issue.secondary) {
                match issue.endpoint_b {
                    Some(0) => *p1 = issue.center,
                    Some(1) => *p2 = issue.center,
                    _ => {}
                }
            }
        }
        IssueKind::LineGap => {
            if let (Some(a), Some(b)) = (entities.get(issue.primary), entities.get(issue.secondary))
            {
                if let (
                    CadEntity::Line {
                        p1: ap1,
                        p2: ap2,
                        layer,
                        linetype,
                        color,
                    },
                    CadEntity::Line {
                        p1: bp1, p2: bp2, ..
                    },
                ) = (a, b)
                {
                    let a_pt = if issue.endpoint_a == Some(1) {
                        *ap2
                    } else {
                        *ap1
                    };
                    let b_pt = if issue.endpoint_b == Some(1) {
                        *bp2
                    } else {
                        *bp1
                    };
                    let new_line = CadEntity::Line {
                        p1: a_pt,
                        p2: b_pt,
                        layer: layer.clone(),
                        linetype: linetype.clone(),
                        color: *color,
                    };
                    entities.push(new_line);
                }
            }
        }
        IssueKind::NearDuplicateLine => {
            if issue.secondary < entities.len() {
                let remaining = trim_secondary_line(entities, issue);
                entities.remove(issue.secondary);
                for r in remaining {
                    entities.push(r);
                }
            }
        }
        _ => {
            if issue.secondary < entities.len() {
                entities.remove(issue.secondary);
            }
        }
    }
}
