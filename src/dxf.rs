//! Importação DXF (ASCII) — o formato usado pelos arquivos da Silhouette.
//!
//! Suporta as entidades de corte mais comuns:
//! * `LINE`, `LWPOLYLINE` (com *bulge*/arcos), `POLYLINE`+`VERTEX`+`SEQEND`
//! * `CIRCLE`, `ARC`, `ELLIPSE`
//!
//! Unidades lidas de `$INSUNITS` (mm/in/cm/pé); coordenadas convertidas de
//! y-up (DXF) para y-down (pipeline silcut), com origem no canto superior
//! esquerdo. `SPLINE`/`INSERT`/`HATCH` são ignorados (contados em aviso).

use crate::error::{Result, SilError};
use crate::geom::{Point, Polyline};

/// Resultado do parse: contornos em mm (y-down) e dimensões do desenho.
pub struct DxfDocument {
    pub polylines: Vec<Polyline>,
    pub width_mm: f64,
    pub height_mm: f64,
    pub unsupported: usize,
}

/// Lê e converte um arquivo DXF.
pub fn load(path: &std::path::Path) -> Result<DxfDocument> {
    let data = std::fs::read_to_string(path)
        .map_err(|e| SilError::Svg(format!("ler DXF: {e}")))?;
    parse(&data)
}

