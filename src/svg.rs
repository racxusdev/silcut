//! Importação SVG minimalista e autônoma (sem dependências externas).
//!
//! Suporta o subconjunto necessário para corte:
//! * `<path>` com a gramática completa de dados de caminho (M L H V C S Q T A Z),
//!   incluindo comandos relativos, repetições implícitas e controles suaves.
//! * Formas: `<rect>` (com cantos arredondados), `<circle>`, `<ellipse>`,
//!   `<line>`, `<polyline>`, `<polygon>`.
//! * Aninhamento e composição de `transform` em `<g>` e nos próprios elementos.
//! * Conversão de unidades: px (96 dpi), mm, cm, in, pt, pc e `viewBox`.
//!
//! Limitações documentadas (falham com mensagem clara, nunca silenciosas):
//! * `<text>` — precisa ser convertido em caminhos (sem dependência de fontes).
//! * `<use>`/`<defs>`/`<symbol>`/CSS — não resolvidos.

use crate::error::{Result, SilError};
use crate::geom::{
    flatten_arc, flatten_cubic, flatten_ellipse, flatten_quad, Matrix, Point, Polyline,
};

/// Documento convertido: polilinhas em milímetros + tamanho do documento.
#[derive(Debug, Clone)]
pub struct SvgDocument {
    /// Todos os contornos, já em mm (origem no canto superior esquerdo).
    pub polylines: Vec<Polyline>,
    /// Largura do documento em mm.
    pub width_mm: f64,
    /// Altura do documento em mm.
    pub height_mm: f64,
}

/// Analisa o conteúdo SVG e produz polilinhas em mm.
pub fn parse(data: &str, tolerance_mm: f64) -> Result<SvgDocument> {
    let mut doc = Parser::new(data, tolerance_mm);
    doc.run()?;
    Ok(SvgDocument {
        polylines: doc.out,
        width_mm: doc.doc_w_mm,
        height_mm: doc.doc_h_mm,
    })
}

// ---------------------------------------------------------------------------
// Parser de atributos e elementos
// ---------------------------------------------------------------------------

struct Attrs(Vec<(String, String)>);

impl Attrs {
    fn get(&self, name: &str) -> Option<&str> {
        self.0.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
    fn f64(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(parse_number)
    }
}

/// Extrai atributos `name="value"` / `name='value'` de uma string de tag.
fn parse_attrs(s: &str) -> Attrs {
    let mut out = Vec::new();
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // pula espaços
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        // nome
        let start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = &s[start..i];
        if name.is_empty() {
            i += 1;
            continue;
        }
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            let (value, next) = if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let quote = b[i];
                i += 1;
                let vs = i;
                while i < b.len() && b[i] != quote {
                    i += 1;
                }
                (&s[vs..i.min(s.len())], i + 1)
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                (&s[vs..i], i)
            };
            out.push((name.to_string(), value.to_string()));
            i = next;
        } else {
            out.push((name.to_string(), String::new()));
        }
    }
    Attrs(out)
}

/// Converte número SVG com unidade para (valor, "unidade em px").
/// Unidades aceitas: px, mm, cm, in, pt, pc e sem unidade (= px).
fn parse_number_with_unit(s: &str) -> Option<(f64, &'static str)> {
    let s = s.trim();
    let units = ["px", "mm", "cm", "in", "pt", "pc"];
    for u in units {
        if let Some(stripped) = s.strip_suffix(u) {
            return stripped.trim().parse::<f64>().ok().map(|v| (v, u));
        }
    }
    // Exponentes e números comuns
    s.parse::<f64>().ok().map(|v| (v, "px"))
}

fn parse_number(s: &str) -> Option<f64> {
    parse_number_with_unit(s).map(|(v, _)| v)
}

const PX_TO_MM: f64 = 25.4 / 96.0; // CSS px padrão

fn unit_to_mm(v: f64, unit: &str) -> f64 {
    match unit {
        "mm" => v,
        "cm" => v * 10.0,
        "in" => v * 25.4,
        "pt" => v * 25.4 / 72.0,
        "pc" => v * 25.4 / 6.0,
        _ => v * PX_TO_MM,
    }
}

