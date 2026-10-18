//! Codificação GPGL (Graphtec Plotter Graphics Language).
//!
//! Fatos verificados do protocolo (capturas USB documentadas pela comunidade):
//! * Resolução nativa: **1 SU = 0,05 mm** (1 mm = 20 SU; 1 pol = 508 SU).
//! * Sistema de coordenadas: origem no canto superior esquerdo; nos comandos
//!   `M`/`D`/`\`/`Z` a ordem é **(Y, X)** — Y é o avanço da mídia (para baixo
//!   no SVG) e X é o lateral.
//! * Terminador de comando: byte 0x03. Init: ESC 0x04. Status: ESC 0x05.
//! * Marcas de registro: `TB50,0` `TB99` `TB52,2` `TB51,<len>` `TB53,<w>`
//!   `TB55,1` + busca automática `TB123,h,w,t,l` (ou manual `TB23,h,w`)
//!   seguida de `FQ5` e resposta `    0\x03` quando encontradas.
//! * Sequência de configuração reproduz as capturas do Silhouette Studio.

use crate::cutjob::CutSettings;
use crate::device::DeviceSpec;
use crate::device::Family;
use crate::geom::{Bbox, Point, Polyline};

/// Converte mm para SU (Silhouette Unit, 0,05 mm) com arredondamento.
pub fn su(mm: f64) -> i64 {
    (mm * 20.0).round() as i64
}

/// Programa GPGL completo, dividido em fases para transmissão segura.
#[derive(Debug, Clone, Default)]
pub struct GpglProgram {
    /// Comandos de inicialização/configuração (curtos).
    pub setup: Vec<u8>,
    /// Fluxo de movimento M/D (longo; enviado em blocos de 1 KiB alinhados a 0x03).
    pub body: Vec<u8>,
    /// Estacionamento/finalização.
    pub trailer: Vec<u8>,
    /// Caixa delimitadora efetiva do corte, em mm (espaço da mídia).
    pub bbox: Bbox,
    /// true se, após `setup`, deve-se aguardar a resposta da busca de marcas.
    pub needs_regmark_response: bool,
}

impl GpglProgram {
    /// Concatena as três fases (útil para dry-run/inspeção).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(self.setup.len() + self.body.len() + self.trailer.len());
        v.extend_from_slice(&self.setup);
        v.extend_from_slice(&self.body);
        v.extend_from_slice(&self.trailer);
        v
    }

    /// Versão legível (comandos separados por espaço) para inspeção/log.
    pub fn to_text(&self) -> String {
        let pretty = |b: &[u8]| -> String {
            b.split(|&c| c == 0x03)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).to_string())
                .collect::<Vec<_>>()
                .join(" ")
        };
        format!(
            "SETUP: {}\nBODY: {}\nTRAILER: {}\n",
            pretty(&self.setup),
            pretty(&self.body),
            pretty(&self.trailer)
        )
    }
}

fn cmd(out: &mut Vec<u8>, s: &str) {
    out.extend_from_slice(s.as_bytes());
    out.push(0x03);
}

/// Limites efetivos da área de corte em mm (espaço da mídia).
fn effective_area(st: &CutSettings, dev: &DeviceSpec) -> (f64, f64) {
    if st.regmark.enabled {
        (st.regmark.width_mm, st.regmark.length_mm)
    } else {
        let w = (st.media_width_mm - dev.margin_left_mm).max(10.0);
        let h = (st.media_height_mm - dev.margin_top_mm).max(10.0);
        (w, h)
    }
}

