//! Marcas de registro (print & cut) — geometria das marcas e geração da
//! folha de impressão em SVG, equivalente às marcas tipo "Cameo/Portrait"
//! (`TB52,2`) usadas pelo Silhouette Studio.
//!
//! Esquema fiel ao Silhouette Studio (área de registro = retângulo de
//! `width_mm` × `length_mm` com canto superior esquerdo em
//! `(origin_x, origin_y)`):
//!
//! ```text
//!   ┌──┐                              L
//!   └──┘ quadrado TL            (canto TR, braços p/ esquerda e baixo)
//!   L
//!   (canto BL, braços p/ direita e cima)
//! ```
//!
//! * Quadrado **preenchido de preto** no canto superior esquerdo (o sensor
//!   inicia a leitura por ele) — lado `mark_length_mm` (padrão 10 mm).
//! * "L" nos cantos superior direito e inferior esquerdo, com braços do
//!   mesmo comprimento e traço de `mark_thickness_mm`.

use crate::cutjob::RegmarkSettings;
use crate::geom::{Bbox, Point, Polyline};

/// Margem de exclusão (mm) em volta de cada marca: nada do desenho pode
/// entrar nessa faixa ou o sensor confunde tinta com marca.
pub const KEEPOUT_MARGIN_MM: f64 = 3.0;

/// Gera os contornos das marcas de registro no padrão do Studio, em mm,
/// espaço da página: quadrado no canto superior esquerdo + "L" nos cantos
/// superior direito e inferior esquerdo.
pub fn mark_paths(r: &RegmarkSettings) -> Vec<Polyline> {
    let (ox, oy) = (r.origin_x_mm, r.origin_y_mm);
    let (w, l) = (r.width_mm, r.length_mm);
    let len = r.mark_length_mm.clamp(3.0, 30.0);
    let t = (r.mark_thickness_mm.clamp(0.2, 2.0)) / 2.0;
    let x2 = ox + w;
    let y2 = oy + l;

    let mut paths: Vec<Polyline> = Vec::new();

    // Quadrado TL SÓLIDO (polígono preenchido), lado = comprimento da marca
    paths.push(vec![
        Point::new(ox, oy),
        Point::new(ox + len, oy),
        Point::new(ox + len, oy + len),
        Point::new(ox, oy + len),
        Point::new(ox, oy),
    ]);

    // "L" no canto superior direito: braços para a esquerda e para baixo.
    paths.push(rect_line(x2 - len, oy - t, x2 + t, oy + t)); // horizontal
    paths.push(rect_line(x2 - t, oy - t, x2 + t, oy + len + t)); // vertical

    // "L" no canto inferior esquerdo: braços para a direita e para cima.
    paths.push(rect_line(ox - t, y2 - t, ox + len + t, y2 + t)); // horizontal
    paths.push(rect_line(ox - t, y2 - len - t, ox + t, y2 + t)); // vertical

    paths
}

/// Retângulos de exclusão em volta das 3 marcas (mm): o desenho não pode
/// invadir estas áreas (equivalente à zona hachurada do Studio).
pub fn keepout_boxes(r: &RegmarkSettings) -> [crate::geom::Bbox; 3] {
    let len = r.mark_length_mm.clamp(3.0, 30.0) + r.mark_thickness_mm + KEEPOUT_MARGIN_MM;
    let (ox, oy) = (r.origin_x_mm, r.origin_y_mm);
    let (x2, y2) = (ox + r.width_mm, oy + r.length_mm);
    [
        Bbox { min_x: ox - r.mark_thickness_mm, min_y: oy - r.mark_thickness_mm, max_x: ox + len, max_y: oy + len },
        Bbox { min_x: x2 - len, min_y: oy - r.mark_thickness_mm, max_x: x2 + r.mark_thickness_mm + 1.0, max_y: oy + len },
        Bbox { min_x: ox - r.mark_thickness_mm, min_y: y2 - len, max_x: ox + len, max_y: y2 + r.mark_thickness_mm + 1.0 },
    ]
}

/// Linha retangular (retângulo fechado) entre dois cantos — representação
/// vetorial da marca impressa com espessura constante.
fn rect_line(x1: f64, y1: f64, x2: f64, y2: f64) -> Polyline {
    vec![
        Point::new(x1, y1),
        Point::new(x2, y1),
        Point::new(x2, y2),
        Point::new(x1, y2),
        Point::new(x1, y1),
    ]
}

