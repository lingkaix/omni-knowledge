# Local Multimodal Retrieval

This guide describes the local provider and media methods used to implement the [product requirements](specs/product.md).
Original files remain authoritative; vectors, clips, and transcripts produced by configured tools are derived observations.
Search must return the original path and a source time range when one exists.

## Provider

The first version uses the existing Indexed Swift helper in `/Users/m5pro/Documents/AI/local-ai/embedding-wemm`.
Omni Knowledge does not start it automatically or copy its model into a library.
The helper combines MLX language inference with Core ML vision inference on Apple Silicon.
Its documented `gpu`, `ane`, and `efficient` modes share the same validated package.
The private e5rt mode was measured on this M5 Pro; this project has not established its behavior on the user's M3 Mac.

For interactive tests, start the GPU provider in a separate terminal:

```sh
cd /Users/m5pro/Documents/AI/local-ai/embedding-wemm
./scripts/serve.sh gpu
```

The default identity is explicit:

| Property | Value |
| --- | --- |
| Endpoint | `http://127.0.0.1:18789` |
| Model | `wemm-embedding-2b-apple-2048` |
| Dimension | `2048` |
| Package fingerprint | `2d481d864704eaabf57a57e77544a6439dd8a524752f74ad5b8543b7f7fe3ecb` |
| Embedding space | `wemm-2b-apple-q8-g64-2048-2d481d864704eaab` |

The client checks `GET /health` and `GET /v1/models` before each content request.
It checks the model, dimension, and embedding space again in each response.
A missing field or incompatible value stops the request or rejects its result.
Vectors must have the required length, finite values, and a nonzero norm.
Changing identity requires a fresh vector index; equal vector lengths do not establish compatibility.

Only HTTP loopback endpoints are accepted.
The client converts `localhost` to `127.0.0.1`, ignores ambient proxy settings, and disables redirects.
URLs cannot contain credentials, query strings, fragments, or a path prefix.
Each HTTP request has a configurable deadline from one through 600 seconds; the default is 300 seconds.
An unavailable provider leaves lexical and metadata search available.

## Request Format

Text uses a single string rather than a batch:

```json
{
  "model": "wemm-embedding-2b-apple-2048",
  "input": "Find the video segment that compares GPU and ANE performance."
}
```

Images use one base64 data URI:

```json
{
  "model": "wemm-embedding-2b-apple-2048",
  "messages": [
    {
      "role": "user",
      "content": [
        { "type": "image_url", "image_url": { "url": "data:image/jpeg;base64,..." } }
      ]
    }
  ]
}
```

Video uses the same structure with `video_url` and `data:video/mp4;base64,...`.
The helper accepts one visual input per request.
The client caps input media at 40 MiB, text at 48 KiB, and response JSON at 2 MiB.
Larger text must be split by the indexing caller.
Successful responses contain one `data[0].embedding` plus `model`, `indexed.dimension`, and `indexed.embedding_space`.

## Video

`segment_video` calls the user's `ffprobe` and `ffmpeg` executables.
It produces contiguous clips, normally ten seconds each, and retains a final shorter clip.
Each `MediaSegment` records the original start and end in seconds.
The output MP4 uses H.264, 640-pixel width, 25 frames per second, and no audio.
The Swift helper samples approximately two frames per second and performs its packaged 448-pixel vision preprocessing.
Do not interpret the clip's reset playback clock as its position in the original source.

The caller provides an existing temporary output directory and owns its cleanup.
Conversion rejects an existing target instead of replacing it.
Failed conversion can leave partial outputs in that directory; the caller must remove them through its temporary-directory lifecycle.
Inspection has a 30-second deadline, and each clip conversion has a 120-second deadline.
All command arguments are passed separately; imported files are never treated as shell programs.

## Audio And Transcripts

WeMM does not directly encode audio.
Audio content becomes searchable through imported speech text or optional local automatic speech recognition (ASR).
Without either route, the file still participates in filename, description, and exact-tag search.

The default transcript sidecar is `<filename>.transcript.json` beside the source.
For example, `interview.m4a.transcript.json` accompanies `interview.m4a`.
`parse_transcript` supports OpenAI Whisper's `segments` with second-based `start` and `end`, and whisper.cpp's `transcription` with millisecond-based `offsets.from` and `offsets.to`.
It rejects negative, nonfinite, or reversed ranges and ignores empty text.
A JSON object with only `text` is accepted with a zero-to-zero range because it supplies no timing evidence.
Transcript JSON is limited to 16 MiB.

Optional `transcribe` requires an existing local whisper.cpp executable and model file.
It converts the first audio stream to mono 16 kHz PCM WAV, then runs:

