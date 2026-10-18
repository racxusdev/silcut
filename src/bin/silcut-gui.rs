//! `silcut-gui` — interface gráfica nativa (egui/eframe) do silcut.
//!
//! Organização:
//! * **Toolbar** — máquina, status, salvar padrões, ajustar vista.
//! * **Painel lateral** — Arquivo (SVG/imagem vetorizada, arrastar-e-soltar),
//!   Papel & mídia (presets de folha e tipo de material), Força & lâmina
//!   (pressão/velocidade/profundidade/offset), Posição & tamanho (esticar,
//!   rotacionar, centrar), Print & cut (marcas de registro) e Ações.
//! * **Canvas** — papel desenhado em escala real (mm), réguas, grade, zoom/pan.
//! * **Status bar** — dispositivo, avisos e progresso do corte em thread.

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use eframe::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Shape, Stroke, Vec2};

use silcut::config::{self, AppConfig};
use silcut::cutjob::{self, CutSettings};
use silcut::device;
use silcut::driver::{self, Cutter};
use silcut::geom::{Point, Polyline};
use silcut::regmark;
use silcut::trace;
use silcut::{boolean, offset};

// ---------------------------------------------------------------------------
// Tema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
struct Theme {
    bg_app: Color32,
    bg_panel: Color32,
    bg_card: Color32,
    border: Color32,
    text: Color32,
    muted: Color32,
    accent: Color32,
    accent_soft: Color32,
    paper: Color32,
    paper_edge: Color32,
    cut: Color32,
    ok: Color32,
    warn: Color32,
    err: Color32,
    red_btn: Color32,
    grid_minor: Color32,
    grid_major: Color32,
    ruler: Color32,
    travel: Color32,
    hint: Color32,
}

impl Theme {
    fn dark() -> Self {
        Theme {
            bg_app: Color32::from_rgb(0x0e, 0x10, 0x14),
            bg_panel: Color32::from_rgb(0x15, 0x18, 0x1d),
            bg_card: Color32::from_rgb(0x1c, 0x20, 0x27),
            border: Color32::from_rgb(0x26, 0x2b, 0x33),
            text: Color32::from_rgb(0xe8, 0xea, 0xee),
            muted: Color32::from_rgb(0x8b, 0x93, 0xa1),
            accent: Color32::from_rgb(0x5a, 0xa9, 0xff),
            accent_soft: Color32::from_rgb(0x17, 0x33, 0x4f),
            paper: Color32::from_rgb(0xf7, 0xf4, 0xed),
            paper_edge: Color32::from_rgb(0xd6, 0xd0, 0xc2),
            cut: Color32::from_rgb(0x3b, 0x82, 0xf6),
            ok: Color32::from_rgb(0x4a, 0xde, 0x80),
            warn: Color32::from_rgb(0xfb, 0xbf, 0x24),
            err: Color32::from_rgb(0xf8, 0x71, 0x71),
            red_btn: Color32::from_rgb(0xb9, 0x1c, 0x1c),
            grid_minor: Color32::from_rgb(0xe9, 0xe4, 0xd8),
            grid_major: Color32::from_rgb(0xd2, 0xcb, 0xba),
            ruler: Color32::from_rgb(0x6b, 0x72, 0x80),
            travel: Color32::from_rgb(0xb9, 0x9d, 0x4e),
            hint: Color32::from_rgb(0x6b, 0x72, 0x80),
        }
    }

    fn light() -> Self {
        Theme {
            bg_app: Color32::from_rgb(0xee, 0xf0, 0xf3),
            bg_panel: Color32::from_rgb(0xf7, 0xf8, 0xfa),
            bg_card: Color32::from_rgb(0xff, 0xff, 0xff),
            border: Color32::from_rgb(0xd9, 0xde, 0xe5),
            text: Color32::from_rgb(0x1b, 0x1f, 0x24),
            muted: Color32::from_rgb(0x6b, 0x72, 0x80),
            accent: Color32::from_rgb(0x25, 0x63, 0xeb),
            accent_soft: Color32::from_rgb(0xdb, 0xe7, 0xfd),
            paper: Color32::from_rgb(0xff, 0xff, 0xff),
            paper_edge: Color32::from_rgb(0xc9, 0xd0, 0xd9),
            cut: Color32::from_rgb(0x1d, 0x4e, 0xd8),
            ok: Color32::from_rgb(0x16, 0xa3, 0x4a),
            warn: Color32::from_rgb(0xb4, 0x53, 0x09),
            err: Color32::from_rgb(0xdc, 0x26, 0x26),
            red_btn: Color32::from_rgb(0xdc, 0x26, 0x26),
            grid_minor: Color32::from_rgb(0xf1, 0xf3, 0xf6),
            grid_major: Color32::from_rgb(0xdf, 0xe4, 0xea),
            ruler: Color32::from_rgb(0x9a, 0xa3, 0xaf),
            travel: Color32::from_rgb(0xc2, 0xa2, 0x3a),
            hint: Color32::from_rgb(0x9a, 0xa3, 0xaf),
        }
    }
}

fn apply_theme(ctx: &egui::Context, th: &Theme, dark: bool) {
    let mut v = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.override_text_color = Some(th.text);
    v.panel_fill = th.bg_panel;
    v.window_fill = th.bg_card;
    v.extreme_bg_color = th.bg_app;
    v.faint_bg_color = th.bg_card;
    v.window_stroke = Stroke::new(1.0_f32, th.border);
    v.selection.bg_fill = th.accent_soft;
    v.selection.stroke = Stroke::new(1.0_f32, th.accent);
    v.widgets.noninteractive.bg_fill = th.bg_card;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, th.border);
    v.widgets.inactive.bg_fill = th.bg_card;
    v.widgets.inactive.weak_bg_fill = th.bg_card;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, th.border);
    v.widgets.hovered.bg_fill = th.accent_soft;
    v.widgets.hovered.weak_bg_fill = th.accent_soft;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, th.accent);
    v.widgets.active.bg_fill = th.accent_soft;
    v.widgets.active.weak_bg_fill = th.accent_soft;
    v.widgets.active.bg_stroke = Stroke::new(1.0_f32, th.accent);
    ctx.set_visuals(v);
    ctx.style_mut(|st| {
        st.spacing.item_spacing = egui::vec2(8.0, 6.0);
        st.spacing.button_padding = egui::vec2(10.0, 5.0);
        st.spacing.window_margin = egui::Margin::same(14.0);
        st.spacing.slider_width = 168.0;
    });
}

fn card_frame(th: &Theme) -> egui::Frame {
    egui::Frame::none()
        .fill(th.bg_card)
        .rounding(egui::Rounding::same(12.0))
        .stroke(Stroke::new(1.0_f32, th.border))
        .inner_margin(egui::Margin::same(11.0))
}

// ---------------------------------------------------------------------------
// Presets de folha
// ---------------------------------------------------------------------------

struct SheetPreset {
    name: &'static str,
    w_mm: f64,
    h_mm: f64,
    mat: u8,
}

const SHEETS: &[SheetPreset] = &[
    SheetPreset { name: "A4", w_mm: 210.0, h_mm: 297.0, mat: 0 },
    SheetPreset { name: "A5", w_mm: 148.0, h_mm: 210.0, mat: 0 },
    SheetPreset { name: "A3", w_mm: 297.0, h_mm: 420.0, mat: 0 },
    SheetPreset { name: "Carta", w_mm: 215.9, h_mm: 279.4, mat: 0 },
    SheetPreset { name: "12×12 pol", w_mm: 304.8, h_mm: 304.8, mat: 1 },
    SheetPreset { name: "12×24 pol", w_mm: 304.8, h_mm: 609.6, mat: 2 },
    SheetPreset { name: "8×12 pol", w_mm: 203.2, h_mm: 304.8, mat: 3 },
    SheetPreset { name: "15×15 pol", w_mm: 381.0, h_mm: 381.0, mat: 8 },
    SheetPreset { name: "24×24 pol", w_mm: 609.6, h_mm: 609.6, mat: 9 },
    SheetPreset { name: "Rolo 30 cm", w_mm: 304.8, h_mm: 1000.0, mat: 0 },
];

// ---------------------------------------------------------------------------
// Estado da aplicação
// ---------------------------------------------------------------------------

enum JobEvent {
    Msg(String),
    Progress(f32),
    Done(String),
    Failed(String),
}

struct Snapshot {
    raw: Vec<Vec<Pos2>>,
    doc_w: f32,
    doc_h: f32,
    doc_name: String,
}

#[derive(Default)]
struct JobShared {
    busy: bool,
    progress: f32,
}

struct App {
    cfg: AppConfig,
    settings: CutSettings,
    // documento (mm, sem transformação)
    raw: Vec<Vec<Pos2>>,
    doc_w: f32,
    doc_h: f32,
    doc_name: String,
    // transformação de saída
    scale_x: f32,
    scale_y: f32,
    link_aspect: bool,
    rotation: f32,
    // vista
    zoom: f32,
    pan: Vec2,
    fitted: bool,
    // exibição
    show_grid: bool,
    show_travel: bool,
    show_marks: bool,
    // máquina
    devices: Vec<driver::ListedDevice>,
    selected_device: Option<usize>,
    device_status: String,
    device_fw: String,
    // job
    shared: Arc<Mutex<JobShared>>,
    rx: Receiver<JobEvent>,
    tx: Sender<JobEvent>,
    confirm_cut: bool,
    last_report: String,
    last_error: String,
    // vetorização
    trace_opts: trace::TraceOptions,
    trace_path: Option<PathBuf>,
    // tema
    th: Theme,
    dark: bool,
    // arrastar marcas no papel
    drag_corner: Option<u8>,
    // edição do desenho
    history: Vec<Snapshot>,
    action: u8, // 0 cortar · 1 vincar · 2 esboçar
    offset_dist: f32,
    union_res: f32,
    // export folha
    page_w: f32,
    page_h: f32,
}