/// Gera a folha de impressão completa em SVG: marcas em preto, desenho de
/// referência (opcional) em cinza-claro, moldura da área de registro.
///
/// `design` são as polilinhas do desenho (mm, espaço do SVG original) que
/// serão impressas e depois recortadas usando as marcas.
pub fn print_sheet_svg(
    design: &[Polyline],
    r: &RegmarkSettings,
    page_w_mm: f64,
    page_h_mm: f64,
) -> String {
    let fmt = |v: f64| format!("{v:.2}");
    let mut s = String::with_capacity(64 * 1024);

    s.push_str(&format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="{pw}mm" height="{ph}mm" viewBox="0 0 {pw} {ph}">
<!-- Folha print & cut gerada pelo silcut.
     Imprima em escala 100% (sem ajuste de página), recorte o suporte
     e carregue na máquina com as marcas alinhadas ao sensor. -->
"#,
        pw = fmt(page_w_mm),
        ph = fmt(page_h_mm),
    ));

    // Moldura tracejada da área de registro (referência visual, não é cortada).
    let (ox, oy, w, l) = (r.origin_x_mm, r.origin_y_mm, r.width_mm, r.length_mm);
    s.push_str(&format!(
        "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"none\" stroke=\"#bbbbbb\" stroke-width=\"0.2\" stroke-dasharray=\"2,1.5\"/>\n",
        fmt(ox),
        fmt(oy),
        fmt(w),
        fmt(l),
    ));

    // Desenho (o que será recutado depois com as marcas).
    if !design.is_empty() {
        s.push_str("<g fill=\"none\" stroke=\"#000000\" stroke-width=\"0.15\">\n");
        for pl in design {
            if pl.len() < 2 {
                continue;
            }
            s.push_str("<path d=\"");
            for (i, p) in pl.iter().enumerate() {
                s.push_str(&if i == 0 {
                    format!("M{} {}", fmt(p.x), fmt(p.y))
                } else {
                    format!(" L{} {}", fmt(p.x), fmt(p.y))
                });
            }
            s.push_str("\"/>\n");
        }
        s.push_str("</g>\n");
    }

    // Marcas de registro em preto sólido.
    s.push_str("<g fill=\"#000000\" stroke=\"none\">\n");
    for pl in mark_paths(r) {
        s.push_str("<path d=\"");
        for (i, p) in pl.iter().enumerate() {
            s.push_str(&if i == 0 {
                format!("M{} {}", fmt(p.x), fmt(p.y))
            } else {
                format!(" L{} {}", fmt(p.x), fmt(p.y))
            });
        }
        s.push_str(" Z\"/>\n");
    }
    s.push_str("</g>\n</svg>\n");
    s
}

/// Carimbo identificando a folha (texto curto no rodapé, fora da área de corte).
pub fn footer_note() -> String {
    format!(
        "silcut print&cut — imprima em 100% — area de registro {w}x{l} mm, origem ({x},{y}) mm",
        w = 0,
        l = 0,
        x = 0,
        y = 0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_are_outside_or_on_boundary() {
        let r = RegmarkSettings {
            enabled: true,
            search: true,
            width_mm: 180.0,
            length_mm: 230.0,
            origin_x_mm: 15.0,
            origin_y_mm: 20.0,
            ..Default::default()
        };
        let marks = mark_paths(&r);
        assert_eq!(marks.len(), 5, "quadrado sólido TL + 2 braços em TR + 2 em BL");
        // As marcas só podem ocupar a faixa de borda (comprimento + espessura).
        let band = r.mark_length_mm + r.mark_thickness_mm + 0.01;
        for pl in &marks {
            for p in pl {
                let deep_x = p.x > r.origin_x_mm + band && p.x < r.origin_x_mm + r.width_mm - band;
                let deep_y = p.y > r.origin_y_mm + band && p.y < r.origin_y_mm + r.length_mm - band;
                assert!(
                    !(deep_x && deep_y),
                    "marca entrou no miolo da área: {p:?}"
                );
            }
        }
    }

    #[test]
    fn sheet_svg_valid() {
        let r = RegmarkSettings {
            enabled: true,
            search: true,
            width_mm: 100.0,
            length_mm: 100.0,
            origin_x_mm: 15.0,
            origin_y_mm: 20.0,
            ..Default::default()
        };
        let design = vec![vec![Point::new(20.0, 25.0), Point::new(50.0, 60.0)]];
        let svg = print_sheet_svg(&design, &r, 210.0, 297.0);
        assert!(svg.starts_with("<?xml"));
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.matches("<path").count() >= 6);
    }
}