```sh
whisper-cli -m MODEL -f asr-input.wav -l auto -oj -of asr-result
```

Recognition can run for up to thirty minutes.
The caller owns `asr-input.wav`, `asr-result.json`, and their temporary directory.
This route downloads no model and starts no background service.
ASR text is an observation and can contain errors; the source recording remains available for verification.

## ASR Setup And Evidence

Run the optional setup once from this repository:

```sh
./scripts/setup-asr.sh
```

Prerequisites are Apple Silicon macOS, CMake, Apple command-line tools, Python 3, and curl.
Audio conversion also requires the existing `ffmpeg` executable.
The setup downloads pinned whisper.cpp 1.9.5 source and the multilingual `base` model into ignored `temp/asr-runtime/`.
It checks the source archive and model SHA-256 values before use, rejects symlinks and changed pinned source, and builds only the CLI target.
The build uses static libraries and embedded Metal kernels; it performs no system installation or Core ML conversion.
The script prints absolute `whisper_executable` and `whisper_model` values for the library configuration.
`temp/asr-runtime/setup-manifest.json` records pins, checksums, paths, and artifact sizes.

On 2026-10-14, the M5 Pro built the CLI and recognized a generated 7.54-second English utterance.
The transcript retained `private`, `pineapple recipes`, and `project notes`, with ranges of 0–6 seconds and 6–7.34 seconds.
The recognizer selected the Apple M5 Pro Metal backend and completed in 1.072 seconds wall time.
This small generated fixture proves a runnable pipeline, not general recognition accuracy or a multilingual quality benchmark.
The M3 Mac still needs its own setup and runtime check.

The actual-runtime regression is opt-in because the model and fixture are local generated artifacts.
Create the known speech fixture with `say` outside any sandbox that blocks the speech service:

```sh
say -v Samantha -r 145 -o temp/asr-runtime/fixture-speech-actual.aiff \
  'The knowledge library keeps my files private. Search the audio recording for pineapple recipes and project notes.'
OMNI_ASR_EXECUTABLE="$PWD/temp/asr-runtime/build/bin/whisper-cli" \
OMNI_ASR_MODEL="$PWD/temp/asr-runtime/models/ggml-base.bin" \
OMNI_ASR_FIXTURE="$PWD/temp/asr-runtime/fixture-speech-actual.aiff" \
  cargo test --test embedding actual_local_asr_recognizes_known_speech -- --ignored
```

The test invokes the public `transcribe` function, performs audio normalization, runs real recognition, checks words and time ranges, and cleans its temporary outputs.
Default mock tests do not count the opt-in check as executed.

## Sources And Licenses

The official [WeMM repository](https://github.com/Tencent/WeMM-Embedding) describes a shared space for text, images, video, and visual documents, using the final embedding token and L2 normalization.
It explicitly excludes direct audio input.
The [official 2B model card](https://huggingface.co/tencent/WeMM-Embedding-2B) declares Apache-2.0 for Tencent's public code and weights.

[Indexed](https://github.com/xiaotianfotos/indexed) provides the Apple-native helper used here and demonstrates returning source links, local paths, excerpts, and timestamps.
Its code is [Apache-2.0](https://github.com/xiaotianfotos/indexed/blob/main/LICENSE).
Omni Knowledge communicates through its existing API and does not vendor its implementation.

[WeKnora](https://github.com/Tencent/WeKnora) separates document parsing, vectorization, retrieval, and model providers.
Its ASR and multimodal retrieval support provide architectural references for these boundaries.
Its [license](https://github.com/Tencent/WeKnora/blob/main/LICENSE) is MIT, with separate licenses retained for dependencies.

[whisper.cpp](https://github.com/ggml-org/whisper.cpp) is MIT-licensed and supports Apple Silicon through Metal, with an optional Core ML encoder path.
Its [CLI implementation](https://github.com/ggml-org/whisper.cpp/blob/master/examples/cli/cli.cpp) defines the millisecond offsets parsed here.
The [1.9.5 release](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.5) pins the source used by the setup.
The upstream [model guide](https://github.com/ggml-org/whisper.cpp/blob/d1be6fde11ac6e0407606b4e42fe72d34add8037/models/README.md) identifies `base` as multilingual and links to its converted model repository.

[FFmpeg's license page](https://ffmpeg.org/legal.html) distinguishes its LGPL base from GPL-enabled builds.
The existing Homebrew executable is FFmpeg 9.0.2 with GPL and libx264 enabled.
The first version invokes that installed executable and does not distribute an FFmpeg binary.
The conversion methods follow the [FFmpeg command documentation](https://ffmpeg.org/ffmpeg.html).
