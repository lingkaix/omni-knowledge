# Knowledge Processing

Mature extraction, EPUB, media, and local ASR behavior lives in this crate.
Root `extract`, `media`, `epub`, and `runtime` modules re-export the same public names.
There is no second parser, no fabricated transcript, and no processor switch that turns a failure into success.

The process account is `knowledge_core::admission::Account`.
Pi57 committed that seam as `d51cdde` on `omni-resource-admission-09b73a1`.
This branch's `knowledge-core` does not contain `admission` yet, so this crate does not call it.
This crate does not start an account, keep a queue, or invent a source digest.
Callers pass `admission::SharedAccount`. Each effect calls `reserve` before the read, decompression, parser allocation, output growth, child, retained transcript, or generated write.

## Charges

`admission::reservation` maps a charge onto the existing `jobs::Reservation`:

| Charge | Account field |
| --- | --- |
| `Input`, `Work`, `Output`, `Live` | `memory_bytes` |
| `Storage` | `temporary_bytes` |

These are separate charges on one account. They do not change `backend::Budget::max_memory_bytes`.
File, byte, and time ceilings stay the values already enforced by each adapter.
`Bound::child_rss_bytes` and `Bound::storage_bytes` use `None` when the caller is not setting that promise. `None` does not create a 512 MiB ceiling.

`WaitingMemory` and `WaitingStorage` return to the caller. This crate does not spin.
`TooLargeMemory` and `TooLargeStorage` are whole-limit refusals.

## Host Glue

Implement `SharedAccount` in the root host. Do not drop a holding: a drop does not release the charge.
Store every `Decision::Ready` holding for the whole call. Owned children are killed and waited before the call returns. Finish the holdings after the call returns.

```rust
match knowledge_core::admission::Account::current()?.reserve(
    knowledge_processing::reservation(charge),
    knowledge_core::admission::Origin::BeforeVerification,
)? {
    knowledge_core::admission::Decision::Ready(holding) => store(holding),
    knowledge_core::admission::Decision::Waiting(knowledge_core::admission::Waiting::Memory) => {
        return Err(knowledge_processing::AdmitError::WaitingMemory);
    }
    knowledge_core::admission::Decision::Waiting(knowledge_core::admission::Waiting::Storage) => {
        return Err(knowledge_processing::AdmitError::WaitingStorage);
    }
}
```

Indexing today still calls the legacy bypass `extract::extract(path, MAX_TEXT_BYTES)`.
After `Account` is in this workspace, the root owner replaces that call with `extract::extract_admitted(path, MAX_TEXT_BYTES, account)`.
The same replacement applies to `media::segment_video`, `media::transcribe`, `media::parse_transcript`, `epub::read_epub`, and `epub::export_epub`.

## Admitted Entry Points

Legacy functions keep their signatures and enforce the existing per-call ceilings.
Admitted functions are `extract_admitted`, `preview_html_admitted`, `read_epub_admitted`, `export_epub_admitted`, `segment_video_admitted`, `parse_transcript_admitted`, and `transcribe_admitted`.

`media::clip_processor_facts` is the clip recipe the host copies into `cache::ProcessorIdentity` options.
It is not a digest of the source.

Video clips stay contiguous ranges on the source clock, H.264, width 640, 25 fps, and no audio.
Transcript timestamps stay on the recording clock. ASR still requires an explicit executable and model. This crate does not download a model.

## Children

FFmpeg, ffprobe, whisper.cpp, and macOS `textutil` run in their own process group.
The watchdog reads Linux `VmRSS` or macOS `proc_pidinfo` `PROC_PIDTASKINFO` `pti_resident_size`.
A live child above `Bound::child_rss_bytes` is killed, waited, and reported as `WorkerFailure::RssExceeded`.
A required sample that is unavailable is `WorkerFailure::Unmeasured`.
The watchdog does not call `setrlimit` or `RLIMIT_AS`.
Deadline and cancellation settle that same child before the call returns.
An existing output path is not replaced. A refused charge does not open the source for extraction.

## Builder Checks

Use a private target directory, `TMPDIR`, two Cargo jobs, and `--locked --offline`.
Root package builds also need the repository dictionary supply in `docs/toolchain.md`.
Real ASR stays an explicit environment opt-in in the existing embedding test. Do not add another ignored stand-in.
Native macOS execution of `textutil` and `proc_pidinfo` is unobserved from this Linux host.
