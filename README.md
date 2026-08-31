# mcpiper

Text-to-speech with [Piper](https://github.com/rhasspy/piper) in **a single
executable**. No Python, no loose DLLs, no espeak-ng to install: download the
binary, hand it a model and some text, and it gives you back an `.ogg`.

```
mcpiper --model ./model/ana --text "Hola" -o ./out.ogg
```

**[→ Full usage guide (USAGE.md)](USAGE.md)** — options, recipes, reading speed,
multi-voice models and troubleshooting.

## What is inside

| | |
|---|---|
| Inference | ONNX Runtime, statically linked |
| Phonemization | espeak-ng compiled in, with its data embedded and compressed |
| Output | Ogg Vorbis (default) or WAV |
| Size | ~23 MB, a single file |
| Speed | ~25-30× realtime on a modern x86_64, CPU only |

The only thing left out is the **voice model** (`.onnx` + `.onnx.json`), which
weighs ~60 MB and is chosen separately.

## Installation

There are binaries for **Windows x86_64** and **macOS Apple Silicon** under the
*Releases* tab; download one and put it on your PATH. For Linux or macOS Intel,
see [Building](#building).

```bash
# macOS
chmod +x mcpiper
./mcpiper --self-test          # checks that it runs on this machine
```

```powershell
# Windows
.\mcpiper.exe --self-test
```

> **Windows**: the executable uses the Visual C++ runtime (`vcruntime140.dll`,
> `msvcp140.dll`), which ships with the [VC++ Redistributable
> 2015-2022](https://aka.ms/vs/17/release/vc_redist.x64.exe). It is installed on
> practically any Windows that has run a modern app, but if `mcpiper.exe` does
> not start, that is the missing package. There is no way around it: the
> prebuilt ONNX Runtime we use is linked against the dynamic CRT.

## Getting a voice

The official models live at
[huggingface.co/rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices).
Every voice is two files, the `.onnx` and its `.onnx.json`.

```bash
mkdir -p model
B=https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/davefx/medium
curl -L -o model/ana.onnx      $B/es_ES-davefx-medium.onnx
curl -L -o model/ana.onnx.json $B/es_ES-davefx-medium.onnx.json
```

The name you give them does not matter as long as the JSON is named the same as
the `.onnx` plus `.json`. `--model ./model/ana` finds `ana.onnx` and
`ana.onnx.json` on its own.

## Usage

```bash
# The basics
mcpiper --model ./model/ana --text "Hola" -o ./out.ogg

# From a text file
mcpiper -m ./model/ana -f script.txt -o narration.ogg

# From a pipe, writing to standard output
echo "Hola mundo" | mcpiper -m ./model/ana -o - > out.ogg

# Uncompressed WAV
mcpiper -m ./model/ana -t "Hola" -o out.wav

# Slower and with less variation in the delivery
mcpiper -m ./model/ana -t "Hola" --length-scale 1.2 --noise-scale 0.5 -o out.ogg

# Multi-voice models
mcpiper -m ./model/multi --list-speakers
mcpiper -m ./model/multi -t "Hola" --speaker Ana -o out.ogg
```

### Options

| Option | What it does |
|---|---|
| `-m, --model <PATH>` | The `.onnx`, its name without the extension, or a directory holding one |
| `-c, --config <PATH>` | The JSON, if it is not next to the model |
| `-t, --text <TEXT>` | Text to read (if absent, it is read from stdin) |
| `-f, --text-file <PATH>` | Text from a file |
| `-o, --output <PATH>` | Output; `-` writes to stdout |
| `--format vorbis\|wav` | By default it follows the suffix: `.ogg`/`.oga` → Vorbis, `.wav` → WAV |
| `--quality <Q>` | Vorbis VBR quality, from `-0.2` to `1.0` (default `0.3`, ~52 kbps) |
| `--bitrate <BPS>` | Average bitrate, instead of targeting a quality |
| `-s, --speaker <NAME\|ID>` | Voice, on multi-speaker models |
| `--list-speakers` | Lists the model's voices and exits |
| `--length-scale <F>` | Speed: `>1` slower, `<1` faster |
| `--noise-scale <F>` | Intonation variation |
| `--noise-w <F>` | Per-phoneme duration variation |
| `--sentence-silence <SEC>` | Pause between sentences (default `0.2`) |
| `--phonemes` | The input is already IPA phonemes; skips espeak-ng |
| `--espeak-data <DIR>` | Use an `espeak-ng-data` from disk instead of the embedded one |
| `--self-test` | Checks the binary without needing a model |
| `-q, --quiet` | Do not print the summary |

## Languages

The binary carries the espeak-ng dictionaries for **Spanish and English**. That
is enough for any Piper voice in those languages; with a voice in another
language, espeak-ng fails to phonemize.

To include more languages, pick them at build time:

```bash
MCPIPER_ESPEAK_LANGS=es,en,pt,fr cargo build --release   # +a few hundred KB
MCPIPER_ESPEAK_LANGS=all         cargo build --release   # all ~100 languages, +4 MB
```

You can also point at an external `espeak-ng-data` at runtime with
`--espeak-data`, without rebuilding.

## Building

You need stable Rust, CMake and libclang (for `bindgen`).

```bash
# Debian/Ubuntu
sudo apt-get install -y cmake libclang-dev
# Arch
sudo pacman -S --needed cmake clang
# macOS: cmake from brew, libclang comes with Xcode
brew install cmake

cargo build --release
cargo test --release
```

The binary lands in `target/release/mcpiper`. The first build takes a few
minutes: espeak-ng is compiled with CMake, libvorbis and libogg are compiled, and
the prebuilt ONNX Runtime is downloaded.

Two things about the build worth knowing:

- **espeak-ng gets compiled twice.** `mcpiper` also declares `espeak-rs-sys` as a
  build dependency. It never uses it from code: that is the only way to make
  cargo guarantee `espeak-ng-data` exists before our `build.rs` runs, which is
  what packs it into the executable. Build dependencies use a different profile,
  so they never unify with the normal ones.
- **The project path cannot be very deep.** espeak-ng keeps its data directory in
  a 160-character buffer, and during the build that path is
  `<target>/release/build/espeak-rs-sys-<hash>/out/build` (some 60 characters
  more than the project directory). If it overflows, compiling the data fails
  with `Error processing file '.../phsource/intonation'`. A short
  `CARGO_TARGET_DIR` fixes it.

### Published binaries

`.github/workflows/build.yml` builds on a native macOS Apple Silicon runner and
calls `.github/workflows/windows.yml` for the `.exe`. It runs the tests and
`--self-test` on both, and pushing a `vX.Y.Z` tag publishes a release with both
artifacts.

Those are the two platforms that get published. On Linux, and on macOS Intel,
you have to build from source — it works just the same, there is simply no
ready-made binary. To publish them again, adding the matching entry to
`build.yml`'s matrix is enough.

```bash
git tag v0.1.0 && git push origin v0.1.0
```

**Windows only**: `windows.yml` can also be triggered on its own, from *Actions →
windows → Run workflow*, without dragging the macOS job along. It leaves
`mcpiper-windows-x86_64.zip` as a run artifact and takes two parameters:

| Parameter | Default | What it does |
|---|---|---|
| `langs` | `es,en` | espeak-ng languages to embed; `all` puts them all in |
| `smoke_test` | on | Downloads a real voice and synthesizes, to exercise ONNX Runtime end to end |

From the command line, with the [GitHub CLI](https://cli.github.com):

```bash
gh workflow run windows.yml -f langs=es,en -f smoke_test=true
gh run watch
```

Cross-compilation is deliberately avoided: the stack carries three C/C++ projects
built with CMake, and compiling on each operating system is far more reliable
than fighting with cross toolchains.

## How it works

```
text ──> espeak-ng ──> IPA phonemes ──> model table ──> ids
                                                         │
                                                         ▼
                                     ONNX Runtime (VITS) ──> PCM f32 @22050
                                                         │
                                             libvorbis   ▼
                                      (at 22050 Hz native) ──> Vorbis packets
                                                         │
                                          Ogg container  ▼
                                                     out.ogg
```

A few details that matter:

- **The text is split into sentences** before synthesis. Each sentence goes
  through the model separately and they are glued together with
  `--sentence-silence` of silence in between, which sounds far better than
  feeding it a whole paragraph at once.
- **espeak-ng only knows how to read its data from disk.** On the first run
  `mcpiper` unpacks it into the user's cache (`~/.cache/mcpiper/espeak-<hash>` on
  Linux) and hands it over through an environment variable. Later runs reuse that
  copy. The hash is in the name, so updating the binary leaves no stale data in
  use.
- **Vorbis encodes at the model's native frequency**, be it 22050 or 16000 Hz.
  There is no resampling in between, so nothing is lost on that account.
- **The Ogg stream serial is derived from the content** instead of being drawn at
  random, which is what most encoders do. With `--noise-scale 0 --noise-w 0`, the
  same text always produces the same file byte for byte.

## License

GPL-3.0-or-later. espeak-ng is linked statically and it is GPL-3, so the whole is
too. See [NOTICE.md](NOTICE.md) for the per-component detail and the note about
the voice models' licensing.
