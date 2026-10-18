//! Offset de polilinhas (contorno paralelo) — equivalente ao "Deslocar" do
//! Silhouette Studio, usado para bordas brancas em adesivos e topos.
//!
//! Método direto: desloca cada segmento pela normal e junta os vértices por
//! interseção (miter), com bevel de segurança em ângulos muito agudos.
//! Não remove autointerseções de formas bastante côncavas — use valores
//! pequenos nesses casos.

use crate::geom::{Point, Polyline};

/// Desloca todas as polilinhas em `dist_mm` (positivo = expandir).
pub fn offset_paths(paths: &[Polyline], dist_mm: f64) -> Vec<Polyline> {
    if dist_mm.abs() < 1e-6 {
        return paths.to_vec();
    }
    paths
        .iter()
        .filter_map(|pl| offset_one(pl, dist_mm))
        .collect()
}

fn signed_area(pl: &[Point]) -> f64 {
    let mut a = 0.0;
    for i in 0..pl.len() {
        let p = pl[i];
        let q = pl[(i + 1) % pl.len()];
        a += p.x * q.y - q.x * p.y;
    }
    a / 2.0
}

fn offset_one(pl: &Polyline, dist: f64) -> Option<Polyline> {
    // remove duplicados consecutivos
    let mut pts: Vec<Point> = Vec::with_capacity(pl.len());
    for p in pl {
        if pts.last().map_or(true, |q: &Point| q.dist(*p) > 1e-9) {
            pts.push(*p);
        }
    }
    let n = pts.len();
    if n < 2 {
        return None;
    }
    let closed = n >= 3 && pts[0].dist(pts[n - 1]) < 1e-9;
    if closed {
        pts.pop();
    }
    let m = pts.len();
    if (closed && m < 3) || (!closed && m < 2) {
        return None;
    }

    // em coordenadas y-down, área > 0: a normal (dy, −dx) aponta para fora
    let d = if closed && signed_area(&pts) < 0.0 {
        -dist
    } else {
        dist
    };

    let seg_count = if closed { m } else { m - 1 };
    let mut segs: Vec<(Point, Point)> = Vec::with_capacity(seg_count);
    for i in 0..seg_count {
        let a = pts[i];
        let b = pts[(i + 1) % m];
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-12 {
            continue;
        }
        let (nx, ny) = (dy / len * d, -dx / len * d);
        segs.push((
            Point::new(a.x + nx, a.y + ny),
            Point::new(b.x + nx, b.y + ny),
        ));
    }
    if segs.is_empty() {
        return None;
    }
    let k = segs.len();

    let mut out: Polyline = Vec::new();
    if !closed {
        out.push(segs[0].0);
    }
    let joins = if closed { k } else { k - 1 };
    for i in 0..joins {
        let (a1, a2) = segs[i];
        let (b1, b2) = segs[(i + 1) % k];
        match line_intersect(a1, a2, b1, b2) {
            Some(p) if p.dist(a2) <= 3.0 * d.abs() + 1e-6 => out.push(p),
            _ => {
                // bevel: dois pontos em vez do miter
                out.push(a2);
                out.push(b1);
            }
        }
    }
    if !closed {
        out.push(segs[k - 1].1);
    } else {
        let first = out[0];
        out.push(first);
    }
    Some(out)
}

fn line_intersect(a1: Point, a2: Point, b1: Point, b2: Point) -> Option<Point> {
    let den = (a1.x - a2.x) * (b1.y - b2.y) - (a1.y - a2.y) * (b1.x - b2.x);
    if den.abs() < 1e-12 {
        return None;
    }
    let px = ((a1.x * a2.y - a1.y * a2.x) * (b1.x - b2.x)
        - (a1.x - a2.x) * (b1.x * b2.y - b1.y * b2.x))
        / den;
    let py = ((a1.x * a2.y - a1.y * a2.x) * (b1.y - b2.y)
        - (a1.y - a2.y) * (b1.x * b2.y - b1.y * b2.x))
        / den;
    Some(Point::new(px, py))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> Polyline {
        vec![
            Point::new(x, y),
            Point::new(x + s, y),
            Point::new(x + s, y + s),
            Point::new(x, y + s),
            Point::new(x, y),
        ]
    }

    #[test]
    fn square_grows_outward() {
        let out = offset_paths(&[square(0.0, 0.0, 10.0)], 2.0);
        assert_eq!(out.len(), 1);
        let bb = crate::geom::Bbox::from_points(&out[0]).unwrap();
        assert!((bb.min_x + 2.0).abs() < 1e-6, "min_x {}", bb.min_x);
        assert!((bb.max_x - 12.0).abs() < 1e-6, "max_x {}", bb.max_x);
        assert!((bb.min_y + 2.0).abs() < 1e-6);
        assert!((bb.max_y - 12.0).abs() < 1e-6);
    }

    #[test]
    fn negative_shrinks() {
        let out = offset_paths(&[square(0.0, 0.0, 10.0)], -1.5);
        let bb = crate::geom::Bbox::from_points(&out[0]).unwrap();
        assert!((bb.min_x - 1.5).abs() < 1e-6);
        assert!((bb.max_x - 8.5).abs() < 1e-6);
    }

    #[test]
    fn open_path_offset_keeps_ends() {
        let line = vec![Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        let out = offset_paths(&[line], 1.0);
        assert_eq!(out[0].len(), 2);
        assert!((out[0][0].y + 1.0).abs() < 1e-6);
    }
}