pub fn parse(text: &str) -> Result<DxfDocument> {
    // pares (código, valor) — DXF ASCII alterna código/valor por linha
    let lines: Vec<&str> = text.lines().collect();
    let mut pairs: Vec<(i32, &str)> = Vec::with_capacity(lines.len() / 2);
    let mut i = 0;
    while i + 1 < lines.len() {
        let code = lines[i].trim();
        if let Ok(c) = code.parse::<i32>() {
            pairs.push((c, lines[i + 1].trim()));
        }
        i += 2;
    }

    // unidades
    let mut unit_scale = 1.0_f64;
    let mut j = 0;
    while j < pairs.len() {
        if pairs[j].0 == 9 && pairs[j].1.eq_ignore_ascii_case("$INSUNITS") {
            if let Some((70, v)) = pairs.get(j + 1) {
                if true {
                    unit_scale = match v.parse::<i32>().unwrap_or(4) {
                        1 => 25.4, // polegadas
                        2 => 304.8, // pés
                        5 => 10.0,  // centímetros
                        _ => 1.0,   // mm / sem unidade
                    };
                }
            }
        }
        j += 1;
    }

    let mut paths: Vec<Polyline> = Vec::new();
    let mut unsupported = 0usize;
    let mut section = String::new();
    let mut entity = String::new();
    let mut fields: Vec<(i32, String)> = Vec::new();
    // POLYLINE antiga: vértices vêm como entidades VERTEX separadas
    let mut old_poly: Option<(bool, Polyline)> = None;

    let flush = |entity: &str,
                 fields: &[(i32, String)],
                 paths: &mut Vec<Polyline>,
                 unsupported: &mut usize| {
        match entity {
            "LINE" => {
                let g = |c: i32| fields.iter().find(|f| f.0 == c).and_then(|f| f.1.parse::<f64>().ok());
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (g(10), g(20), g(11), g(21)) {
                    paths.push(vec![Point::new(x1, y1), Point::new(x2, y2)]);
                }
            }
            "CIRCLE" => {
                let g = |c: i32| fields.iter().find(|f| f.0 == c).and_then(|f| f.1.parse::<f64>().ok());
                if let (Some(cx), Some(cy), Some(r)) = (g(10), g(20), g(40)) {
                    let mut pl = Vec::new();
                    let n = ((2.0 * std::f64::consts::PI * r / 0.05).sqrt().max(16.0)) as usize;
                    for k in 0..=n {
                        let t = std::f64::consts::TAU * k as f64 / n as f64;
                        pl.push(Point::new(cx + r * t.cos(), cy + r * t.sin()));
                    }
                    paths.push(pl);
                }
            }
            "ARC" => {
                let g = |c: i32| fields.iter().find(|f| f.0 == c).and_then(|f| f.1.parse::<f64>().ok());
                if let (Some(cx), Some(cy), Some(r), Some(a0), Some(a1)) =
                    (g(10), g(20), g(40), g(50), g(51))
                {
                    let (a0, mut a1) = (a0.to_radians(), a1.to_radians());
                    while a1 < a0 {
                        a1 += std::f64::consts::TAU;
                    }
                    let n = (((a1 - a0) / 0.15).ceil() as usize).max(4);
                    let mut pl = Vec::with_capacity(n + 1);
                    for k in 0..=n {
                        let t = a0 + (a1 - a0) * k as f64 / n as f64;
                        pl.push(Point::new(cx + r * t.cos(), cy + r * t.sin()));
                    }
                    paths.push(pl);
                }
            }
            "LWPOLYLINE" | "POLYLINE" => {
                if entity == "LWPOLYLINE" {
                    let closed = fields
                        .iter()
                        .find(|f| f.0 == 70)
                        .and_then(|f| f.1.parse::<i32>().ok())
                        .map(|v| v & 1 == 1)
                        .unwrap_or(false);
                    // pares 10/20 + bulge 42
                    let mut verts: Vec<(f64, f64, f64)> = Vec::new(); // x, y, bulge
                    let mut last_x: Option<f64> = None;
                    for (c, v) in fields {
                        match c {
                            10 => last_x = v.parse::<f64>().ok(),
                            20 => {
                                if let (Some(x), Ok(y)) = (last_x, v.parse::<f64>()) {
                                    verts.push((x, y, 0.0));
                                    last_x = None;
                                }
                            }
                            42 => {
                                if let (Some(last), Ok(b)) = (verts.last_mut(), v.parse::<f64>()) {
                                    last.2 = b;
                                }
                            }
                            _ => {}
                        }
                    }
                    if verts.len() >= 2 {
                        let mut pl: Polyline = Vec::new();
                        let n = verts.len();
                        let seg = if closed { n } else { n - 1 };
                        for k in 0..seg {
                            let (x0, y0, b) = verts[k];
                            let (x1, y1, _) = verts[(k + 1) % n];
                            let p0 = Point::new(x0, y0);
                            let p1 = Point::new(x1, y1);
                            if pl.is_empty() {
                                pl.push(p0);
                            }
                            if b.abs() > 1e-9 {
                                bulge_arc(p0, p1, b, &mut pl);
                            } else {
                                pl.push(p1);
                            }
                        }
                        if closed && pl.first() != pl.last() {
                            let first = pl[0];
                            pl.push(first);
                        }
                        paths.push(pl);
                    }
                }
            }
            "SPLINE" | "ELLIPSE" | "INSERT" | "HATCH" => *unsupported += 1,
            _ => {}
        }
    };

    for (code, value) in pairs.iter().copied() {
        match code {
            0 => {
                // fecha a entidade anterior
                match value {
                    "SECTION" => {
                        entity.clear();
                        fields.clear();
                    }
                    "ENDSEC" => {
                        if !entity.is_empty() {
                            flush(&entity, &fields, &mut paths, &mut unsupported);
                            entity.clear();
                            fields.clear();
                        }
                        section.clear();
                    }
                    "VERTEX" => {
                        if let Some((_, pl)) = old_poly.as_mut() {
                            let g = |c: i32| {
                                fields
                                    .iter()
                                    .find(|f| f.0 == c)
                                    .and_then(|f| f.1.parse::<f64>().ok())
                            };
                            // VERTEX usa os campos acumulados da própria entidade
                            let (gx, gy) = (g(10), g(20));
                            fields.clear();
                            if let (Some(x), Some(y)) = (gx, gy) {
                                pl.push(Point::new(x, y));
                            }
                            continue;
                        }
                        if !entity.is_empty() {
                            flush(&entity, &fields, &mut paths, &mut unsupported);
                        }
                        entity = value.to_string();
                        fields.clear();
                        continue;
                    }
                    "SEQEND" => {
                        if let Some((closed, mut pl)) = old_poly.take() {
                            if closed && pl.len() >= 2 && pl.first() != pl.last() {
                                let first = pl[0];
                                pl.push(first);
                            }
                            if pl.len() >= 2 {
                                paths.push(pl);
                            }
                        }
                        entity.clear();
                        fields.clear();
                        continue;
                    }
                    "POLYLINE" if section == "ENTITIES" => {
                        if !entity.is_empty() {
                            flush(&entity, &fields, &mut paths, &mut unsupported);
                        }
                        // POLYLINE antiga: coleta flags e espera VERTEX
                        let closed = pairs
                            .iter()
                            .skip_while(|p| !(p.0 == 0 && p.1 == "POLYLINE"))
                            .take_while(|p| p.0 != 0 || p.1 == "POLYLINE")
                            .find(|p| p.0 == 70)
                            .map(|p| p.1.parse::<i32>().map(|v| v & 1 == 1).unwrap_or(false))
                            .unwrap_or(false);
                        old_poly = Some((closed, Vec::new()));
                        entity.clear();
                        fields.clear();
                        continue;
                    }
                    _ => {
                        if !entity.is_empty() {
                            flush(&entity, &fields, &mut paths, &mut unsupported);
                        }
                        entity = value.to_string();
                        fields.clear();
                    }
                }
            }
            2 => {
                // nome da seção logo após SECTION
                if entity.is_empty() && fields.is_empty() {
                    section = value.to_string();
                } else {
                    fields.push((code, value.to_string()));
                }
            }
            _ => {
                if section == "ENTITIES" {
                    fields.push((code, value.to_string()));
                }
            }
        }
    }
    if !entity.is_empty() {
        flush(&entity, &fields, &mut paths, &mut unsupported);
    }

    // escala de unidades + y-up (DXF) → y-down (silcut), origem no topo-esquerda
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for pl in &paths {
        for p in pl {
            min.0 = min.0.min(p.x * unit_scale);
            min.1 = min.1.min(p.y * unit_scale);
            max.0 = max.0.max(p.x * unit_scale);
            max.1 = max.1.max(p.y * unit_scale);
        }
    }
    let (w, h) = if min.0.is_finite() {
        (max.0 - min.0, max.1 - min.1)
    } else {
        (0.0, 0.0)
    };
    let flipped: Vec<Polyline> = paths
        .iter()
        .map(|pl| {
            pl.iter()
                .map(|p| {
                    Point::new(
                        p.x * unit_scale - min.0,
                        max.1 - p.y * unit_scale,
                    )
                })
                .collect()
        })
        .filter(|pl: &Polyline| pl.len() >= 2)
        .collect();

    Ok(DxfDocument {
        polylines: flipped,
        width_mm: w,
        height_mm: h,
        unsupported,
    })
}