// ---------------------------------------------------------------------------
// Estado do parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    src: &'a str,
    pos: usize,
    tolerance: f64,
    out: Vec<Polyline>,
    /// Pilha de transformações de <g> (espaço do usuário).
    stack: Vec<Matrix>,
    /// Profundidade de skip (dentro de <defs>, <symbol>, <marker>, <clipPath>...).
    skip_depth: usize,
    /// Escala usuário→mm extraída do <svg> raiz.
    user_to_mm: Matrix,
    doc_w_mm: f64,
    doc_h_mm: f64,
    saw_root: bool,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, tolerance: f64) -> Self {
        Parser {
            src,
            pos: 0,
            tolerance,
            out: Vec::new(),
            stack: vec![Matrix::IDENTITY],
            skip_depth: 0,
            user_to_mm: Matrix::scale(PX_TO_MM, PX_TO_MM),
            doc_w_mm: 0.0,
            doc_h_mm: 0.0,
            saw_root: false,
        }
    }

    fn run(&mut self) -> Result<()> {
        while self.pos < self.src.len() {
            let rest = &self.src[self.pos..];
            let Some(lt) = rest.find('<') else { break };
            self.pos += lt + 1;
            let rest = &self.src[self.pos..];
            if rest.starts_with("!--") {
                // comentário
                match rest.find("-->") {
                    Some(end) => self.pos += end + 3,
                    None => break,
                }
            } else if rest.starts_with("![CDATA[") {
                match rest.find("]]>") {
                    Some(end) => self.pos += end + 3,
                    None => break,
                }
            } else if rest.starts_with('!') || rest.starts_with('?') {
                // DOCTYPE / processing instruction: pula até '>'
                match rest.find('>') {
                    Some(end) => self.pos += end + 1,
                    None => break,
                }
            } else if rest.starts_with('/') {
                // tag de fechamento
                let Some(gt) = rest.find('>') else { break };
                let name = rest[1..gt].trim().to_ascii_lowercase();
                self.on_close(&name)?;
                self.pos += gt + 1;
            } else {
                // tag de abertura
                let Some(gt) = rest.find('>') else { break };
                let inner = &rest[..gt];
                self.pos += gt + 1;
                let self_closing = inner.ends_with('/');
                let inner = inner.strip_suffix('/').unwrap_or(inner);
                // Separa nome de atributos
                let name_end = inner
                    .find(|c: char| c.is_ascii_whitespace())
                    .unwrap_or(inner.len());
                let name = inner[..name_end].to_ascii_lowercase();
                let attrs = parse_attrs(&inner[name_end..]);
                if self_closing {
                    self.on_open_self_closing(&name, &attrs)?;
                } else {
                    self.on_open(&name, &attrs)?;
                }
            }
        }

        if !self.saw_root {
            return Err(SilError::Svg("elemento <svg> raiz não encontrado".into()));
        }
        Ok(())
    }

    fn on_open(&mut self, name: &str, attrs: &Attrs) -> Result<()> {
        if matches!(
            name,
            "defs" | "symbol" | "marker" | "clippath" | "mask" | "pattern" | "metadata"
        ) {
            self.skip_depth += 1;
            return Ok(());
        }
        if self.skip_depth > 0 {
            if name == "g" || is_shape(name) || name == "text" {
                // elementos com corpo dentro de skip: contam profundidade extra
                self.skip_depth += 0; // nada: fechos de g só caem quando skip==0
            }
            return Ok(());
        }
        match name {
            "svg" => {
                self.saw_root = true;
                self.setup_root_units(attrs);
                self.push_transform(attrs);
            }
            "g" => {
                self.push_transform(attrs);
            }
            "text" | "tspan" => {
                return Err(SilError::SvgHasText);
            }
            "path" => {
                if let Some(d) = attrs.get("d") {
                    let m = self.cur_matrix();
                    let lines = self.path_to_polylines(d, m)?;
                    self.emit(lines);
                }
            }
            "rect" => {
                let m = self.cur_matrix();
                self.emit(self.rect_polylines(attrs, m));
            }
            "circle" => {
                let m = self.cur_matrix();
                if let (Some(cx), Some(cy), Some(r)) =
                    (attrs.f64("cx"), attrs.f64("cy"), attrs.f64("r"))
                {
                    let mut pl = Vec::new();
                    flatten_ellipse(cx, cy, r, r, self.tolerance, &mut pl);
                    self.emit(vec![map_polyline(&pl, m)]);
                }
            }
            "ellipse" => {
                let m = self.cur_matrix();
                if let (Some(cx), Some(cy), Some(rx), Some(ry)) = (
                    attrs.f64("cx"),
                    attrs.f64("cy"),
                    attrs.f64("rx"),
                    attrs.f64("ry"),
                ) {
                    let mut pl = Vec::new();
                    flatten_ellipse(cx, cy, rx, ry, self.tolerance, &mut pl);
                    self.emit(vec![map_polyline(&pl, m)]);
                }
            }
            "line" => {
                let m = self.cur_matrix();
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                    attrs.f64("x1"),
                    attrs.f64("y1"),
                    attrs.f64("x2"),
                    attrs.f64("y2"),
                ) {
                    self.emit(vec![vec![
                        m.apply(Point::new(x1, y1)),
                        m.apply(Point::new(x2, y2)),
                    ]]);
                }
            }
            "polyline" | "polygon" => {
                let m = self.cur_matrix();
                if let Some(points) = attrs.get("points") {
                    let mut pl = parse_points_list(points);
                    if name == "polygon" && pl.len() >= 2 && pl[0] != *pl.last().unwrap() {
                        pl.push(pl[0]);
                    }
                    self.emit(vec![map_polyline(&pl, m)]);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn on_open_self_closing(&mut self, name: &str, attrs: &Attrs) -> Result<()> {
        // Elementos sem corpo usam a mesma lógica; transform não entra na pilha.
        if self.skip_depth > 0 {
            return Ok(());
        }
        match name {
            "svg" => {
                // <svg/> aninhado raro: só registra tamanho se for raiz.
                if !self.saw_root {
                    self.saw_root = true;
                    self.setup_root_units(attrs);
                }
            }
            "path" => {
                if let Some(d) = attrs.get("d") {
                    let m = self.cur_matrix();
                    let lines = self.path_to_polylines(d, m)?;
                    self.emit(lines);
                }
            }
            "rect" => {
                let m = self.cur_matrix();
                self.emit(self.rect_polylines(attrs, m));
            }
            "circle" => {
                let m = self.cur_matrix();
                if let (Some(cx), Some(cy), Some(r)) =
                    (attrs.f64("cx"), attrs.f64("cy"), attrs.f64("r"))
                {
                    let mut pl = Vec::new();
                    flatten_ellipse(cx, cy, r, r, self.tolerance, &mut pl);
                    self.emit(vec![map_polyline(&pl, m)]);
                }
            }
            "ellipse" => {
                let m = self.cur_matrix();
                if let (Some(cx), Some(cy), Some(rx), Some(ry)) = (
                    attrs.f64("cx"),
                    attrs.f64("cy"),
                    attrs.f64("rx"),
                    attrs.f64("ry"),
                ) {
                    let mut pl = Vec::new();
                    flatten_ellipse(cx, cy, rx, ry, self.tolerance, &mut pl);
                    self.emit(vec![map_polyline(&pl, m)]);
                }
            }
            "line" => {
                let m = self.cur_matrix();
                if let (Some(x1), Some(y1), Some(x2), Some(y2)) = (
                    attrs.f64("x1"),
                    attrs.f64("y1"),
                    attrs.f64("x2"),
                    attrs.f64("y2"),
                ) {
                    self.emit(vec![vec![
                        m.apply(Point::new(x1, y1)),
                        m.apply(Point::new(x2, y2)),
                    ]]);
                }
            }
            "polyline" | "polygon" => {
                let m = self.cur_matrix();
                if let Some(points) = attrs.get("points") {
                    let mut pl = parse_points_list(points);
                    if name == "polygon" && pl.len() >= 2 && pl[0] != *pl.last().unwrap() {
                        pl.push(pl[0]);
                    }
                    self.emit(vec![map_polyline(&pl, m)]);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn on_close(&mut self, name: &str) -> Result<()> {
        if self.skip_depth > 0 {
            if matches!(
                name,
                "defs" | "symbol" | "marker" | "clippath" | "mask" | "pattern" | "metadata"
            ) {
                self.skip_depth -= 1;
            }
            return Ok(());
        }
        match name {
            "svg" | "g" => {
                if self.stack.len() > 1 {
                    self.stack.pop();
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn push_transform(&mut self, attrs: &Attrs) {
        let parent = *self.stack.last().unwrap();
        let m = match attrs.get("transform") {
            Some(t) => parent.mul(&parse_transform(t)),
            None => parent,
        };
        self.stack.push(m);
    }

    /// Matriz corrente de usuário→mm: unidades do documento raiz ∘ transforms de <g>.
    fn cur_matrix(&self) -> Matrix {
        self.user_to_mm.mul(self.stack.last().unwrap())
    }

    fn emit(&mut self, lines: Vec<Polyline>) {
        for l in lines {
            if l.len() >= 2 {
                self.out.push(l);
            }
        }
    }

    /// Configura `user_to_mm` e o tamanho do documento a partir de width/height/viewBox.
    fn setup_root_units(&mut self, attrs: &Attrs) {
        let w_attr = attrs.get("width").and_then(parse_number_with_unit);
        let h_attr = attrs.get("height").and_then(parse_number_with_unit);
        let vb: Option<[f64; 4]> = attrs.get("viewBox").and_then(|s| {
            let nums: Vec<f64> =
                s.split(|c: char| c == ',' || c.is_ascii_whitespace())
                    .filter(|t| !t.is_empty())
                    .filter_map(|t| t.parse::<f64>().ok())
                    .collect();
            if nums.len() == 4 {
                Some(nums.try_into().unwrap())
            } else {
                None
            }
        });

        let (doc_w, doc_h, k) = match (w_attr, h_attr, vb) {
            (Some((w, uw)), Some((h, uh)), _) => {
                let wmm = unit_to_mm(w, uw);
                let hmm = unit_to_mm(h, uh);
                let k = match vb {
                    Some([_, _, vw, vh]) if vw > 0.0 && vh > 0.0 => wmm / vw,
                    // Sem viewBox as unidades do usuário são px (spec SVG).
                    _ => PX_TO_MM,
                };
                (wmm, hmm, k)
            }
            (Some((w, uw)), None, Some([_, _, vw, _])) => {
                let wmm = unit_to_mm(w, uw);
                (wmm, wmm * (vb_height(vb) / vw), wmm / vw)
            }
            (None, Some((h, uh)), Some([_, _, vw, _])) => {
                let hmm = unit_to_mm(h, uh);
                (hmm * (vw / vb_height(vb)), hmm, hmm / vb_height(vb))
            }
            (_, Some((h, uh)), None) => {
                let hmm = unit_to_mm(h, uh);
                (hmm * PX_TO_MM, hmm, PX_TO_MM)
            }
            (Some((w, uw)), _, None) => {
                let wmm = unit_to_mm(w, uw);
                (wmm, wmm, PX_TO_MM)
            }
            (None, None, Some([_, _, vw, vh])) => {
                // Sem dimensões: viewBox vale px.
                let k = PX_TO_MM;
                (vw * k, vh * k, k)
            }
            (None, None, None) => (210.0, 297.0, PX_TO_MM),
        };

        let mut m = Matrix::scale(k, k);
        if let Some([vx, vy, _, _]) = vb {
            m = Matrix::translate(-vx * k, -vy * k).mul(&m);
        }
        self.user_to_mm = m;
        self.doc_w_mm = doc_w;
        self.doc_h_mm = doc_h;
    }

    fn rect_polylines(&self, attrs: &Attrs, m: Matrix) -> Vec<Polyline> {
        let x = attrs.f64("x").unwrap_or(0.0);
        let y = attrs.f64("y").unwrap_or(0.0);
        let Some(w) = attrs.f64("width") else { return Vec::new() };
        let Some(h) = attrs.f64("height") else { return Vec::new() };
        if w <= 0.0 || h <= 0.0 {
            return Vec::new();
        }
        let rx = attrs.f64("rx").unwrap_or(0.0).min(w / 2.0);
        let ry = attrs
            .f64("ry")
            .or(attrs.f64("rx"))
            .unwrap_or(0.0)
            .min(h / 2.0);

        let mut pl: Polyline = Vec::new();
        if rx <= 0.0 || ry <= 0.0 {
            pl.push(Point::new(x, y));
            pl.push(Point::new(x + w, y));
            pl.push(Point::new(x + w, y + h));
            pl.push(Point::new(x, y + h));
            pl.push(Point::new(x, y));
        } else {
            // Retângulo arredondado: retas entre 4 arcos de canto (90°, raio rx/ry).
            // Canto superior esquerdo tem centro (x+rx, y+ry); arco vai de (x, y+ry) a (x+rx, y).
            pl.push(Point::new(x + rx, y));
            // topo → canto sup. direito
            pl.push(Point::new(x + w - rx, y));
            flatten_arc(
                *pl.last().unwrap(),
                rx,
                ry,
                0.0,
                false,
                true,
                Point::new(x + w, y + ry),
                self.tolerance,
                &mut pl,
            );
            // lateral direita → canto inf. direito
            pl.push(Point::new(x + w, y + h - ry));
            flatten_arc(
                *pl.last().unwrap(),
                rx,
                ry,
                0.0,
                false,
                true,
                Point::new(x + w - rx, y + h),
                self.tolerance,
                &mut pl,
            );
            // base → canto inf. esquerdo
            pl.push(Point::new(x + rx, y + h));
            flatten_arc(
                *pl.last().unwrap(),
                rx,
                ry,
                0.0,
                false,
                true,
                Point::new(x, y + h - ry),
                self.tolerance,
                &mut pl,
            );
            // lateral esquerda → canto sup. esquerdo
            pl.push(Point::new(x, y + ry));
            flatten_arc(
                *pl.last().unwrap(),
                rx,
                ry,
                0.0,
                false,
                true,
                Point::new(x + rx, y),
                self.tolerance,
                &mut pl,
            );
        }
        vec![map_polyline(&pl, m)]
    }

    // -----------------------------------------------------------------
    // Dados de caminho (d="...")
    // -----------------------------------------------------------------

    fn path_to_polylines(&self, d: &str, base: Matrix) -> Result<Vec<Polyline>> {
        let mut toks = PathTokens::new(d);
        let mut result: Vec<Polyline> = Vec::new();
        let mut cur = Point::new(0.0, 0.0);
        let mut start = cur;
        let mut last_ctl_cubic: Option<Point> = None;
        let mut last_ctl_quad: Option<Point> = None;
        let mut cmd = '\0';
        let mut open: Option<Polyline> = None;

        macro_rules! finish_open {
            () => {
                if let Some(pl) = open.take() {
                    if pl.len() >= 2 {
                        result.push(map_polyline(&pl, base));
                    }
                }
            };
        }

        while let Some(tok) = toks.next() {
            match tok {
                Token::Cmd(c) => {
                    cmd = c;
                }
                Token::Num(_) => {
                    // número sem comando novo: repete o comando anterior com
                    // novos parâmetros (comportamento de continuação implícita)
                }
            }
            if cmd == '\0' {
                return Err(SilError::Svg("dados de caminho começam sem comando".into()));
            }

            let consume_point = |toks: &mut PathTokens, cur: Point, relative: bool| -> Point {
                let x = toks.read_number().unwrap_or(0.0);
                let y = toks.read_number().unwrap_or(0.0);
                if relative {
                    Point::new(cur.x + x, cur.y + y)
                } else {
                    Point::new(x, y)
                }
            };

            let relative = cmd.is_ascii_lowercase();
            match cmd.to_ascii_uppercase() {
                'M' => {
                    finish_open!();
                    cur = consume_point(&mut toks, cur, relative);
                    start = cur;
                    open = Some(vec![cur]);
                    // Após M, repetições implícitas viram L.
                    cmd = if relative { 'l' } else { 'L' };
                    last_ctl_cubic = None;
                    last_ctl_quad = None;
                }
                'L' => {
                    let p = consume_point(&mut toks, cur, relative);
                    if let Some(pl) = open.as_mut() {
                        pl.push(p);
                    }
                    cur = p;
                    last_ctl_cubic = None;
                    last_ctl_quad = None;
                }
                'H' => {
                    let x = toks.read_number().unwrap_or(0.0);
                    let p = Point::new(if relative { cur.x + x } else { x }, cur.y);
                    if let Some(pl) = open.as_mut() {
                        pl.push(p);
                    }
                    cur = p;
                    last_ctl_cubic = None;
                    last_ctl_quad = None;
                }
                'V' => {
                    let y = toks.read_number().unwrap_or(0.0);
                    let p = Point::new(cur.x, if relative { cur.y + y } else { y });
                    if let Some(pl) = open.as_mut() {
                        pl.push(p);
                    }
                    cur = p;
                    last_ctl_cubic = None;
                    last_ctl_quad = None;
                }
                'C' => {
                    let c1 = consume_point(&mut toks, cur, relative);
                    let c2 = consume_point(&mut toks, cur, relative);
                    let p = consume_point(&mut toks, cur, relative);
                    if let Some(pl) = open.as_mut() {
                        flatten_cubic(cur, c1, c2, p, self.tolerance, pl);
                    }
                    last_ctl_cubic = Some(c2);
                    last_ctl_quad = None;
                    cur = p;
                }
                'S' => {
                    // c1 = reflexo do último controle
                    let c1 = match last_ctl_cubic {
                        Some(lc) => Point::new(2.0 * cur.x - lc.x, 2.0 * cur.y - lc.y),
                        None => cur,
                    };
                    let c2 = consume_point(&mut toks, cur, relative);
                    let p = consume_point(&mut toks, cur, relative);
                    if let Some(pl) = open.as_mut() {
                        flatten_cubic(cur, c1, c2, p, self.tolerance, pl);
                    }
                    last_ctl_cubic = Some(c2);
                    last_ctl_quad = None;
                    cur = p;
                }
                'Q' => {
                    let c = consume_point(&mut toks, cur, relative);
                    let p = consume_point(&mut toks, cur, relative);
                    if let Some(pl) = open.as_mut() {
                        flatten_quad(cur, c, p, self.tolerance, pl);
                    }
                    last_ctl_quad = Some(c);
                    last_ctl_cubic = None;
                    cur = p;
                }
                'T' => {
                    let c = match last_ctl_quad {
                        Some(lc) => Point::new(2.0 * cur.x - lc.x, 2.0 * cur.y - lc.y),
                        None => cur,
                    };
                    let p = consume_point(&mut toks, cur, relative);
                    if let Some(pl) = open.as_mut() {
                        flatten_quad(cur, c, p, self.tolerance, pl);
                    }
                    last_ctl_quad = Some(c);
                    last_ctl_cubic = None;
                    cur = p;
                }
                'A' => {
                    let rx = toks.read_number().unwrap_or(0.0);
                    let ry = toks.read_number().unwrap_or(0.0);
                    let rot = toks.read_number().unwrap_or(0.0);
                    let large = toks.read_flag() != 0.0;
                    let sweep = toks.read_flag() != 0.0;
                    let p = consume_point(&mut toks, cur, relative);
                    if let Some(pl) = open.as_mut() {
                        flatten_arc(cur, rx, ry, rot, large, sweep, p, self.tolerance, pl);
                    }
                    cur = p;
                    last_ctl_cubic = None;
                    last_ctl_quad = None;
                }
                'Z' => {
                    if let Some(mut pl) = open.take() {
                        if pl.len() >= 2 {
                            pl.push(start);
                            result.push(map_polyline(&pl, base));
                        }
                    }
                    cur = start;
                    last_ctl_cubic = None;
                    last_ctl_quad = None;
                }
                _ => {
                    return Err(SilError::Svg(format!(
                        "comando de caminho desconhecido: {cmd}"
                    )));
                }
            }

            // Continuação implícita: se ainda há números, o comando corrente
            // se repete com novos parâmetros (após M o cmd já é L/l).
            // 'Z' não consome parâmetros: nunca repete (evita loop infinito).
            if matches!(
                cmd.to_ascii_uppercase(),
                'L' | 'H' | 'V' | 'C' | 'S' | 'Q' | 'T' | 'A'
            ) && matches!(toks.peek(), Some(Token::Num(_)))
            {
                toks.pending_cmd = Some(cmd);
            }
        }
        finish_open!();
        Ok(result)
    }
}

fn vb_height(vb: Option<[f64; 4]>) -> f64 {
    vb.map(|v| v[3]).unwrap_or(1.0).max(f64::EPSILON)
}

fn is_shape(name: &str) -> bool {
    matches!(
        name,
        "path" | "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon"
    )
}

fn map_polyline(pl: &Polyline, m: Matrix) -> Polyline {
    pl.iter().map(|p| m.apply(*p)).collect()
}

fn parse_points_list(s: &str) -> Polyline {
    let mut nums = Vec::new();
    let mut it = s.split(|c: char| c == ',' || c.is_ascii_whitespace()).filter(|t| !t.is_empty());
    while let (Some(x), Some(y)) = (it.next(), it.next()) {
        if let (Ok(x), Ok(y)) = (x.parse::<f64>(), y.parse::<f64>()) {
            nums.push(Point::new(x, y));
        }
    }
    nums
}

/// Converte o atributo `transform` em matriz (lista de funções composta).
fn parse_transform(s: &str) -> Matrix {
    let mut acc = Matrix::IDENTITY;
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // nome da função
        while i < b.len() && !b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let start = i;
        while i < b.len() && b[i].is_ascii_alphabetic() {
            i += 1;
        }
        let name = &s[start..i];
        // '(' args ')'
        while i < b.len() && b[i] != b'(' {
            i += 1;
        }
        i += 1;
        let args_start = i;
        while i < b.len() && b[i] != b')' {
            i += 1;
        }
        let args_s = &s[args_start..i.min(s.len())];
        i += 1;
        let args: Vec<f64> = args_s
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|t| !t.is_empty())
            .filter_map(|t| t.parse::<f64>().ok())
            .collect();

        let m = match name {
            "matrix" if args.len() >= 6 => Matrix {
                a: args[0],
                b: args[1],
                c: args[2],
                d: args[3],
                e: args[4],
                f: args[5],
            },
            "translate" if !args.is_empty() => {
                Matrix::translate(args[0], args.get(1).copied().unwrap_or(0.0))
            }
            "scale" if !args.is_empty() => {
                let s0 = args[0];
                Matrix::scale(s0, args.get(1).copied().unwrap_or(s0))
            }
            "rotate" if !args.is_empty() => {
                let (cx, cy) = match args.len() {
                    3 => (args[1], args[2]),
                    _ => (0.0, 0.0),
                };
                Matrix::rotate_deg(args[0], cx, cy)
            }
            "skewX" if !args.is_empty() => Matrix::skew_x(args[0]),
            "skewY" if !args.is_empty() => Matrix::skew_y(args[0]),
            _ => Matrix::IDENTITY,
        };
        acc = acc.mul(&m);
    }
    acc
}

// ---------------------------------------------------------------------------
// Tokenizador de dados de caminho
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token {
    Cmd(char),
    Num(f64),
}

struct PathTokens {
    src: Vec<char>,
    pos: usize,
    pending_cmd: Option<char>,
}

impl PathTokens {
    fn new(s: &str) -> Self {
        PathTokens {
            src: s.chars().collect(),
            pos: 0,
            pending_cmd: None,
        }
    }

    fn next(&mut self) -> Option<Token> {
        if let Some(c) = self.pending_cmd.take() {
            return Some(Token::Cmd(c));
        }
        self.skip_ws();
        if self.pos >= self.src.len() {
            return None;
        }
        let c = self.src[self.pos];
        if c.is_ascii_alphabetic() {
            self.pos += 1;
            Some(Token::Cmd(c))
        } else {
            let n = self.read_number()?;
            Some(Token::Num(n))
        }
    }

    fn peek(&mut self) -> Option<Token> {
        if self.pending_cmd.is_some() {
            return self.pending_cmd.map(Token::Cmd);
        }
        let save = self.pos;
        let t = self.next();
        self.pos = save;
        t
    }
}

// ---------------------------------------------------------------------------
// Números
// ---------------------------------------------------------------------------

impl PathTokens {
    fn skip_ws(&mut self) {
        while self.pos < self.src.len() {
            let c = self.src[self.pos];
            if c.is_ascii_whitespace() || c == ',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Lê um flag do comando de arco (um único dígito 0/1, conforme a
    /// especificação SVG — pode estar adjacente ao número anterior).
    fn read_flag(&mut self) -> f64 {
        self.skip_ws();
        if self.pos < self.src.len()
            && (self.src[self.pos] == '0' || self.src[self.pos] == '1')
        {
            let v = if self.src[self.pos] == '1' { 1.0 } else { 0.0 };
            self.pos += 1;
            v
        } else {
            0.0
        }
    }

    fn read_number(&mut self) -> Option<f64> {
        self.skip_ws();
        let start = self.pos;
        let b = &mut self.src;
        if self.pos >= b.len() {
            return None;
        }
        // sinal
        if b[self.pos] == '-' || b[self.pos] == '+' {
            self.pos += 1;
        }
        // inteiro
        while self.pos < b.len() && b[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        // fração
        if self.pos < b.len() && b[self.pos] == '.' {
            self.pos += 1;
            while self.pos < b.len() && b[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        // expoente
        if self.pos < b.len() && (b[self.pos] == 'e' || b[self.pos] == 'E') {
            let save = self.pos;
            self.pos += 1;
            if self.pos < b.len() && (b[self.pos] == '-' || b[self.pos] == '+') {
                self.pos += 1;
            }
            if self.pos < b.len() && b[self.pos].is_ascii_digit() {
                while self.pos < b.len() && b[self.pos].is_ascii_digit() {
                    self.pos += 1;
                }
            } else {
                self.pos = save; // 'e' não pertence ao número
            }
        }
        if self.pos == start {
            self.pos += 1; // evita loop infinito em lixo
            return None;
        }
        let s: String = b[start..self.pos].iter().collect();
        s.parse::<f64>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOL: f64 = 0.01;

    #[test]
    fn simple_rect_svg() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect x="2" y="3" width="10" height="5"/></svg>"##;
        let doc = parse(svg, TOL).unwrap();
        assert_eq!(doc.polylines.len(), 1);
        let pl = &doc.polylines[0];
        // viewBox 20 == width 20mm => 1 unidade = 1mm
        assert!((doc.width_mm - 20.0).abs() < 1e-9);
        assert_eq!(pl.len(), 5);
        assert!((pl[0].x - 2.0).abs() < 1e-9 && (pl[0].y - 3.0).abs() < 1e-9);
        assert!((pl[2].x - 12.0).abs() < 1e-9 && (pl[2].y - 8.0).abs() < 1e-9);
    }

    #[test]
    fn path_px_scale() {
        // 96px == 1in == 25.4mm
        let svg = r#"<svg width="96" height="96"><path d="M 0 0 L 96 0 L 96 96 Z"/></svg>"#;
        let doc = parse(svg, TOL).unwrap();
        assert!((doc.width_mm - 25.4).abs() < 1e-9);
        let pl = &doc.polylines[0];
        assert!((pl[1].x - 25.4).abs() < 1e-9);
    }

    #[test]
    fn implicit_lineto_after_moveto() {
        let svg = r#"<svg width="10mm" height="10mm" viewBox="0 0 10 10"><path d="M1 1 5 1 5 5Z"/></svg>"#;
        let doc = parse(svg, TOL).unwrap();
        let pl = &doc.polylines[0];
        // M1,1 L5,1 L5,5 Z(1,1)
        assert_eq!(pl.len(), 4);
        assert!((pl[1].x - 5.0).abs() < 1e-9);
    }

    #[test]
    fn relative_and_smooth() {
        let svg = r#"<svg width="10mm" height="10mm" viewBox="0 0 10 10"><path d="M1,1 c1,0 2,0 3,0 s2,1 3,0"/></svg>"#;
        let doc = parse(svg, TOL).unwrap();
        let pl = &doc.polylines[0];
        let last = *pl.last().unwrap();
        assert!((last.x - 7.0).abs() < 1e-9, "x final {last:?}");
    }

    #[test]
    fn nested_transform() {
        let svg = r##"<svg width="40mm" height="40mm" viewBox="0 0 40 40">
            <g transform="translate(10,0)"><g transform="scale(2,1)"><rect x="1" y="1" width="2" height="2"/></g></g>
        </svg>"##;
        let doc = parse(svg, TOL).unwrap();
        let pl = &doc.polylines[0];
        // x: 1*2+10=12, y: 1
        assert!((pl[0].x - 12.0).abs() < 1e-9 && (pl[0].y - 1.0).abs() < 1e-9);
        assert!((pl[2].x - 16.0).abs() < 1e-9 && (pl[2].y - 3.0).abs() < 1e-9);
    }

    #[test]
    fn defs_skipped() {
        let svg = r#"<svg width="10mm" height="10mm" viewBox="0 0 10 10"><defs><rect x="1" y="1" width="50" height="50"/></defs><circle cx="5" cy="5" r="2"/></svg>"#;
        let doc = parse(svg, TOL).unwrap();
        assert_eq!(doc.polylines.len(), 1);
    }

    #[test]
    fn text_rejected() {
        let svg = r#"<svg width="10mm" height="10mm"><text x="1" y="1">oi</text></svg>"#;
        let err = parse(svg, TOL).unwrap_err();
        assert!(matches!(err, SilError::SvgHasText));
    }

    #[test]
    fn circle_in_mm_doc() {
        let svg = r#"<svg width="50mm" height="50mm" viewBox="0 0 50 50"><circle cx="10" cy="10" r="5"/></svg>"#;
        let doc = parse(svg, TOL).unwrap();
        let pl = &doc.polylines[0];
        // fechado: primeiro == último
        assert_eq!(pl[0], *pl.last().unwrap());
        // diâmetro 10mm
        let min_x = pl.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let max_x = pl.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        assert!((max_x - min_x - 10.0).abs() < 0.05);
    }
}
