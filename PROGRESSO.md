# PROGRESSO — silcut (ferramenta nativa Linux para Silhouette/Graphtec)

> Documento de continuidade. Atualizado em: 2026-02-18 (build verde + GUI v2).
> Objetivo: substituto **nativo** (sem Wine) do fluxo essencial do Silhouette
> Studio no Linux, em Rust: importar SVG, pré-visualizar, configurar corte,
> **marcas de registro (print & cut)** e enviar GPGL pela USB direto.

---

## 1. CONTEXTO E DECISÕES

| Decisão | Detalhe |
|---|---|
| Sem Wine | O usuário pediu explicitamente app nativo standalone. |
| Linguagem | Rust (toolchain 1.99 disponível em ~/.cargo). |
| Local do projeto | `~/silcut` |
| GUI | `eframe`/`egui` 0.29 (janela própria, leve, sem GTK). |
| USB | `rusb 0.9` com feature **`vendored`** (libusb embutida — a máquina NÃO tem `libusb-1.0-dev`; gcc presente, então o vendored compila). |
| Config | TOML em `~/.config/silcut/config.toml` (equivalente do Registro). |
| Licença | GPL-3.0-or-later (fatos de protocolo vêm de projeto GPL; implementação é original). |
| App original analisado | instalação Windows do Silhouette Studio — Xojo + plugins MBS; NÃO descompilado (proibido por EULA); análise factual apenas. |
| Pasta de desenhos de referência | `~/Silhouette` (arquivos .studio3/.dxf/.png — referência de uso, não é código). |

## 2. FATOS DE PROTOCOLO VERIFICADOS (base de todo o driver)

Fonte: `fablabnbg/inkscape-silhouette` → `Commands.md` + `Graphtec.py` (GPL).
Lidos e confirmados por capturas USB reais do Silhouette Studio:

- Protocolo **GPGL**. Resolução **1 SU = 0,05 mm** (1 mm = 20 SU; 1 pol = 508 SU).
- Endpoints USB: **out 0x01 / in 0x82**; blocos 4096; fluxos longos em fatias
  de 1 KiB alinhadas ao terminador **0x03**, aguardando "ready" entre fatias.
- Init `\x1b\x04`; status `\x1b\x05` → `0`=pronta, `1`=movendo, `2`=sem mídia.
- `FG` versão; `TI` nome; Cameo3+ manda ainda `TB71`, `FA` (e `TC` só Cameo3).
- Config: `FN0` `TB50,0` (retrato); mat `TG{n}` (0,1,2,3,8,9,11); limites
  `\y,x` (superior-esq) e `Zy,x` (inferior-dir) — **ordem (Y,X)**: Y=avanço.
- Corte: `FX{pressão}` `!{velocidade}` `FC{offset}` `FE` `FF` `L0` `TJ{acc}`;
  Cameo4/5 duplica parâmetros com sufixo `,toolholder` e usa `J1`.
- Movimento: `My,x` (levanta) / `Dy,x` (corta) — coordenadas em SU.
- **Marcas de registro**: `TB50,0` `TB99` `TB52,2` `TB51,400` `TB53,10`
  `TB55,1` + `TB123,h,w,t,l` (busca automática) ou `TB23,h,w` (manual);
  resposta esperada `    0\x03`. Tipo de marca 2 = Cameo/Portrait (4 marcas).
- Finalização: 'below' → `M{maxY},0` `SO0`; 'start' → C3+: `L0 \0,0 M0,0 J0
  FN0 TB50,0`, antigos: `H`. Ejeção: `FO0`.
- IDs USB: Graphtec `0x0b4d` (Cameo 0x1121, Cameo2 0x112b, Cameo3 0x112f,
  Cameo4 0x1137, 4+ 0x1138, 4 Pro 0x1139, Cameo5 0x1140, 5+ 0x1141, Pro MkII
  0x1146, Portrait 0x1123/2 0x1132/3 0x113a/4 0x113f, SD 0x111c/0x111d,
  CraftRobo 0x110a/0x111a) e Cameo5 Alpha `0x3844` (0x0001/0x0002).