impl App {
    fn new() -> Self {
        let cfg = config::load();
        let dark = cfg.dark;
        let th = if dark { Theme::dark() } else { Theme::light() };
        let settings = cfg.default;
        let (tx, rx) = channel();
        App {
            settings,
            cfg,
            raw: Vec::new(),
            doc_w: 0.0,
            doc_h: 0.0,
            doc_name: String::new(),
            scale_x: 100.0,
            scale_y: 100.0,
            link_aspect: true,
            rotation: 0.0,
            zoom: 2.0,
            pan: Vec2::ZERO,
            fitted: false,
            show_grid: true,
            show_travel: false,
            show_marks: true,
            devices: Vec::new(),
            selected_device: None,
            device_status: String::from("verifique a máquina (🔄 na barra superior)"),
            device_fw: String::new(),
            shared: Arc::new(Mutex::new(JobShared { busy: false, progress: 0.0 })),
            rx,
            tx,
            confirm_cut: false,
            last_report: String::new(),
            last_error: String::new(),
            trace_opts: trace::TraceOptions::default(),
            trace_path: None,
            th,
            dark,
            drag_corner: None,
            history: Vec::new(),
            action: if settings.pen { 2 } else { 0 },
            offset_dist: 3.0,
            union_res: 0.1,
            page_w: 210.0,
            page_h: 297.0,
        }
    }

    // -- máquina ------------------------------------------------------------

