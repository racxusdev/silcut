//! Geometria 2D: pontos, matrizes de transformação afim e achatamento de curvas
//! (Bézier cúbica/quadrática e arcos elípticos) em polilinhas.

/// Ponto em coordenadas de qualquer espaço (usuário, mm ou dispositivos).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub fn new(x: f64, y: f64) -> Self {
        Point { x, y }
    }

    /// Distância euclidiana até outro ponto.
    pub fn dist(self, o: Point) -> f64 {
        let dx = self.x - o.x;
        let dy = self.y - o.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Interpolação linear entre `self` (t=0) e `o` (t=1).
    pub fn lerp(self, o: Point, t: f64) -> Point {
        Point::new(
            self.x + (o.x - self.x) * t,
            self.y + (o.y - self.y) * t,
        )
    }
}

/// Caminho contínuo (ferramenta abaixada) formado por pontos já achatados.
pub type Polyline = Vec<Point>;

/// Matriz de transformação afim no formato SVG:
///
/// ```text
/// | a c e |
/// | b d f |
/// | 0 0 1 |
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    /// Multiplicação de matrizes: `self ∘ other` (aplica `other` primeiro).
    pub fn mul(&self, other: &Matrix) -> Matrix {
        Matrix {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    /// Aplica a transformação a um ponto.
    pub fn apply(&self, p: Point) -> Point {
        Point::new(
            self.a * p.x + self.c * p.y + self.e,
            self.b * p.x + self.d * p.y + self.f,
        )
    }

    /// `translate(tx, ty)`.
    pub fn translate(tx: f64, ty: f64) -> Matrix {
        Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: ty }
    }

    /// `scale(sx, sy)`.
    pub fn scale(sx: f64, sy: f64) -> Matrix {
        Matrix { a: sx, b: 0.0, c: 0.0, d: sy, e: 0.0, f: 0.0 }
    }

    /// `rotate(deg, cx, cy)` — rotação em graus em torno de `(cx, cy)`.
    pub fn rotate_deg(deg: f64, cx: f64, cy: f64) -> Matrix {
        let r = deg.to_radians();
        let (s, c) = (r.sin(), r.cos());
        let rot = Matrix { a: c, b: s, c: -s, d: c, e: 0.0, f: 0.0 };
        Matrix::translate(cx, cy).mul(&rot).mul(&Matrix::translate(-cx, -cy))
    }

    /// `skewX(deg)`.
    pub fn skew_x(deg: f64) -> Matrix {
        let t = deg.to_radians().tan();
        Matrix { a: 1.0, b: 0.0, c: t, d: 1.0, e: 0.0, f: 0.0 }
    }

    /// `skewY(deg)`.
    pub fn skew_y(deg: f64) -> Matrix {
        let t = deg.to_radians().tan();
        Matrix { a: 1.0, b: t, c: 0.0, d: 1.0, e: 0.0, f: 0.0 }
    }
}

