# Security Policy

## Arsitektur & Model Keamanan Kurowser

Kurowser dirancang dari awal tanpa server `localhost` HTTP lokal, sehingga seluruh halaman internal (`kurowser://start`, `kurowser://history`, `kurowser://bookmarks`, `kurowser://downloads`, `kurowser://settings`) dilayani langsung dari memori di dalam proses browser via **WebView2 Custom Scheme Registration** (`ICoreWebView2CustomSchemeRegistration`):

1. **Zero Localhost Attack Surface**: Tidak ada port TCP lokal (`127.0.0.1`) yang dibuka; proses eksternal di mesin pengguna maupun skrip situs web (`http://127.0.0.1:*` / DNS rebinding) tidak dapat menjangkau internal UI/API Kurowser.
2. **Origin Isolation & Custom Scheme Guards**:
   - Skema `kurowser://` didaftarkan dengan `TreatAsSecure = true` dan `HasAuthorityComponent = true` (`AllowedOrigins = ["kurowser://*"]`).
   - `NavigationStarting`, `FrameNavigationStarting`, dan `NewWindowRequested` memblokir situs eksternal/iframe agar tidak bisa melakukan navigasi, redirect, atau embed terhadap `kurowser://` maupun skema berbahaya (`javascript:`, `vbscript:`, `data:text/html`, `data:image/svg+xml`).
   - Endpoint internal `/favicon/*` memeriksa bahwa permintaan berasal dari dokumen `kurowser://` sebelum menyajikan file PNG dari cache lokal.
3. **CSPRNG Session Token & Per-Page CSP Nonce**:
   - Token API internal (`X-Kurowser-Token`) dan setiap *CSP nonce* dibuat saat runtime menggunakan CSPRNG OS (`BCryptGenRandom`) dan diverifikasi dengan perbandingan *constant-time*.
   - Seluruh halaman internal menerapkan `Content-Security-Policy` ketat (`script-src 'nonce-...'`, `frame-ancestors 'none'`, `object-src 'none'`), `X-Frame-Options: DENY`, `X-Content-Type-Options: nosniff`, dan nol event handler inline (`onclick=...`).
4. **Hardened WebView2 Settings & Permissions**:
   - `AreHostObjectsAllowed = false`, `IsWebMessageEnabled = false`, `IsPasswordAutosaveEnabled = false`, serta DevTools otomatis dikunci pada halaman internal di build rilis.
   - Permintaan izin berisiko (`NOTIFICATIONS`, `LOCAL_FONTS`, `OTHER_SENSORS`, `FILE_READ_WRITE`, `MULTIPLE_AUTOMATIC_DOWNLOADS`, `MIDI_SYSTEM_EXCLUSIVE_MESSAGES`) ditolak otomatis (`DENY`); izin sensitif lain wajib diinisiasi interaksi pengguna (*user-initiated*) dan ditolak pada halaman internal.
5. **Path & URL Sanitization**:
   - Cache domain favicon, nama file unduhan, bookmark/quick-link URL, serta data folder dari disk disanitasi terhadap *path traversal* (`..`, `/` , `\`, kontrol karakter) dan skema berbahaya.
6. **Kurowser Shields (Ad & Tracker Blocker)**:
   - Pemblokiran domain & jalur iklan/telemetry pihak ketiga berlangsung langsung di `WebResourceRequested` sebelum permintaan jaringan keluar.

## Melaporkan Kerentanan (Reporting a Vulnerability)

Jika Anda menemukan celah keamanan pada Kurowser:
- **Jangan** membuka *public issue* dengan exploit detail sebelum diperbaiki.
- Laporkan melalui fitur **GitHub Security Advisories** ("Report a vulnerability") di repositori ini atau hubungi maintainer `@devkecelup` secara privat dengan langkah reproduksi dan versi build.