    fn device_spec(&self) -> &'static device::DeviceSpec {
        self.devices
            .get(self.selected_device.unwrap_or(usize::MAX))
            .and_then(|d| d.spec)
            .or(self.cfg.last_device.and_then(|(v, p)| device::find_by_ids(v, p)))
            .unwrap_or_else(|| device::find_by_ids(device::VID_GRAPHTEC, 0x1137).unwrap())
    }

    fn max_pressure(&self) -> u8 {
        self.device_spec().max_pressure
    }

    // -- documento e transformação -------------------------------------------

    fn has_doc(&self) -> bool {
        !self.raw.is_empty()
    }

    /// Aplica escala + rotação (em torno do centro do documento) → mm finais.
    fn transformed(&self) -> Vec<Polyline> {
        let sx = (self.scale_x / 100.0) as f64;
        let sy = (self.scale_y / 100.0) as f64;
        let (cx, cy) = (self.doc_w as f64 * sx / 2.0, self.doc_h as f64 * sy / 2.0);
        let rot = (self.rotation as f64).to_radians();
        let (sin, cos) = rot.sin_cos();
        self.raw
            .iter()
            .map(|pl| {
                pl.iter()
                    .map(|p| {
                        let x = p.x as f64 * sx;
                        let y = p.y as f64 * sy;
                        let (dx, dy) = (x - cx, y - cy);
                        Point::new(cx + dx * cos - dy * sin, cy + dx * sin + dy * cos)
                    })
                    .collect()
            })
            .collect()
    }

    /// Desenho transformado + deslocamento na mídia (coordenadas da folha).
    fn positioned_paths(&self) -> Vec<Polyline> {
        let (dx, dy) = (self.settings.offset_x_mm, self.settings.offset_y_mm);
        self.transformed()
            .into_iter()
            .map(|pl| pl.into_iter().map(|p| Point::new(p.x + dx, p.y + dy)).collect())
            .collect()
    }

    fn fit_view(&mut self, canvas: Rect) {
        let pad = 56.0;
        let (pw, ph) = (
            self.settings.media_width_mm as f32,
            self.settings.media_height_mm as f32,
        );
        let zx = (canvas.width() - pad) / pw.max(1.0);
        let zy = (canvas.height() - pad - 14.0) / ph.max(1.0);
        self.zoom = zx.min(zy).clamp(0.05, 40.0);
        self.pan = Vec2::ZERO;
        self.fitted = true;
    }

    fn eff_bbox(&self) -> (f32, f32) {
        let mut mx = 0.0f32;
        let mut my = 0.0f32;
        for pl in self.transformed() {
            for p in pl {
                mx = mx.max(p.x as f32);
                my = my.max(p.y as f32);
            }
        }
        (mx, my)
    }

    fn load_svg_file(&mut self, path: PathBuf) {
        match cutjob::load_file(&path, self.settings.tolerance_mm) {
            Ok(doc) => self.set_doc(&doc, path),
            Err(e) => self.last_error = format!("{e}"),
        }
    }

    fn set_doc(&mut self, doc: &silcut::svg::SvgDocument, path: PathBuf) {
        if doc.polylines.is_empty() {
            self.last_error = "o arquivo não contém caminhos de corte".into();
            return;
        }
        self.push_history();
        self.raw = doc
            .polylines
            .iter()
            .map(|pl| pl.iter().map(|p| Pos2::new(p.x as f32, p.y as f32)).collect())
            .collect();
        self.doc_w = doc.width_mm as f32;
        self.doc_h = doc.height_mm as f32;
        self.doc_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        self.scale_x = 100.0;
        self.scale_y = 100.0;
        self.rotation = 0.0;
        self.fitted = false;
        self.last_error.clear();
        self.device_status = format!("carregado: {}", self.doc_name);
    }

    fn open_svg_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Desenho (SVG/DXF)", &["svg", "dxf"])
            .pick_file()
        {
            self.load_svg_file(path);
        }
    }

    fn trace_file(&mut self, path: PathBuf) {
        if !trace::potrace_available(&self.cfg.potrace_path) {
            self.last_error = "potrace não encontrado — instale: sudo apt install potrace".into();
            return;
        }
        let out = std::env::temp_dir().join(format!("silcut_trace_{}.svg", std::process::id()));
        match trace::trace_image(&path, &out, &self.trace_opts, &self.cfg.potrace_path) {
            Ok(()) => {
                self.trace_path = Some(path);
                self.load_svg_file(out);
                self.device_status = "imagem vetorizada — ajuste fino na seção Arquivo".into();
            }
            Err(e) => self.last_error = format!("{e}"),
        }
    }

    fn trace_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagens", &["png", "jpg", "jpeg", "bmp", "webp"])
            .pick_file()
        {
            self.trace_file(path);
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect()
        });
        for path in dropped {
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if ext == "svg" || ext == "dxf" {
                self.load_svg_file(path);
            } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "webp") {
                self.trace_file(path);
            } else {
                self.last_error = format!("formato não suportado: .{ext} (use SVG ou imagem)");
            }
        }
    }

    // -- mídia ---------------------------------------------------------------

    fn apply_media_preset(&mut self) {
        if let Some(m) = device::media_by_code(self.settings.media) {
            if let Some(p) = m.pressure {
                self.settings.pressure = p.min(self.max_pressure());
            }
            if let Some(s) = m.speed {
                self.settings.speed = s;
            }
            self.settings.depth = m.depth;
            self.settings.pen = m.code == 113;
        }
    }

    fn sheet_matches(&self, s: &SheetPreset) -> bool {
        (self.settings.media_width_mm - s.w_mm).abs() < 0.05
            && (self.settings.media_height_mm - s.h_mm).abs() < 0.05
    }

    fn apply_sheet(&mut self, s: &SheetPreset) {
        self.settings.media_width_mm = s.w_mm;
        self.settings.media_height_mm = s.h_mm;
        if self.device_spec().mat_support || s.mat == 0 {
            self.settings.mat = s.mat;
        }
        self.auto_fit_marks();
        self.fitted = false;
    }

    /// Área de marcas assume as bordas da folha (margem de 10 mm) e a folha
    /// de impressão acompanha o tamanho do papel.
    fn auto_fit_marks(&mut self) {
        const INSET: f64 = 10.0;
        let w = self.settings.media_width_mm;
        let h = self.settings.media_height_mm;
        self.settings.regmark.origin_x_mm = INSET;
        self.settings.regmark.origin_y_mm = INSET;
        self.settings.regmark.width_mm = (w - 2.0 * INSET).max(30.0);
        self.settings.regmark.length_mm = (h - 2.0 * INSET).max(30.0);
        self.page_w = w as f32;
        self.page_h = h as f32;
    }

    // -- ações sobre o desenho ------------------------------------------------

    /// Ajusta proporcionalmente dentro da folha (com margem de 5 mm).
    fn fit_to_sheet(&mut self) {
        if !self.has_doc() {
            return;
        }
        let mw = self.settings.media_width_mm as f32 - 10.0;
        let mh = self.settings.media_height_mm as f32 - 10.0;
        let sx = (mw / self.doc_w.max(0.1)) * 100.0;
        let sy = (mh / self.doc_h.max(0.1)) * 100.0;
        let k = sx.min(sy).clamp(1.0, 1000.0);
        self.scale_x = k;
        self.scale_y = k;
        self.center_doc();
    }

    /// Estica (distorce) preenchendo a folha inteira.
    fn stretch_to_sheet(&mut self) {
        if !self.has_doc() {
            return;
        }
        self.scale_x = ((self.settings.media_width_mm as f32 - 10.0) / self.doc_w.max(0.1)) * 100.0;
        self.scale_y = ((self.settings.media_height_mm as f32 - 10.0) / self.doc_h.max(0.1)) * 100.0;
        self.center_doc();
    }

    fn center_doc(&mut self) {
        let (w, h) = self.eff_bbox();
        self.settings.offset_x_mm = (((self.settings.media_width_mm as f32 - w) / 2.0).max(0.0)) as f64;
        self.settings.offset_y_mm =
            (((self.settings.media_height_mm as f32 - h) / 2.0).max(0.0)) as f64;
    }

    fn reset_transform(&mut self) {
        self.scale_x = 100.0;
        self.scale_y = 100.0;
        self.rotation = 0.0;
        self.settings.offset_x_mm = 15.0;
        self.settings.offset_y_mm = 15.0;
        self.fitted = false;
    }

    // -- edição do desenho ----------------------------------------------------

    fn push_history(&mut self) {
        self.history.push(Snapshot {
            raw: self.raw.clone(),
            doc_w: self.doc_w,
            doc_h: self.doc_h,
            doc_name: self.doc_name.clone(),
        });
        if self.history.len() > 20 {
            self.history.remove(0);
        }
    }

    fn undo(&mut self) {
        if let Some(s) = self.history.pop() {
            self.raw = s.raw;
            self.doc_w = s.doc_w;
            self.doc_h = s.doc_h;
            self.doc_name = s.doc_name;
            self.fitted = false;
            self.last_error.clear();
            self.device_status = "desfeito".into();
        } else {
            self.device_status = "nada para desfazer".into();
        }
    }

    fn raw_as_polylines(&self) -> Vec<Polyline> {
        self.raw
            .iter()
            .map(|pl| {
                pl.iter()
                    .map(|p| Point::new(p.x as f64, p.y as f64))
                    .collect()
            })
            .collect()
    }

    fn set_from_polylines(&mut self, pls: &[Polyline]) {
        let mut bb: Option<silcut::geom::Bbox> = None;
        for p in pls {
            if let Some(b) = silcut::geom::Bbox::from_points(p) {
                bb = Some(match bb {
                    Some(prev) => prev.union(&b),
                    None => b,
                });
            }
        }
        let bb = bb.unwrap_or_default();
        self.raw = pls
            .iter()
            .map(|pl| {
                pl.iter()
                    .map(|p| {
                        Pos2::new((p.x - bb.min_x) as f32, (p.y - bb.min_y) as f32)
                    })
                    .collect()
            })
            .collect();
        self.doc_w = bb.width() as f32;
        self.doc_h = bb.height() as f32;
        self.fitted = false;
    }

    fn apply_offset(&mut self) {
        if !self.has_doc() || self.offset_dist.abs() < 0.05 {
            return;
        }
        self.push_history();
        let out = offset::offset_paths(&self.raw_as_polylines(), self.offset_dist as f64);
        if out.is_empty() {
            self.last_error = "offset não gerou contornos".into();
            return;
        }
        self.set_from_polylines(&out);
        self.device_status = format!("offset de {:.1} mm aplicado", self.offset_dist);
    }

    fn apply_union(&mut self) {
        if !self.has_doc() {
            return;
        }
        self.push_history();
        let out = boolean::union(&self.raw_as_polylines(), self.union_res as f64);
        if out.is_empty() {
            self.last_error = "união não gerou contornos".into();
            return;
        }
        self.set_from_polylines(&out);
        self.device_status = format!("{} contorno(s) fundido(s)", out.len());
    }

    fn keepout_violation(&self) -> bool {
        if !self.settings.regmark.enabled || !self.has_doc() {
            return false;
        }
        let bb = bbox_of(&self.positioned_paths());
        regmark::keepout_boxes(&self.settings.regmark)
            .iter()
            .any(|k| k.intersects(&bb))
    }

    // -- persistência ---------------------------------------------------------

    fn save_defaults(&mut self) {
        self.cfg.default = self.settings;
        if let Some(i) = self.selected_device {
            if let Some(d) = self.devices.get(i) {
                self.cfg.last_device = Some((d.vid, d.pid));
            }
        }
        match config::save(&self.cfg) {
            Ok(()) => self.device_status = "padrões salvos em ~/.config/silcut/config.toml".into(),
            Err(e) => self.last_error = format!("salvar configuração: {e}"),
        }
    }

    // -- trabalho em thread ----------------------------------------------------

    fn spawn<F>(&mut self, f: F)
    where
        F: FnOnce(Sender<JobEvent>, Arc<Mutex<JobShared>>) + Send + 'static,
    {
        {
            let mut sh = self.shared.lock().unwrap();
            if sh.busy {
                self.last_error = "já existe uma operação em andamento".into();
                return;
            }
            sh.busy = true;
            sh.progress = 0.0;
        }
        let tx = self.tx.clone();
        let shared = Arc::clone(&self.shared);
        std::thread::spawn(move || f(tx, shared));
    }

    fn drain_events(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                JobEvent::Msg(m) => self.device_status = m,
                JobEvent::Progress(p) => self.shared.lock().unwrap().progress = p,
                JobEvent::Done(report) => {
                    self.last_report = report;
                    self.last_error.clear();
                    let mut sh = self.shared.lock().unwrap();
                    sh.busy = false;
                    sh.progress = 1.0;
                }
                JobEvent::Failed(e) => {
                    self.last_error = e;
                    let mut sh = self.shared.lock().unwrap();
                    sh.busy = false;
                }
            }
        }
    }

    fn refresh_devices(&mut self) {
        self.devices = driver::list_devices().unwrap_or_default();
        if self.devices.is_empty() {
            self.selected_device = None;
            self.device_status = "nenhum cortador encontrado — verifique USB/energia e a regra udev".into();
        } else {
            self.selected_device = self
                .devices
                .iter()
                .position(|d| self.cfg.last_device == Some((d.vid, d.pid)))
                .or(Some(0));
            self.device_status = format!("{} dispositivo(s) conectado(s)", self.devices.len());
        }
    }

    fn query_status(&mut self) {
        self.spawn(|tx, _| match Cutter::open_first() {
            Ok(mut c) => {
                let name = c.spec.name.to_string();
                match c.initialize() {
                    Ok(v) => {
                        let st = c
                            .status()
                            .map(|s| s.as_str().to_string())
                            .unwrap_or_else(|e| format!("{e}"));
                        tx.send(JobEvent::Done(format!(
                            "✔ {name} · firmware {v} · status: {st}"
                        )))
                        .ok();
                    }
                    Err(e) => {
                        tx.send(JobEvent::Failed(format!("{e}"))).ok();
                    }
                }
            }
            Err(e) => {
                tx.send(JobEvent::Failed(format!("{e}"))).ok();
            }
        });
    }

    fn start_cut(&mut self) {
        self.save_defaults();
        let settings = self.settings;
        let paths = self.transformed();
        self.spawn(move |tx, shared| {
            let mut progress = |pct: f32, msg: &str| {
                tx.send(JobEvent::Progress(pct)).ok();
                tx.send(JobEvent::Msg(msg.to_string())).ok();
            };
            match Cutter::open_first() {
                Ok(mut cutter) => {
                    progress(0.0, &format!("conectado: {}", cutter.spec.name));
                    match cutjob::cut(&mut cutter, paths, &settings, &mut progress) {
                        Ok(r) => {
                            tx.send(JobEvent::Done(format!(
                                "✔ corte concluído — {} caminhos · {:.1}×{:.1} mm · {:.0} mm de percurso",
                                r.paths,
                                r.bbox_mm.width(),
                                r.bbox_mm.height(),
                                r.cut_mm
                            )))
                            .ok();
                        }
                        Err(e) => {
                            tx.send(JobEvent::Failed(format!("{e}"))).ok();
                        }
                    }
                }
                Err(e) => {
                    tx.send(JobEvent::Failed(format!("{e}"))).ok();
                }
            }
            let mut sh = shared.lock().unwrap();
            sh.busy = false;
        });
    }

    fn export_dry_run(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("GPGL", &["gpgl", "txt"])
            .set_file_name("corte.gpgl")
            .save_file()
        {
            let settings = self.settings;
            let paths = self.transformed();
            match cutjob::dry_run(paths, &settings, self.device_spec(), &path) {
                Ok(r) => {
                    self.last_report = format!(
                        "GPGL salvo: {} · {} bytes · {:.1}×{:.1} mm",
                        path.display(),
                        r.bytes,
                        r.bbox_mm.width(),
                        r.bbox_mm.height()
                    );
                    self.last_error.clear();
                }
                Err(e) => self.last_error = format!("{e}"),
            }
        }
    }

    fn export_print_sheet(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("SVG", &["svg"])
            .set_file_name("folha-print-cut.svg")
            .save_file()
        {
            let paths = self.positioned_paths();
            let mut r = self.settings.regmark;
            // a folha de impressão usa a origem das marcas como referência
            if !r.enabled {
                r.enabled = true;
            }
            let svg = regmark::print_sheet_svg(&paths, &r, self.page_w as f64, self.page_h as f64);
            match std::fs::write(&path, svg) {
                Ok(()) => {
                    self.last_report =
                        format!("folha print & cut: {} (imprimir em 100%)", path.display());
                    self.last_error.clear();
                }
                Err(e) => self.last_error = format!("{e}"),
            }
        }
    }

    fn estimate_minutes(&self) -> f32 {
        let paths = self.transformed();
        let mut total = 0.0f64;
        for pl in &paths {
            for w in pl.windows(2) {
                total += w[0].dist(w[1]);
            }
        }
        // velocidade Graphtec ≈ cm/s
        (total / 10.0 / self.settings.speed.max(1) as f64 / 60.0) as f32
    }
}

fn bbox_of(paths: &[Polyline]) -> silcut::geom::Bbox {
    let mut acc: Option<silcut::geom::Bbox> = None;
    for p in paths {
        if let Some(b) = silcut::geom::Bbox::from_points(p) {
            acc = Some(match acc {
                Some(prev) => prev.union(&b),
                None => b,
            });
        }
    }
    acc.unwrap_or_default()
}