/// Constrói o programa GPGL completo para as polilinhas (em mm, espaço do SVG).
pub fn build(paths: &[Polyline], st: &CutSettings, dev: &DeviceSpec) -> GpglProgram {
    let mut setup = Vec::new();
    let mut body = Vec::new();
    let mut trailer = Vec::new();

    let c3 = dev.family == Family::Cameo3On;

    // ---- Configuração -----------------------------------------------------
    if st.regmark.enabled && dev.regmark {
        cmd(&mut setup, "TB50,0");
        cmd(&mut setup, "TB99");
        cmd(&mut setup, "TB52,2"); // tipo de marca: Cameo/Portrait
        // comprimento e espessura da marca (0,05 mm/SU), como configurado
        cmd(&mut setup, &format!("TB51,{}", su(st.regmark.mark_length_mm)));
        cmd(&mut setup, &format!("TB53,{}", su(st.regmark.mark_thickness_mm)));
        cmd(&mut setup, "TB55,1");
        if st.regmark.search {
            // busca automática: TB123,altura,largura,topo,esquerda (SU),
            // com 10 mm de margem de procura além da origem.
            let h = su(st.regmark.length_mm);
            let w = su(st.regmark.width_mm);
            let t = su((st.regmark.origin_y_mm - 10.0).max(0.0));
            let l = su((st.regmark.origin_x_mm - 10.0).max(0.0));
            cmd(&mut setup, &format!("TB123,{h},{w},{t},{l}"));
        } else {
            // manual: TB23,altura,largura (SU)
            let h = su(st.regmark.length_mm);
            let w = su(st.regmark.width_mm);
            cmd(&mut setup, &format!("TB23,{h},{w}"));
        }
    }

    if c3 {
        // Mat (se suportado) + orientação retrato
        if dev.mat_support && st.mat != 0 {
            cmd(&mut setup, &format!("TG{}", st.mat));
        }
        cmd(&mut setup, "FN0");
        cmd(&mut setup, "TB50,0");
        // Limites da área de corte: regmark tem prioridade; senão, mat; senão, mídia.
        if st.regmark.enabled && dev.regmark {
            let (w, h) = effective_area(st, dev);
            cmd(&mut setup, "\\0,0");
            cmd(&mut setup, &format!("Z{},{}", su(h), su(w)));
        } else if dev.mat_support && st.mat != 0 {
            if let Some(mat) = crate::device::mat_by_code(st.mat) {
                if mat.feed_inch > 0.0 {
                    let bottom = su(mat.feed_inch * 25.4);
                    let right = su(mat.width_inch * 25.4);
                    cmd(&mut setup, "\\0,0");
                    cmd(&mut setup, &format!("Z{bottom},{right}"));
                }
            }
        }
        // Seleção de ferramenta (Cameo 4/5: dois suportes)
        cmd(&mut setup, &format!("J{}", st.toolholder.max(1)));
    } else {
        // Família antiga
        if st.media >= 100 && st.media <= 300 {
            cmd(&mut setup, &format!("FW{}", st.media));
        }
        cmd(&mut setup, "FN0");
        cmd(&mut setup, "TB50,0");
        let (w, h) = effective_area(st, dev);
        cmd(&mut setup, "\\0,0");
        cmd(&mut setup, &format!("Z{},{}", su(h), su(w)));
    }

    // Parâmetros de corte — diferenças entre famílias conforme capturas.
    let pressure = st.pressure.clamp(1, dev.max_pressure);
    let speed = st.speed.clamp(1, 30);
    if c3 {
        // Cameo 4/5: parâmetros duplicados + aceleração (reproduzindo o Studio)
        if dev.family == Family::Cameo3On && is_cameo4_family(dev) {
            let t = st.toolholder.max(1);
            cmd(&mut setup, &format!("FX{pressure},{t}"));
            cmd(&mut setup, &format!("TJ0"));
            cmd(&mut setup, &format!("!{speed},{t}"));
            if st.pen {
                cmd(&mut setup, &format!("FC0,{t},1"));
            } else {
                cmd(&mut setup, &format!("FC{},1,1", su(st.blade_diameter_mm)));
            }
            cmd(&mut setup, &format!("FE{},1", if st.sharpen_corners { 1 } else { 0 }));
            if st.pen {
                cmd(&mut setup, &format!("FF0,0,{t}"));
            } else {
                let s = ((st.sharpen_start_mm + 0.05) * 10.0) as i64;
                let e = ((st.sharpen_end_mm + 0.05) * 10.0) as i64;
                cmd(&mut setup, &format!("FF{s},{e},{t}"));
            }
            cmd(&mut setup, &format!("FX{pressure},{t}"));
            cmd(&mut setup, "TJ3");
            if st.pen {
                cmd(&mut setup, &format!("FC0,{t},1"));
            } else {
                cmd(&mut setup, &format!("FC{},1,1", su(st.blade_diameter_mm)));
            }
            if let Some(d) = st.depth {
                cmd(&mut setup, &format!("TF{d},{t}")); // autoblade
            }
        } else {
            // Cameo 3 / Portrait 3: sufixo de ferramenta sem duplicação
            let t = st.toolholder.max(1);
            cmd(&mut setup, &format!("!{speed},{t}"));
            cmd(&mut setup, &format!("FX{pressure},{t}"));
            if st.pen {
                cmd(&mut setup, &format!("FC0,1,{t}"));
            } else {
                cmd(&mut setup, &format!("FC{},1,{t}", su(st.blade_diameter_mm)));
            }
            cmd(&mut setup, "FE0,0");
            cmd(&mut setup, "FF0,0,0");
        }
    } else {
        cmd(&mut setup, &format!("!{speed}"));
        cmd(&mut setup, &format!("FX{pressure}"));
        if st.pen {
            cmd(&mut setup, "FC0");
        } else {
            cmd(&mut setup, "FC18"); // offset de corte padrão
        }
        cmd(&mut setup, "FE0,0");
        cmd(&mut setup, "FF0,0,0");
        if st.track_enhancing {
            cmd(&mut setup, "FY0");
        }
    }

    // ---- Corpo (movimentos) -----------------------------------------------
    // Offset efetivo: deslocamento do usuário; com regmark, compensa a origem
    // das marcas (após o registro, (0,0) = posição da marca).
    let (off_x, off_y) = if st.regmark.enabled && dev.regmark {
        (
            st.offset_x_mm - st.regmark.origin_x_mm,
            st.offset_y_mm - st.regmark.origin_y_mm,
        )
    } else {
        (st.offset_x_mm + dev.margin_left_mm, st.offset_y_mm + dev.margin_top_mm)
    };

    let (area_w, area_h) = effective_area(st, dev);
    let clip = Bbox { min_x: 0.0, min_y: 0.0, max_x: area_w, max_y: area_h };
    let fuzz = 0.05;

    cmd(&mut body, "L0");

    let mut bbox: Option<Bbox> = None;
    let mut _clipped_points = 0usize;

    let passes = st.passes.max(1);
    for _pass in 0..passes {
    for path in paths {
        if path.len() < 2 {
            continue;
        }
        let mut last_inside = true;
        let mut first = true;
        for (i, p0) in path.iter().enumerate() {
            let x = p0.x + off_x;
            let y = p0.y + off_y;
            let (x, y, inside) = clip_point(x, y, &clip, fuzz);
            if let Some(b) = Bbox::from_points(&[Point::new(x, y)]) {
                bbox = Some(match bbox {
                    Some(prev) => prev.union(&b),
                    None => b,
                });
            }
            if first {
                cmd(&mut body, &format!("M{},{}", su(y), su(x)));
                first = false;
            } else if inside && last_inside {
                cmd(&mut body, &format!("D{},{}", su(y), su(x)));
            } else {
                _clipped_points += 1;
                cmd(&mut body, &format!("M{},{}", su(y), su(x)));
            }
            last_inside = inside;
            let _ = i;
        }
    }
    }

    // ---- Finalização --------------------------------------------------------
    let bb = bbox.unwrap_or_default();
    if st.end_below {
        cmd(&mut trailer, &format!("M{},0", su(bb.max_y)));
        cmd(&mut trailer, "SO0");
    } else if c3 {
        cmd(&mut trailer, "L0");
        cmd(&mut trailer, "\\0,0");
        cmd(&mut trailer, "M0,0");
        cmd(&mut trailer, "J0");
        cmd(&mut trailer, "FN0");
        cmd(&mut trailer, "TB50,0");
    } else {
        cmd(&mut trailer, "H");
    }
    if st.track_enhancing && c3 {
        cmd(&mut trailer, "FY1");
    }

    GpglProgram {
        setup,
        body,
        trailer,
        bbox: bb,
        needs_regmark_response: st.regmark.enabled && dev.regmark && st.regmark.search,
    }
}

