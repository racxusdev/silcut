//! # silcut
//!
//! Ferramenta nativa Linux para cortadores Silhouette/Graphtec.
//!
//! Arquitetura em camadas:
//!
//! * [`device`]  — tabela de hardware (VID/PID, dimensões, famílias, mídias, mats).
//! * [`driver`]  — transporte USB (libusb) + comandos de nível de máquina.
//! * [`gpgl`]    — codificação GPGL (Graphtec Plotter Graphics Language, 1 SU = 0,05 mm).
//! * [`cutjob`]  — orquestração: polilinhas (mm) + configurações → sequência completa → corte.
//! * [`geom`]    — pontos, matrizes de transformação e achatamento de curvas.
//! * [`svg`]     — importação SVG (caminhos, formas, transformações) → polilinhas em mm.
//! * [`offset`]  — contorno paralelo (borda branca de adesivos).
//! * [`boolean`] — união de contornos sobrepostos ("fundir").
//! * [`dxf`]     — importação DXF (LINE/LWPOLYLINE/ARC/CIRCLE…).
//! * [`optimize`]— reordenação dos percursos para reduzir deslocamentos.
//! * [`trace`]   — vetorização de bitmap via potrace (processo externo).
//! * [`config`]  — configuração persistente TOML em `~/.config/silcut`.
//! * [`error`]   — tipos de erro unificados.

pub mod boolean;
pub mod config;
pub mod cutjob;
pub mod dxf;
pub mod device;
pub mod driver;
pub mod error;
pub mod geom;
pub mod gpgl;
pub mod offset;
pub mod optimize;
pub mod regmark;
pub mod svg;
pub mod trace;

pub const APP_NAME: &str = "silcut";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
