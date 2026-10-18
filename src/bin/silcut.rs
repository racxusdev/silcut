//! `silcut` — CLI da ferramenta nativa Linux para cortadores Silhouette/Graphtec.
//!
//! Exemplos:
//!   silcut devices
//!   silcut info
//!   silcut cut desenho.svg --media 102 --dry-run -o corte.gpgl
//!   silcut cut folha.svg --regmarks --reg-width 180 --reg-length 230
//!   silcut trace foto.png vetor.svg
//!   silcut marks -o folha-marcas.svg

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};

use silcut::config::{self, AppConfig};
use silcut::cutjob::{self, CutSettings};
use silcut::device;
use silcut::driver::{self, Cutter};
use silcut::regmark;
use silcut::trace;

#[derive(Parser)]
#[command(
    name = "silcut",
    version,
    about = "Corte Silhouette/Graphtec nativo no Linux (GPGL, USB direto, sem Wine)",
    after_help = "Configuração persistente: ~/.config/silcut/config.toml"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Lista cortadores conectados.
    Devices,
    /// Status, firmware e limites do cortador.
    Info,
    /// Corta um arquivo SVG.
    Cut {
        /// Arquivo SVG de entrada.
        file: PathBuf,
        /// Velocidade 1–30 (padrão: mídia ou 10).
        #[arg(long)]
        speed: Option<u8>,
        /// Pressão 1–33 (padrão: mídia ou 10).
        #[arg(long)]
        pressure: Option<u8>,
        /// Código de mídia (silcut media para listar… use `media` abaixo).
        #[arg(long)]
        media: Option<u16>,
        /// Código do tapete (0 sem, 1 12x12, 2 12x24, 3 8x12, 8 15x15, 9 24x24, 11 8.5x12).
        #[arg(long)]
        mat: Option<u8>,
        /// Deslocamento X em mm.
        #[arg(long, default_value_t = 15.0)]
        offset_x: f64,
        /// Deslocamento Y em mm.
        #[arg(long, default_value_t = 15.0)]
        offset_y: f64,
        /// Largura da mídia em mm.
        #[arg(long, default_value_t = 210.0)]
        media_width: f64,
        /// Comprimento da mídia em mm.
        #[arg(long, default_value_t = 297.0)]
        media_height: f64,
        /// Ativa marcas de registro (print & cut).
        #[arg(long)]
        regmarks: bool,
        /// Busca automática das marcas pelo sensor.
        #[arg(long, default_value_t = true)]
        reg_search: bool,
        /// Largura da área de marcas (mm).
        #[arg(long, default_value_t = 180.0)]
        reg_width: f64,
        /// Altura da área de marcas (mm).
        #[arg(long, default_value_t = 230.0)]
        reg_length: f64,
        /// X da marca superior esquerda (mm).
        #[arg(long, default_value_t = 15.0)]
        reg_origin_x: f64,
        /// Y da marca superior esquerda (mm).
        #[arg(long, default_value_t = 20.0)]
        reg_origin_y: f64,
        /// Não corta: grava o programa GPGL (útil para inspeção).
        #[arg(long)]
        dry_run: bool,
        /// Arquivo de saída do dry-run.
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Não reordenar caminhos.
        #[arg(long)]
        no_optimize: bool,
        /// Número de passagens sobre o mesmo traço (1–6).
        #[arg(long, default_value_t = 1)]
        passes: u8,
        /// Comprimento da marca de registro em mm (6–30, padrão 15).
        #[arg(long, default_value_t = 15.0)]
        reg_mark_length: f64,
        /// Espessura do traço da marca em mm (padrão 0,5; 1,0 se falhar leitura).
        #[arg(long, default_value_t = 0.5)]
        reg_mark_thickness: f64,
        /// Tolerância de achatamento em mm (padrão 0,05).
        #[arg(long)]
        tolerance: Option<f64>,
    },
    /// Vetoriza um bitmap (PNG/JPG/BMP) para SVG usando potrace.
    Trace {
        input: PathBuf,
        output: PathBuf,
        /// Limiar de binarização 0–255.
        #[arg(long, default_value_t = 128)]
        threshold: u8,
        /// Inverte a imagem (traça o claro em vez do escuro).
        #[arg(long)]
        invert: bool,
        /// Descarta manchas menores que N pixels.
        #[arg(long, default_value_t = 2)]
        turd: u32,
        /// Suavização de cantos 0–1.34 (0 = quinas vivas).
        #[arg(long, default_value_t = 1.0)]
        alpha: f32,
        /// Otimização da curva 0–1 (maior = mais simples).
        #[arg(long, default_value_t = 0.2)]
        opt_tolerance: f32,
    },
    /// Gera folha de impressão com marcas de registro (SVG).
    Marks {
        /// Arquivo SVG de saída.
        #[arg(short, long)]
        out: PathBuf,
        /// SVG com o desenho a imprimir junto (opcional).
        #[arg(long)]
        design: Option<PathBuf>,
        /// Largura da página (mm).
        #[arg(long, default_value_t = 210.0)]
        page_width: f64,
        /// Altura da página (mm).
        #[arg(long, default_value_t = 297.0)]
        page_height: f64,
        /// Largura da área de marcas (mm).
        #[arg(long, default_value_t = 180.0)]
        reg_width: f64,
        /// Altura da área de marcas (mm).
        #[arg(long, default_value_t = 230.0)]
        reg_length: f64,
        /// X da marca superior esquerda (mm).
        #[arg(long, default_value_t = 15.0)]
        reg_origin_x: f64,
        /// Y da marca superior esquerda (mm).
        #[arg(long, default_value_t = 20.0)]
        reg_origin_y: f64,
        /// Comprimento da marca em mm (6–30, padrão 15).
        #[arg(long, default_value_t = 15.0)]
        reg_mark_length: f64,
        /// Espessura do traço da marca em mm (padrão 0,5).
        #[arg(long, default_value_t = 0.5)]
        reg_mark_thickness: f64,
    },
    /// Lista códigos de mídia com pré-ajustes.
    Media,
    /// Configuração persistente.
    Config {
        #[command(subcommand)]
        cmd: Option<ConfigCmd>,
    },
}

