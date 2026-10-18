//! Tabela de hardware Silhouette/Graphtec — IDs USB, dimensões, famílias de
//! protocolo, mídias e tapetes de corte.
//!
//! Fonte dos fatos: protocolo GPGL documentado publicamente pela comunidade
//! (fablabnbg/inkscape-silhouette `Commands.md` + `Graphtec.py`, GPLv2+),
//! reimplementado originalmente em Rust.

/// Família de protocolo — muda a sequência de comandos de configuração.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Cameo 3 e mais novos (Cameo 4/5, Portrait 3/4): suporta mats `TG`,
    /// comandos de ferramenta com sufixo `,n`, aceleração `TJ`.
    Cameo3On,
    /// Modelos antigos (Cameo 1/2, Portrait 1/2, SD, Craft Robo): comandos
    /// simples `!n`/`FXn`, sem mat.
    Legacy,
}

/// Especificação de um modelo de cortador.
#[derive(Debug, Clone, Copy)]
pub struct DeviceSpec {
    pub vid: u16,
    pub pid: u16,
    pub name: &'static str,
    /// Largura útil de corte em mm (eixo lateral).
    pub width_mm: f64,
    /// Comprimento máximo de mídia em mm (eixo de avanço).
    pub length_mm: f64,
    /// Margem esquerda física (mm) quando conhecida.
    pub margin_left_mm: f64,
    /// Margem superior de segurança (mm) quando conhecida.
    pub margin_top_mm: f64,
    pub family: Family,
    /// Pressão máxima aceita (Cameo 5 Alpha: 40; demais: 33).
    pub max_pressure: u8,
    /// Suporta seleção de tapete (`TG`).
    pub mat_support: bool,
    /// Suporta marcas de registro ópticas.
    pub regmark: bool,
}

pub const VID_GRAPHTEC: u16 = 0x0b4d;
pub const VID_CAMEO5_ALPHA: u16 = 0x3844;

/// Todos os modelos conhecidos (dados da comunidade, verificados por captura USB).
pub const DEVICES: &[DeviceSpec] = &[
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1123, name: "Silhouette Portrait", width_mm: 206.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1132, name: "Silhouette Portrait 2", width_mm: 203.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x113a, name: "Silhouette Portrait 3", width_mm: 203.0, length_mm: 18290.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x113f, name: "Silhouette Portrait 4", width_mm: 216.0, length_mm: 18290.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1121, name: "Silhouette Cameo", width_mm: 304.0, length_mm: 3000.0, margin_left_mm: 9.0, margin_top_mm: 1.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x112b, name: "Silhouette Cameo 2", width_mm: 304.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x112f, name: "Silhouette Cameo 3", width_mm: 304.8, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1137, name: "Silhouette Cameo 4", width_mm: 304.8, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1138, name: "Silhouette Cameo 4 Plus", width_mm: 372.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1139, name: "Silhouette Cameo 4 Pro", width_mm: 600.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1140, name: "Silhouette Cameo 5", width_mm: 330.2, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1141, name: "Silhouette Cameo 5 Plus", width_mm: 372.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x1146, name: "Silhouette Cameo Pro Mk II", width_mm: 609.0, length_mm: 18290.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 33, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_CAMEO5_ALPHA, pid: 0x0001, name: "Silhouette Cameo 5 Alpha", width_mm: 330.2, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 40, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_CAMEO5_ALPHA, pid: 0x0002, name: "Silhouette Cameo 5 Alpha Plus", width_mm: 372.0, length_mm: 3000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Cameo3On, max_pressure: 40, mat_support: true, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x110a, name: "Craft Robo CC200-20", width_mm: 200.0, length_mm: 1000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x111a, name: "Craft Robo CC300-20", width_mm: 200.0, length_mm: 1000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x111c, name: "Silhouette SD 1", width_mm: 203.0, length_mm: 1000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
    DeviceSpec { vid: VID_GRAPHTEC, pid: 0x111d, name: "Silhouette SD 2", width_mm: 203.0, length_mm: 1000.0, margin_left_mm: 0.0, margin_top_mm: 0.0, family: Family::Legacy, max_pressure: 33, mat_support: false, regmark: true },
];

