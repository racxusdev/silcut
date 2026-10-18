# silcut

[Português](README.md) · **English**

Native **Linux** tool (Rust, no Wine) for **Silhouette / Graphtec** cutters:
imports SVG/DXF, previews, configures and sends cuts straight over USB using
the **GPGL** protocol (1 unit = 0.05 mm). Includes **registration marks** for a
*print & cut* workflow, like Silhouette Studio.

Two programs:

| binary | what it does |
|---|---|
| `silcut` | CLI: devices, cut, trace, mark sheet, media |
| `silcut-gui` | GUI: canvas with zoom/pan and grid, parameters, print & cut, cutting |

## AppImage (portable, no install)

```bash
chmod +x silcut-*.AppImage
./silcut-*.AppImage
```

`./build.sh` produces the AppImage (needs `appimagetool`). If `linuxdeploy` is
on the PATH, `build.sh` uses it and bundles GL/xcb/xkbcommon so the AppImage is
fully self-contained.

## Install without AppImage

```bash
./install.sh        # builds, copies to ~/.local/bin, adds menu entry
./install.sh --uninstall
```

## Build from source

```bash
cargo build --release          # binaries in target/release/
./build.sh                     # release + portable package in dist/
```

libusb is compiled in (the `vendored` feature of `rusb`) — no `libusb-1.0-dev`
required.

## USB permissions (required once)

```bash
sudo cp 70-silhouette.rules /etc/udev/rules.d/
sudo udevadm control --reload && sudo udevadm trigger
# unplug and replug the machine
```

## Test without hardware (dry run)

```bash
cargo run --bin silcut -- cut examples/teste-quadrado.svg --dry-run -o cut.gpgl
cargo run --bin silcut -- devices
```

`--dry-run` validates the whole pipeline and writes the GPGL program as text —
useful to inspect the commands (`FG`, `TB50,0`, `M/D`, `TB123` regmarks, …)
without a machine.

## GUI

```bash
silcut-gui
```

* **Canvas** — paper drawn to scale, 10 mm grid, mm rulers, paths in blue,
  registration marks in black. Wheel = zoom at cursor, drag = pan, drag the
  marks to reposition them.
* **Drag & drop** an SVG (or PNG/JPG) straight into the window.
* **Side panels** — file (SVG/DXF + image tracing), sheet & media presets (28
  media), force/blade/speed/passes, position & size (fit/stretch/center),
  offset and weld, print & cut marks, GPGL dry run.
* **✂ CUT** opens a full summary (machine, sheet, force, drawing, position,
  estimated time, validation warnings) before running; cutting happens on a
  worker thread with a live progress bar.
* Settings are stored in `~/.config/silcut/config.toml`.

## Print & cut (registration marks)

Marks follow the Silhouette Studio layout: a **solid black 10 × 10 mm square**
in the top-left corner (the sensor starts reading there) plus two **L** marks
in the top-right and bottom-left corners, 10 mm arms (adjustable 6–30 mm),
0.5 mm thick (up to 2.0 mm for hard-to-read media). The preview shows the
**exclusion zone** around the marks.

1. Enable registration marks in the GUI and click
   **Export print sheet…** — writes an SVG with the drawing plus the marks.
2. **Print at 100 % scale** (disable "fit to page").
3. Stick the media on the mat and load it.
4. Cut with marks enabled (GUI, or `silcut cut --regmarks`): the optical
   sensor locates the marks (`TB123` automatic search, or `TB23` manual) and
   compensates for print distortion before cutting.

CLI:

```bash
silcut marks --design drawing.svg -o sheet.svg
silcut cut sheet.svg --regmarks --reg-width 180 --reg-length 230 \
                     --reg-origin-x 15 --reg-origin-y 20
```

## Supported machines

Cameo (1/2/3/4/4 Plus/4 Pro/5/5 Plus/Pro MkII/5 Alpha), Portrait (1/2/3/4),
SD, Craft Robo — VID `0b4d` (and `3844` for Cameo 5 Alpha). Full table with
`silcut media` (28 media presets).

## Honest limitations

* **Formats**: SVG (paths and basic shapes with transforms and units) and
  **DXF** (LINE/LWPOLYLINE with bulge/POLYLINE/CIRCLE/ARC; `INSERT`/`SPLINE`
  are skipped and counted). No `<text>` in SVG — convert it to paths first.
* **Tracing** depends on external `potrace` (black/white contrast, not
  per-colour tracing).
* **Editing**: offset and weld are approximations; there is no full drawing
  editor (no shapes, text, knife, eraser, boolean ops, PixScan, cloud library).
* **Print & cut**: always print at 100 %; the generated sheet has no bleed or
  mirroring.
* **Not validated on every machine**: the protocol comes from community
  documentation (fablabnbg/inkscape-silhouette, GPL) and real USB captures —
  the first real cut should be cheap paper, low speed.
* **Cameo 4/5 multi-toolholder**: sends to tool 1 (`J1`).

## License

GPL-3.0-or-later. Protocol facts come from GPL-licensed community
documentation (inkscape-silhouette); the implementation is original.
