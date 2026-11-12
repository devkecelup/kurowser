# Kurowser v1.1.3 Baseline Memory Report (`28cfbab`)

- **Branch:** `perf/webview2-memory` (baseline commit `28cfbab`)
- **Build:** `cargo build --release` (`target/release/kurowser.exe`)
- **Edge WebView2 Runtime:** `157.0.4341.46`
- **Existing `settings.json`:** `max_live_tabs = 3`, `adblock_enabled = true`, `adblock_cosmetic = true`, `sidebar_pinned = false`, `sidebar_hover_expand = true`
- **Primary metric:** Whole process tree **Private Memory (MB)** via [`measure_memory.ps1`](../measure_memory.ps1), alongside Working Set (MB) and Process Count.

## Standardized 5-Tab Workload

Used identically across Scenarios B and C for Baseline and all subsequent optimizations:
1. `https://example.com/` (Cold-start tab)
2. `https://github.com/rust-lang/rust`
3. `https://www.bbc.com/news`
4. `https://www.google.com/search?q=rust+programming`
5. `https://www.youtube.com/watch?v=aqz-KE-bpKQ` (Left active in paused state via `--autoplay-policy=user-gesture-required`)

*(With `max_live_tabs = 3`, Tabs 1 and 2 are LRU-evicted while opening Tabs 4 and 5, leaving Tabs 3, 4 live in background suspended state and Tab 5 active at the end of Scenario B.)*

## Baseline Summary

| Scenario | Description | Private Memory (MB) | Working Set (MB) | Process Count |
|---|---|---:|---:|---:|
| **A** | Cold start (`kurowser://start`), +10s | **185.76** | 391.95 | **7** |
| **B** | 5 tabs open, +20s after last load | **1033.83** | 1332.21 | **10** |
| **C** | Same as B, +5 minutes (300s) idle | **754.83** | 939.79 | **10** |

## Process Tree Breakdown (from `measure_memory.ps1`)

### Scenario A: Cold Start (+10s)
- **Private Total:** `185.76 MB` | **Working Set Total:** `391.95 MB` | **Processes:** `7`
- `msedgewebview2` (`gpu-process`): `99.06 MB` Private (`110.90 MB` WS)
- `msedgewebview2` (`browser-host`): `35.44 MB` Private (`122.36 MB` WS)
- `msedgewebview2` (`renderer`): `23.50 MB` Private (`56.33 MB` WS)
- `msedgewebview2` (`utility:NetworkService`): `10.55 MB` Private (`33.21 MB` WS)
- `kurowser` (`root`): `7.40 MB` Private (`39.83 MB` WS)
- `msedgewebview2` (`utility:StorageService`): `7.22 MB` Private (`19.30 MB` WS)
- `msedgewebview2` (`crashpad-handler`): `2.59 MB` Private (`10.02 MB` WS)

### Scenario B: 5 Tabs Open (+20s)
- **Private Total:** `1033.83 MB` | **Working Set Total:** `1332.21 MB` | **Processes:** `10`
- `msedgewebview2` (`renderer`, YouTube active): `405.94 MB` Private (`492.20 MB` WS)
- `msedgewebview2` (`gpu-process`): `356.14 MB` Private (`393.29 MB` WS)
- `msedgewebview2` (`renderer`, bg 1): `96.25 MB` Private (`73.88 MB` WS)
- `msedgewebview2` (`renderer`, bg 2): `64.65 MB` Private (`52.27 MB` WS)
- `msedgewebview2` (`browser-host`): `63.20 MB` Private (`163.00 MB` WS)
- `msedgewebview2` (`utility:NetworkService`): `20.34 MB` Private (`52.39 MB` WS)
- `kurowser` (`root`): `8.22 MB` Private (`41.42 MB` WS)
- `msedgewebview2` (`utility:AudioService`): `8.12 MB` Private (`24.80 MB` WS)
- `msedgewebview2` (`utility:StorageService`): `7.51 MB` Private (`20.32 MB` WS)
- `msedgewebview2` (`crashpad-handler`): `3.46 MB` Private (`18.64 MB` WS)

### Scenario C: 5 Tabs (+300s Idle)
- **Private Total:** `754.83 MB` | **Working Set Total:** `939.79 MB` | **Processes:** `10`
- `msedgewebview2` (`renderer`, YouTube active): `294.97 MB` Private (`379.24 MB` WS)
- `msedgewebview2` (`gpu-process`): `200.25 MB` Private (`237.76 MB` WS)
- `msedgewebview2` (`renderer`, bg 1 suspended): `95.84 MB` Private (`6.07 MB` WS)
- `msedgewebview2` (`renderer`, bg 2 suspended): `61.58 MB` Private (`6.77 MB` WS)
- `msedgewebview2` (`browser-host`): `57.89 MB` Private (`160.04 MB` WS)
- `msedgewebview2` (`utility:NetworkService`): `17.90 MB` Private (`50.41 MB` WS)
- `msedgewebview2` (`utility:AudioService`): `7.96 MB` Private (`24.85 MB` WS)
- `kurowser` (`root`): `7.81 MB` Private (`41.21 MB` WS)
- `msedgewebview2` (`utility:StorageService`): `7.51 MB` Private (`20.41 MB` WS)
- `msedgewebview2` (`crashpad-handler`): `3.12 MB` Private (`13.03 MB` WS)