/// Pré-ajuste de mídia: código FW, pressão/velocidade/profundidade padrão.
#[derive(Debug, Clone, Copy)]
pub struct MediaPreset {
    pub code: u16,
    pub name: &'static str,
    pub pressure: Option<u8>,
    pub speed: Option<u8>,
    pub depth: Option<u8>,
}

/// Tabela de mídias (portada da tabela verificada da comunidade).
pub const MEDIA: &[MediaPreset] = &[
    MediaPreset { code: 300, name: "Personalizado", pressure: None, speed: None, depth: None },
    MediaPreset { code: 100, name: "Cartão sem backing", pressure: Some(27), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 101, name: "Cartão com backing", pressure: Some(27), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 102, name: "Adesivo vinílico", pressure: Some(10), speed: Some(5), depth: Some(1) },
    MediaPreset { code: 106, name: "Etiqueta filme", pressure: Some(14), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 111, name: "Mídia espessa", pressure: Some(27), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 112, name: "Mídia fina", pressure: Some(2), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 113, name: "Caneta (pen)", pressure: Some(18), speed: Some(10), depth: None },
    MediaPreset { code: 120, name: "Papel bond 105 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 121, name: "Bristol 145 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 122, name: "Cardstock 90 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 123, name: "Cover 170 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 124, name: "Filme translúcido", pressure: Some(1), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 125, name: "Filme vinílico adesivo", pressure: Some(1), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 126, name: "Filme janela", pressure: Some(1), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 127, name: "Index 165 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 128, name: "Fotográfico 70 g", pressure: Some(20), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 129, name: "Fotográfico 110 g", pressure: Some(27), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 130, name: "Folha magnética", pressure: Some(30), speed: Some(3), depth: Some(1) },
    MediaPreset { code: 131, name: "Offset 90 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 132, name: "Papel impressão leve", pressure: Some(5), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 133, name: "Papel impressão médio", pressure: Some(25), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 134, name: "Folha de adesivos", pressure: Some(20), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 135, name: "Tag 275 g", pressure: Some(20), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 136, name: "Papel texto 105 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 137, name: "Vellum Bristol 145 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
    MediaPreset { code: 138, name: "Papel escrita 105 g", pressure: Some(30), speed: Some(10), depth: Some(1) },
];

/// Tapete de corte (`TG`), apenas famílias Cameo3On.
#[derive(Debug, Clone, Copy)]
pub struct MatSpec {
    pub code: u8,
    pub name: &'static str,
    /// Dimensões em polegadas: (comprimento/avanço Y, largura lateral X).
    pub feed_inch: f64,
    pub width_inch: f64,
}

pub const MATS: &[MatSpec] = &[
    MatSpec { code: 0, name: "Sem tapete", feed_inch: 0.0, width_inch: 0.0 },
    MatSpec { code: 1, name: "Cameo 12x12", feed_inch: 12.0, width_inch: 12.0 },
    MatSpec { code: 2, name: "Cameo 12x24", feed_inch: 24.0, width_inch: 12.0 },
    MatSpec { code: 3, name: "Portrait 8x12", feed_inch: 12.0, width_inch: 8.0 },
    MatSpec { code: 8, name: "Cameo Plus 15x15", feed_inch: 15.0, width_inch: 15.0 },
    MatSpec { code: 9, name: "Cameo Pro 24x24", feed_inch: 24.0, width_inch: 24.0 },
    MatSpec { code: 11, name: "Portrait 8.5x12", feed_inch: 12.0, width_inch: 8.5 },
];

pub fn find_by_ids(vid: u16, pid: u16) -> Option<&'static DeviceSpec> {
    DEVICES.iter().find(|d| d.vid == vid && d.pid == pid)
}

pub fn media_by_code(code: u16) -> Option<&'static MediaPreset> {
    MEDIA.iter().find(|m| m.code == code)
}

pub fn mat_by_code(code: u8) -> Option<&'static MatSpec> {
    MATS.iter().find(|m| m.code == code)
}
