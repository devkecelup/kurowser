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

---

## Optimization 3: Verified Edge 157 Feature Flags and In-Process Network Service

- **Status:** Kept (`lean_webview_flags = true`, `network_in_process = true`; both configurable in `settings.json` / `kurowser://api/settings`)
- **Files changed:** [`src/storage.rs`](../src/storage.rs), [`src/engine.rs`](../src/engine.rs), [`src/main.rs`](../src/main.rs)
- **Summary:** Inspected Edge WebView2 `157.0.4341.46` (`msedge.dll` and `msedgewebview2.exe`) for exact supported flags, then extended `--disable-features` (adding `Translate`, `LanguageDetection`, `EdgeFeedback`, `OptimizationHintsFetchingSRP`, `MediaRouter`, `DialMediaRouteProvider`, `CastMediaRouteProvider`, `GlobalMediaControls`, `EdgeShopping`, `EdgeWallet`, `SpareRendererForSitePerProcess`, `Prerender2`), appended `--disable-sync --disable-breakpad --disable-speech-api --disable-speech-synthesis-api --disable-component-extensions-with-background-pages --disable-fetching-hints-at-navigation-start`, and enabled `--enable-features=OverlayScrollbar,NetworkServiceInProcess2` (`utility:NetworkService` previously already ran unsandboxed with `--service-sandbox-type=none`, so folding it into `browser-host` saves an entire process without changing network sandboxing).

### Part A: `lean_webview_flags` Only (Before `NetworkServiceInProcess2`)

| Scenario | Baseline Private (MB) | Opt 3a Private (MB) | Delta vs Baseline (MB) | Delta (%) | Working Set (MB) | Process Count |
|---|---:|---:|---:|---:|---:|---:|
| **A** (Cold Start +10s) | 185.76 | 187.08 | +1.32 | +0.7% | 392.95 (vs 391.95) | 7 (vs 7) |
| **B** (5 Tabs +20s) | 1033.83 | 1008.04 | -25.79 | -2.5% | 1311.08 (vs 1332.21) | 10 (vs 10) |
| **C** (5 Tabs +300s idle) | 754.83 | 596.08 | -158.75 | -21.0% | 913.06 (vs 939.79) | 8 (vs 10) |

### Part B: `lean_webview_flags` + `network_in_process` (`NetworkServiceInProcess2`, Kept Default)

| Scenario | Baseline Private (MB) | Opt 3 Private (MB) | Delta vs Baseline (MB) | Delta (%) | Working Set (MB) | Process Count |
|---|---:|---:|---:|---:|---:|---:|
| **A** (Cold Start +10s) | 185.76 | **178.40** | **-7.36** | **-4.0%** | **363.70** (vs 391.95) | **6** (vs 7) |
| **B** (5 Tabs +20s) | 1033.83 | 973.35 | -60.48 | -5.9% | 1236.78 (vs 1332.21) | **9** (vs 10) |
| **C** (5 Tabs +300s idle) | 754.83 | **581.85** | **-172.98** | **-22.9%** | **867.81** (vs 939.79) | **7** (vs 10) |

- **Rejected / Ineffective sub-flags tested during Opt 3:**
  - `--renderer-process-limit=2`: Tested and **reverted**. Edge 157 Site Isolation still spawned all 3 distinct-origin renderers (`bbc.com`, `google.com`, `youtube.com` at `413.7 + 96.47 + 82.52 MB`) while total Private spiked to `1096.06 MB`, so the flag was removed instead of shipping a no-op switch.
  - `--disable-breakpad`: Left alongside `--disable-sync`, though note `msedgewebview2.exe` still launches `crashpad-handler` (`~2.6-3.5 MB`) independently of breakpad flag parsing.
- **Notes & Verification:**
  - Against Opt 2 (`A 187.13 / B 908.61 / C 607.65`, `7 / 10 / 8` procs): `lean_webview_flags` trimmed `browser-host` (`56.82 -> 53.23 MB` in Scenario C), and adding `NetworkServiceInProcess2` eliminated the `utility:NetworkService` process across every scenario (`6 / 9 / 7` procs total), saving `-8.73 MB` Private (`-30.49 MB` WS) in A and `-25.80 MB` Private (`-57.75 MB` WS) in C vs Opt 2. (Scenario B total vs Opt 2 differs only because `gpu-process` was `334.71 MB` at +20s vs `220.05 MB` in Opt 2's +20s run; Scenario B renderers dropped `574.10 -> 538.77 MB` and browser+network dropped `86.36 -> 72.34 MB`.)
  - Verified live compatibility with `accounts.google.com` (`myaccount.google.com` signed-in session cookies intact), `github.com/login`, and YouTube 4K (`Big Buck Bunny`), plus `cargo test` (`34/34` passed).



