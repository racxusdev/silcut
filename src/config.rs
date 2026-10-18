//! Configuração persistente em TOML (`~/.config/silcut/config.toml`) —
//! equivalente nativo Linux do Registro do Windows.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::cutjob::CutSettings;
use crate::error::{Result, SilError};

/// Configuração completa do aplicativo.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Padrões de corte usados pela GUI/CLI.
    pub default: CutSettings,
    /// Último dispositivo usado (vid, pid) para pré-seleção.
    pub last_device: Option<(u16, u16)>,
    /// Caminho do executável potrace (vazio = procurar no PATH).
    pub potrace_path: String,
    /// Tema escuro (true) ou claro (false).
    pub dark: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            default: CutSettings::default(),
            last_device: None,
            potrace_path: String::new(),
            dark: true,
        }
    }
}

/// Caminho do arquivo de configuração respeitando XDG_CONFIG_HOME.
pub fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("silcut").join("config.toml")
}

/// Carrega a configuração; se inexistente ou inválida, devolve o padrão.
pub fn load() -> AppConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str(&text) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("aviso: config inválida em {} ({e}); usando padrões", path.display());
                AppConfig::default()
            }
        },
        Err(_) => AppConfig::default(),
    }
}

/// Persiste a configuração no disco.
pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(cfg)
        .map_err(|e| SilError::Config(format!("serialização TOML: {e}")))?;
    std::fs::write(&path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_toml() {
        let cfg = AppConfig::default();
        let text = toml::to_string_pretty(&cfg).unwrap();
        let back: AppConfig = toml::from_str(&text).unwrap();
        assert_eq!(back.default.speed, cfg.default.speed);
        assert_eq!(back.default.pressure, cfg.default.pressure);
    }
}
