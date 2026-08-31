# mcpiper

Text-to-speech con [Piper](https://github.com/rhasspy/piper) en **un solo
ejecutable**. Sin Python, sin DLLs sueltas, sin instalar espeak-ng: bajás el
binario, le pasás un modelo y un texto, y te devuelve un `.ogg`.

```
mcpiper --model ./model/ana --text "Hola" -o ./out.ogg
```

**[→ Guía de uso completa (USO.md)](USO.md)** — opciones, recetas, velocidad de
lectura, modelos multi-voz y solución de problemas.

## Qué trae adentro

| | |
|---|---|
| Inferencia | ONNX Runtime enlazado estáticamente |
| Fonemización | espeak-ng compilado adentro, con sus datos embebidos y comprimidos |
| Salida | Ogg Vorbis (por defecto) o WAV |
| Peso | ~23 MB, un único archivo |
| Velocidad | ~25-30× tiempo real en un x86_64 moderno, sólo CPU |

Lo único que queda afuera es el **modelo de voz** (`.onnx` + `.onnx.json`), que
pesa ~60 MB y se elige aparte.

## Instalación

Bajá el binario de tu plataforma desde la pestaña *Releases* y ponelo en el PATH.

```bash
# Linux / macOS
chmod +x mcpiper
./mcpiper --self-test          # verifica que corra en esta máquina
```

```powershell
# Windows
.\mcpiper.exe --self-test
```

> **Windows**: el ejecutable usa el runtime de Visual C++ (`vcruntime140.dll`,
> `msvcp140.dll`), que viene con el [VC++ Redistributable
> 2015-2022](https://aka.ms/vs/17/release/vc_redist.x64.exe). Está instalado en
> prácticamente cualquier Windows que haya corrido una app moderna, pero si
> `mcpiper.exe` no arranca, ése es el paquete que falta. No se puede evitar: el
> ONNX Runtime precompilado que usamos está enlazado contra el CRT dinámico.

## Conseguir una voz

Los modelos oficiales están en
[huggingface.co/rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices).
Cada voz son dos archivos, el `.onnx` y su `.onnx.json`.

```bash
mkdir -p model
B=https://huggingface.co/rhasspy/piper-voices/resolve/main/es/es_ES/davefx/medium
curl -L -o model/ana.onnx      $B/es_ES-davefx-medium.onnx
curl -L -o model/ana.onnx.json $B/es_ES-davefx-medium.onnx.json
```

El nombre que les pongas no importa mientras el JSON se llame igual que el
`.onnx` más `.json`. `--model ./model/ana` encuentra `ana.onnx` y
`ana.onnx.json` solo.

## Uso

```bash
# Lo básico
mcpiper --model ./model/ana --text "Hola" -o ./out.ogg

# Desde un archivo de texto
mcpiper -m ./model/ana -f guion.txt -o narracion.ogg

# Desde una tubería, escribiendo a la salida estándar
echo "Hola mundo" | mcpiper -m ./model/ana -o - > out.ogg

# WAV sin comprimir
mcpiper -m ./model/ana -t "Hola" -o out.wav

# Más lento y con menos variación en la entonación
mcpiper -m ./model/ana -t "Hola" --length-scale 1.2 --noise-scale 0.5 -o out.ogg

# Modelos con varias voces
mcpiper -m ./model/multi --list-speakers
mcpiper -m ./model/multi -t "Hola" --speaker Ana -o out.ogg
```

### Opciones

| Opción | Qué hace |
|---|---|
| `-m, --model <RUTA>` | El `.onnx`, su nombre sin extensión, o un directorio con uno |
| `-c, --config <RUTA>` | El JSON, si no está al lado del modelo |
| `-t, --text <TEXTO>` | Texto a leer (si falta, se lee de stdin) |
| `-f, --text-file <RUTA>` | Texto desde un archivo |
| `-o, --output <RUTA>` | Salida; `-` escribe a stdout |
| `--format vorbis\|wav` | Por defecto sale del sufijo: `.ogg`/`.oga` → Vorbis, `.wav` → WAV |
| `--quality <Q>` | Calidad VBR de Vorbis, de `-0.2` a `1.0` (por defecto `0.3`, ~52 kbps) |
| `--bitrate <BPS>` | Bitrate medio, en vez de apuntar a una calidad |
| `-s, --speaker <NOMBRE\|ID>` | Voz, en modelos multi-hablante |
| `--list-speakers` | Lista las voces del modelo y sale |
| `--length-scale <F>` | Velocidad: `>1` más lento, `<1` más rápido |
| `--noise-scale <F>` | Variación de la entonación |
| `--noise-w <F>` | Variación de la duración de cada fonema |
| `--sentence-silence <SEG>` | Pausa entre frases (por defecto `0.2`) |
| `--phonemes` | La entrada ya son fonemas IPA; saltea espeak-ng |
| `--espeak-data <DIR>` | Usar un `espeak-ng-data` del disco en vez del embebido |
| `--self-test` | Verifica el binario sin necesitar un modelo |
| `-q, --quiet` | No imprimir el resumen |

## Idiomas

El binario trae los diccionarios de espeak-ng de **español e inglés**. Alcanza
para cualquier voz Piper de esos idiomas; con una voz de otro idioma, espeak-ng
falla al fonemizar.

Para incluir más idiomas, se elige al compilar:

```bash
MCPIPER_ESPEAK_LANGS=es,en,pt,fr cargo build --release   # +unos pocos cientos de KB
MCPIPER_ESPEAK_LANGS=all         cargo build --release   # los ~100 idiomas, +4 MB
```

También se puede apuntar a un `espeak-ng-data` externo en tiempo de ejecución con
`--espeak-data`, sin recompilar.

## Compilar

Hace falta Rust estable, CMake y libclang (para `bindgen`).

```bash
# Debian/Ubuntu
sudo apt-get install -y cmake libclang-dev
# Arch
sudo pacman -S --needed cmake clang
# macOS: cmake por brew, libclang viene con Xcode
brew install cmake

cargo build --release
cargo test --release
```

El binario queda en `target/release/mcpiper`. La primera compilación tarda unos
minutos: se compila espeak-ng con CMake, se compilan libvorbis y libogg, y se baja el ONNX
Runtime precompilado.

### Binarios para las tres plataformas

`.github/workflows/build.yml` compila en runners nativos de Linux (x86_64 y
aarch64) y macOS (Intel y Apple Silicon), y llama a
`.github/workflows/windows.yml` para el `.exe`. Corre los tests y el
`--self-test` en cada plataforma, y al pushear un tag `vX.Y.Z` publica un
release con todos los artefactos.

```bash
git tag v0.1.0 && git push origin v0.1.0
```

**Sólo Windows**: `windows.yml` también se dispara solo, desde *Actions →
windows → Run workflow*, sin arrastrar Linux ni macOS. Deja
`mcpiper-windows-x86_64.zip` como artefacto del run y acepta dos parámetros:

| Parámetro | Por defecto | Qué hace |
|---|---|---|
| `langs` | `es,en` | Idiomas de espeak-ng a embeber; `all` los mete todos |
| `smoke_test` | activado | Baja una voz real y sintetiza, para probar ONNX Runtime de punta a punta |

Desde la línea de comandos, con el [CLI de GitHub](https://cli.github.com):

```bash
gh workflow run windows.yml -f langs=es,en -f smoke_test=true
gh run watch
```

No se cross-compila desde una sola máquina a propósito: el stack lleva tres
proyectos en C/C++ con CMake, y compilar en cada sistema operativo es mucho más
confiable que pelearse con toolchains cruzados.

## Cómo funciona

```
texto ──> espeak-ng ──> fonemas IPA ──> tabla del modelo ──> ids
                                                              │
                                                              ▼
                                          ONNX Runtime (VITS) ──> PCM f32 @22050
                                                              │
                                                  libvorbis   ▼
                                            (a 22050 Hz nativo) ──> paquetes Vorbis
                                                              │
                                            contenedor Ogg    ▼
                                                          out.ogg
```

Un par de detalles que importan:

- **El texto se corta en frases** antes de sintetizar. Cada frase pasa por el
  modelo por separado y se pega con `--sentence-silence` de silencio en el
  medio, que suena mucho mejor que darle un párrafo entero de una.
- **espeak-ng sólo sabe leer sus datos del disco.** En el primer arranque
  `mcpiper` los descomprime en la caché del usuario
  (`~/.cache/mcpiper/espeak-<hash>` en Linux) y se los pasa por variable de
  entorno. Los arranques siguientes reusan esa copia. El hash está en el nombre,
  así que actualizar el binario no deja basura vieja en uso.
- **Vorbis codifica a la frecuencia nativa del modelo**, sean 22050 o 16000 Hz.
  No hay remuestreo en el medio, así que no se pierde nada por ese lado.
- **El serial del flujo Ogg se deriva del contenido** en vez de sortearse, que
  es lo que hace la mayoría de los codificadores. Con `--noise-scale 0
  --noise-w 0`, el mismo texto produce siempre el mismo archivo byte a byte.

## Licencia

GPL-3.0-or-later. Se enlaza espeak-ng estáticamente, que es GPL-3, así que el
conjunto lo es. Ver [NOTICE.md](NOTICE.md) para el detalle de cada componente y
la nota sobre la licencia de los modelos de voz.