/// Limites axis-aligned em mm.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Bbox {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Bbox {
    pub fn from_points(pts: &[Point]) -> Option<Bbox> {
        let first = *pts.first()?;
        let mut bb = Bbox { min_x: first.x, min_y: first.y, max_x: first.x, max_y: first.y };
        for p in pts.iter().skip(1) {
            bb.min_x = bb.min_x.min(p.x);
            bb.min_y = bb.min_y.min(p.y);
            bb.max_x = bb.max_x.max(p.x);
            bb.max_y = bb.max_y.max(p.y);
        }
        Some(bb)
    }

    pub fn union(&self, other: &Bbox) -> Bbox {
        Bbox {
            min_x: self.min_x.min(other.min_x),
            min_y: self.min_y.min(other.min_y),
            max_x: self.max_x.max(other.max_x),
            max_y: self.max_y.max(other.max_y),
        }
    }

    /// Interseção não vazia com outra caixa (tolerância 1e-9).
    pub fn intersects(&self, other: &Bbox) -> bool {
        self.min_x < other.max_x
            && self.max_x > other.min_x
            && self.min_y < other.max_y
            && self.max_y > other.min_y
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Limite de recursão do achatamento adaptativo.
const MAX_DEPTH: u32 = 24;

/// Achata uma curva de Bézier cúbica em pontos, com erro máximo `tolerance`.
/// O ponto inicial `p0` NÃO é inserido; a saída começa em `p1`-derivados até `p3`.
pub fn flatten_cubic(p0: Point, p1: Point, p2: Point, p3: Point, tolerance: f64, out: &mut Polyline) {
    let mut stack: Vec<(Point, Point, Point, Point, u32)> =
        vec![(p0, p1, p2, p3, 0)];

    while let Some((a, b, c, d, depth)) = stack.pop() {
        // Flatness = distância máxima dos pontos de controle à corda a→d.
        let flat = flatness_2_ctl(a, d, b).max(flatness_2_ctl(a, d, c));
        if flat <= tolerance || depth >= MAX_DEPTH {
            out.push(d);
        } else {
            // Subdivisão de Casteljau em t = 0,5.
            let ab = a.lerp(b, 0.5);
            let bc = b.lerp(c, 0.5);
            let cd = c.lerp(d, 0.5);
            let abc = ab.lerp(bc, 0.5);
            let bcd = bc.lerp(cd, 0.5);
            let mid = abc.lerp(bcd, 0.5);
            stack.push((mid, bcd, cd, d, depth + 1));
            stack.push((a, ab, abc, mid, depth + 1));
        }
    }
}

/// Achata uma curva de Bézier quadrática.
pub fn flatten_quad(p0: Point, p1: Point, p2: Point, tolerance: f64, out: &mut Polyline) {
    flatten_cubic(
        p0,
        p0.lerp(p1, 2.0 / 3.0),
        p2.lerp(p1, 2.0 / 3.0),
        p2,
        tolerance,
        out,
    );
}

fn flatness_2_ctl(p0: Point, p1: Point, ctl: Point) -> f64 {
    // Distância do ponto de controle à reta p0→p1 (normal da projeção).
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < f64::EPSILON {
        return ctl.dist(p0);
    }
    let nx = -dy / len;
    let ny = dx / len;
    ((ctl.x - p0.x) * nx + (ctl.y - p0.y) * ny).abs()
}

/// Achata um arco elíptico definido no formato de endpoint do SVG
/// (parâmetros F.6.5 da especificação SVG 1.1).
///
/// `phi_deg` é a rotação do eixo X da elipse; `large_arc`/`sweep` são os flags.
/// O ponto inicial é implicitamente o ponto corrente; o ponto final é `end`.
/// Retorna também o novo ponto corrente (== `end`).
pub fn flatten_arc(
    start: Point,
    rx_in: f64,
    ry_in: f64,
    phi_deg: f64,
    large_arc: bool,
    sweep: bool,
    end: Point,
    tolerance: f64,
    out: &mut Polyline,
) {
    let mut rx = rx_in.abs();
    let mut ry = ry_in.abs();
    if rx < f64::EPSILON || ry < f64::EPSILON || start.dist(end) < f64::EPSILON {
        out.push(end);
        return;
    }

    let phi = phi_deg.to_radians();
    let (cos_phi, sin_phi) = (phi.cos(), phi.sin());

    // Passo 1: (x1', y1')
    let dx2 = (start.x - end.x) / 2.0;
    let dy2 = (start.y - end.y) / 2.0;
    let x1p = cos_phi * dx2 + sin_phi * dy2;
    let y1p = -sin_phi * dx2 + cos_phi * dy2;

    // Corrige raios muito pequenos (F.6.6).
    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }

    // Passo 2: centro (cx', cy')
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let num = (rx2 * ry2 - rx2 * y1p * y1p - ry2 * x1p * x1p).max(0.0);
    let den = rx2 * y1p * y1p + ry2 * x1p * x1p;
    let mut coef = (num / den).sqrt();
    if large_arc == sweep {
        coef = -coef;
    }
    let cxp = coef * rx * y1p / ry;
    let cyp = -coef * ry * x1p / rx;

    // Passo 3: centro no sistema global
    let cx = cos_phi * cxp - sin_phi * cyp + (start.x + end.x) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (start.y + end.y) / 2.0;

    // Passo 4: ângulos inicial e de varredura
    let angle = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let n = ((ux * ux + uy * uy) * (vx * vx + vy * vy)).sqrt();
        let mut a = (dot / n).clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 {
            a = -a;
        }
        a
    };
    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && dtheta > 0.0 {
        dtheta -= std::f64::consts::TAU;
    } else if sweep && dtheta < 0.0 {
        dtheta += std::f64::consts::TAU;
    }

    // Raio efetivo no eixo "médio" para escolher o passo angular.
    let r_mean = (rx + ry) / 2.0;
    let step = if r_mean <= tolerance {
        std::f64::consts::FRAC_PI_2
    } else {
        // dθ tal que a sagitta ≈ tolerance: sagitta = r(1 - cos(dθ/2)) ≤ tolerance
        let c = 1.0 - (tolerance / r_mean).min(1.0);
        2.0 * c.clamp(-1.0, 1.0).acos().max(1e-3)
    };

    let n_steps = ((dtheta.abs() / step).ceil() as usize).max(1);
    let dt = dtheta / n_steps as f64;
    for i in 1..=n_steps {
        let t = theta1 + dt * i as f64;
        let px = rx * t.cos();
        let py = ry * t.sin();
        out.push(Point::new(
            cos_phi * px - sin_phi * py + cx,
            sin_phi * px + cos_phi * py + cy,
        ));
    }
}

