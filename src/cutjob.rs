//! Orquestração do corte: polilinhas (mm) + configurações → programa GPGL →
//! transmissão ao cortador (ou dry-run em arquivo).

use serde::{Deserialize, Serialize};

use crate::device::{self, DeviceSpec};
use crate::driver::{self, Cutter};
use crate::error::{Result, SilError};
use crate::geom::{Bbox, Polyline};
use crate::gpgl;
use crate::optimize;
use crate::svg;

/// Configuração das marcas de registro (print & cut).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RegmarkSettings {
    /// Ativar busca de marcas antes do corte.
    pub enabled: bool,
    /// Busca automática pelo sensor óptico (`TB123`); false = manual (`TB23`).
    pub search: bool,
    /// Distância horizontal entre as marcas (mm).
    pub width_mm: f64,
    /// Distância vertical entre as marcas (mm).
    pub length_mm: f64,
    /// Posição X da marca superior esquerda (mm, a partir da margem esquerda).
    pub origin_x_mm: f64,
    /// Posição Y da marca superior esquerda (mm, a partir da borda superior).
    pub origin_y_mm: f64,
    /// Comprimento da marca em mm (padrão 10, faixa 6–30).
    #[serde(default = "mark_len_default")]
    pub mark_length_mm: f64,
    /// Espessura do traço da marca em mm (Studio: padrão 0,5; 1,0 se falhar leitura).
    #[serde(default = "mark_thick_default")]
    pub mark_thickness_mm: f64,
}

fn passes_default() -> u8 {
    1
}

fn mark_len_default() -> f64 {
    10.0
}
fn mark_thick_default() -> f64 {
    0.5
}

impl Default for RegmarkSettings {
    fn default() -> Self {
        RegmarkSettings {
            enabled: false,
            search: true,
            width_mm: 180.0,
            length_mm: 230.0,
            origin_x_mm: 15.0,
            origin_y_mm: 20.0,
            mark_length_mm: mark_len_default(),
            mark_thickness_mm: mark_thick_default(),
        }
    }
}

/// Configurações completas de um corte.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CutSettings {
    /// Velocidade 1–30 (Cameo 4/5) ou 1–10 (antigos).
    pub speed: u8,
    /// Pressão/força 1–33 (1–40 no Cameo 5 Alpha).
    pub pressure: u8,
    /// Profundidade da lâmina (autoblade, Cameo 4+); None = não ajustar.
    pub depth: Option<u8>,
    /// Suporte da ferramenta (1 ou 2).
    pub toolholder: u8,
    /// Modo caneta (não corta, desenha).
    pub pen: bool,
    /// Código de mídia (tabela `device::MEDIA`).
    pub media: u16,
    /// Código do tapete (`device::MATS`, 0 = sem tapete).
    pub mat: u8,
    /// Diâmetro da lâmina em mm (offset de corte).
    pub blade_diameter_mm: f64,
    /// Reforço de trilha (track enhancing) — pressão ≥ 19 recomendada.
    pub track_enhancing: bool,
    /// Levantar a ferramenta em cantos agudos.
    pub sharpen_corners: bool,
    pub sharpen_start_mm: f64,
    pub sharpen_end_mm: f64,
    /// Deslocamento do desenho na mídia (mm).
    pub offset_x_mm: f64,
    pub offset_y_mm: f64,
    /// Largura da mídia carregada (mm).
    pub media_width_mm: f64,
    /// Comprimento da mídia carregada (mm).
    pub media_height_mm: f64,
    /// true: mídia avança para depois do corte; false: volta à posição inicial.
    pub end_below: bool,
    /// Ejetar a mídia no final (`FO0`).
    pub eject: bool,
    /// Tolerância de achatamento de curvas (mm) — 0,05 = resolução da máquina.
    pub tolerance_mm: f64,
    /// Reordenar caminhos para reduzir deslocamentos.
    pub optimize_order: bool,
    /// Número de passagens sobre o mesmo traço (materiais espessos).
    #[serde(default = "passes_default")]
    pub passes: u8,
    /// Marcas de registro (print & cut).
    pub regmark: RegmarkSettings,
}

impl Default for CutSettings {
    fn default() -> Self {
        CutSettings {
            speed: 10,
            pressure: 10,
            depth: None,
            toolholder: 1,
            pen: false,
            media: 102, // adesivo vinílico: pré-ajuste comum
            mat: 0,
            blade_diameter_mm: 0.9,
            track_enhancing: false,
            sharpen_corners: false,
            sharpen_start_mm: 0.1,
            sharpen_end_mm: 0.1,
            offset_x_mm: 15.0,
            offset_y_mm: 15.0,
            media_width_mm: 210.0,
            media_height_mm: 297.0,
            end_below: false,
            eject: false,
            tolerance_mm: 0.05,
            optimize_order: true,
            passes: 1,
            regmark: RegmarkSettings::default(),
        }
    }
}

/// Resultado de um corte/dry-run.
#[derive(Debug, Clone, Copy)]
pub struct CutReport {
    pub paths: usize,
    pub points: usize,
    pub bbox_mm: Bbox,
    pub bytes: usize,
    pub travel_mm: f64,
    pub cut_mm: f64,
}

/// Carrega um SVG e devolve as polilinhas em mm.
/// Carrega um desenho pelo tipo de arquivo: `.svg` ou `.dxf`.
pub fn load_file(path: &std::path::Path, tolerance_mm: f64) -> Result<svg::SvgDocument> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "dxf" {
        let doc = crate::dxf::load(path)?;
        if doc.polylines.is_empty() {
            return Err(SilError::Svg("DXF sem entidades de corte reconhecidas".into()));
        }
        return Ok(svg::SvgDocument {
            polylines: doc.polylines,
            width_mm: doc.width_mm,
            height_mm: doc.height_mm,
        });
    }
    load_svg(path, tolerance_mm)
}