// ---------------------------------------------------------------------------
// eframe
// ---------------------------------------------------------------------------

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([1000.0, 640.0])
            .with_title("silcut — corte Silhouette nativo para Linux"),
        ..Default::default()
    };
    let cfg = config::load();
    let dark = cfg.dark;
    let th = if dark { Theme::dark() } else { Theme::light() };
    eframe::run_native(
        "silcut",
        options,
        Box::new(move |cc| {
            apply_theme(&cc.egui_ctx, &th, dark);
            Ok(Box::new(App::new()))
        }),
    )
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_events();
        self.handle_dropped_files(ctx);

        self.toolbar(ctx);
        self.side_panel(ctx);
        self.canvas(ctx);
        self.status_bar(ctx);
        self.confirm_window(ctx);

        if self.shared.lock().unwrap().busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
    }
}

/// Cabeçalho de seção estilizado.
fn section(
    ui: &mut egui::Ui,
    th: &Theme,
    icon: &str,
    title: &str,
    default_open: bool,
    body: impl FnOnce(&mut egui::Ui),
) {
    let header = RichText::new(format!("{icon}  {title}")).strong().size(13.5).color(th.text);
    egui::CollapsingHeader::new(header)
        .default_open(default_open)
        .show(ui, |ui| {
            ui.add_space(2.0);
            body(ui);
            ui.add_space(2.0);
        });
    ui.add_space(2.0);
}

fn chip_button(ui: &mut egui::Ui, th: &Theme, text: &str, selected: bool) -> bool {
    let btn = egui::Button::new(
        RichText::new(text)
            .size(12.0)
            .color(if selected { Color32::WHITE } else { th.muted }),
    )
    .fill(if selected { th.accent_soft } else { th.bg_card })
    .stroke(Stroke::new(
        1.0_f32,
        if selected { th.accent } else { th.border },
    ))
    .rounding(egui::Rounding::same(7.0));
    ui.add(btn).clicked()
}

fn slider_row(ui: &mut egui::Ui, th: &Theme, label: &str, slider: egui::Slider<'_>) {
    ui.horizontal(|ui| {
        ui.add_sized([96.0, 18.0], egui::Label::new(RichText::new(label).color(th.muted).size(12.0)));
        ui.add(slider);
    });
}

// ---------------------------------------------------------------------------
// Barra superior
// ---------------------------------------------------------------------------