/// Achata um círculo/elipse completo (usado por `<circle>`/`<ellipse>`).
pub fn flatten_ellipse(cx: f64, cy: f64, rx: f64, ry: f64, tolerance: f64, out: &mut Polyline) {
    if rx <= 0.0 || ry <= 0.0 {
        return;
    }
    let r_mean = (rx + ry) / 2.0;
    let c = (1.0 - (tolerance / r_mean).min(1.0)).clamp(-1.0, 1.0);
    let step = 2.0 * c.acos().max(1e-3);
    let n = ((std::f64::consts::TAU / step).ceil() as usize).max(8);
    for i in 0..n {
        let t = std::f64::consts::TAU * i as f64 / n as f64;
        out.push(Point::new(cx + rx * t.cos(), cy + ry * t.sin()));
    }
    // Fecha o círculo repetindo o primeiro ponto.
    out.push(out[0]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_identity() {
        let p = Point::new(3.0, 4.0);
        assert_eq!(Matrix::IDENTITY.apply(p), p);
    }

    #[test]
    fn matrix_translate_scale_compose() {
        // translate(10,0) ∘ scale(2,2) aplicado a (1,1) => (12,2)
        let m = Matrix::translate(10.0, 0.0).mul(&Matrix::scale(2.0, 2.0));
        let r = m.apply(Point::new(1.0, 1.0));
        assert!((r.x - 12.0).abs() < 1e-12 && (r.y - 2.0).abs() < 1e-12);
    }

    #[test]
    fn flatten_line_cubic() {
        // Bézier colinear degenerada vira segmentos na linha.
        let mut out = Vec::new();
        flatten_cubic(
            Point::new(0.0, 0.0),
            Point::new(1.0, 0.0),
            Point::new(2.0, 0.0),
            Point::new(3.0, 0.0),
            0.05,
            &mut out,
        );
        assert!(out.len() >= 1);
        assert_eq!(*out.last().unwrap(), Point::new(3.0, 0.0));
        for p in &out {
            assert!(p.y.abs() < 1e-9);
        }
    }

    #[test]
    fn flatten_quarter_arc() {
        // Arco de (0,1) para (1,0) com raio 1 — quarto de círculo centrado em (0,0)... na
        // verdade centro calculado; verificamos distância máxima do arco à origem ≈ raio.
        let mut out = Vec::new();
        // sweep=false ⇒ centro cai em (0,0) (com sweep=true seria (1,1), spec F.6.5)
        flatten_arc(
            Point::new(0.0, 1.0),
            1.0,
            1.0,
            0.0,
            false,
            false,
            Point::new(1.0, 0.0),
            0.01,
            &mut out,
        );
        // passo = 2·acos(1 − tol/r) ≈ 0,283 rad → ⌈(π/2)/0,283⌉ = 6 segmentos
        assert!(out.len() >= 6);
        for p in &out {
            let r = (p.x * p.x + p.y * p.y).sqrt();
            assert!((r - 1.0).abs() < 0.05, "raio {r} fora do esperado");
        }
        assert_eq!(*out.last().unwrap(), Point::new(1.0, 0.0));
    }

    #[test]
    fn ellipse_closed() {
        let mut out = Vec::new();
        flatten_ellipse(0.0, 0.0, 10.0, 5.0, 0.05, &mut out);
        assert_eq!(out[0], *out.last().unwrap());
        assert!(out.len() >= 16);
    }
}
