# Licencias de terceros

`mcpiper` enlaza estáticamente estos proyectos dentro del ejecutable:

| Proyecto | Licencia | Rol |
|---|---|---|
| [espeak-ng](https://github.com/espeak-ng/espeak-ng) | **GPL-3.0-or-later** | Fonemización del texto (y los datos `espeak-ng-data` embebidos) |
| [piper-rs](https://github.com/thewh1teagle/piper-rs) | MIT | Inferencia del modelo Piper |
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) | MIT | Ejecución de la red neuronal |
| [libvorbis / aoTuV](https://xiph.org/vorbis/) | BSD-3-Clause | Codificación de audio Vorbis |
| [libogg](https://xiph.org/ogg/) | BSD-3-Clause | Contenedor Ogg |

Como espeak-ng es GPL-3.0-or-later y se enlaza de forma estática, **el ejecutable
resultante en su conjunto queda bajo la GPL-3.0-or-later**. Por eso `mcpiper` se
publica con esa licencia: es la única compatible con todo lo que lleva adentro.

Si necesitás una licencia más permisiva para redistribuir, la salida es sacar
espeak-ng del binario y llamarlo como proceso externo, o reemplazar la
fonemización por otro motor.

Los **modelos de voz** de Piper tienen su propia licencia, que depende de cada voz
(muchas son CC BY 4.0 o CC0). Revisá la ficha de la voz que uses; no se
distribuyen con este programa.