impl App {
    fn toolbar(&mut self, ctx: &egui::Context) {
        let th = self.th;
        egui::TopBottomPanel::top("toolbar")
            .frame(
                egui::Frame::none()
                    .fill(th.bg_panel)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("silcut")
                            .strong()
                            .size(21.0)
                            .color(th.accent),
                    );
                    ui.label(
                        RichText::new(format!("v{}", silcut::APP_VERSION))
                            .small()
                            .color(th.muted),
                    );
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(6.0);

                    // máquina
                    let label = self
                        .devices
                        .get(self.selected_device.unwrap_or(usize::MAX))
                        .map(|d| d.display_name())
                        .unwrap_or_else(|| "— sem máquina —".into());
                    egui::ComboBox::from_id_salt("device")
                        .selected_text(RichText::new(label).size(12.5))
                        .width(230.0)
                        .show_ui(ui, |ui| {
                            for (i, d) in self.devices.iter().enumerate() {
                                ui.selectable_value(
                                    &mut self.selected_device,
                                    Some(i),
                                    d.display_name(),
                                );
                            }
                            if self.devices.is_empty() {
                                ui.weak("nenhum cortador conectado");
                            }
                        });
                    if ui
                        .button("🔄")
                        .on_hover_text("Procurar cortadores na USB")
                        .clicked()
                    {
                        self.refresh_devices();
                    }
                    if ui
                        .button("⚡ status")
                        .on_hover_text("Firmware e status da máquina")
                        .clicked()
                    {
                        self.query_status();
                    }

                    // lado direito
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (icon, tip) = if self.dark {
                            ("☀", "Mudar para tema claro")
                        } else {
                            ("🌙", "Mudar para tema escuro")
                        };
                        if ui.button(icon).on_hover_text(tip).clicked() {
                            self.dark = !self.dark;
                            self.th = if self.dark { Theme::dark() } else { Theme::light() };
                            apply_theme(ctx, &self.th, self.dark);
                            self.cfg.dark = self.dark;
                            let _ = config::save(&self.cfg);
                        }
                        if ui
                            .button("↶ desfazer")
                            .on_hover_text("Desfaz a última edição (offset/fundir/abrir)")
                            .clicked()
                        {
                            self.undo();
                        }
                        if ui
                            .button("💾 salvar padrões")
                            .on_hover_text("Grava em ~/.config/silcut/config.toml")
                            .clicked()
                        {
                            self.save_defaults();
                        }
                        if ui
                            .button("🎯 ajustar vista")
                            .on_hover_text("Enquadra a folha na janela")
                            .clicked()
                        {
                            self.fitted = false;
                        }
                        if !self.device_fw.is_empty() {
                            ui.label(RichText::new(&self.device_fw).small().color(th.muted));
                        }
                    });
                });
            });
    }

    // -----------------------------------------------------------------------
    // Painel lateral
    // -----------------------------------------------------------------------

    fn side_panel(&mut self, ctx: &egui::Context) {
        let th = self.th;
        egui::SidePanel::left("config")
            .resizable(true)
            .default_width(352.0)
            .min_width(320.0)
            .max_width(460.0)
            .frame(
                egui::Frame::none()
                    .fill(th.bg_panel)
                    .inner_margin(egui::Margin::symmetric(10.0, 8.0)),
            )
            .show(ctx, |ui| {
                let bottom_h = 74.0;
                let scroll_h = (ui.available_height() - bottom_h).max(120.0);
                egui::ScrollArea::vertical()
                    .max_height(scroll_h)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.file_section(ui);
                        self.sheet_section(ui);
                        self.force_section(ui);
                        self.transform_section(ui);
                        self.edit_section(ui);
                        self.printcut_section(ui);
                        self.output_section(ui);
                        ui.add_space(6.0);
                    });

                // ---- ação principal fixa embaixo ----
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
                let busy = self.shared.lock().unwrap().busy;
                let can_cut = !busy && self.has_doc();
                let w = ui.available_width();
                let cut = egui::Button::new(
                    RichText::new("✂   CORTAR").size(17.0).strong().color(Color32::WHITE),
                )
                .fill(th.red_btn)
                .rounding(egui::Rounding::same(9.0))
                .min_size(Vec2::new(w, 40.0));
                ui.add_enabled_ui(can_cut, |ui| {
                    if ui.add(cut).clicked() {
                        self.confirm_cut = true;
                    }
                });
                if !can_cut && !busy {
                    ui.label(
                        RichText::new("carregue um SVG para habilitar o corte")
                            .small()
                            .color(th.muted),
                    );
                }
            });
    }

    fn file_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "📂", "Arquivo", true, |ui| {
            let w = ui.available_width();
            if ui
                .add_sized(
                    [w, 30.0],
                    egui::Button::new(RichText::new("Abrir SVG / DXF…").size(13.0))
                        .fill(th.bg_card)
                        .rounding(egui::Rounding::same(7.0)),
                )
                .clicked()
            {
                self.open_svg_dialog();
            }

            // vetorização
            let potrace_ok = trace::potrace_available(&self.cfg.potrace_path);
            egui::CollapsingHeader::new(
                RichText::new("🖼  Vetorizar imagem (foto → corte)").size(12.5),
            )
            .default_open(false)
            .show(ui, |ui| {
                if !potrace_ok {
                    ui.label(
                        RichText::new("potrace ausente: sudo apt install potrace")
                            .small()
                            .color(th.warn),
                    );
                }
                egui::Grid::new("trace_grid")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("limiar").size(12.0).color(th.muted));
                        ui.add(
                            egui::Slider::new(&mut self.trace_opts.threshold, 1..=254)
                                .show_value(true),
                        );
                        ui.end_row();
                        ui.label(RichText::new("suavizar cantos").size(12.0).color(th.muted));
                        ui.add(
                            egui::Slider::new(&mut self.trace_opts.alphamax, 0.0..=1.34)
                                .fixed_decimals(2),
                        );
                        ui.end_row();
                        ui.label(RichText::new("simplificar curvas").size(12.0).color(th.muted));
                        ui.add(
                            egui::Slider::new(&mut self.trace_opts.opt_tolerance, 0.0..=1.0)
                                .fixed_decimals(2),
                        );
                        ui.end_row();
                        ui.label(RichText::new("remover ruído (px)").size(12.0).color(th.muted));
                        ui.add(egui::Slider::new(&mut self.trace_opts.turd, 0..=20));
                        ui.end_row();
                    });
                ui.checkbox(&mut self.trace_opts.invert, "inverter (traçar o claro)");
                let w = ui.available_width();
                ui.add_enabled_ui(potrace_ok, |ui| {
                    if ui
                        .add_sized(
                            [w, 28.0],
                            egui::Button::new("Escolher imagem e vetorizar…")
                                .fill(th.accent_soft)
                                .rounding(egui::Rounding::same(7.0)),
                        )
                        .clicked()
                    {
                        self.trace_dialog();
                    }
                });
                if let Some(p) = &self.trace_path {
                    ui.label(
                        RichText::new(format!("fonte: {}", p.display()))
                            .small()
                            .color(th.muted),
                    );
                }
            });

            ui.add_space(4.0);
            // cartão de informações do documento
            let frame = card_frame(&th).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if self.has_doc() {
                    let (w, h) = self.eff_bbox();
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📄").size(15.0));
                        ui.vertical(|ui| {
                            ui.label(
                                RichText::new(&self.doc_name)
                                    .size(12.5)
                                    .strong()
                                    .color(th.text),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} caminhos · saída {:.1} × {:.1} mm",
                                    self.raw.len(),
                                    w,
                                    h
                                ))
                                .size(11.0)
                                .color(th.muted),
                            );
                        });
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("📄").size(15.0).color(th.muted));
                        ui.label(
                            RichText::new("nenhum arquivo — abra um SVG ou arraste para a janela")
                                .size(11.5)
                                .color(th.muted),
                        );
                    });
                }
            });
            drop(frame);
        });
    }

    fn sheet_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "📐", "Papel & mídia", true, |ui| {
            ui.label(RichText::new("tamanho da folha").size(11.5).color(th.muted));
            let mut chosen: Option<usize> = None;
            ui.horizontal_wrapped(|ui| {
                for (i, s) in SHEETS.iter().enumerate() {
                    if chip_button(ui, &th, s.name, self.sheet_matches(s)) {
                        chosen = Some(i);
                    }
                }
            });
            if let Some(i) = chosen {
                self.apply_sheet(&SHEETS[i]);
            }

            egui::Grid::new("sheet_dims")
                .num_columns(3)
                .spacing([6.0, 4.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("largura").size(12.0).color(th.muted));
                    let w_resp = ui.add(
                        egui::DragValue::new(&mut self.settings.media_width_mm)
                            .range(20.0..=610.0)
                            .speed(1.0)
                            .suffix(" mm"),
                    );
                    if w_resp.changed() {
                        self.auto_fit_marks();
                        self.fitted = false;
                    }
                    if ui.small_button("⇄").on_hover_text("Trocar largura/altura").clicked() {
                        std::mem::swap(
                            &mut self.settings.media_width_mm,
                            &mut self.settings.media_height_mm,
                        );
                        self.auto_fit_marks();
                        self.fitted = false;
                    }
                    ui.end_row();
                    ui.label(RichText::new("altura").size(12.0).color(th.muted));
                    let h_resp = ui.add(
                        egui::DragValue::new(&mut self.settings.media_height_mm)
                            .range(20.0..=3000.0)
                            .speed(1.0)
                            .suffix(" mm"),
                    );
                    if h_resp.changed() {
                        self.auto_fit_marks();
                        self.fitted = false;
                    }
                    ui.label("");
                    ui.end_row();
                });

            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("material").size(12.0).color(th.muted));
                let prev = self.settings.media;
                egui::ComboBox::from_id_salt("media")
                    .selected_text(
                        device::media_by_code(self.settings.media)
                            .map(|m| m.name)
                            .unwrap_or("Personalizado"),
                    )
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for m in device::MEDIA {
                            let extra = match (m.pressure, m.speed) {
                                (Some(p), Some(s)) => format!("  ·  força {p} · vel {s}"),
                                _ => String::new(),
                            };
                            ui.selectable_value(
                                &mut self.settings.media,
                                m.code,
                                format!("{}{}", m.name, extra),
                            );
                        }
                    });
                if prev != self.settings.media {
                    self.apply_media_preset();
                }
            });

            // tapete (só Cameo3+)
            let mat_ok = self.device_spec().mat_support;
            ui.add_enabled_ui(mat_ok, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("tapete").size(12.0).color(th.muted));
                    egui::ComboBox::from_id_salt("mat")
                        .selected_text(
                            device::mat_by_code(self.settings.mat)
                                .map(|m| m.name)
                                .unwrap_or("Sem tapete"),
                        )
                        .width(200.0)
                        .show_ui(ui, |ui| {
                            for m in device::MATS {
                                ui.selectable_value(&mut self.settings.mat, m.code, m.name);
                            }
                        });
                });
            });
            if !mat_ok {
                ui.label(
                    RichText::new("máquina sem suporte a tapete — ignore")
                        .small()
                        .color(th.muted),
                );
            }
        });
    }

    fn force_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "💪", "Força & lâmina", true, |ui| {
            let maxp = self.max_pressure();
            ui.horizontal(|ui| {
                ui.label(RichText::new("ação").size(12.0).color(th.muted));
                for (i, name) in ["cortar", "vincar", "esboçar"].iter().enumerate() {
                    if chip_button(ui, &th, name, self.action == i as u8) {
                        self.action = i as u8;
                        match self.action {
                            0 => self.settings.pen = false,
                            1 => {
                                self.settings.pen = false;
                                self.settings.pressure = 3.min(maxp);
                            }
                            _ => {
                                self.settings.pen = true;
                                self.settings.pressure = 18.min(maxp);
                            }
                        }
                    }
                }
            });
            if self.action == 1 {
                ui.label(
                    RichText::new("vincar: use a lâmina de vincar com profundidade baixa")
                        .small()
                        .color(th.muted),
                );
            }
            slider_row(
                ui,
                &th,
                "força (pressão)",
                egui::Slider::new(&mut self.settings.pressure, 1..=maxp)
                    .suffix(" g?"),
            );
            // realce visual da força
            ui.add_space(-4.0);
            let frac = self.settings.pressure as f32 / maxp as f32;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 5.0), Sense::hover());
            ui.painter().rect_filled(rect, egui::Rounding::same(2.5), th.bg_card);
            let fill_col = if frac < 0.4 {
                th.ok
            } else if frac < 0.7 {
                th.warn
            } else {
                th.err
            };
            ui.painter().rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(rect.width() * frac, rect.height())),
                egui::Rounding::same(2.5),
                fill_col,
            );

            slider_row(
                ui,
                &th,
                "velocidade",
                egui::Slider::new(&mut self.settings.speed, 1..=30),
            );
            ui.horizontal(|ui| {
                ui.label(RichText::new("passagens").size(12.0).color(th.muted));
                ui.add(egui::DragValue::new(&mut self.settings.passes).range(1..=6));
                ui.label(
                    RichText::new("repassa o mesmo traço (materiais espessos)")
                        .small()
                        .color(th.muted),
                );
            });
            ui.horizontal(|ui| {
                let mut has_depth = self.settings.depth.is_some();
                if ui.checkbox(&mut has_depth, "profundidade da lâmina").changed() {
                    self.settings.depth = if has_depth {
                        Some(self.settings.depth.unwrap_or(1))
                    } else {
                        None
                    };
                }
                if let Some(d) = self.settings.depth.as_mut() {
                    ui.add(egui::Slider::new(d, 1..=10));
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("diâmetro da lâmina").size(12.0).color(th.muted));
                ui.add(
                    egui::DragValue::new(&mut self.settings.blade_diameter_mm)
                        .range(0.1..=1.5)
                        .speed(0.05)
                        .fixed_decimals(2)
                        .suffix(" mm"),
                );
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("ferramenta").size(12.0).color(th.muted));
                ui.selectable_value(
                    &mut self.settings.toolholder,
                    1,
                    RichText::new("1 (lâmina)").size(12.0),
                );
                ui.selectable_value(
                    &mut self.settings.toolholder,
                    2,
                    RichText::new("2 (caneta)").size(12.0),
                );
                ui.checkbox(&mut self.settings.pen, "modo caneta");
            });

            egui::CollapsingHeader::new(RichText::new("⚙  Avançado").size(12.0))
                .default_open(false)
                .show(ui, |ui| {
                    ui.checkbox(
                        &mut self.settings.track_enhancing,
                        "reforço de trilha (pressão ≥ 19)",
                    );
                    ui.checkbox(&mut self.settings.sharpen_corners, "levantar em cantos agudos");
                    if self.settings.sharpen_corners {
                        egui::Grid::new("sharp")
                            .num_columns(2)
                            .spacing([8.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("entrada (mm)").size(11.5).color(th.muted));
                                ui.add(
                                    egui::DragValue::new(&mut self.settings.sharpen_start_mm)
                                        .range(0.0..=5.0)
                                        .speed(0.05)
                                        .suffix(" mm"),
                                );
                                ui.end_row();
                                ui.label(RichText::new("saída (mm)").size(11.5).color(th.muted));
                                ui.add(
                                    egui::DragValue::new(&mut self.settings.sharpen_end_mm)
                                        .range(0.0..=5.0)
                                        .speed(0.05)
                                        .suffix(" mm"),
                                );
                                ui.end_row();
                            });
                    }
                    ui.checkbox(&mut self.settings.optimize_order, "otimizar ordem dos cortes");
                    ui.checkbox(&mut self.settings.end_below, "avançar mídia no final");
                    ui.checkbox(&mut self.settings.eject, "ejetar mídia no final");
                    slider_row(
                        ui,
                        &th,
                        "tolerância (mm)",
                        egui::Slider::new(&mut self.settings.tolerance_mm, 0.02..=0.3)
                            .fixed_decimals(2),
                    );
                });
        });
    }

    fn transform_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "📏", "Posição & tamanho", true, |ui| {
            // escala
            ui.horizontal(|ui| {
                ui.label(RichText::new("⤢  escala").size(12.0).color(th.muted));
                let mut link = self.link_aspect;
                if ui.checkbox(&mut link, "proporção").changed() {
                    self.link_aspect = link;
                }
            });
            ui.horizontal(|ui| {
                ui.label(RichText::new("X").size(12.0).color(th.muted));
                let rx = ui.add(
                    egui::DragValue::new(&mut self.scale_x)
                        .range(1.0..=1000.0)
                        .speed(1.0)
                        .suffix(" %"),
                );
                if rx.changed() && self.link_aspect {
                    self.scale_y = self.scale_x;
                }
                ui.label(RichText::new("Y").size(12.0).color(th.muted));
                let ry = ui.add(
                    egui::DragValue::new(&mut self.scale_y)
                        .range(1.0..=1000.0)
                        .speed(1.0)
                        .suffix(" %"),
                );
                if ry.changed() && self.link_aspect {
                    self.scale_x = self.scale_y;
                }
            });

            // rotação
            ui.horizontal(|ui| {
                ui.label(RichText::new("⟳  rotação").size(12.0).color(th.muted));
                ui.add(
                    egui::DragValue::new(&mut self.rotation)
                        .range(-180.0..=180.0)
                        .speed(1.0)
                        .suffix("°"),
                );
                if ui.small_button("-90°").clicked() {
                    self.rotation -= 90.0;
                }
                if ui.small_button("+90°").clicked() {
                    self.rotation += 90.0;
                }
                if ui.small_button("0").clicked() {
                    self.rotation = 0.0;
                }
            });

            // posição
            ui.horizontal(|ui| {
                ui.label(RichText::new("✥  posição").size(12.0).color(th.muted));
                ui.add(
                    egui::DragValue::new(&mut self.settings.offset_x_mm)
                        .range(0.0..=610.0)
                        .speed(0.5)
                        .prefix("x ")
                        .suffix(" mm"),
                );
                ui.add(
                    egui::DragValue::new(&mut self.settings.offset_y_mm)
                        .range(0.0..=3000.0)
                        .speed(0.5)
                        .prefix("y ")
                        .suffix(" mm"),
                );
            });

            let w = ui.available_width();
            ui.horizontal(|ui| {
                let bw = (w - 12.0) / 3.0;
                if ui
                    .add_sized(
                        [bw, 26.0],
                        egui::Button::new(RichText::new("cabê na folha").size(11.5))
                            .fill(th.bg_card)
                            .rounding(egui::Rounding::same(6.0)),
                    )
                    .on_hover_text("Ajusta proporcionalmente dentro da folha")
                    .clicked()
                {
                    self.fit_to_sheet();
                }
                if ui
                    .add_sized(
                        [bw, 26.0],
                        egui::Button::new(RichText::new("esticar").size(11.5))
                            .fill(th.bg_card)
                            .rounding(egui::Rounding::same(6.0)),
                    )
                    .on_hover_text("Estica/alonga preenchendo toda a folha")
                    .clicked()
                {
                    self.stretch_to_sheet();
                }
                if ui
                    .add_sized(
                        [bw, 26.0],
                        egui::Button::new(RichText::new("centrar").size(11.5))
                            .fill(th.bg_card)
                            .rounding(egui::Rounding::same(6.0)),
                    )
                    .clicked()
                {
                    self.center_doc();
                }
            });
            if ui
                .add_sized(
                    [w, 24.0],
                    egui::Button::new(RichText::new("↺  restaurar 100 %").size(11.5))
                        .fill(th.bg_card)
                        .rounding(egui::Rounding::same(6.0)),
                )
                .clicked()
            {
                self.reset_transform();
            }
        });
    }

    fn edit_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "🛠", "Editar desenho", false, |ui| {
            ui.add_enabled_ui(self.has_doc(), |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("offset").size(12.0).color(th.muted));
                    ui.add(
                        egui::DragValue::new(&mut self.offset_dist)
                            .range(-20.0..=20.0)
                            .speed(0.5)
                            .suffix(" mm"),
                    );
                    if ui
                        .button("aplicar")
                        .on_hover_text("Contorno paralelo — borda branca de adesivos/topos")
                        .clicked()
                    {
                        self.apply_offset();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("fundir").size(12.0).color(th.muted));
                    ui.add(
                        egui::DragValue::new(&mut self.union_res)
                            .range(0.05..=0.5)
                            .speed(0.05)
                            .fixed_decimals(2)
                            .suffix(" mm"),
                    );
                    if ui
                        .button("unir contornos")
                        .on_hover_text("Weld: une formas sobrepostas em um contorno só")
                        .clicked()
                    {
                        self.apply_union();
                    }
                });
                if ui.button("↶ desfazer").clicked() {
                    self.undo();
                }
            });
            ui.label(
                RichText::new("as edições valem para a sessão — exporte/salve se precisar")
                    .small()
                    .color(th.muted),
            );
        });
    }

    fn printcut_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "🎯", "Print & cut (marcas)", false, |ui| {
            ui.checkbox(
                &mut self.settings.regmark.enabled,
                "usar marcas de registro",
            );
            ui.add_enabled_ui(self.settings.regmark.enabled, |ui| {
                ui.checkbox(&mut self.settings.regmark.search, "busca automática pelo sensor");
                egui::Grid::new("reg")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("largura da área").size(11.5).color(th.muted));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.regmark.width_mm)
                                .range(30.0..=600.0)
                                .speed(1.0)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label(RichText::new("altura da área").size(11.5).color(th.muted));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.regmark.length_mm)
                                .range(30.0..=3000.0)
                                .speed(1.0)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label(RichText::new("origem X").size(11.5).color(th.muted));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.regmark.origin_x_mm)
                                .range(0.0..=600.0)
                                .speed(0.5)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label(RichText::new("origem Y").size(11.5).color(th.muted));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.regmark.origin_y_mm)
                                .range(0.0..=3000.0)
                                .speed(0.5)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label(RichText::new("comp. da marca").size(11.5).color(th.muted));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.regmark.mark_length_mm)
                                .range(6.0..=30.0)
                                .speed(0.5)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                        ui.label(RichText::new("espessura").size(11.5).color(th.muted));
                        ui.add(
                            egui::DragValue::new(&mut self.settings.regmark.mark_thickness_mm)
                                .range(0.3..=2.0)
                                .speed(0.1)
                                .fixed_decimals(1)
                                .suffix(" mm"),
                        );
                        ui.end_row();
                    });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("folha da impressão").size(11.5).color(th.muted));
                    ui.add(
                        egui::DragValue::new(&mut self.page_w)
                            .range(50.0..=610.0)
                            .speed(1.0)
                            .suffix(" mm"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.page_h)
                            .range(50.0..=3000.0)
                            .speed(1.0)
                            .suffix(" mm"),
                    );
                });
                ui.label(
                    RichText::new("arraste as marcas no papel: quadrado move a área · cantos ajustam largura/altura")
                        .small()
                        .color(th.muted),
                );
                ui.label(
                    RichText::new("⚠ imprima a folha em escala 100 %")
                        .small()
                        .color(th.warn),
                );
                let w = ui.available_width();
                ui.add_enabled_ui(self.has_doc(), |ui| {
                    if ui
                        .add_sized(
                            [w, 28.0],
                            egui::Button::new("🖨  Exportar folha de impressão…")
                                .fill(th.accent_soft)
                                .rounding(egui::Rounding::same(7.0)),
                        )
                        .clicked()
                    {
                        self.export_print_sheet();
                    }
                });
            });
        });
    }

    fn output_section(&mut self, ui: &mut egui::Ui) {
        let th = self.th;
        section(ui, &th, "📤", "Saída / teste", false, |ui| {
            let w = ui.available_width();
            ui.add_enabled_ui(self.has_doc(), |ui| {
                if ui
                    .add_sized(
                        [w, 26.0],
                        egui::Button::new(RichText::new("Gerar GPGL (dry-run, sem cortar)…").size(11.5))
                            .fill(th.bg_card)
                            .rounding(egui::Rounding::same(6.0)),
                    )
                    .clicked()
                {
                    self.export_dry_run();
                }
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.show_grid, "grade");
                ui.checkbox(&mut self.show_travel, "deslocamentos");
                ui.checkbox(&mut self.show_marks, "marcas no preview");
            });
            let mins = self.estimate_minutes();
            ui.label(
                RichText::new(format!(
                    "⏱ tempo estimado de corte: {}",
                    fmt_time(mins)
                ))
                .size(11.5)
                .color(th.muted),
            );
        });
    }
}

