# silcut

**Português** · [English](README.en.md)

Ferramenta **nativa para Linux** (Rust, sem Wine) para cortadores
**Silhouette / Graphtec**: importa SVG, pré-visualiza, configura e envia o
corte pela USB direto usando o protocolo **GPGL** (1 unidade = 0,05 mm).
Inclui **marcas de registro** para fluxo *print & cut*, como no Silhouette
Studio.

Dois programas:

| binário | o que faz |
|---|---|
| `silcut` | linha de comando: dispositivos, corte, vetorização, folha de marcas, mídias |
| `silcut-gui` | janela gráfica: preview com zoom/pan e grade, parâmetros, print & cut, corte |

## Abrir o AppImage (portátil, sem instalar)

```bash
chmod +x silcut-*.AppImage
./silcut-*.AppImage
```

Gera-se um AppImage com `./build.sh` (requer `appimagetool`). Para um
AppImage **100 % autocontido** (não exige nenhuma lib no sistema alvo —
GL/xcb/xkbcommon embutidos), use o `linuxdeploy` — se ele estiver no PATH,
o `build.sh` já o usa automaticamente e embute tudo.

## Instalar no sistema (sem AppImage)

Compila (se preciso), copia para `~/.local/bin`, cria entrada no **menu de
aplicativos** com ícone e orienta a regra udev:

```bash
./install.sh
```

Depois:

* **Menu de aplicativos**: procure **silcut** (sem precisar de terminal).
* **Terminal**: `silcut-gui` (GUI) ou `silcut --help` (CLI). Na primeira
  vez, reabra o terminal ou rode `source ~/.profile` para o PATH pegar
  `~/.local/bin`.

Para remover: `./install.sh --uninstall` (mantém `~/.config/silcut`).

## Compilar (manual)

```bash
cargo build --release          # binários em target/release/
./build.sh                     # release + pacote portátil em dist/
```

Binários ficam em `target/release/`. A libusb é compilada embutida
(feature `vendored` do `rusb`) — não é preciso instalar `libusb-1.0-dev`.

## Instalar permissões USB (obrigatório, uma vez)

Sem isso os dispositivos listam mas não abrem:

```bash
sudo cp 70-silhouette.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
# desconecte e reconecte a máquina
```

(o `./install.sh` sem sudo já deixa o arquivo pronto em
`~/.config/silcut/70-silhouette.rules` e mostra esse comando)

## Testar sem hardware (dry-run)

```bash
cargo run --bin silcut -- cut examples/teste-quadrado.svg --dry-run -o corte.gpgl
cargo run --bin silcut -- devices
```

`--dry-run` valida todo o pipeline e grava o programa GPGL em texto — útil
para inspecionar os comandos (`FG`, `TB50,0`, `M/D`, regmarks `TB123` etc.)
sem a máquina.

## Uso da GUI

Abra pelo **menu de aplicativos** ("silcut") ou no terminal:

```bash
silcut-gui
```

### Interface

* **Barra superior** — escolha da máquina, 🔄 procurar, ⚡ status (firmware),
  salvar padrões e ajustar vista.
* **Canvas** — papel desenhado em escala real (com sombra), grade de 10 mm,
  **réguas em mm** nas bordas, caminhos em azul, marcas de registro em preto.
  Roda do mouse = zoom no cursor, arrastar = mover, duplo clique = ajustar,
  **arrastar as marcas = reposicionar** (quadrado move a área; cantos
  redimensionam). Mostra posição do cursor em mm e zoom %.
* **Tema claro / escuro** — botão ☀/🌙 na barra superior (preferência salva
  em `~/.config/silcut/config.toml`).
* **Arrastar-e-soltar** — arraste um `.svg` (ou PNG/JPG) direto para a janela.

### Painéis laterais

| Seção | O que faz |
|---|---|
| 📂 **Arquivo** | Abrir **SVG ou DXF**; **Vetorizar imagem** com limiar, suavização de cantos, simplificação de curvas, remoção de ruído e inversão; cartão com nome/nº de caminhos/tamanho final |
| 📐 **Papel & mídia** | Presets de folha: **A4, A5, A3, Carta, 12×12, 12×24, 8×12, 15×15, 24×24, rolo 30 cm**; largura/altura com botão de troca; **tipo de material** (28 mídias com pré-ajuste de força/velocidade/profundidade); tapete |
| 💪 **Força & lâmina** | **Ação** (cortar / **vincar** / **esboçar**); **Força (pressão)** com barra de intensidade colorida, **velocidade**, **passagens** (multipass para materiais espessos), **profundidade da lâmina**, **diâmetro da lâmina**, ferramenta 1/2; avançado: reforço de trilha, levantar em cantos (+ entrada/saída), otimizar ordem, avançar/ejetar mídia, tolerância de curvas |
| 📏 **Posição & tamanho** | **Escala X/Y** (com trava de proporção), **rotação** (±90° rápido), **posição X/Y**, botões **cabê na folha** (proporcional), **esticar** (alonga preenchendo a folha) e **centrar** |
| 🛠 **Editar desenho** | **Offset** (contorno paralelo / borda branca, ±20 mm), **fundir** contornos sobrepostos (Weld, união por rasterização com resolução ajustável) e **desfazer** (20 níveis) |
| 🎯 **Print & cut** | Marcas no padrão do Studio: **quadrado preto sólido 10×10 mm + "L" de 10 mm** nos cantos, **arrastáveis com o mouse** no papel (quadrado move, cantos redimensionam); comprimento (6–30 mm) e espessura ajustáveis; zona de exclusão com **aviso vermelho** quando o desenho invade; busca automática/manual; tamanho da folha de impressão; exportar folha SVG |
| 📤 **Saída/teste** | Gerar GPGL sem cortar (dry-run) + tempo estimado de corte |

