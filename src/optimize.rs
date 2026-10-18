//! Otimização da ordem de corte: heurística do vizinho mais próximo para
//! reduzir percursos de deslocamento (ferramenta levantada).

use crate::geom::{Point, Polyline};

/// Reordena os caminhos escolhendo, a cada passo, o caminho cujo ponto
/// inicial ou final está mais perto da posição atual (começando na origem
/// do canto superior esquerdo). Inverte o caminho quando o final é mais perto.
///
/// Custo O(n²) — adequado para centenas/milhares de caminhos de corte.
pub fn reorder_nearest(paths: Vec<Polyline>) -> (Vec<Polyline>, f64) {
    let mut remaining: Vec<Polyline> = paths.into_iter().filter(|p| p.len() >= 2).collect();
    remaining.sort_by_key(|p| {
        let bb = crate::geom::Bbox::from_points(p);
        let b = bb.unwrap_or_default();
        ((b.min_y * 1000.0) as i64, (b.min_x * 1000.0) as i64)
    });
    let mut cur = Point::new(0.0, 0.0);
    let mut ordered: Vec<Polyline> = Vec::with_capacity(remaining.len());
    let mut travel = 0.0f64;

    while !remaining.is_empty() {
        let mut best_i = 0usize;
        let mut best_rev = false;
        let mut best_d = f64::INFINITY;
        for (i, p) in remaining.iter().enumerate() {
            let d_start = cur.dist(p[0]);
            let d_end = cur.dist(*p.last().unwrap());
            if d_start < best_d {
                best_d = d_start;
                best_i = i;
                best_rev = false;
            }
            if d_end < best_d {
                best_d = d_end;
                best_i = i;
                best_rev = true;
            }
        }
        let mut path = remaining.swap_remove(best_i);
        if best_rev {
            path.reverse();
        }
        travel += best_d;
        cur = *path.last().unwrap();
        ordered.push(path);
    }
    (ordered, travel)
}

/// Distância total perccorrida com a ferramenta levantada (entre caminhos).
pub fn total_travel(paths: &[Polyline]) -> f64 {
    let mut cur = Point::new(0.0, 0.0);
    let mut t = 0.0;
    for p in paths {
        if p.is_empty() {
            continue;
        }
        t += cur.dist(p[0]);
        cur = *p.last().unwrap();
    }
    t
}

/// Comprimento total de corte (soma dos segmentos).
pub fn total_cut_length(paths: &[Polyline]) -> f64 {
    paths
        .iter()
        .flat_map(|p| p.windows(2))
        .map(|w| w[0].dist(w[1]))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_neighbor_picks_close_first() {
        let paths = vec![
            vec![Point::new(100.0, 100.0), Point::new(110.0, 100.0)],
            vec![Point::new(1.0, 1.0), Point::new(2.0, 1.0)],
            vec![Point::new(50.0, 50.0), Point::new(60.0, 50.0)],
        ];
        let (ordered, _t) = reorder_nearest(paths);
        assert!((ordered[0][0].x - 1.0).abs() < 1e-9);
        assert!((ordered[1][0].x - 50.0).abs() < 1e-9);
        assert!((ordered[2][0].x - 100.0).abs() < 1e-9);
    }

    #[test]
    fn travel_and_length() {
        let paths = vec![vec![Point::new(0.0, 0.0), Point::new(3.0, 0.0)]];
        assert!((total_cut_length(&paths) - 3.0).abs() < 1e-9);
        assert!((total_travel(&paths) - 0.0).abs() < 1e-9);
    }
}
