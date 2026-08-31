# Guía de uso de mcpiper

Todo lo que necesitás para convertir texto en voz con `mcpiper`, desde el primer
comando hasta las recetas menos obvias.

- [Instalación](#instalación)
- [Conseguir una voz](#conseguir-una-voz)
- [Primer uso](#primer-uso)
- [Referencia de opciones](#referencia-de-opciones)
- [Controlar la velocidad y el tono](#controlar-la-velocidad-y-el-tono)
- [Formato y peso del archivo](#formato-y-peso-del-archivo)
- [Modelos con varias voces](#modelos-con-varias-voces)
- [Recetas](#recetas)
- [Idiomas](#idiomas)
- [Solución de problemas](#solución-de-problemas)
- [Preguntas frecuentes](#preguntas-frecuentes)

---

## Instalación

Bajá el binario de tu plataforma desde *Releases*. Es un solo archivo, no
instala nada, no toca el registro ni el sistema.

```bash
# Linux / macOS
chmod +x mcpiper
sudo mv mcpiper /usr/local/bin/     # opcional, para tenerlo en el PATH
mcpiper --self-test
```

```powershell
# Windows
.\mcpiper.exe --self-test
```

`--self-test` verifica que el ejecutable arranque, descomprima sus datos de
fonemización y produzca un Ogg Vorbis válido. No necesita ningún modelo. Si eso
pasa, el resto va a funcionar.

```
mcpiper 0.1.0
espeak-ng-data : /home/vos/.cache/mcpiper/espeak-096be95fc4eb23ea (idiomas: es,en)
fonemas es    : Hola mundo. -> ˈola mˈundo
fonemas en-us : Hello world. -> həlˈoʊ wˈɜːld
ogg vorbis     : 4805 bytes para 1,00 s de tono
wav            : 44144 bytes

todo en orden.
```

> **Windows**: hace falta el [VC++ Redistributable
> 2015-2022](https://aka.ms/vs/17/release/vc_redist.x64.exe). Casi siempre ya
> está instalado; si `mcpiper.exe` no abre y no dice nada, es eso.

---

## Conseguir una voz

`mcpiper` no trae voces adentro: pesan ~60 MB cada una y hay que poder elegir.
Están todas en
[huggingface.co/rhasspy/piper-voices](https://huggingface.co/rhasspy/piper-voices).

Cada voz son **dos archivos**: el modelo `.onnx` y su configuración
`.onnx.json`. Los dos, siempre, en el mismo directorio y con el mismo nombre
base.

### Voces en español

| Voz | Región | Calidad | Hablantes | Peso |
|---|---|---|---|---|
| `es_ES-carlfm-x_low` | España | x_low | 1 | 28 MB |
| `es_ES-davefx-medium` | España | medium | 1 | 63 MB |
| `es_ES-sharvard-medium` | España | medium | **2** (M/F) | 77 MB |
| `es_ES-mls_9972-low` | España | low | 1 | 63 MB |
| `es_ES-mls_10246-low` | España | low | 1 | 63 MB |
| `es_MX-ald-x_low` | México | x_low | 1 | 21 MB |
| `es_MX-ald-medium` | México | medium | 1 | 63 MB |
| `es_MX-claude-high` | México | high | 1 | 63 MB |
| `es_AR-daniela-high` | Argentina | high | 1 | 114 MB |

También hay 38 voces en inglés (`en_US-*` y `en_GB-*`), que este binario soporta
sin recompilar.

`x_low` → `low` → `medium` → `high` es la escala de calidad. `medium` es el punto
dulce; `x_low` sirve si el peso importa más que el resultado.

### Bajarla

La ruta en el repositorio sigue siempre el mismo patrón:
`<idioma>/<región>/<nombre>/<calidad>/<voz>.onnx`

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

Podés renombrarlos como quieras mientras el JSON se llame igual que el `.onnx`
más `.json`. Con `ana.onnx` + `ana.onnx.json`, `--model ./model/ana` encuentra
los dos solo.

---

## Primer uso

```bash
mcpiper --model ./model/ana --text "Hola" -o ./out.ogg
```

```
mcpiper: 0.39s de audio en 0.02s (19.2x tiempo real) -> ./out.ogg [2 KiB]
```

Esa línea de resumen va a la salida de error, no a la estándar, así que no
ensucia las tuberías. Se apaga con `-q`.

### Las tres formas de darle el texto

```bash
mcpiper -m ./model/ana -t "Hola mundo" -o out.ogg      # directo
mcpiper -m ./model/ana -f guion.txt    -o out.ogg      # desde un archivo
echo "Hola mundo" | mcpiper -m ./model/ana -o out.ogg  # desde una tubería
```

### Las tres formas de indicar el modelo

```bash
mcpiper -m ./model/ana        ...   # sin extensión: busca ana.onnx
mcpiper -m ./model/ana.onnx   ...   # ruta completa
mcpiper -m ./model            ...   # un directorio, si adentro hay un solo .onnx
```

---

## Referencia de opciones

### Entrada

| Opción | Por defecto | Qué hace |
|---|---|---|
| `-m, --model <RUTA>` | *(obligatoria)* | El `.onnx`, su nombre sin extensión, o un directorio que contenga uno |
| `-c, --config <RUTA>` | `<modelo>.onnx.json` | El JSON de configuración, si no está al lado del modelo |
| `-t, --text <TEXTO>` | — | Texto a leer |
| `-f, --text-file <RUTA>` | — | Texto desde un archivo |
| `--phonemes` | apagado | La entrada ya son fonemas IPA; saltea espeak-ng |

Si no pasás ni `--text` ni `--text-file`, lee de la entrada estándar.

### Salida

| Opción | Por defecto | Qué hace |
|---|---|---|
| `-o, --output <RUTA>` | *(obligatoria)* | Archivo de salida; `-` escribe a la salida estándar |
| `--format vorbis\|wav` | por la extensión | `.ogg`/`.oga` → Vorbis, `.wav` → WAV |
| `--quality <Q>` | `0.3` | Calidad VBR de Vorbis, de `-0.2` a `1.0` |
| `--bitrate <BPS>` | *(sin usar)* | Bitrate medio fijo, en vez de calidad. Excluyente con `--quality` |

Los directorios intermedios de `--output` se crean solos.

### Voz y prosodia

| Opción | Por defecto | Qué hace |
|---|---|---|
| `-s, --speaker <NOMBRE\|ID>` | `0` | Voz, en modelos multi-hablante |
| `--length-scale <F>` | la del modelo | Velocidad: `>1` más lento, `<1` más rápido |
| `--noise-scale <F>` | la del modelo | Variación de la entonación |
| `--noise-w <F>` | la del modelo | Variación de la duración de cada fonema |
| `--sentence-silence <SEG>` | `0.2` | Pausa entre frases |

### Utilidades

| Opción | Qué hace |
|---|---|
| `--list-speakers` | Muestra idioma, sample rate y voces del modelo, y sale |
| `--self-test` | Verifica el binario sin necesitar un modelo |
| `--espeak-data <DIR>` | Usa un `espeak-ng-data` del disco en vez del embebido |
| `-q, --quiet` | No imprime el resumen final |
| `-h, --help` / `-V, --version` | Ayuda y versión |

---

## Controlar la velocidad y el tono

### Velocidad: `--length-scale`

Es un multiplicador de la duración de cada fonema. **Mayor = más lento.**

Medido sobre la misma frase con la voz `davefx`:

| `--length-scale` | Duración | Cómo suena |
|---|---|---|
| `0.7` | 2.03 s | apurado, empieza a comerse sílabas |
| `0.85` | 2.39 s | ágil, natural |
| `1.0` | 2.64 s | por defecto |
| `1.2` | 2.86 s | pausado |
| `1.5` | 3.54 s | muy lento, el tono se vuelve artificial |

El rango usable es **0.75 – 1.4**. Más allá, el modelo estira o comprime los
fonemas más de lo que vio entrenando y la voz se degrada.

```bash
mcpiper -m ./model/ana -t "Hola" --length-scale 0.85 -o rapido.ogg
mcpiper -m ./model/ana -t "Hola" --length-scale 1.25 -o lento.ogg
```

Si siempre lo vas a usar a la misma velocidad, editá `inference.length_scale`
dentro del `.onnx.json` y te olvidás de la opción.

### Pausas: `--sentence-silence`

Controla el silencio **entre frases**, no la velocidad del habla. Para una
narración pausada suele quedar mejor subir esto que bajar la velocidad:

```bash
mcpiper -m ./model/ana -f capitulo.txt \
  --length-scale 1.1 --sentence-silence 0.45 -o capitulo.ogg
```

### Expresividad: `--noise-scale` y `--noise-w`

Los modelos Piper son VITS: meten ruido aleatorio para que cada lectura suene
distinta. Eso da naturalidad, pero también hace que dos corridas del mismo texto
no den el mismo archivo.

| | Qué controla | Bajarlo |
|---|---|---|
| `--noise-scale` | Variación de la entonación | Voz más plana y uniforme |
| `--noise-w` | Variación de la duración de los fonemas | Ritmo parejo y predecible |

```bash
# Lectura sobria y consistente, útil para documentación o mensajes de sistema
mcpiper -m ./model/ana -t "Hola" --noise-scale 0.4 --noise-w 0.4 -o out.ogg

# Salida determinista: el mismo texto da siempre el mismo audio
mcpiper -m ./model/ana -t "Hola" --noise-scale 0 --noise-w 0 -o out.ogg
```

`--noise-w 0` es lo que querés si vas a sincronizar el audio con video o
subtítulos: la duración deja de variar entre corridas.

---

## Formato y peso del archivo

La salida comprimida es **Ogg Vorbis**, a la frecuencia nativa del modelo (no se
remuestrea nada). Hay dos maneras de pedir el tamaño, y son excluyentes:
`--quality` (VBR, lo recomendado) o `--bitrate` (bitrate medio fijo).

### `--quality`

Sobre 6.48 s de habla, con un modelo de 22050 Hz:

| Ajuste | Tamaño | Bitrate real |
|---|---|---|
| `--quality -0.2` | 20.1 KiB | 25 kbps |
| `--quality 0.0` | 28.4 KiB | 36 kbps |
| `--quality 0.2` | 36.0 KiB | 45 kbps |
| *(por defecto, `0.3`)* | 40.8 KiB | 52 kbps |
| `--quality 0.4` | 46.3 KiB | 59 kbps |
| `--quality 0.6` | 59.6 KiB | 75 kbps |
| `--quality 0.8` | 73.2 KiB | 93 kbps |
| `--quality 1.0` | 86.7 KiB | 110 kbps |
| `--format wav` | 279.0 KiB | 353 kbps |

Para voz sintetizada, **el default ya es transparente**: por encima de `0.3` casi
no se gana nada audible. Si te importa el peso (mensajes, IVR, apps móviles),
`--quality 0.0` suena bien y ocupa un tercio menos.

```bash
mcpiper -m ./model/ana -f texto.txt --quality 0.0 -o liviano.ogg
```

### `--bitrate`

Sirve cuando necesitás un tamaño previsible (un ancho de banda fijo, una cuota).

| Ajuste | Tamaño | Bitrate real |
|---|---|---|
| `--bitrate 24000` | 22.1 KiB | 28 kbps |
| `--bitrate 32000` | 33.0 KiB | 42 kbps |
| `--bitrate 48000` | 49.7 KiB | 63 kbps |
| `--bitrate 64000` | 64.6 KiB | 82 kbps |
| `--bitrate 88000` | 85.0 KiB | 108 kbps |

Ojo: libvorbis sólo trae modos de bitrate manejado para ciertos rangos, y cuál
es depende de la frecuencia del modelo. Mono, los rangos utilizables son:

| Frecuencia del modelo | Rango de `--bitrate` |
|---|---|
| 16000 Hz | 16000 – 96000 |
| 22050 Hz | 24000 – 88000 |
| 48000 Hz | 32000 – 192000 |

Si pedís uno fuera de rango, `mcpiper` te dice cuál es el rango real de tu
modelo. Con `--quality` no existe esa limitación.

WAV sirve para cuando el audio va a seguir procesándose (mezcla, edición,
otro codificador): no tiene pérdida y evita recodificar dos veces.

```bash
mcpiper -m ./model/ana -t "Hola" -o master.wav
```

---

## Modelos con varias voces

Algunos modelos traen más de un hablante. `--list-speakers` te dice cuáles:

```bash
mcpiper -m ./model/multi --list-speakers
```

```
voz espeak-ng : es
sample rate   : 22050 Hz
hablantes     : 2
espeak-ng     : idiomas embebidos = es,en

   0  M
   1  F
```

Se eligen por nombre o por número, da lo mismo:

```bash
mcpiper -m ./model/multi -t "Hola" --speaker F -o ella.ogg
mcpiper -m ./model/multi -t "Hola" --speaker 1 -o ella.ogg
```

Sin `--speaker`, usa el hablante `0`.

---

## Recetas

### Un archivo por línea

```bash
n=1
while IFS= read -r linea; do
  mcpiper -m ./model/ana -t "$linea" -o "audio/$(printf '%03d' $n).ogg" -q
  n=$((n+1))
done < frases.txt
```

### Un audiolibro desde varios capítulos

```bash
for f in capitulos/*.txt; do
  mcpiper -m ./model/ana -f "$f" \
    --length-scale 1.1 --sentence-silence 0.5 --bitrate 32000 \
    -o "audio/$(basename "$f" .txt).ogg"
done
```

### Como parte de una tubería

`-o -` escribe el Ogg a la salida estándar, así que se puede encadenar sin
archivos intermedios:

```bash
# Reproducir sin guardar nada
echo "Hola mundo" | mcpiper -m ./model/ana -o - | ffplay -nodisp -autoexit -

# Convertir a MP3 al vuelo
mcpiper -m ./model/ana -f texto.txt -o - | ffmpeg -i - salida.mp3

# Mandarlo por la red
mcpiper -m ./model/ana -t "Alerta" -o - | curl -X POST --data-binary @- https://…
```

Si `-o -` apunta a una terminal, `mcpiper` se niega y avisa, en vez de vomitar
binario en la pantalla.

### Leer la salida de otro programa

```bash
df -h / | tail -1 | awk '{print "El disco está al " $5}' \
  | mcpiper -m ./model/ana -o /tmp/aviso.ogg -q
```

### Notificaciones desde un script

```bash
avisar() {
  mcpiper -m "$HOME/voces/ana" -t "$1" -o /tmp/aviso.ogg -q && \
  paplay /tmp/aviso.ogg 2>/dev/null || ffplay -nodisp -autoexit -v quiet /tmp/aviso.ogg
}

make && avisar "Compilación terminada" || avisar "La compilación falló"
```

### Windows / PowerShell

```powershell
# Varias frases desde un archivo
Get-Content frases.txt | ForEach-Object -Begin { $i = 1 } -Process {
  .\mcpiper.exe -m .\model\ana -t $_ -o "audio\$('{0:d3}' -f $i).ogg" -q
  $i++
}

# Reproducir al toque
.\mcpiper.exe -m .\model\ana -t "Listo" -o out.ogg -q
Start-Process out.ogg
```

### Pronunciación a medida con `--phonemes`

Cuando espeak-ng se equivoca con un nombre propio o una sigla, podés pasarle los
fonemas IPA vos mismo:

```bash
mcpiper -m ./model/ana --phonemes -t "ˈola mˈundo" -o out.ogg
```

Para ver qué fonemas genera espeak-ng normalmente y usarlos de base:

```bash
espeak-ng -v es -q --ipa "tu texto"     # si tenés espeak-ng instalado aparte
```

Con `--phonemes`, el texto **no** se corta en frases: entra tal cual, y
`--sentence-silence` no aplica.

---

## Idiomas

El binario trae los diccionarios de espeak-ng de **español e inglés**. Con eso
funciona cualquiera de las 47 voces Piper de esos idiomas.

Con un modelo de otro idioma, falla de entrada y te dice qué hacer:

```
mcpiper: este binario no trae los datos de espeak-ng para la voz `fr` que pide el modelo (incluye: es,en).
  Opciones: recompilar con MCPIPER_ESPEAK_LANGS="es,en,fr", o pasar --espeak-data con un espeak-ng-data completo del sistema.
```

Dos salidas, según prefieras:

**Recompilar con más idiomas** — el binario queda autocontenido:

```bash
MCPIPER_ESPEAK_LANGS=es,en,fr,pt cargo build --release   # +unos cientos de KB
MCPIPER_ESPEAK_LANGS=all         cargo build --release   # los ~100 idiomas, +4 MB
```

**Usar los datos del sistema** — sin recompilar, pero deja de ser un solo archivo:

```bash
sudo apt-get install espeak-ng-data
mcpiper -m ./model/fr --espeak-data /usr/share/espeak-ng-data -t "Bonjour" -o out.ogg
```

`--espeak-data` acepta tanto el directorio `espeak-ng-data` como el que lo
contiene.

---

## Solución de problemas

### `no encontré el modelo: probé ... y ...`

La ruta de `--model` no existe ni con ni sin `.onnx`. Revisá el nombre; ojo con
`.onnx.json` vs `.onnx`.

### `no encontré la configuración del modelo; probé ... y ...`

Está el `.onnx` pero falta su JSON. Bajalo del mismo directorio del repositorio
de voces, con el mismo nombre base. O pasalo a mano con `--config`.

### `este binario no trae los datos de espeak-ng para la voz X`

El modelo es de un idioma que no está embebido. Ver [Idiomas](#idiomas).

### `Can't read dictionary file: ...`

Ese mensaje lo imprime espeak-ng, y viene siempre acompañado del error anterior.
Es la misma causa: falta el idioma.

### `el texto no produjo ningún fonema; ¿está vacío o es solo puntuación?`

El texto no tiene nada pronunciable — sólo signos, espacios o emojis.

### `no sé qué hacer con la extensión '.mp3'; usá --format vorbis|wav`

`mcpiper` sólo escribe Ogg Vorbis y WAV. Para MP3, encadená con ffmpeg:

```bash
mcpiper -m ./model/ana -t "Hola" -o - | ffmpeg -i - out.mp3
```

### `no conozco al hablante 'X'. Disponibles: ...` / `está fuera de rango`

Corré `--list-speakers` para ver los nombres y números reales.

### `este modelo tiene una sola voz, --speaker no aplica`

Sacá `--speaker`, o usá un modelo multi-hablante.

### `me negué a volcar audio binario a la terminal`

Usaste `-o -` sin redirigir. Agregá `> archivo.ogg` o encadená con una tubería.

### El ejecutable no arranca en Windows

Falta el [VC++ Redistributable
2015-2022](https://aka.ms/vs/17/release/vc_redist.x64.exe).

### El primer arranque tarda un poco más

Normal: la primera vez descomprime los datos de espeak-ng a la caché
(`~/.cache/mcpiper` en Linux, `~/Library/Caches/mcpiper` en macOS,
`%LOCALAPPDATA%\mcpiper` en Windows). Después se reusa. Podés borrar esa carpeta
cuando quieras; se regenera sola.

---

## Preguntas frecuentes

**¿Necesita internet?**
No. Ni para arrancar ni para sintetizar. Sólo para bajar el binario y las voces
la primera vez.

**¿Necesita GPU?**
No, corre en CPU. Da ~20-30× tiempo real en una máquina de escritorio moderna:
un minuto de audio en un par de segundos.

**¿Dos corridas del mismo texto dan el mismo archivo?**
No por defecto — el modelo tiene aleatoriedad. Con `--noise-scale 0 --noise-w 0`
sí, es determinista byte a byte.

**¿Puedo pasarle un texto largo de una?**
Sí. Se corta en frases automáticamente y cada una pasa por el modelo por
separado, que suena mucho mejor que darle un párrafo entero. No hay límite
práctico de longitud.

**¿A qué frecuencia sale el `.ogg`?**
A la del modelo, tal cual: 22050 Hz en casi todas las voces, 16000 Hz en algunas
`x_low`. Vorbis codifica a la frecuencia nativa, así que no hay remuestreo ni
pérdida de por medio. Lo confirma `--list-speakers`, que imprime el sample rate
del modelo.

**¿Cómo cambio la velocidad?**
`--length-scale`. Ver [Controlar la velocidad y el
tono](#controlar-la-velocidad-y-el-tono).

**¿Se puede usar en un servidor o en un producto?**
Técnicamente sí. Tené en cuenta que `mcpiper` es GPL-3.0-or-later (por
espeak-ng, que va enlazado estáticamente) y que **cada voz tiene su propia
licencia** — revisá el `MODEL_CARD` de la que uses. Ver
[NOTICE.md](NOTICE.md).

**¿Hay una biblioteca, no sólo el CLI?**
Este proyecto es sólo el ejecutable. Por debajo usa
[piper-rs](https://github.com/thewh1teagle/piper-rs), que sí es una biblioteca
de Rust.