* **✂ CORTAR** (fixo no rodapé do painel) abre um resumo completo
  (máquina, folha, força, desenho, posição, tempo estimado e avisos de
  validação) antes de executar. O corte roda em thread separada com barra
  de progresso real — a janela nunca trava.

Ajustes do material aplicam automaticamente força/velocidade/profundidade
recomendadas; você pode sobrepor tudo manualmente. Padrões são salvos em
`~/.config/silcut/config.toml` (botão 💾).

## Print & cut (marcas de registro)

Marcas no **padrão do Silhouette Studio**: um **quadrado totalmente
preenchido de preto, 10 × 10 mm**, no canto superior esquerdo (o sensor começa
a leitura por ele) e dois **"L"** nos cantos superior direito e inferior
esquerdo, com braços de 10 mm (comprimento ajustável 6–30 mm) e espessura
0,5 mm (até 2,0 mm para leitura difícil). O preview mostra a **zona de
exclusão** em volta das marcas — nada do desenho pode invadi-la (aviso
vermelho quando invade), como a área hachurada do Studio.

O quadrado é **totalmente preenchido de preto** (sem vazio), tanto no preview
quanto na folha impressa. A área de registro **assume automaticamente as
bordas da folha escolhida** (A4, A3, 12×12…): margem de 10 mm em cada lado, e
a folha de impressão passa a ter o mesmo tamanho do papel — muda o tamanho da
folha e as marcas acompanham.

**Ajuste com o mouse**: no canvas, arraste o **quadrado** para mover a área
inteira de registro, o canto **superior direito** para ajustar a largura e o
**inferior esquerdo** para ajustar a altura (passo de 0,5 mm).

1. Na GUI, ative **usar marcas de registro** e clique
   **🖨️ Exportar folha de impressão…** — gera um SVG com o desenho + as
   marcas nos cantos da área configurada.
2. **Imprima em escala 100%** (desative "ajustar à página") no papel
   adesivo/mídia.
3. Cole a mídia no tapete e carregue na máquina.
4. Ative as marcas no corte (GUI ou `silcut cut --regmarks`): a máquina
   usa o sensor óptico para localizar as marcas (`TB123`, busca automática
   — ou `TB23` manual) e compensa a distorção de impressão antes de cortar.

Por CLI:

```bash
# 1) gerar a folha de impressão (com o desenho dentro)
silcut marks --design desenho.svg -o folha.svg
# 2) imprimir folha.svg em 100%, colar e carregar
# 3) cortar com registro
silcut cut folha.svg --regmarks --reg-width 180 --reg-length 230 \
                     --reg-origin-x 15 --reg-origin-y 20
```

## Máquinas suportadas

Cameo (1/2/3/4/4 Plus/4 Pro/5/5 Plus/Pro MkII/5 Alpha), Portrait
(1/2/3/4), SD, Craft Robo — VID `0b4d` (e `3844` para Cameo 5 Alpha).
Tabela completa com `silcut media` (28 mídias com pressão/velocidade
padrão).

## Configuração

`~/.config/silcut/config.toml` guarda os padrões (mídia, offsets, última
máquina, caminho do potrace). `silcut config show|init|path`.

## Limitações honestas

* **Formatos**: SVG (caminhos e formas básicas com transforms e unidades) e
  **DXF** (LINE/LWPOLYLINE com bulge/POLYLINE/CIRCLE/ARC; `INSERT`/`SPLINE`
  ignorados e contados). Sem `<text>` em SVG — converta em caminhos antes.
* **Vetorização**: depende do `potrace` externo (contraste preto/branco, não
  traçado por cor como a "borboleta" do Studio).
* **Edição**: offset e fundir são operações por aproximação (offset direto
  com miter/bevel; união por rasterização ~0,1 mm). Não há editor de desenho
  (formas, texto, faca, borracha, subtrair/intercetar/dividir, replicar,
  PixScan, biblioteca/nuvem). **Vincar** é feito com a lâmina de vincar +
  força baixa (não há comando específico de score no protocolo documentado).
* **Print & cut**: imprima sempre em 100%; a folha gerada não embute
  sangria/espelhamento (para vinil, espelhe antes no seu editor).
* **Não testado em todas as máquinas**: o protocolo foi validado contra
  documentação da comunidade (fablabnbg/inkscape-silhouette, GPL) e
  capturas reais; o primeiro corte real deve ser em papel/velil barato,
  velocidade baixa.
* **Multi-toolholder Cameo4/5**: envia para a ferramenta 1 (`J1`).

## Licença

GPL-3.0-or-later. Os fatos de protocolo vêm de documentação de projeto GPL
(inkscape-silhouette); a implementação é original.
