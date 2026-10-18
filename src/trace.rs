//! Vetorização de bitmap (PNG/JPEG) via `potrace` — processo externo.
//!
//! Estratégia: decodifica com o crate `image`, binariza por limiar,
//! escreve um PGM temporário e invoca `potrace -s` (saída SVG).
//! O SVG resultante pode ser aberto/cortado pelo silcut normalmente.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Result, SilError};

/// Localiza o executável potrace: configuração explícita > PATH.
pub fn find_potrace(configured: &str) -> Option<PathBuf> {
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        if p.is_file() {
            return Some(p);
        }
    }
    let path_var = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path_var) {
        let cand = dir.join("potrace");
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

pub fn potrace_available(configured: &str) -> bool {
    find_potrace(configured).is_some()
}

/// Opções de vetorização (equivalente aos ajustes do Studio).
#[derive(Debug, Clone, Copy)]
pub struct TraceOptions {
    /// Limiar de binarização 0–255 (pixels < limiar ficam pretos).
    pub threshold: u8,
    /// Inverte a imagem (traçar o claro em vez do escuro).
    pub invert: bool,
    /// Descarta manchas menores que N pixels (ruído de varredura).
    pub turd: u32,
    /// Suavização de cantos 0–1,34 (0 = quinas vivas, 1,34 = máxima).
    pub alphamax: f32,
    /// Tolerância de otimização da curva (0 = fiel, 1 = simplificada).
    pub opt_tolerance: f32,
}

impl Default for TraceOptions {
    fn default() -> Self {
        TraceOptions {
            threshold: 128,
            invert: false,
            turd: 2,
            alphamax: 1.0,
            opt_tolerance: 0.2,
        }
    }
}

/// Vetoriza `input` (PNG/JPEG/BMP) escrevendo SVG em `output`.
pub fn trace_image(
    input: &Path,
    output: &Path,
    opts: &TraceOptions,
    potrace_exe: &str,
) -> Result<()> {
    let exe = find_potrace(potrace_exe).ok_or_else(|| {
        SilError::Trace(
            "potrace não encontrado. Instale com: sudo apt install potrace (ou defina potrace_path na configuração)".into(),
        )
    })?;

    // Decodifica e binariza
    let img = image::open(input).map_err(|e| SilError::Trace(format!("abrir imagem: {e}")))?;
    let gray = img.to_luma8();
    let (w, h) = (gray.width(), gray.height());

    // PGM binário (P5)
    let mut pgm: Vec<u8> = Vec::with_capacity((w * h) as usize + 64);
    let _ = write!(pgm, "P5\n{w} {h}\n255\n");
    for px in gray.pixels() {
        // potrace traça pixels ESCUROS como sólidos: 1 = preto
        let dark = (px[0] < opts.threshold) ^ opts.invert;
        pgm.push(if dark { 0 } else { 255 });
    }

    // PGM temporário
    let tmp_dir = std::env::temp_dir();
    let tmp_in = tmp_dir.join(format!("silcut_{}.pgm", std::process::id()));
    std::fs::write(&tmp_in, &pgm)?;

    // potrace: -s SVG, -t turdsize, -a suavização de canto, -O otimização de curva
    let status = Command::new(&exe)
        .arg("-s")
        .arg("-t")
        .arg(opts.turd.to_string())
        .arg("-a")
        .arg(format!("{:.2}", opts.alphamax.clamp(0.0, 1.34)))
        .arg("-O")
        .arg(format!("{:.3}", opts.opt_tolerance.clamp(0.0, 1.0)))
        .arg("-o")
        .arg(output)
        .arg(&tmp_in)
        .status();

    let _ = std::fs::remove_file(&tmp_in);

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(SilError::Trace(format!(
            "potrace terminou com status {s}"
        ))),
        Err(e) => Err(SilError::Trace(format!("executar potrace: {e}"))),
    }
}