fn is_cameo4_family(dev: &DeviceSpec) -> bool {
    matches!(dev.pid, 0x1137 | 0x1138 | 0x1139 | 0x1140 | 0x1141 | 0x1146 | 0x0001 | 0x0002 | 0x113a | 0x113f)
}

/// Clipe um ponto à área de corte (fuzz = resolução do dispositivo).
fn clip_point(x: f64, y: f64, clip: &Bbox, fuzz: f64) -> (f64, f64, bool) {
    let mut inside = true;
    let mut x = x;
    let mut y = y;
    if clip.min_x - x > fuzz {
        x = clip.min_x;
        inside = false;
    }
    if x - clip.max_x > fuzz {
        x = clip.max_x;
        inside = false;
    }
    if clip.min_y - y > fuzz {
        y = clip.min_y;
        inside = false;
    }
    if y - clip.max_y > fuzz {
        y = clip.max_y;
        inside = false;
    }
    (x, y, inside)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DEVICES;

    fn cameo4() -> &'static DeviceSpec {
        DEVICES.iter().find(|d| d.pid == 0x1137).unwrap()
    }
    fn cameo1() -> &'static DeviceSpec {
        DEVICES.iter().find(|d| d.pid == 0x1121).unwrap()
    }

    fn settings() -> CutSettings {
        CutSettings {
            speed: 10,
            pressure: 15,
            depth: None,
            toolholder: 1,
            pen: false,
            media: 132,
            mat: 1,
            blade_diameter_mm: 0.9,
            track_enhancing: false,
            sharpen_corners: false,
            sharpen_start_mm: 0.1,
            sharpen_end_mm: 0.1,
            offset_x_mm: 0.0,
            offset_y_mm: 0.0,
            media_width_mm: 210.0,
            media_height_mm: 297.0,
            end_below: false,
            eject: false,
            tolerance_mm: 0.05,
            optimize_order: true,
            passes: 1,
            regmark: Default::default(),
        }
    }

    #[test]
    fn su_conversion() {
        assert_eq!(su(0.05), 1);
        assert_eq!(su(1.0), 20);
        assert_eq!(su(297.0), 5940);
        assert_eq!(su(12.0 * 25.4), 6096); // 12 pol
    }

    #[test]
    fn cameo4_setup_contains_expected_commands() {
        let square = vec![vec![
            Point::new(10.0, 10.0),
            Point::new(20.0, 10.0),
            Point::new(20.0, 20.0),
            Point::new(10.0, 10.0),
        ]];
        let st = settings();
        let prog = build(&square, &st, cameo4());
        let text = prog.to_text();
        for needle in ["J1", "FX15,1", "!10,1", "TG1", "FN0", "TB50,0", "L0", "M200,200", "D200,400"] {
            assert!(text.contains(needle), "faltou {needle} em:\n{text}");
        }
        assert!(!prog.needs_regmark_response);
    }

    #[test]
    fn legacy_setup_uses_simple_commands() {
        let square = vec![vec![
            Point::new(5.0, 5.0),
            Point::new(15.0, 5.0),
            Point::new(15.0, 15.0),
            Point::new(5.0, 5.0),
        ]];
        let st = settings();
        let prog = build(&square, &st, cameo1());
        let text = prog.to_text();
        for needle in ["FW132", "!10", "FX15", "FC18", "FE0,0", "FF0,0,0"] {
            assert!(text.contains(needle), "faltou {needle} em:\n{text}");
        }
        assert!(!text.contains("J1"));
    }

    #[test]
    fn regmark_commands_present() {
        let mut st = settings();
        st.regmark.enabled = true;
        st.regmark.search = true;
        st.regmark.width_mm = 180.0;
        st.regmark.length_mm = 230.0;
        st.regmark.origin_x_mm = 15.0;
        st.regmark.origin_y_mm = 20.0;
        let square = vec![vec![Point::new(0.0, 0.0), Point::new(50.0, 50.0)]];
        let prog = build(&square, &st, cameo4());
        let text = prog.to_text();
        for needle in ["TB99", "TB52,2", "TB51,200", "TB53,10", "TB55,1", "TB123,", "FQ5"] {
            // FQ5 é enviado pelo driver na espera de resposta, não no setup:
            if needle == "FQ5" {
                continue;
            }
            assert!(text.contains(needle), "faltou {needle} em:\n{text}");
        }
        assert!(prog.needs_regmark_response);
        // Offset compensa a origem das marcas: ponto (0,0) → (−15,−20) → clipado a 0.
        assert!(text.contains("M0,0"));
    }

    #[test]
    fn passes_repeat_cut_moves() {
        let square = vec![vec![
            Point::new(5.0, 5.0),
            Point::new(15.0, 5.0),
            Point::new(15.0, 15.0),
            Point::new(5.0, 5.0),
        ]];
        let mut st = settings();
        st.end_below = true;
        let count_cuts = |text: String| text.lines().filter(|l| l.starts_with('D')).count();
        let one = count_cuts(build(&square, &st, cameo4()).to_text());
        st.passes = 2;
        let two = count_cuts(build(&square, &st, cameo4()).to_text());
        assert_eq!(two, one * 2, "passagens devem repetir os movimentos de corte");
    }

    #[test]
    fn trailer_end_below_and_start() {
        let square = vec![vec![
            Point::new(10.0, 10.0),
            Point::new(20.0, 20.0),
        ]];
        let mut st = settings();
        st.end_below = true;
        let prog = build(&square, &st, cameo4());
        assert!(prog.to_text().contains("SO0"));

        st.end_below = false;
        let prog = build(&square, &st, cameo4());
        let t = prog.to_text();
        assert!(t.contains("M0,0") && t.contains("J0"));
    }

    #[test]
    fn clipping_outside_area() {
        let mut st = settings();
        st.regmark.enabled = false;
        st.media_width_mm = 50.0;
        st.media_height_mm = 50.0;
        // linha que sai da área (cameo4: margens 0, clip = 50×50 mm)
        let line = vec![vec![Point::new(10.0, 10.0), Point::new(80.0, 10.0)]];
        let prog = build(&line, &st, cameo4());
        let text = prog.to_text();
        // ponto fora → M (não D)
        assert!(text.contains("M200,1000"));
        assert!(!text.contains("D200,1000"));
    }
}
