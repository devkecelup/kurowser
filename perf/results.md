# Kurowser WebView2 Memory Optimization Log

Baseline (`28cfbab`, Release build):
- **Scenario A (Cold Start +10s):** `185.76 MB` Private (`391.95 MB` WS), `7` processes
- **Scenario B (5 Tabs +20s):** `1033.83 MB` Private (`1332.21 MB` WS), `10` processes
- **Scenario C (5 Tabs +300s idle):** `754.83 MB` Private (`939.79 MB` WS), `10` processes

All deltas below are measured against the Baseline (`target/release/kurowser.exe`, `measure_memory.ps1`).

---

## Optimization 1: Timer-Driven Idle Discard (`WM_TIMER`, Default 180s)

- **Status:** Kept
- **Files changed:** [`src/storage.rs`](../src/storage.rs), [`src/tabs.rs`](../src/tabs.rs), [`src/engine.rs`](../src/engine.rs), [`src/main.rs`](../src/main.rs)
- **Summary:** Added a 15-second `WM_TIMER` (`TIMER_IDLE_DISCARD`) that discards background WebViews after `idle_discard_secs` (default `180`, `0` disables). Skips active, pinned, loading, audible, downloading (`DownloadStarting`/`StateChanged`), and uploading (`fetch`/`XHR` upload body tracking) tabs, and saves/restores `(scrollX, scrollY)` alongside URL and title on reload.
- **What is lost on discard:** Native Chromium Back/Forward history stack, live DOM/JS memory, unsaved form inputs, and media playback state. WebView2 (`ICoreWebView2` through `ICoreWebView2_22`) has no COM API to export or restore Chromium's native navigation history entries across WebView recreation.

| Scenario | Baseline Private (MB) | Opt 1 Private (MB) | Delta (MB) | Delta (%) | Working Set (MB) | Process Count |
|---|---:|---:|---:|---:|---:|---:|
| **A** (Cold Start +10s) | 185.76 | 189.14 | +3.38 | +1.8% | 397.78 (vs 391.95) | 7 (vs 7) |
| **B** (5 Tabs +20s) | 1033.83 | 937.07 | -96.76 | -9.4% | 1305.01 (vs 1332.21) | 10 (vs 10) |
| **C** (5 Tabs +300s idle) | 754.83 | 617.99 | -136.84 | -18.1% | 949.11 (vs 939.79) | **8** (vs 10) |

- **Notes:**
  - In Scenario B (`task-4724`), `Get-TreeSnapshot` at `+20.0s` saw `1080.72 MB` before `gpu-process` released staging buffers (`381.03 -> 237.90 MB`) 0.5s later when `measure_memory.ps1` recorded `937.07 MB`; both background renderers (`87.09 + 62.83 = 149.92 MB`) were lower than Baseline (`96.25 + 64.65 = 160.90 MB`).
  - In Scenario C, both idle background renderer processes were destroyed (`10 -> 8` processes), reclaiming committed Private memory (`-136.84 MB`). Baseline's suspended background renderers had only `12.84 MB` Working Set left in RAM while holding `157.42 MB` Private commit, explaining why total Working Set stayed similar while Private dropped.
  - Verified live scroll restore on discarded local test page (`scroll=1294` -> discarded -> clicked -> reloaded -> `scroll=1294`) and `cargo test` (`33/33` passed).


