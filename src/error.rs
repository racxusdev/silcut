//! Tipos de erro unificados do silcut.

use thiserror::Error;

/// Erros de alto nível da biblioteca.
#[derive(Debug, Error)]
pub enum SilError {
    /// Nenhum cortador compatível conectado.
    #[error("nenhum cortador Silhouette/Graphtec encontrado — verifique o cabo USB e a energia da máquina")]
    DeviceNotFound,

    /// Dispositivo encontrado, mas o acesso foi negado (permissão udev).
    #[error("acesso negado ao dispositivo USB (permissão). Instale a regra udev 70-silhouette.rules ou execute como root. Detalhe: {0}")]
    AccessDenied(String),

    /// Erro do transporte USB.
    #[error("erro de USB: {0}")]
    Usb(#[from] rusb::Error),

    /// Tempo limite ao conversar com o cortador.
    #[error("tempo limite esgotado esperando resposta do cortador ({0}s). Verifique se a mídia está carregada e se a máquina responde")]
    Timeout(u64),

    /// Resposta inesperada do protocolo GPGL.
    #[error("resposta inesperada do cortador: {0}")]
    Protocol(String),

    /// Erro ao interpretar arquivo SVG.
    #[error("SVG inválido: {0}")]
    Svg(String),

    /// O SVG contém texto (precisa ser convertido em caminhos).
    #[error("o SVG contém elementos <text>. Converta o texto em caminhos (no Inkscape: Caminho > Objeto em Caminho) e salve como 'SVG simples'")]
    SvgHasText,

    /// Erro de geometria (ex.: arco inválido).
    #[error("erro de geometria: {0}")]
    Geometry(String),

    /// Ferramenta externa (potrace) indisponível ou com falha.
    #[error("potrace: {0}")]
    Trace(String),

    /// Erro de E/S genérico.
    #[error("erro de E/S: {0}")]
    Io(#[from] std::io::Error),

    /// Configuração persistente inválida.
    #[error("configuração inválida: {0}")]
    Config(String),
}

/// Resultado padrão da biblioteca.
pub type Result<T> = std::result::Result<T, SilError>;