fn fmt_time(minutes: f32) -> String {
    if minutes < 1.0 {
        format!("{:.0} s", minutes * 60.0)
    } else {
        format!("{:.1} min", minutes)
    }
}


// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

impl App {
    /// Origem (px) do ponto (0,0) em mm: folha centrada + pan.
    fn view_origin(&self, canvas: Rect) -> Pos2 {
        let (pw, ph) = (
            self.settings.media_width_mm as f32,
            self.settings.media_height_mm as f32,
        );
        let base = Pos2::new(
            canvas.center().x - pw * self.zoom / 2.0,
            canvas.center().y - ph * self.zoom / 2.0,
        );
        base + self.pan
    }

    fn canvas(&mut self, ctx: &egui::Context) {
        let th = self.th;
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(th.bg_app))
            .show(ctx, |ui| {
                let (canvas, resp) = ui.allocate_exact_size(
                    ui.available_size_before_wrap(),
                    Sense::click_and_drag(),
                );
                let painter = ui.painter_at(canvas);

                if !self.fitted {
                    self.fit_view(canvas);
                }

                // pré-cálculo para o hit-test das marcas de registro
                let pre_origin = self.view_origin(canvas);
                let zoom0 = self.zoom;
                let r0 = self.settings.regmark;
                let near_corner = |pt: Pos2| -> Option<u8> {
                    let corners = [
                        (
                            0u8,
                            pre_origin.x + r0.origin_x_mm as f32 * zoom0,
                            pre_origin.y + r0.origin_y_mm as f32 * zoom0,
                        ),
                        (
                            1u8,
                            pre_origin.x + (r0.origin_x_mm + r0.width_mm) as f32 * zoom0,
                            pre_origin.y + r0.origin_y_mm as f32 * zoom0,
                        ),
                        (
                            2u8,
                            pre_origin.x + r0.origin_x_mm as f32 * zoom0,
                            pre_origin.y + (r0.origin_y_mm + r0.length_mm) as f32 * zoom0,
                        ),
                    ];
                    let mut best: Option<(f32, u8)> = None;
                    for (id, x, y) in corners {
                        let d = pt.distance(Pos2::new(x, y));
                        if d < 26.0 && best.map_or(true, |(bd, _)| d < bd) {
                            best = Some((d, id));
                        }
                    }
                    best.map(|(_, id)| id)
                };
                if self.settings.regmark.enabled {
                    if let Some(p) = ui.input(|i| i.pointer.hover_pos()) {
                        if near_corner(p).is_some() {
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                        }
                    }
                }
                if resp.drag_started() {
                    self.drag_corner = if self.settings.regmark.enabled {
                        ui.input(|i| i.pointer.interact_pos())
                            .and_then(near_corner)
                    } else {
                        None
                    };
                }
                if resp.dragged() {
                    match self.drag_corner {
                        Some(c) => {
                            let d = resp.drag_delta();
                            let (dx, dy) = (d.x as f64 / self.zoom as f64, d.y as f64 / self.zoom as f64);
                            let half = |v: f64| (v * 2.0).round() / 2.0;
                            match c {
                                0 => {
                                    self.settings.regmark.origin_x_mm =
                                        half((self.settings.regmark.origin_x_mm + dx).clamp(0.0, 600.0));
                                    self.settings.regmark.origin_y_mm =
                                        half((self.settings.regmark.origin_y_mm + dy).clamp(0.0, 3000.0));
                                }
                                1 => {
                                    self.settings.regmark.width_mm =
                                        half((self.settings.regmark.width_mm + dx).clamp(30.0, 600.0));
                                }
                                _ => {
                                    self.settings.regmark.length_mm =
                                        half((self.settings.regmark.length_mm + dy).clamp(30.0, 3000.0));
                                }
                            }
                            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                        }
                        None => self.pan += resp.drag_delta(),
                    }
                }
                if resp.drag_stopped() {
                    self.drag_corner = None;
                }
                // zoom pela roda, ancorado no cursor
                let scroll = ui.input(|i| i.raw_scroll_delta.y);
                if scroll != 0.0 && resp.hovered() {
                    let cursor =
                        ui.input(|i| i.pointer.hover_pos()).unwrap_or(canvas.center());
                    let origin = self.view_origin(canvas);
                    let mm = Pos2::new(
                        (cursor.x - origin.x) / self.zoom,
                        (cursor.y - origin.y) / self.zoom,
                    );
                    let factor = (1.0 + scroll * 0.0015).clamp(0.5, 2.0);
                    self.zoom = (self.zoom * factor).clamp(0.05, 40.0);
                    let (pw, ph) = (
                        self.settings.media_width_mm as f32,
                        self.settings.media_height_mm as f32,
                    );
                    let base = Pos2::new(
                        canvas.center().x - pw * self.zoom / 2.0,
                        canvas.center().y - ph * self.zoom / 2.0,
                    );
                    self.pan = Vec2::new(
                        cursor.x - mm.x * self.zoom - base.x,
                        cursor.y - mm.y * self.zoom - base.y,
                    );
                }
                if resp.double_clicked() {
                    self.fit_view(canvas);
                }

                let origin = self.view_origin(canvas);
                let zoom = self.zoom;
                let (pw, ph) = (
                    self.settings.media_width_mm as f32,
                    self.settings.media_height_mm as f32,
                );
                let to_screen = |x: f32, y: f32| {
                    Pos2::new(origin.x + x * zoom, origin.y + y * zoom)
                };
                let paper = Rect::from_min_max(to_screen(0.0, 0.0), to_screen(pw, ph));

                // sombra + papel
                painter.rect_filled(
                    paper.translate(Vec2::new(5.0, 6.0)),
                    egui::Rounding::same(2.0),
                    Color32::from_black_alpha(110),
                );
                painter.rect_filled(paper, egui::Rounding::same(2.0), th.paper);
                painter.rect_stroke(
                    paper,
                    egui::Rounding::same(2.0),
                    Stroke::new(1.0_f32, th.paper_edge),
                );

                // grade (dentro do papel)
                if self.show_grid && zoom > 0.9 {
                    let clip = paper.intersect(canvas);
                    let mut x = 0.0;
                    while x <= pw {
                        let sx = origin.x + x * zoom;
                        if sx >= clip.min.x && sx <= clip.max.x {
                            let major = (x as i32) % 50 == 0;
                            painter.line_segment(
                                [
                                    Pos2::new(sx, paper.min.y.max(clip.min.y)),
                                    Pos2::new(sx, paper.max.y.min(clip.max.y)),
                                ],
                                Stroke::new(
                                    if major { 1.0_f32 } else { 0.6_f32 },
                                    if major {
                                        th.grid_major
                                    } else {
                                        th.grid_minor
                                    },
                                ),
                            );
                        }
                        x += 10.0;
                    }
                    let mut y = 0.0;
                    while y <= ph {
                        let sy = origin.y + y * zoom;
                        if sy >= clip.min.y && sy <= clip.max.y {
                            let major = (y as i32) % 50 == 0;
                            painter.line_segment(
                                [
                                    Pos2::new(paper.min.x.max(clip.min.x), sy),
                                    Pos2::new(paper.max.x.min(clip.max.x), sy),
                                ],
                                Stroke::new(
                                    if major { 1.0_f32 } else { 0.6_f32 },
                                    if major {
                                        th.grid_major
                                    } else {
                                        th.grid_minor
                                    },
                                ),
                            );
                        }
                        y += 10.0;
                    }
                }

                // réguas: ticks na borda superior e esquerda, rótulos a cada 50 mm
                let tick_col = th.ruler;
                let mut x = 0.0;
                while x <= pw {
                    let sx = origin.x + x * zoom;
                    if sx >= canvas.min.x - 1.0 && sx <= canvas.max.x + 1.0 {
                        let major = (x as i32) % 50 == 0;
                        painter.line_segment(
                            [
                                Pos2::new(sx, paper.min.y - if major { 7.0 } else { 4.0 }),
                                Pos2::new(sx, paper.min.y),
                            ],
                            Stroke::new(1.0_f32, tick_col),
                        );
                        if major {
                            painter.text(
                                Pos2::new(sx, paper.min.y - 9.0),
                                Align2::CENTER_BOTTOM,
                                format!("{x:.0}"),
                                FontId::proportional(9.0),
                                th.muted,
                            );
                        }
                    }
                    x += 10.0;
                }
                let mut y = 0.0;
                while y <= ph {
                    let sy = origin.y + y * zoom;
                    if sy >= canvas.min.y - 1.0 && sy <= canvas.max.y + 1.0 {
                        let major = (y as i32) % 50 == 0;
                        painter.line_segment(
                            [
                                Pos2::new(paper.min.x - if major { 7.0 } else { 4.0 }, sy),
                                Pos2::new(paper.min.x, sy),
                            ],
                            Stroke::new(1.0_f32, tick_col),
                        );
                        if major {
                            painter.text(
                                Pos2::new(paper.min.x - 9.0, sy),
                                Align2::RIGHT_CENTER,
                                format!("{y:.0}"),
                                FontId::proportional(9.0),
                                th.muted,
                            );
                        }
                    }
                    y += 10.0;
                }

                // marcas de registro no papel (pretas, como impressas)
                if self.settings.regmark.enabled && self.show_marks {
                    let mark_color = Color32::from_rgb(0x08, 0x08, 0x08);
                    for pl in regmark::mark_paths(&self.settings.regmark) {
                        let mut pts: Vec<Pos2> = pl
                            .iter()
                            .map(|p| to_screen(p.x as f32, p.y as f32))
                            .collect();
                        let closed = pts.len() >= 4
                            && pts.first().map_or(false, |a| a.distance(*pts.last().unwrap()) < 0.5);
                        if closed {
                            pts.pop(); // remove o ponto repetido do fechamento
                            painter.add(Shape::convex_polygon(
                                pts,
                                mark_color,
                                Stroke::NONE,
                            ));
                        } else {
                            painter.add(Shape::line(
                                pts,
                                Stroke::new(1.6_f32, mark_color),
                            ));
                        }
                    }
                    let r = &self.settings.regmark;
                    let a = to_screen(r.origin_x_mm as f32, r.origin_y_mm as f32);
                    let b = to_screen(
                        (r.origin_x_mm + r.width_mm) as f32,
                        (r.origin_y_mm + r.length_mm) as f32,
                    );
                    painter.rect_stroke(
                        Rect::from_two_pos(a, b),
                        egui::Rounding::ZERO,
                        Stroke::new(1.0_f32, th.paper_edge),
                    );

                    // zona de exclusão das marcas (como a área hachurada do Studio)
                    let violating = self.keepout_violation();
                    let zone = if violating { th.err } else { th.warn };
                    for k in regmark::keepout_boxes(&self.settings.regmark) {
                        let zr = Rect::from_two_pos(
                            to_screen(k.min_x as f32, k.min_y as f32),
                            to_screen(k.max_x as f32, k.max_y as f32),
                        );
                        painter.rect_filled(zr, egui::Rounding::ZERO, zone.gamma_multiply(0.16));
                        painter.rect_stroke(
                            zr,
                            egui::Rounding::ZERO,
                            Stroke::new(1.0_f32, zone.gamma_multiply(0.8)),
                        );
                        painter.text(
                            zr.center(),
                            Align2::CENTER_CENTER,
                            "marca",
                            FontId::proportional(8.0),
                            zone,
                        );
                    }
                    if violating {
                        painter.text(
                            Pos2::new(paper.center().x, paper.min.y - 24.0),
                            Align2::CENTER_CENTER,
                            "⚠ o desenho invade a zona das marcas",
                            FontId::proportional(12.5),
                            th.err,
                        );
                    }
                }

                // desenho posicionado
                let paths = self.positioned_paths();

                // deslocamentos (ordem aproximada da máquina)
                if self.show_travel && paths.len() > 1 {
                    for w in paths.windows(2) {
                        if let (Some(a), Some(b)) = (w[0].last(), w[1].first()) {
                            painter.line_segment(
                                [
                                    to_screen(a.x as f32, a.y as f32),
                                    to_screen(b.x as f32, b.y as f32),
                                ],
                                Stroke::new(0.7_f32, th.travel),
                            );
                        }
                    }
                }

                // caminhos de corte
                let cut_w = (zoom * 0.7).clamp(1.0, 2.6) as f32;
                for pl in &paths {
                    if pl.len() < 2 {
                        continue;
                    }
                    let pts: Vec<Pos2> = pl
                        .iter()
                        .map(|p| to_screen(p.x as f32, p.y as f32))
                        .collect();
                    painter.add(Shape::line(pts, Stroke::new(cut_w, th.cut)));
                }

                // estado vazio
                if !self.has_doc() {
                    painter.rect_stroke(
                        paper,
                        egui::Rounding::same(2.0),
                        Stroke::new(1.5_f32, th.accent),
                    );
                    painter.text(
                        canvas.center(),
                        Align2::CENTER_CENTER,
                        "arraste um SVG (ou imagem) para cá\nou use “Abrir SVG…”",
                        FontId::proportional(15.0),
                        th.accent,
                    );
                }

                // controles de zoom (canto superior direito)
                let badges = [
                    ("⤢", "fit"),
                    ("+", "in"),
                    ("−", "out"),
                ];
                let mut bx = canvas.max.x - 12.0 - 26.0 * badges.len() as f32;
                for (label, id) in badges {
                    let r = Rect::from_min_size(Pos2::new(bx, canvas.min.y + 10.0), Vec2::splat(24.0));
                    let resp = ui.interact(r, ui.id().with(("zoom", id)), Sense::click());
                    painter.rect_filled(
                        r,
                        egui::Rounding::same(7.0),
                        if resp.hovered() { th.accent_soft } else { th.bg_card },
                    );
                    painter.rect_stroke(
                        r,
                        egui::Rounding::same(7.0),
                        Stroke::new(1.0_f32, th.border),
                    );
                    painter.text(
                        r.center(),
                        Align2::CENTER_CENTER,
                        label,
                        FontId::proportional(14.0),
                        th.text,
                    );
                    if resp.clicked() {
                        match id {
                            "fit" => self.fit_view(canvas),
                            "in" => self.zoom = (self.zoom * 1.3).min(40.0),
                            _ => self.zoom = (self.zoom / 1.3).max(0.05),
                        }
                    }
                    bx += 26.0;
                }

                // dica + posição do cursor
                painter.text(
                    Pos2::new(canvas.min.x + 12.0, canvas.max.y - 14.0),
                    Align2::LEFT_CENTER,
                    "roda: zoom · arrastar: mover · duplo clique: ajustar",
                    FontId::proportional(11.0),
                    th.hint,
                );
                painter.text(
                    Pos2::new(canvas.max.x - 12.0, canvas.max.y - 14.0),
                    Align2::RIGHT_CENTER,
                    format!("zoom {:.0} %", zoom * 100.0 / 4.0),
                    FontId::proportional(11.0),
                    th.muted,
                );
                if let Some(pos) = ui.input(|i| i.pointer.hover_pos()) {
                    if paper.contains(pos) {
                        let mmx = (pos.x - origin.x) / zoom;
                        let mmy = (pos.y - origin.y) / zoom;
                        painter.text(
                            Pos2::new(canvas.max.x - 12.0, canvas.max.y - 30.0),
                            Align2::RIGHT_CENTER,
                            format!("x {mmx:.1} · y {mmy:.1} mm"),
                            FontId::proportional(11.0),
                            th.muted,
                        );
                    }
                }
            });
    }

    fn status_bar(&mut self, ctx: &egui::Context) {
        let th = self.th;
        egui::TopBottomPanel::bottom("status")
            .frame(
                egui::Frame::none()
                    .fill(th.bg_panel)
                    .inner_margin(egui::Margin::symmetric(12.0, 6.0)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let busy;
                    let progress;
                    {
                        let sh = self.shared.lock().unwrap();
                        busy = sh.busy;
                        progress = sh.progress;
                    }
                    if busy {
                        ui.add(
                            egui::ProgressBar::new(progress.clamp(0.02, 1.0))
                                .desired_width(220.0)
                                .text(format!("{:.0} %", progress * 100.0)),
                        );
                        ui.label(RichText::new(&self.device_status).color(th.text));
                    } else if !self.last_error.is_empty() {
                        ui.label(RichText::new(format!("⚠  {}", self.last_error)).color(th.err));
                    } else if self.keepout_violation() {
                        ui.label(
                            RichText::new(
                                "⚠ o desenho invade a zona das marcas — afaste ou diminua",
                            )
                            .color(th.warn),
                        );
                    } else if !self.last_report.is_empty() {
                        ui.label(RichText::new(&self.last_report).color(th.ok));
                    } else {
                        ui.label(RichText::new(format!("● {}", self.device_status)).color(th.muted));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.has_doc() {
                            let mins = self.estimate_minutes();
                            ui.label(
                                RichText::new(format!("⏱ {}", fmt_time(mins))).color(th.muted),
                            );
                            ui.separator();
                            ui.label(
                                RichText::new(format!(
                                    "força {} · vel {} · {}",
                                    self.settings.pressure,
                                    self.settings.speed,
                                    device::media_by_code(self.settings.media)
                                        .map(|m| m.name)
                                        .unwrap_or("—")
                                ))
                                .color(th.muted),
                            );
                        }
                    });
                });
            });
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let th = self.th;
        if !self.confirm_cut {
            return;
        }
        let mut open = true;
        let warnings = if self.has_doc() {
            cutjob::validate(
                &self.settings,
                self.device_spec(),
                bbox_of(&self.positioned_paths()),
            )
        } else {
            Vec::new()
        };
        egui::Window::new("Confirmar corte")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(430.0);
                let (w, h) = self.eff_bbox();
                egui::Grid::new("confirm")
                    .num_columns(2)
                    .spacing([14.0, 5.0])
                    .show(ui, |ui| {
                        let row = |ui: &mut egui::Ui, k: &str, v: String| {
                            ui.label(RichText::new(k).color(th.muted).size(12.0));
                            ui.label(RichText::new(v).size(12.5));
                            ui.end_row();
                        };
                        row(
                            ui,
                            "Máquina",
                            self.devices
                                .get(self.selected_device.unwrap_or(usize::MAX))
                                .map(|d| d.display_name())
                                .unwrap_or_else(|| self.device_spec().name.to_string()),
                        );
                        row(
                            ui,
                            "Folha",
                            format!(
                                "{:.0} × {:.0} mm · {}",
                                self.settings.media_width_mm,
                                self.settings.media_height_mm,
                                device::media_by_code(self.settings.media)
                                    .map(|m| m.name)
                                    .unwrap_or("Personalizado")
                            ),
                        );
                        row(
                            ui,
                            "Força / velocidade",
                            format!("{} / {}", self.settings.pressure, self.settings.speed),
                        );
                        if let Some(d) = self.settings.depth {
                            row(ui, "Profundidade", format!("{d}"));
                        }
                        row(
                            ui,
                            "Desenho",
                            format!("{} caminhos · {:.1} × {:.1} mm", self.raw.len(), w, h),
                        );
                        row(
                            ui,
                            "Posição",
                            format!(
                                "x {:.1} · y {:.1} mm",
                                self.settings.offset_x_mm, self.settings.offset_y_mm
                            ),
                        );
                        if self.settings.regmark.enabled {
                            row(
                                ui,
                                "Marcas",
                                format!(
                                    "{:.0} × {:.0} mm · {}",
                                    self.settings.regmark.width_mm,
                                    self.settings.regmark.length_mm,
                                    if self.settings.regmark.search {
                                        "busca automática"
                                    } else {
                                        "manual"
                                    }
                                ),
                            );
                        }
                        row(ui, "Tempo estimado", fmt_time(self.estimate_minutes()));
                    });
                if self.keepout_violation() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("⚠  o desenho invade a zona das marcas — afaste-o")
                            .color(th.err)
                            .size(12.0),
                    );
                }
                if !warnings.is_empty() {
                    ui.add_space(4.0);
                    for wmsg in &warnings {
                        ui.label(RichText::new(format!("⚠  {wmsg}")).color(th.warn).size(12.0));
                    }
                }
                ui.add_space(6.0);
                ui.separator();
                ui.label(
                    RichText::new("Confira: mídia carregada, lâmina instalada e área livre.")
                        .size(11.5)
                        .color(th.muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("  Cortar agora  ")
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(th.red_btn)
                            .rounding(egui::Rounding::same(8.0)),
                        )
                        .clicked()
                    {
                        self.confirm_cut = false;
                        self.start_cut();
                    }
                    if ui.button("Cancelar").clicked() {
                        self.confirm_cut = false;
                    }
                });
            });
        if !open {
            self.confirm_cut = false;
        }
    }
}