#[derive(Subcommand)]
enum ConfigCmd {
    /// Mostra o conteúdo atual.
    Show,
    /// Cria arquivo padrão se não existir.
    Init,
    /// Mostra o caminho do arquivo.
    Path,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli.cmd) {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cmd) -> Result<()> {
    let cfg: AppConfig = config::load();
    match cli {
        Cmd::Devices => {
            let devs = driver::list_devices().context("acesso ao barramento USB")?;
            if devs.is_empty() {
                println!("Nenhum cortador encontrado (vendor 0b4d/3844).");
                println!("Verifique USB/energia; se a máquina estiver ligada,");
                println!("instale a regra udev: sudo cp 70-silhouette.rules /etc/udev/rules.d/");
                return Ok(());
            }
            for d in &devs {
                println!("{:04x}:{:04x} bus {:03} addr {:03} — {}", d.vid, d.pid, d.bus, d.address, d.display_name());
            }
        }
        Cmd::Info => {
            let mut c = open_cutter()?;
            let v = c.initialize().context("inicialização do cortador")?;
            println!("Modelo:    {}", c.spec.name);
            println!("Firmware:  {v}");
            println!("Local:     {}", c.location);
            println!(
                "Área:      {:.0} x {} mm (margem esq {:.1} mm)",
                c.spec.width_mm, c.spec.length_mm, c.spec.margin_left_mm
            );
            let st = c.status()?;
            println!("Status:    {}", st.as_str());
        }
        Cmd::Media => {
            println!("{:<6} {:<8} {:<8} {:<7} nome", "código", "pressão", "velocid.", "profund");
            for m in device::MEDIA {
                println!(
                    "{:<6} {:<8} {:<8} {:<7} {}",
                    m.code,
                    m.pressure.map(|v| v.to_string()).unwrap_or("-".into()),
                    m.speed.map(|v| v.to_string()).unwrap_or("-".into()),
                    m.depth.map(|v| v.to_string()).unwrap_or("-".into()),
                    m.name
                );
            }
        }
        Cmd::Trace { input, output, threshold, invert, turd, alpha, opt_tolerance } => {
            let opts = trace::TraceOptions {
                threshold,
                invert,
                turd,
                alphamax: alpha,
                opt_tolerance,
            };
            trace::trace_image(&input, &output, &opts, &cfg.potrace_path)
                .context("vetorização")?;
            println!("SVG gerado: {}", output.display());
        }
        Cmd::Marks { out, design, page_width, page_height, reg_width, reg_length, reg_origin_x, reg_origin_y, reg_mark_length, reg_mark_thickness } => {
            let r = silcut::cutjob::RegmarkSettings {
                enabled: true,
                search: true,
                width_mm: reg_width,
                length_mm: reg_length,
                origin_x_mm: reg_origin_x,
                origin_y_mm: reg_origin_y,
                mark_length_mm: reg_mark_length,
                mark_thickness_mm: reg_mark_thickness,
            };
            let paths = match design {
                Some(d) => cutjob::load_file(&d, 0.05)
                    .with_context(|| format!("carregar {}", d.display()))?
                    .polylines,
                None => Vec::new(),
            };
            let svg = regmark::print_sheet_svg(&paths, &r, page_width, page_height);
            std::fs::write(&out, svg).context("escrever folha")?;
            println!("Folha de impressão gerada: {}", out.display());
            println!("Imprima em escala 100% (sem 'ajustar à página').");
        }
        Cmd::Config { cmd } => match cmd.unwrap_or(ConfigCmd::Show) {
            ConfigCmd::Show => {
                println!("# {}", config::config_path().display());
                println!("{}", toml::to_string_pretty(&cfg)?);
            }
            ConfigCmd::Init => {
                let path = config::config_path();
                if path.exists() {
                    println!("já existe: {}", path.display());
                } else {
                    config::save(&AppConfig::default())?;
                    println!("criado: {}", path.display());
                }
            }
            ConfigCmd::Path => println!("{}", config::config_path().display()),
        },
        Cmd::Cut {
            file,
            speed,
            pressure,
            media,
            mat,
            offset_x,
            offset_y,
            media_width,
            media_height,
            regmarks,
            reg_search,
            reg_width,
            reg_length,
            reg_origin_x,
            reg_origin_y,
            dry_run,
            out,
            no_optimize,
            tolerance,
            passes,
            reg_mark_length,
            reg_mark_thickness,
        } => {
            let mut st: CutSettings = cfg.default;
            st.offset_x_mm = offset_x;
            st.offset_y_mm = offset_y;
            st.media_width_mm = media_width;
            st.media_height_mm = media_height;
            st.optimize_order = !no_optimize;
            if let Some(t) = tolerance {
                st.tolerance_mm = t.max(0.005);
            }
            if let Some(m) = media {
                st.media = m;
            }
            if let Some(code) = mat {
                st.mat = code;
            }
            // Pré-ajustes da mídia quando o usuário não definiu velocidade/pressão explícitas.
            if let Some(mp) = device::media_by_code(st.media) {
                if speed.is_none() {
                    if let Some(s) = mp.speed {
                        st.speed = s;
                    }
                }
                if pressure.is_none() {
                    if let Some(p) = mp.pressure {
                        st.pressure = p;
                    }
                }
                if speed.is_none() && pressure.is_none() {
                    if let Some(d) = mp.depth {
                        st.depth = Some(d);
                    }
                }
                st.pen = mp.code == 113;
            }
            if let Some(s) = speed {
                st.speed = s;
            }
            if let Some(p) = pressure {
                st.pressure = p;
            }
            st.regmark.enabled = regmarks;
            st.regmark.search = reg_search;
            st.regmark.width_mm = reg_width;
            st.regmark.length_mm = reg_length;
            st.regmark.origin_x_mm = reg_origin_x;
            st.regmark.origin_y_mm = reg_origin_y;
            st.regmark.mark_length_mm = reg_mark_length;
            st.regmark.mark_thickness_mm = reg_mark_thickness;
            st.passes = passes.clamp(1, 6);

            let doc = cutjob::load_file(&file, st.tolerance_mm)
                .with_context(|| format!("carregar {}", file.display()))?;
            println!(
                "SVG: {} caminhos, {:.1}x{:.1} mm",
                doc.polylines.len(),
                doc.width_mm,
                doc.height_mm
            );
            if doc.polylines.is_empty() {
                bail!("nenhum caminho de corte no SVG");
            }

            if dry_run {
                let out = out.unwrap_or_else(|| PathBuf::from("corte.gpgl"));
                let report = cutjob::dry_run(doc.polylines, &st, force_spec(&cfg)?, &out)?;
                print_report(&report, &format!("GPGL: {}", out.display()));
                return Ok(());
            }

            let mut c = open_cutter()?;
            let warnings = cutjob::validate(&st, c.spec, bbox_of(&doc.polylines));
            for w in &warnings {
                println!("aviso: {w}");
            }
            let report = cutjob::cut(&mut c, doc.polylines, &st, &mut |_phase, msg| {
                eprint!("\r\x1b[K{msg}");
            })?;
            eprintln!();
            print_report(&report, "");
        }
    }
    Ok(())
}