pub fn load_svg(path: &std::path::Path, tolerance_mm: f64) -> Result<svg::SvgDocument> {
    let text = std::fs::read_to_string(path)?;
    svg::parse(&text, tolerance_mm)
}

/// Prepara os caminhos: otimização de ordem (opcional) e estatísticas.
pub fn prepare(paths: Vec<Polyline>, st: &CutSettings) -> (Vec<Polyline>, f64, f64) {
    if st.optimize_order {
        let (ordered, travel) = optimize::reorder_nearest(paths);
        let cut = optimize::total_cut_length(&ordered);
        (ordered, travel, cut)
    } else {
        let travel = optimize::total_travel(&paths);
        let cut = optimize::total_cut_length(&paths);
        (paths, travel, cut)
    }
}

/// Constrói o programa GPGL (usado por cut e dry_run).
pub fn build_program(paths: &[Polyline], st: &CutSettings, dev: &DeviceSpec) -> gpgl::GpglProgram {
    gpgl::build(paths, st, dev)
}

/// Corte completo no dispositivo com a configuração dada.
///
/// `progress` recebe `(fase 0.0–1.0, mensagem)` para feedback na UI/CLI.
pub fn cut(
    cutter: &mut Cutter,
    paths: Vec<Polyline>,
    st: &CutSettings,
    progress: &mut dyn FnMut(f32, &str),
) -> Result<CutReport> {
    let dev = cutter.spec;
    let (paths, travel, cut_len) = prepare(paths, st);
    let points = paths.iter().map(|p| p.len()).sum();

    progress(0.05, "inicializando dispositivo…");
    cutter.initialize()?;

    let prog = build_program(&paths, st, dev);

    progress(0.15, "enviando configuração…");
    cutter.safe_send(&prog.setup)?;

    if prog.needs_regmark_response {
        progress(0.3, "buscando marcas de registro (não mova a mídia)…");
        // O comando TB123 dispara a busca; aguarda a resposta de conclusão.
        let resp = cutter.read_response(std::time::Duration::from_secs(45))?;
        if resp.trim() != "0" && !resp.trim().is_empty() {
            return Err(SilError::Protocol(format!(
                "marcas de registro não encontradas (resposta: '{resp}'). \
                Confira a impressão em 100% e o alinhamento com o sensor"
            )));
        }
        progress(0.4, "marcas encontradas.");
    }

    progress(0.5, "cortando…");
    cutter.safe_send(&prog.body)?;
    progress(0.85, "finalizando…");
    cutter.safe_send(&prog.trailer)?;

    if st.eject {
        progress(0.95, "ejetando mídia…");
        cutter.feed_out()?;
    }

    let status = cutter.status()?;
    progress(1.0, &format!("concluído — máquina {}", status.as_str()));

    Ok(CutReport {
        paths: paths.len(),
        points,
        bbox_mm: prog.bbox,
        bytes: prog.setup.len() + prog.body.len() + prog.trailer.len(),
        travel_mm: travel,
        cut_mm: cut_len,
    })
}

/// Dry-run: grava o programa GPGL em um arquivo sem tocar na máquina.
pub fn dry_run(
    paths: Vec<Polyline>,
    st: &CutSettings,
    dev: &DeviceSpec,
    out: &std::path::Path,
) -> Result<CutReport> {
    let (paths, travel, cut_len) = prepare(paths, st);
    let points = paths.iter().map(|p| p.len()).sum();
    let prog = build_program(&paths, st, dev);
    std::fs::write(out, prog.to_bytes())?;
    Ok(CutReport {
        paths: paths.len(),
        points,
        bbox_mm: prog.bbox,
        bytes: prog.setup.len() + prog.body.len() + prog.trailer.len(),
        travel_mm: travel,
        cut_mm: cut_len,
    })
}

/// Lista dispositivos em formato textual (usada pelo CLI).
pub fn list_devices_text() -> Result<Vec<String>> {
    let devs = driver::list_devices()?;
    Ok(devs
        .iter()
        .map(|d| {
            format!(
                "{:04x}:{:04x} bus {:03} addr {:03} — {}",
                d.vid,
                d.pid,
                d.bus,
                d.address,
                d.display_name()
            )
        })
        .collect())
}

/// Verificações de sanidade antes do corte (mensagem clara para o operador).
pub fn validate(st: &CutSettings, dev: &DeviceSpec, bbox: Bbox) -> Vec<String> {
    let mut warn = Vec::new();
    if bbox.width() > st.media_width_mm || bbox.height() > st.media_height_mm {
        warn.push(format!(
            "o desenho ({:.1}x{:.1} mm) não cabe na mídia ({:.1}x{:.1} mm)",
            bbox.width(),
            bbox.height(),
            st.media_width_mm,
            st.media_height_mm
        ));
    }
    if st.regmark.enabled && !dev.regmark {
        warn.push("este modelo não suporta marcas de registro ópticas".into());
    }
    if st.mat != 0 && !dev.mat_support {
        warn.push("este modelo não usa tapete de corte (TG)".into());
    }
    if st.pressure as u32 > dev.max_pressure as u32 {
        warn.push(format!(
            "pressão {p} acima do máximo do modelo ({m}); será limitada",
            p = st.pressure,
            m = dev.max_pressure
        ));
    }
    if st.track_enhancing && st.pressure < 19 {
        warn.push("track enhancing exige pressão ≥ 19 para ser efetivo".into());
    }
    if device::media_by_code(st.media).is_none() {
        warn.push(format!("código de mídia {m} desconhecido", m = st.media));
    }
    warn
}
