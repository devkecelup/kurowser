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

---

## Optimization 2: Configurable V8 Old-Space Heap Cap (`--js-flags=--max-old-space-size=N`)

- **Status:** Kept (default `256` MB; configurable via `js_heap_limit_mb` in `settings.json` / `kurowser://api/settings`, `0` disables)
- **Files changed:** [`src/storage.rs`](../src/storage.rs), [`src/engine.rs`](../src/engine.rs), [`src/main.rs`](../src/main.rs)
- **Summary:** Replaced static `BROWSER_ARGS` with `build_browser_args(js_heap_limit_mb)` appending `--js-flags=--max-old-space-size=<N>` (clamped `128..=4096` MB when non-zero). Verified via `Win32_Process.CommandLine` that Edge merges this cleanly with its internal `--ms-user-locale` flag (`--js-flags="--max-old-space-size=256 --ms-user-locale=en_ID"`). Tested both `256` MB and `192` MB limits against heavy JS allocation, Google Docs landing, YouTube 4K (`aqz-KE-bpKQ`), GitHub, and Google Search.

### Default Choice: `256 MB` (Kept Default)

| Scenario | Baseline Private (MB) | Opt 2 (`256 MB`) Private (MB) | Delta vs Baseline (MB) | Delta (%) | Working Set (MB) | Process Count |
|---|---:|---:|---:|---:|---:|---:|
| **A** (Cold Start +10s) | 185.76 | 187.13 | +1.37 | +0.7% | 394.19 (vs 391.95) | 7 (vs 7) |
| **B** (5 Tabs +20s) | 1033.83 | 908.61 | -125.22 | -12.1% | 1225.23 (vs 1332.21) | 10 (vs 10) |
| **C** (5 Tabs +300s idle) | 754.83 | 607.65 | -147.18 | -19.5% | 925.56 (vs 939.79) | 8 (vs 10) |

### Alternative Tested: `192 MB`

| Scenario | Baseline Private (MB) | Opt 2 (`192 MB`) Private (MB) | Delta vs Baseline (MB) | Delta (%) | Working Set (MB) | Process Count |
|---|---:|---:|---:|---:|---:|---:|
| **A** (Cold Start +10s) | 185.76 | 189.58 | +3.82 | +2.1% | 396.81 (vs 391.95) | 7 (vs 7) |
| **B** (5 Tabs +20s) | 1033.83 | 1023.78 | -10.05 | -1.0% | 1381.73 (vs 1332.21) | 10 (vs 10) |
| **C** (5 Tabs +300s idle) | 754.83 | 598.74 | -156.09 | -20.7% | 930.19 (vs 939.79) | 8 (vs 10) |

- **Why `256 MB` was kept over `192 MB` as default:**
  - Compared against Opt 1 (`A 189.14 / B 937.07 / C 617.99`), `256 MB` improved every scenario (`A -2.01`, `B -28.46`, `C -10.34 MB` Private) with the lowest Working Set across all runs.
  - `192 MB` tightened V8 allocation soft-limits further (synthetic 55-chunk Docs stress page compacted to `10 MB` live JS heap vs `26 MB` under `256 MB`; non-GPU renderer sum in Scenario B dropped to `559.28 MB` vs `574.10 MB`; Scenario C dropped `8.91 MB` further to `598.74 MB`), though Scenario B hit a transient `350.20 MB` `gpu-process` spike at +20s.
  - Priority is compatibility first, RAM second: `256 MB` gives 33% more JS heap headroom against renderer OOM on complex live Docs/Sheets/Figma/GitHub diff tabs while still capping runaway scripts. Power users can set `js_heap_limit_mb` to `192` or `0` in `settings.json`.
  - Compatibility checks passed for both caps: 55-chunk DOM/JS stress page (`heavy-ok`), `docs.google.com/document/u/0/`, YouTube 4K (`Big Buck Bunny`), and `cargo test` (`34/34` passed).