fn open_cutter() -> Result<Cutter> {
    let mut c = Cutter::open_first().context("nenhum cortador acessível")?;
    // Aquece o diálogo: versão rápida para log
    if let Ok(v) = c.version() {
        eprintln!("dispositivo: {} (firmware {v})", c.spec.name);
    }
    Ok(c)
}

fn force_spec(cfg: &AppConfig) -> Result<&'static device::DeviceSpec> {
    // dry-run sem máquina: usa Cameo 4 como referência ou o último modelo salvo
    let spec = cfg
        .last_device
        .and_then(|(vid, pid)| device::find_by_ids(vid, pid))
        .unwrap_or_else(|| device::find_by_ids(device::VID_GRAPHTEC, 0x1137).unwrap());
    Ok(spec)
}

fn bbox_of(paths: &[silcut::geom::Polyline]) -> silcut::geom::Bbox {
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

fn print_report(r: &cutjob::CutReport, extra: &str) {
    println!("{extra}");
    println!(
        "caminhos: {} | pontos: {} | bbox: {:.1}x{:.1} mm",
        r.paths,
        r.points,
        r.bbox_mm.width(),
        r.bbox_mm.height()
    );
    println!(
        "corte: {:.1} mm | deslocamentos: {:.1} mm | bytes GPGL: {}",
        r.cut_mm, r.travel_mm, r.bytes
    );
}