- Tabela de mídias (28 códigos 100–300 com pressão/velocidade padrão) portada.

## 3. O QUE JÁ ESTÁ FEITO ✅

- **Análise completa** da instalação Windows (componentes, licenças, framework).
- **`Cargo.toml`** — lib + 2 bins, profile release otimizado (LTO, strip, panic=abort).
- **Módulos lib** (todos com testes): `geom` (matriz afim + achatamento
  Bézier/arcos F.6.5), `svg` (parser próprio completo), `optimize`
  (vizinho mais próximo), `device` (tabela HW/mídias/mats), `gpgl`
  (codificador completo + clipes + regmarks), `regmark` (4 marcas + folha
  de impressão SVG), `driver` (rusb: listagem, abertura, status, envio em
  fatias de 1 KiB), `config` (TOML), `trace` (potrace), `cutjob`
  (orquestração completa: load/prepare/cut/dry-run/validate), `error`.
- **`src/bin/silcut.rs`** — CLI clap completa: `devices info cut trace marks
  media config`, dry-run, pré-ajustes de mídia, regmarks.
- **`src/bin/silcut-gui.rs`** — GUI egui completa: canvas zoom/pan/grade/
  moldura/marcas no preview, painel com mídias+mats, corte em thread com
  progresso real (canal), confirmação, export folha e GPGL.
- **`build.sh`** ✅ — release + AppImage (com fallback tar.gz já testado).
- **`70-silhouette.rules`** ✅ (udev 0b4d/3844, uaccess).
- **`README.md`** ✅ (pt-BR: build, udev, uso CLI/GUI, print&cut, limitações).
- **`examples/teste-quadrado.svg`** ✅ (quadrado 50 mm + círculo Ø20 mm).
- **BUILD VERDE**: `cargo build --release` ✅ (2m48s), `cargo test` ✅
  **24/24**. Smoke test CLI: dry-run do exemplo gera GPGL correto
  (478 B, 2 caminhos); `marks` gera folha print&cut; `devices`/`media`
  funcionam sem hardware.
- **Pacote**: `dist/silcut-0.1.0-x86_64-linux.tar.gz` (5,7 MB; AppImage
  pendente só por falta do appimagetool na máquina).

### Bugs corrigidos na sessão de build (2026-02-18)

- `driver.rs`: faltava `use rusb::UsbContext` (trait methods não resolvidos).
- `svg.rs`: tokenizador tinha `read_number` mas o parser chamava
  `next_number` (código nunca tinha compilado).
- `svg.rs` **bug real de escala**: `user_to_mm` era calculado mas nunca
  aplicado; sem `viewBox` o k caía em 1.0 em vez de `PX_TO_MM`.
  Corrigido com `cur_matrix()` = `user_to_mm ∘ pilha` nos 12 pontos.
- `geom.rs`/teste: `flatten_arc` estava correto; o teste assumia centro
  errado (sweep=true ⇒ centro (1,1) pela spec F.6.5; trocado para
  sweep=false ⇒ centro (0,0), que era o que o teste verificava).
- `gpgl.rs`/teste: clipping testava com Cameo 1 (margens 9/1 mm) mas
  esperava valores de margem zero; trocado para Cameo 4.
- GUI: braços de `match` com `.ok()` sem `;` (E0308), closure do progresso
  com 1 arg (cutjob espera `FnMut(f32, &str)`), `ui.add(btn)` sem
  `.clicked()`, paths da GUI agora `Vec<Vec<Pos2>>` (conversão para
  `Point` na hora de cortar/exportar), percentual ligado à barra de
  progresso.
- Avisos: floats com sufixo `_f32` (egui 0.29 `Stroke::new` recebe
  `impl Into<f32>`), imports/`mut` desnecessários.

