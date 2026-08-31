# mcpiper usage guide

Everything you need to turn text into speech with `mcpiper`, from the first
command to the less obvious recipes.

- [Installation](#installation)
- [Getting a voice](#getting-a-voice)
- [First run](#first-run)
- [Option reference](#option-reference)
- [Controlling speed and delivery](#controlling-speed-and-delivery)
- [File format and size](#file-format-and-size)
- [Multi-speaker models](#multi-speaker-models)
- [Recipes](#recipes)
- [Languages](#languages)
- [Troubleshooting](#troubleshooting)
- [FAQ](#faq)

---

## Installation

There are binaries for **Windows x86_64** and **macOS Apple Silicon** under
*Releases*. It is a single file: it installs nothing and touches neither the
registry nor the system. On Linux or macOS Intel you have to build from source,
see the README.

```bash
# macOS
chmod +x mcpiper
sudo mv mcpiper /usr/local/bin/     # optional, to have it on the PATH
mcpiper --self-test
```

```powershell
# Windows
.\mcpiper.exe --self-test
```

`--self-test` checks that the executable starts, unpacks its phonemization data
and produces a valid Ogg Vorbis file. It needs no model at all. If that passes,
the rest will work.

```
mcpiper 0.1.0
espeak-ng-data : /home/you/.cache/mcpiper/espeak-096be95fc4eb23ea (languages: es,en)
phonemes es   : Hola mundo. -> ˈola mˈundo
phonemes en-us: Hello world. -> həlˈoʊ wˈɜːld
ogg vorbis     : 4805 bytes for 1.00 s of tone
wav            : 44144 bytes

all good.
```

> **Windows**: you need the [VC++ Redistributable
> 2015-2022](https://aka.ms/vs/17/release/vc_redist.x64.exe). It is almost always
> already installed; if `mcpiper.exe` does not open and says nothing, that is it.

---

## Getting a voice

`mcpiper` carries no voices inside: they weigh ~60 MB each and you need to be
able to pick. They all live at
[huggingface.co/rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices).

Every voice is **two files**: the `.onnx` model and its `.onnx.json`
configuration. Both, always, in the same directory and with the same base name.

### Spanish voices

| Voice | Region | Quality | Speakers | Size |
|---|---|---|---|---|
| `es_ES-carlfm-x_low` | Spain | x_low | 1 | 28 MB |
| `es_ES-davefx-medium` | Spain | medium | 1 | 63 MB |
| `es_ES-sharvard-medium` | Spain | medium | **2** (M/F) | 77 MB |
| `es_ES-mls_9972-low` | Spain | low | 1 | 63 MB |
| `es_ES-mls_10246-low` | Spain | low | 1 | 63 MB |
| `es_MX-ald-x_low` | Mexico | x_low | 1 | 21 MB |
| `es_MX-ald-medium` | Mexico | medium | 1 | 63 MB |
| `es_MX-claude-high` | Mexico | high | 1 | 63 MB |
| `es_AR-daniela-high` | Argentina | high | 1 | 114 MB |

There are also 38 English voices (`en_US-*` and `en_GB-*`), which this binary
supports without rebuilding.

`x_low` → `low` → `medium` → `high` is the quality scale. `medium` is the sweet
spot; `x_low` is there when size matters more than the result.

### Downloading one

The path in the repository always follows the same pattern:
`<language>/<region>/<name>/<quality>/<voice>.onnx`

```bash
mkdir -p model
B=https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/davefx/medium
curl -L -o model/ana.onnx      $B/es_ES-davefx-medium.onnx
curl -L -o model/ana.onnx.json $B/es_ES-davefx-medium.onnx.json
```

```powershell
# Windows
mkdir model
$B = "https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/davefx/medium"
curl.exe -L -o model\ana.onnx      "$B/es_ES-davefx-medium.onnx"
curl.exe -L -o model\ana.onnx.json "$B/es_ES-davefx-medium.onnx.json"
```

You can rename them however you like as long as the JSON is named the same as
the `.onnx` plus `.json`. With `ana.onnx` + `ana.onnx.json`, `--model
./model/ana` finds both on its own.

---

## First run

```bash
mcpiper --model ./model/ana --text "Hola" -o ./out.ogg
```

```
mcpiper: 0.39s of audio in 0.02s (19.2x realtime) -> ./out.ogg [2 KiB]
```

That summary line goes to standard error, not standard output, so it does not
pollute pipes. Turn it off with `-q`.

### The three ways to give it text

```bash
mcpiper -m ./model/ana -t "Hola mundo" -o out.ogg      # inline
mcpiper -m ./model/ana -f script.txt   -o out.ogg      # from a file
echo "Hola mundo" | mcpiper -m ./model/ana -o out.ogg  # from a pipe
```

### The three ways to point at the model

```bash
mcpiper -m ./model/ana        ...   # no extension: looks for ana.onnx
mcpiper -m ./model/ana.onnx   ...   # full path
mcpiper -m ./model            ...   # a directory, if it holds a single .onnx
```

---

## Option reference

### Input

| Option | Default | What it does |
|---|---|---|
| `-m, --model <PATH>` | *(required)* | The `.onnx`, its name without the extension, or a directory holding one |
| `-c, --config <PATH>` | `<model>.onnx.json` | The configuration JSON, if it is not next to the model |
| `-t, --text <TEXT>` | — | Text to read |
| `-f, --text-file <PATH>` | — | Text from a file |
| `--phonemes` | off | The input is already IPA phonemes; skips espeak-ng |

If you pass neither `--text` nor `--text-file`, it reads standard input.

### Output

| Option | Default | What it does |
|---|---|---|
| `-o, --output <PATH>` | *(required)* | Output file; `-` writes to standard output |
| `--format vorbis\|wav` | from the extension | `.ogg`/`.oga` → Vorbis, `.wav` → WAV |
| `--quality <Q>` | `0.3` | Vorbis VBR quality, from `-0.2` to `1.0` |
| `--bitrate <BPS>` | *(unused)* | Fixed average bitrate instead of a quality. Mutually exclusive with `--quality` |

Intermediate directories for `--output` are created automatically.

### Voice and prosody

| Option | Default | What it does |
|---|---|---|
| `-s, --speaker <NAME\|ID>` | `0` | Voice, on multi-speaker models |
| `--length-scale <F>` | the model's | Speed: `>1` slower, `<1` faster |
| `--noise-scale <F>` | the model's | Intonation variation |
| `--noise-w <F>` | the model's | Per-phoneme duration variation |
| `--sentence-silence <SEC>` | `0.2` | Pause between sentences |

### Utilities

| Option | What it does |
|---|---|
| `--list-speakers` | Shows the model's language, sample rate and voices, then exits |
| `--self-test` | Checks the binary without needing a model |
| `--espeak-data <DIR>` | Uses an `espeak-ng-data` from disk instead of the embedded one |
| `-q, --quiet` | Does not print the closing summary |
| `-h, --help` / `-V, --version` | Help and version |

---

## Controlling speed and delivery

### Speed: `--length-scale`

It is a multiplier on each phoneme's duration. **Higher = slower.**

Measured on the same sentence with the `davefx` voice:

| `--length-scale` | Duration | How it sounds |
|---|---|---|
| `0.7` | 2.03 s | rushed, starts swallowing syllables |
| `0.85` | 2.39 s | brisk, natural |
| `1.0` | 2.64 s | the default |
| `1.2` | 2.86 s | unhurried |
| `1.5` | 3.54 s | very slow, the delivery turns artificial |

The usable range is **0.75 – 1.4**. Beyond that, the model stretches or squeezes
the phonemes further than it ever saw in training and the voice degrades.

```bash
mcpiper -m ./model/ana -t "Hola" --length-scale 0.85 -o fast.ogg
mcpiper -m ./model/ana -t "Hola" --length-scale 1.25 -o slow.ogg
```

If you are always going to use the same speed, edit `inference.length_scale`
inside the `.onnx.json` and forget about the option.

### Pauses: `--sentence-silence`

This controls the silence **between sentences**, not the speaking rate. For an
unhurried narration, raising this usually beats lowering the speed:

```bash
mcpiper -m ./model/ana -f chapter.txt \
  --length-scale 1.1 --sentence-silence 0.45 -o chapter.ogg
```

### Expressiveness: `--noise-scale` and `--noise-w`

Piper models are VITS: they inject random noise so each reading sounds different.
That is what makes them natural, but it also means two runs over the same text do
not produce the same file.

| | What it controls | Lowering it |
|---|---|---|
| `--noise-scale` | Intonation variation | Flatter, more uniform voice |
| `--noise-w` | Phoneme duration variation | Even, predictable rhythm |

```bash
# Sober, consistent reading, useful for docs or system messages
mcpiper -m ./model/ana -t "Hola" --noise-scale 0.4 --noise-w 0.4 -o out.ogg

# Deterministic output: the same text always gives the same audio
mcpiper -m ./model/ana -t "Hola" --noise-scale 0 --noise-w 0 -o out.ogg
```

`--noise-w 0` is what you want if you are going to sync the audio with video or
subtitles: durations stop varying between runs.

---

## File format and size

The compressed output is **Ogg Vorbis**, at the model's native frequency (nothing
is resampled). There are two ways to ask for a size, and they are mutually
exclusive: `--quality` (VBR, the recommended one) or `--bitrate` (fixed average
bitrate).

### `--quality`

Over 6.48 s of speech, with a 22050 Hz model:

| Setting | Size | Actual bitrate |
|---|---|---|
| `--quality -0.2` | 20.1 KiB | 25 kbps |
| `--quality 0.0` | 28.4 KiB | 36 kbps |
| `--quality 0.2` | 36.0 KiB | 45 kbps |
| *(default, `0.3`)* | 40.8 KiB | 52 kbps |
| `--quality 0.4` | 46.3 KiB | 59 kbps |
| `--quality 0.6` | 59.6 KiB | 75 kbps |
| `--quality 0.8` | 73.2 KiB | 93 kbps |
| `--quality 1.0` | 86.7 KiB | 110 kbps |
| `--format wav` | 279.0 KiB | 353 kbps |

For synthesized speech, **the default is already transparent**: above `0.3` there
is next to nothing audible to gain. If size matters to you (messaging, IVR,
mobile apps), `--quality 0.0` sounds fine and takes a third less.

```bash
mcpiper -m ./model/ana -f text.txt --quality 0.0 -o light.ogg
```

### `--bitrate`

Useful when you need a predictable size (a fixed bandwidth, a quota).

| Setting | Size | Actual bitrate |
|---|---|---|
| `--bitrate 24000` | 22.1 KiB | 28 kbps |
| `--bitrate 32000` | 33.0 KiB | 42 kbps |
| `--bitrate 48000` | 49.7 KiB | 63 kbps |
| `--bitrate 64000` | 64.6 KiB | 82 kbps |
| `--bitrate 88000` | 85.0 KiB | 108 kbps |

Watch out: libvorbis only ships managed-bitrate modes for certain ranges, and
which range depends on the model's frequency. In mono, the usable ranges are:

| Model frequency | `--bitrate` range |
|---|---|
| 16000 Hz | 16000 – 96000 |
| 22050 Hz | 24000 – 88000 |
| 48000 Hz | 32000 – 192000 |

If you ask for one outside the range, `mcpiper` tells you what your model's real
range is. `--quality` has no such limitation.

WAV is for when the audio will keep being processed (mixing, editing, another
encoder): it is lossless and avoids encoding twice.

```bash
mcpiper -m ./model/ana -t "Hola" -o master.wav
```

---

## Multi-speaker models

Some models carry more than one speaker. `--list-speakers` tells you which:

```bash
mcpiper -m ./model/multi --list-speakers
```

```
espeak-ng voice : es
sample rate     : 22050 Hz
speakers        : 2
espeak-ng       : embedded languages = es,en

   0  M
   1  F
```

They can be picked by name or by number, it makes no difference:

```bash
mcpiper -m ./model/multi -t "Hola" --speaker F -o her.ogg
mcpiper -m ./model/multi -t "Hola" --speaker 1 -o her.ogg
```

Without `--speaker`, it uses speaker `0`.

---

## Recipes

### One file per line

```bash
n=1
while IFS= read -r line; do
  mcpiper -m ./model/ana -t "$line" -o "audio/$(printf '%03d' $n).ogg" -q
  n=$((n+1))
done < lines.txt
```

### An audiobook from several chapters

```bash
for f in chapters/*.txt; do
  mcpiper -m ./model/ana -f "$f" \
    --length-scale 1.1 --sentence-silence 0.5 --bitrate 32000 \
    -o "audio/$(basename "$f" .txt).ogg"
done
```

### As part of a pipeline

`-o -` writes the Ogg to standard output, so it chains without intermediate
files:

```bash
# Play without saving anything
echo "Hola mundo" | mcpiper -m ./model/ana -o - | ffplay -nodisp -autoexit -

# Convert to MP3 on the fly
mcpiper -m ./model/ana -f text.txt -o - | ffmpeg -i - out.mp3

# Send it over the network
mcpiper -m ./model/ana -t "Alerta" -o - | curl -X POST --data-binary @- https://…
```

If `-o -` points at a terminal, `mcpiper` refuses and says so, instead of
spewing binary onto the screen.

### Reading another program's output

```bash
df -h / | tail -1 | awk '{print "El disco está al " $5}' \
  | mcpiper -m ./model/ana -o /tmp/notice.ogg -q
```

### Notifications from a script

```bash
notify() {
  mcpiper -m "$HOME/voices/ana" -t "$1" -o /tmp/notice.ogg -q && \
  paplay /tmp/notice.ogg 2>/dev/null || ffplay -nodisp -autoexit -v quiet /tmp/notice.ogg
}

make && notify "Compilación terminada" || notify "La compilación falló"
```

### Windows / PowerShell

```powershell
# Several lines from a file
Get-Content lines.txt | ForEach-Object -Begin { $i = 1 } -Process {
  .\mcpiper.exe -m .\model\ana -t $_ -o "audio\$('{0:d3}' -f $i).ogg" -q
  $i++
}

# Play it right away
.\mcpiper.exe -m .\model\ana -t "Listo" -o out.ogg -q
Start-Process out.ogg
```

### Custom pronunciation with `--phonemes`

When espeak-ng gets a proper noun or an acronym wrong, you can hand it the IPA
phonemes yourself:

```bash
mcpiper -m ./model/ana --phonemes -t "ˈola mˈundo" -o out.ogg
```

To see what phonemes espeak-ng normally produces and use them as a base:

```bash
espeak-ng -v es -q --ipa "your text"    # if you have espeak-ng installed separately
```

With `--phonemes` the text is **not** split into sentences: it goes in as it is,
and `--sentence-silence` does not apply.

---

## Languages

The binary carries the espeak-ng dictionaries for **Spanish and English**. That
covers any of the 47 Piper voices in those languages.

With a model in another language it fails up front and tells you what to do:

```
mcpiper: this binary does not carry the espeak-ng data for the voice `fr` the model asks for (it includes: es,en).
  Options: rebuild with MCPIPER_ESPEAK_LANGS="es,en,fr", or pass --espeak-data pointing at a full espeak-ng-data from the system.
```

Two ways out, whichever you prefer:

**Rebuild with more languages** — the binary stays self-contained:

```bash
MCPIPER_ESPEAK_LANGS=es,en,fr,pt cargo build --release   # +a few hundred KB
MCPIPER_ESPEAK_LANGS=all         cargo build --release   # all ~100 languages, +4 MB
```

**Use the system's data** — no rebuild, but it stops being a single file:

```bash
sudo apt-get install espeak-ng-data
mcpiper -m ./model/fr --espeak-data /usr/share/espeak-ng-data -t "Bonjour" -o out.ogg
```

`--espeak-data` accepts either the `espeak-ng-data` directory or the one
containing it.

---

## Troubleshooting

### `could not find the model: tried ... and ...`

The `--model` path exists neither with nor without `.onnx`. Check the name; mind
`.onnx.json` vs `.onnx`.

### `could not find the model's configuration; tried ... and ...`

The `.onnx` is there but its JSON is missing. Download it from the same directory
of the voices repository, with the same base name. Or pass it by hand with
`--config`.

### `this binary does not carry the espeak-ng data for the voice X`

The model is in a language that is not embedded. See [Languages](#languages).

### `Can't read dictionary file: ...`

That message comes from espeak-ng, and it always arrives alongside the previous
error. Same cause: the language is missing.

### `the text produced no phonemes; is it empty, or only punctuation?`

The text has nothing pronounceable in it — only signs, spaces or emoji.

### `don't know what to do with the '.mp3' extension; use --format vorbis|wav`

`mcpiper` only writes Ogg Vorbis and WAV. For MP3, chain it with ffmpeg:

```bash
mcpiper -m ./model/ana -t "Hola" -o - | ffmpeg -i - out.mp3
```

### `unknown speaker 'X'. Available: ...` / `is out of range`

Run `--list-speakers` to see the real names and numbers.

### `this model has a single voice, --speaker does not apply`

Drop `--speaker`, or use a multi-speaker model.

### `refusing to dump binary audio to the terminal`

You used `-o -` without redirecting. Add `> file.ogg` or chain it into a pipe.

### The executable does not start on Windows

The [VC++ Redistributable
2015-2022](https://aka.ms/vs/17/release/vc_redist.x64.exe) is missing.

### The first start takes a little longer

That is normal: the first time, it unpacks the espeak-ng data into the cache
(`~/.cache/mcpiper` on Linux, `~/Library/Caches/mcpiper` on macOS,
`%LOCALAPPDATA%\mcpiper` on Windows). After that it is reused. You can delete
that folder whenever you like; it regenerates itself.

---

## FAQ

**Does it need internet?**
No. Neither to start nor to synthesize. Only to download the binary and the
voices the first time.

**Does it need a GPU?**
No, it runs on CPU. It gives ~20-30× realtime on a modern desktop machine: a
minute of audio in a couple of seconds.

**Do two runs over the same text give the same file?**
Not by default — the model has randomness in it. With `--noise-scale 0
--noise-w 0` they do, byte for byte.

**Can I hand it a long text in one go?**
Yes. It is split into sentences automatically and each one goes through the model
separately, which sounds far better than feeding it a whole paragraph. There is
no practical length limit.

**What frequency does the `.ogg` come out at?**
The model's, exactly: 22050 Hz on almost every voice, 16000 Hz on some `x_low`
ones. Vorbis encodes at the native frequency, so there is no resampling and no
loss in between. `--list-speakers` confirms it by printing the model's sample
rate.

**How do I change the speed?**
`--length-scale`. See [Controlling speed and
delivery](#controlling-speed-and-delivery).

**Can I use it on a server or in a product?**
Technically yes. Bear in mind that `mcpiper` is GPL-3.0-or-later (because of
espeak-ng, which is linked statically) and that **each voice carries its own
license** — check the `MODEL_CARD` of the one you use. See [NOTICE.md](NOTICE.md).

**Is there a library, not just the CLI?**
This project is only the executable. Underneath it uses
[piper-rs](https://github.com/thewh1teagle/piper-rs), which is a Rust library.
