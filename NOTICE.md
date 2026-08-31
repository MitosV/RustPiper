# Third-party licenses

`mcpiper` statically links these projects into the executable:

| Project | License | Role |
|---|---|---|
| [espeak-ng](https://github.com/espeak-ng/espeak-ng) | **GPL-3.0-or-later** | Text phonemization (and the embedded `espeak-ng-data`) |
| [piper-rs](https://github.com/thewh1teagle/piper-rs) | MIT | Piper model inference |
| [ONNX Runtime](https://github.com/microsoft/onnxruntime) | MIT | Neural network execution |
| [libvorbis / aoTuV](https://xiph.org/vorbis/) | BSD-3-Clause | Vorbis audio encoding |
| [libogg](https://xiph.org/ogg/) | BSD-3-Clause | Ogg container |

Because espeak-ng is GPL-3.0-or-later and is linked statically, **the resulting
executable as a whole falls under GPL-3.0-or-later**. That is why `mcpiper` is
published under that license: it is the only one compatible with everything it
carries inside.

If you need a more permissive license to redistribute, the way out is to take
espeak-ng out of the binary and call it as an external process, or to replace
phonemization with another engine.

Piper's **voice models** carry their own license, which varies per voice (many
are CC BY 4.0 or CC0). Check the model card of the voice you use; they are not
distributed with this program.