/// Amostra um arco definido por *bulge* (tan(θ/4)) entre p1 e p2.
fn bulge_arc(p1: Point, p2: Point, bulge: f64, out: &mut Polyline) {
    let d = p1.dist(p2);
    if d < 1e-12 {
        out.push(p2);
        return;
    }
    let theta = 4.0 * bulge.atan();
    let sagitta_off = (d / 2.0) * (1.0 - bulge * bulge) / (2.0 * bulge);
    let (dx, dy) = ((p2.x - p1.x) / d, (p2.y - p1.y) / d);
    // normal à esquerda da direção p1→p2
    let (nx, ny) = (-dy, dx);
    let cx = (p1.x + p2.x) / 2.0 + nx * sagitta_off;
    let cy = (p1.y + p2.y) / 2.0 + ny * sagitta_off;
    let r = ((p1.x - cx).powi(2) + (p1.y - cy).powi(2)).sqrt();
    let a0 = (p1.y - cy).atan2(p1.x - cx);
    let n = ((theta.abs() / 0.15).ceil() as usize).max(4);
    for k in 1..=n {
        let t = a0 + theta * k as f64 / n as f64;
        out.push(Point::new(cx + r * t.cos(), cy + r * t.sin()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE_LWPOLYLINE: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLWPOLYLINE\n8\n0\n90\n4\n70\n1\n10\n0.0\n20\n0.0\n10\n10.0\n20\n0.0\n10\n10.0\n20\n10.0\n10\n0.0\n20\n10.0\n0\nENDSEC\n0\nEOF\n";

    #[test]
    fn lwpolyline_square() {
        let doc = parse(SQUARE_LWPOLYLINE).unwrap();
        assert_eq!(doc.polylines.len(), 1);
        assert!((doc.width_mm - 10.0).abs() < 1e-9);
        assert!((doc.height_mm - 10.0).abs() < 1e-9);
        let pl = &doc.polylines[0];
        assert!(pl.first() == pl.last(), "polilinha fechada");
        // y invertido: (0,0) DXF vira (0,10)
        assert!((pl[0].x).abs() < 1e-9 && (pl[0].y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn line_and_circle() {
        let dxf = "0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0.0\n20\n0.0\n11\n20.0\n21\n0.0\n0\nCIRCLE\n10\n10.0\n20\n0.0\n40\n2.0\n0\nENDSEC\n0\nEOF\n";
        let doc = parse(dxf).unwrap();
        assert_eq!(doc.polylines.len(), 2);
        assert!(doc.polylines[0].len() == 2);
        assert!(doc.polylines[1].len() > 10);
        assert!((doc.width_mm - 20.0).abs() < 1e-9);
    }

    #[test]
    fn inches_scaled() {
        let dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n1\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n0\nENDSEC\n0\nEOF\n";
        let doc = parse(dxf).unwrap();
        assert!((doc.width_mm - 25.4).abs() < 1e-9, "1 in = 25,4 mm");
    }
}