### GUI v2 — redesenho (2026-02-18, mesma sessão)

Interface reescrita de cima a baixo:

- **Tema próprio** (paleta escura, cartões, chips, seções com ícones) e
  toolbar com máquina/status/salvar padrões.
- **Canvas real**: papel desenhado (bege + sombra), grade de 10 mm, réguas
  em mm nas bordas, zoom ancorado no cursor, posição do cursor em mm,
  botões fit/+/−, caminhos em azul e marcas em preto.
- **Arrastar-e-soltar** SVG/imagem na janela.
- **Presets de folha** (A4/A5/A3/Carta/12×12/12×24/8×12/15×15/24×24/rolo)
  com troca de orientação.
- **Tipo de material** (28 mídias) aplicando força/velocidade/profundidade.
- **Força & lâmina explícitos**: força com barra de intensidade colorida,
  velocidade, profundidade, diâmetro da lâmina, ferramenta 1/2, caneta;
  avançado com reforço de trilha, cantos, otimização, tolerância.
- **Posição & tamanho**: escala X/Y com trava, rotação, posição, e botões
  **cabê na folha / esticar (alongar) / centrar / restaurar 100 %**.
- **Print & cut** com tamanho de folha de impressão configurável.
- **Modal de confirmação** com resumo completo + avisos de validação + tempo
  estimado; progresso real na barra de status.
- **Vetorização melhorada** (`trace.rs` → `TraceOptions`): limiar,
  inverter, remover ruído, suavização de cantos (`-a`) e simplificação de
  curvas (`-O`), tudo ajustável na UI e na CLI
  (`silcut trace --threshold --invert --turd --alpha --opt-tolerance`).
- **`build.sh`**: AppImage com AppRun symlink (correção do usuário mantida);
  se `linuxdeploy` estiver presente, **embute GL/xcb/xkbcommon** deixando o
  AppImage autocontido.

## 4. O QUE FALTA ⬜

1. **Teste físico** com a máquina real (feito pelo usuário) — GUI v2:

```bash
chmod +x dist/silcut-*.AppImage && ./dist/silcut-*.AppImage   # portátil
silcut-gui                                                    # ou instalado
```

Depois, o roteiro original:
   - `sudo cp 70-silhouette.rules /etc/udev/rules.d/ && sudo udevadm control --reload && sudo udevadm trigger`
   - `silcut devices` → `silcut info` → dry-run → corte em papel/velil
     barato com velocidade baixa.
   - Se quiser AppImage: `sudo apt install appimagetool` (ou baixar o
     binário do GitHub) e rodar `./build.sh` de novo.
2. Opcional/futuro: importar `.dxf` diretamente (hoje só SVG), suporte a
   segunda ferramenta (toolholder 2), sangria automática na folha.

## 5. RISCOS / PONTOS DE ATENÇÃO

- **Permissões USB**: sem udev rule → `AccessDenied` com instrução embutida.
- **Corte real**: sem máquina conectada no momento (lsusb não achou
  0b4d/3844). O caminho `dry-run` valida GPGL sem hardware.
- **Marcas**: exigem impressão em 100% (sem ajuste de escala) — reforçado
  na UI e no README.
- potrace não está instalado (`sudo apt install potrace`) — trace.rs degrada
  com mensagem clara.
- `eframe 0.29.1` fixado na árvore de dependências; API usada é a estável.

## 6. COMANDOS ÚTEIS (continuar de onde paramos)

```bash
cd ~/silcut
cargo test                          # 24 testes, todos verdes
cargo build --release               # 2m48s
cargo run --bin silcut -- devices   # listar cortadores
cargo run --bin silcut -- cut examples/teste-quadrado.svg --dry-run -o /tmp/c.gpgl
cargo run --bin silcut-gui          # abrir GUI
./build.sh                          # release + pacote em dist/
```

Estado do build: **VERDE** (lib + CLI + GUI, release, pacote tar.gz).
