//! Operações booleanas por rasterização — "fundir" (união) contornos que se
//! sobrepõem, equivalente ao Weld do Silhouette Studio.
//!
//! Método: rasteriza os contornos com regra par-ímpar em uma grade de
//! `step_mm`, extrai as isolinhas com marching squares e simplifica
//! (remove colineares + Ramer–Douglas–Peucker). A precisão fica em torno de
//! meio passo da grade (padrão 0,1 mm), adequada à resolução da máquina
//! (0,05 mm) para contornos de corte.

use std::collections::{HashMap, HashSet};

use crate::geom::{Point, Polyline};

/// União de todos os contornos (paths abertos são fechados implicitamente).
pub fn union(paths: &[Polyline], step_mm: f64) -> Vec<Polyline> {
    let step = step_mm.clamp(0.05, 1.0);
    let mut min = (f64::INFINITY, f64::INFINITY);
    let mut max = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    for pl in paths {
        for p in pl {
            min.0 = min.0.min(p.x);
            min.1 = min.1.min(p.y);
            max.0 = max.0.max(p.x);
            max.1 = max.1.max(p.y);
        }
    }
    if !min.0.is_finite() || !max.0.is_finite() {
        return Vec::new();
    }
    let pad = 2.0 * step;
    min.0 -= pad;
    min.1 -= pad;
    max.0 += pad;
    max.1 += pad;
    let w = (((max.0 - min.0) / step).ceil() as usize) + 2;
    let h = (((max.1 - min.1) / step).ceil() as usize) + 2;
    if w * h > 40_000_000 {
        return paths.to_vec(); // grade grande demais: devolve original
    }

    // ---- rasterização (par-ímpar, união de todos) -------------------------
    let mut inside = vec![false; w * h];
    let mut xs: Vec<f64> = Vec::new();
    // preenchimento por caminho com OR: união (sobreposição não vira buraco)
    for pl in paths {
        for y in 0..h {
            let cy = min.1 + (y as f64 + 0.5) * step;
            xs.clear();
            for i in 0..pl.len() {
                let a = pl[i];
                let b = pl[(i + 1) % pl.len()];
                if (a.y <= cy) != (b.y <= cy) {
                    let t = (cy - a.y) / (b.y - a.y);
                    xs.push(a.x + t * (b.x - a.x));
                }
            }
            if xs.len() < 2 {
                continue;
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for pair in xs.chunks_exact(2) {
                let (xa, xb) = (pair[0], pair[1]);
                let x0 = (((xa - min.0) / step - 0.5).ceil()).max(0.0) as usize;
                let x1 = (((xb - min.0) / step - 0.5).floor()).min(w as f64 - 1.0) as isize;
                let mut x = x0;
                while (x as isize) <= x1 {
                    inside[y * w + x] = true;
                    x += 1;
                }
            }
        }
    }

    // ---- marching squares ------------------------------------------------
    let g = |x: usize, y: usize| inside[y * w + x];
    // nó = (2x+1, 2y) horizontal · (2x, 2y+1) vertical
    let mut adj: HashMap<(usize, usize), Vec<(usize, usize)>> = HashMap::new();
    let mut add = |a: (usize, usize), b: (usize, usize)| {
        adj.entry(a).or_default().push(b);
        adj.entry(b).or_default().push(a);
    };
    for y in 0..h - 1 {
        for x in 0..w - 1 {
            let case = g(x, y) as u8
                | (g(x + 1, y) as u8) << 1
                | (g(x + 1, y + 1) as u8) << 2
                | (g(x, y + 1) as u8) << 3;
            let top = (2 * x + 1, 2 * y);
            let right = (2 * x + 2, 2 * y + 1);
            let bottom = (2 * x + 1, 2 * y + 2);
            let left = (2 * x, 2 * y + 1);
            match case {
                1 | 14 => add(top, left),
                2 | 13 => add(top, right),
                3 | 12 => add(left, right),
                4 | 11 => add(right, bottom),
                5 | 10 => {
                    add(top, left);
                    add(right, bottom);
                }
                6 | 9 => add(top, bottom),
                7 | 8 => add(left, bottom),
                _ => {}
            }
        }
    }

    // ---- costura em polilinhas ------------------------------------------
    let mut used: HashSet<((usize, usize), (usize, usize))> = HashSet::new();
    let mut out: Vec<Polyline> = Vec::new();
    let node_pos = |n: (usize, usize)| {
        Point::new(
            min.0 + (n.0 as f64) * 0.5 * step,
            min.1 + (n.1 as f64) * 0.5 * step,
        )
    };
    let key = |a: (usize, usize), b: (usize, usize)| if a <= b { (a, b) } else { (b, a) };

    for start in adj.keys().copied().collect::<Vec<_>>() {
        loop {
            let avail = adj
                .get(&start)
                .map(|v| v.iter().copied().find(|n| !used.contains(&key(start, *n))))
                .flatten();
            let Some(next) = avail else { break };
            // segue a corrente
            let mut chain: Vec<(usize, usize)> = vec![start];
            let mut cur = next;
            used.insert(key(start, next));
            chain.push(cur);
            loop {
                let mut stepped = false;
                if let Some(neigh) = adj.get(&cur) {
                    for n in neigh {
                        if !used.contains(&key(cur, *n)) {
                            used.insert(key(cur, *n));
                            cur = *n;
                            chain.push(cur);
                            stepped = true;
                            break;
                        }
                    }
                }
                if !stepped {
                    break;
                }
                if cur == start {
                    break;
                }
            }
            if chain.len() >= 4 {
                let pts: Polyline = chain.iter().map(|n| node_pos(*n)).collect();
                let simplified = rdp(&remove_collinear(&pts), step * 0.5);
                if simplified.len() >= 3 {
                    out.push(simplified);
                }
            }
        }
    }
    out
}

fn remove_collinear(pl: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(pl.len());
    for i in 0..pl.len() {
        let a = out.last().copied().unwrap_or(pl[i]);
        let b = pl[i];
        let c = pl[(i + 1) % pl.len()];
        if (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x) != 0.0 || out.is_empty() {
            out.push(b);
        } else if let Some(last) = out.last_mut() {
            *last = b;
        }
    }
    out
}

/// Ramer–Douglas–Peucker (recursivo, entradas pequenas).
fn rdp(pl: &[Point], eps: f64) -> Polyline {
    if pl.len() < 3 {
        return pl.to_vec();
    }
    let (first, last) = (pl[0], pl[pl.len() - 1]);
    let (mut idx, mut dmax) = (0usize, 0.0f64);
    for (i, p) in pl.iter().enumerate().take(pl.len() - 1).skip(1) {
        let d = dist_to_seg(*p, first, last);
        if d > dmax {
            dmax = d;
            idx = i;
        }
    }
    if dmax > eps {
        let mut a = rdp(&pl[..=idx], eps);
        let b = rdp(&pl[idx..], eps);
        a.pop();
        a.extend(b);
        a
    } else {
        vec![first, last]
    }
}

fn dist_to_seg(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-12 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    let q = Point::new(a.x + t * dx, a.y + t * dy);
    p.dist(q)
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

    fn contour_area(pl: &Polyline) -> f64 {
        let mut a = 0.0;
        for i in 0..pl.len() {
            let p = pl[i];
            let q = pl[(i + 1) % pl.len()];
            a += p.x * q.y - q.x * p.y;
        }
        (a / 2.0).abs()
    }

    #[test]
    fn overlapping_squares_weld_into_one() {
        let paths = vec![square(0.0, 0.0, 10.0), square(5.0, 5.0, 10.0)];
        let out = union(&paths, 0.1);
        assert_eq!(out.len(), 1, "esperava 1 contorno unido");
        let bb = crate::geom::Bbox::from_points(&out[0]).unwrap();
        assert!((bb.min_x).abs() < 0.3 && (bb.max_x - 15.0).abs() < 0.3);
        // área ≈ 100 + 100 − 25 = 175
        let area = contour_area(&out[0]);
        assert!((area - 175.0).abs() < 8.0, "área {area}");
    }

    #[test]
    fn disjoint_squares_keep_two_contours() {
        let paths = vec![square(0.0, 0.0, 5.0), square(20.0, 20.0, 5.0)];
        let out = union(&paths, 0.1);
        assert_eq!(out.len(), 2);
    }
}
