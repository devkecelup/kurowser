// ─── updater.rs ─── Secure HTTPS GitHub Releases Auto-Updater (WinHTTP + CNG SHA-256) ───
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, WIN32_ERROR};
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::Security::Cryptography::{
    BCryptCloseAlgorithmProvider, BCryptCreateHash, BCryptDestroyHash, BCryptFinishHash,
    BCryptHashData, BCryptOpenAlgorithmProvider, BCRYPT_ALG_HANDLE, BCRYPT_HASH_HANDLE,
    BCRYPT_SHA256_ALGORITHM,
};
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};

use crate::storage::constant_time_eq;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const API_HOST: &str = "api.github.com";
const API_PATH: &str = "/repos/devkecelup/kurowser/releases/latest";
const TRUSTED_DOWNLOAD_PREFIX: &str = "https://github.com/devkecelup/kurowser/releases/download/";
const MAX_API_BYTES: usize = 512 * 1024;
const MIN_BINARY_BYTES: usize = 200 * 1024;
const MAX_BINARY_BYTES: usize = 50 * 1024 * 1024;

static UPDATE_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateReadyInfo {
    pub version: String,
    pub staged_path: PathBuf,
    pub is_setup: bool,
    pub sha256_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateRuntimeState {
    Idle,
    Checking,
    Downloading(String),
    Ready(UpdateReadyInfo),
    UpToDate,
    Failed(String),
}

impl UpdateRuntimeState {
    pub fn as_json(&self) -> serde_json::Value {
        match self {
            Self::Idle => serde_json::json!({
                "status": "idle",
                "current_version": APP_VERSION,
                "installed": is_installed_mode(),
            }),
            Self::Checking => serde_json::json!({
                "status": "checking",
                "current_version": APP_VERSION,
                "installed": is_installed_mode(),
            }),
            Self::Downloading(ver) => serde_json::json!({
                "status": "downloading",
                "version": ver,
                "current_version": APP_VERSION,
                "installed": is_installed_mode(),
            }),
            Self::Ready(info) => serde_json::json!({
                "status": "ready",
                "version": info.version,
                "is_setup": info.is_setup,
                "current_version": APP_VERSION,
                "installed": is_installed_mode(),
            }),
            Self::UpToDate => serde_json::json!({
                "status": "up_to_date",
                "current_version": APP_VERSION,
                "installed": is_installed_mode(),
            }),
            Self::Failed(msg) => serde_json::json!({
                "status": "failed",
                "error": msg,
                "current_version": APP_VERSION,
                "installed": is_installed_mode(),
            }),
        }
    }
}

pub fn begin_update_check_guard() -> bool {
    UPDATE_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

pub fn end_update_check_guard() {
    UPDATE_IN_FLIGHT.store(false, Ordering::SeqCst);
}

pub fn is_installed_mode() -> bool {
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    let Some(dir) = exe.parent() else {
        return false;
    };
    if dir.join("Uninstall.exe").exists() {
        return true;
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let installed_dir = PathBuf::from(local).join("Programs").join("Kurowser");
        return dir
            .to_string_lossy()
            .eq_ignore_ascii_case(&installed_dir.to_string_lossy());
    }
    false
}

pub fn parse_semver(raw: &str) -> Option<(u32, u32, u32)> {
    let trimmed = raw.trim().trim_start_matches(['v', 'V']);
    if trimmed.is_empty() || trimmed.contains('-') || trimmed.contains('+') {
        return None;
    }
    let mut parts = trimmed.split('.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    let patch = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

pub fn is_newer_version(remote: &str, current: &str) -> bool {
    match (parse_semver(remote), parse_semver(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

pub fn is_trusted_download_url(url: &str) -> bool {
    url.starts_with(TRUSTED_DOWNLOAD_PREFIX)
        && url.len() < 512
        && !url.chars().any(char::is_control)
}

pub fn parse_sha256_digest(digest: &str) -> Option<String> {
    let hex = digest.trim().strip_prefix("sha256:")?;
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(hex.to_ascii_lowercase())
}

fn sanitize_version_folder(ver: &str) -> Option<String> {
    let (maj, min, pat) = parse_semver(ver)?;
    Some(format!("{maj}.{min}.{pat}"))
}

fn updates_root_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("Kurowser").join("updates")
}

pub fn cleanup_old_staged_updates() {
    let root = updates_root_dir();
    let Ok(entries) = fs::read_dir(&root) else {
        return;
    };
    let Some(cur) = parse_semver(APP_VERSION) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if let Some(ver) = parse_semver(&name_str) {
            if ver <= cur {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    }
}

fn wide_null(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

struct WinHttpHandle(*mut core::ffi::c_void);

impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

struct Sha256Hasher {
    alg: BCRYPT_ALG_HANDLE,
    hash: BCRYPT_HASH_HANDLE,
}

impl Sha256Hasher {
    fn new() -> Result<Self, String> {
        unsafe {
            let mut alg = BCRYPT_ALG_HANDLE::default();
            BCryptOpenAlgorithmProvider(&mut alg, BCRYPT_SHA256_ALGORITHM, None, Default::default())
                .ok()
                .map_err(|e| format!("BCryptOpenAlgorithmProvider failed: {:?}", e))?;

            let mut hash = BCRYPT_HASH_HANDLE::default();
            if let Err(e) = BCryptCreateHash(alg, &mut hash, None, None, 0).ok() {
                let _ = BCryptCloseAlgorithmProvider(alg, 0);
                return Err(format!("BCryptCreateHash failed: {:?}", e));
            }
            Ok(Self { alg, hash })
        }
    }

    fn update(&mut self, data: &[u8]) -> Result<(), String> {
        if data.is_empty() {
            return Ok(());
        }
        unsafe {
            BCryptHashData(self.hash, data, 0)
                .ok()
                .map_err(|e| format!("BCryptHashData failed: {:?}", e))
        }
    }

    fn finish_hex(self) -> Result<String, String> {
        let mut digest = [0u8; 32];
        unsafe {
            BCryptFinishHash(self.hash, &mut digest, 0)
                .ok()
                .map_err(|e| format!("BCryptFinishHash failed: {:?}", e))?;
        }
        let mut hex = String::with_capacity(64);
        for b in digest {
            hex.push_str(&format!("{:02x}", b));
        }
        Ok(hex)
    }
}

impl Drop for Sha256Hasher {
    fn drop(&mut self) {
        unsafe {
            if !self.hash.is_invalid() {
                let _ = BCryptDestroyHash(self.hash);
            }
            if !self.alg.is_invalid() {
                let _ = BCryptCloseAlgorithmProvider(self.alg, 0);
            }
        }
    }
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    let mut f = File::open(path).map_err(|e| format!("Gagal membuka file: {}", e))?;
    let mut hasher = Sha256Hasher::new()?;
    let mut buf = [0u8; 16384];
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("Gagal membaca file: {}", e))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n])?;
    }
    hasher.finish_hex()
}

fn parse_https_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("https://")?;
    let (host_port, path_part) = match rest.find('/') {
        Some(idx) => (&rest[..idx], &rest[idx..]),
        None => (rest, "/"),
    };
    let (host, port) = match host_port.split_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()?),
        None => (host_port, 443),
    };
    if host.is_empty() || host.contains('@') {
        return None;
    }
    Some((host.to_string(), port, path_part.to_string()))
}

fn winhttp_open_session() -> Result<WinHttpHandle, String> {
    let ua = wide_null(&format!("Kurowser/{APP_VERSION}"));
    unsafe {
        let h = WinHttpOpen(
            PCWSTR(ua.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            None,
            None,
            0,
        );
        if h.is_null() {
            return Err(format!("WinHttpOpen error: {:?}", WIN32_ERROR::from_error(
                &windows::core::Error::from_win32(),
            )));
        }
        let _ = WinHttpSetTimeouts(h, 10_000, 10_000, 10_000, 25_000);
        let policy = WINHTTP_OPTION_REDIRECT_POLICY_DISALLOW_HTTPS_TO_HTTP;
        let _ = WinHttpSetOption(
            Some(h),
            WINHTTP_OPTION_REDIRECT_POLICY,
            Some(&policy.to_ne_bytes()),
        );
        Ok(WinHttpHandle(h))
    }
}

fn winhttp_get_stream<F>(url: &str, headers: &str, max_bytes: usize, mut on_chunk: F) -> Result<(), String>
where
    F: FnMut(&[u8]) -> Result<(), String>,
{
    let (host, port, path) = parse_https_url(url).ok_or("URL HTTPS tidak valid")?;
    let session = winhttp_open_session()?;

    let host_w = wide_null(&host);
    let path_w = wide_null(&path);
    let hdr_w = wide_null(headers);

    unsafe {
        let conn = WinHttpConnect(session.0, PCWSTR(host_w.as_ptr()), port, 0);
        if conn.is_null() {
            return Err("Gagal menghubungi server GitHub".to_string());
        }
        let _conn_guard = WinHttpHandle(conn);

        let req = WinHttpOpenRequest(
            conn,
            w!("GET"),
            PCWSTR(path_w.as_ptr()),
            None,
            None,
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if req.is_null() {
            return Err("Gagal membuat permintaan HTTPS".to_string());
        }
        let _req_guard = WinHttpHandle(req);

        let hdr_slice = if headers.is_empty() {
            None
        } else {
            Some(&hdr_w[..hdr_w.len() - 1])
        };
        WinHttpSendRequest(req, hdr_slice, None, 0, 0, 0)
            .map_err(|_| "Koneksi HTTPS gagal (offline atau diblokir)".to_string())?;

        WinHttpReceiveResponse(req, std::ptr::null_mut())
            .map_err(|_| "Server tidak merespons".to_string())?;

        let mut status_code: u32 = 0;
        let mut status_size = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            req,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            None,
            Some(&mut status_code as *mut u32 as *mut _),
            &mut status_size,
            std::ptr::null_mut(),
        )
        .map_err(|_| "Gagal membaca status HTTP".to_string())?;

        if status_code != 200 {
            return Err(format!("Server mengembalikan HTTP {}", status_code));
        }

        let mut total_read = 0usize;
        let mut buf = vec![0u8; 16384];
        loop {
            let mut bytes_read: u32 = 0;
            WinHttpReadData(req, buf.as_mut_ptr() as *mut _, buf.len() as u32, &mut bytes_read)
                .map_err(|_| "Gagal membaca data unduhan".to_string())?;
            if bytes_read == 0 {
                break;
            }
            let n = bytes_read as usize;
            total_read = total_read.saturating_add(n);
            if total_read > max_bytes {
                return Err("Ukuran respons melebihi batas aman".to_string());
            }
            on_chunk(&buf[..n])?;
        }
    }

    Ok(())
}

#[derive(serde::Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(serde::Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

pub fn check_and_stage_update<F>(on_progress: F) -> UpdateRuntimeState
where
    F: Fn(UpdateRuntimeState),
{
    let api_url = format!("https://{API_HOST}{API_PATH}");
    let mut json_bytes = Vec::new();
    let api_res = winhttp_get_stream(
        &api_url,
        "Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\n",
        MAX_API_BYTES,
        |chunk| {
            json_bytes.extend_from_slice(chunk);
            Ok(())
        },
    );

    if let Err(err) = api_res {
        return UpdateRuntimeState::Failed(err);
    }

    let Ok(release) = serde_json::from_slice::<GhRelease>(&json_bytes) else {
        return UpdateRuntimeState::Failed("Format data rilis GitHub tidak valid".to_string());
    };

    if release.draft || release.prerelease {
        return UpdateRuntimeState::UpToDate;
    }

    if !is_newer_version(&release.tag_name, APP_VERSION) {
        return UpdateRuntimeState::UpToDate;
    }

    let Some(clean_ver) = sanitize_version_folder(&release.tag_name) else {
        return UpdateRuntimeState::UpToDate;
    };

    let installed = is_installed_mode();
    let want_name = if installed {
        "KurowserSetup.exe"
    } else {
        "kurowser.exe"
    };

    let Some(asset) = release.assets.iter().find(|a| a.name == want_name) else {
        return UpdateRuntimeState::Failed(format!("Aset {} tidak ditemukan di rilis", want_name));
    };

    if !is_trusted_download_url(&asset.browser_download_url) {
        return UpdateRuntimeState::Failed("URL unduhan ditolak kebijakan keamanan".to_string());
    }

    if (asset.size as usize) < MIN_BINARY_BYTES || (asset.size as usize) > MAX_BINARY_BYTES {
        return UpdateRuntimeState::Failed("Ukuran berkas pembaruan di luar batas aman".to_string());
    }

    let Some(expected_sha) = asset.digest.as_deref().and_then(parse_sha256_digest) else {
        return UpdateRuntimeState::Failed("Digest SHA-256 tidak tersedia pada rilis".to_string());
    };

    let stage_dir = updates_root_dir().join(&clean_ver);
    if fs::create_dir_all(&stage_dir).is_err() {
        return UpdateRuntimeState::Failed("Gagal membuat folder cache pembaruan".to_string());
    }

    let final_path = stage_dir.join(want_name);

    // Cache hit check: if already downloaded & SHA-256 matches, ready immediately.
    if final_path.exists() {
        if let Ok(actual) = sha256_file(&final_path) {
            if constant_time_eq(&actual, &expected_sha) {
                return UpdateRuntimeState::Ready(UpdateReadyInfo {
                    version: clean_ver,
                    staged_path: final_path,
                    is_setup: installed,
                    sha256_hex: expected_sha,
                });
            }
        }
        let _ = fs::remove_file(&final_path);
    }

    on_progress(UpdateRuntimeState::Downloading(clean_ver.clone()));

    let tmp_path = stage_dir.join(format!("{}.tmp", want_name));
    let _ = fs::remove_file(&tmp_path);

    let download_result: Result<(), String> = (|| {
        let mut out_file =
            File::create(&tmp_path).map_err(|e| format!("Gagal membuat file sementara: {}", e))?;
        let mut hasher = Sha256Hasher::new()?;
        let mut magic = [0u8; 2];
        let mut magic_len = 0usize;
        let mut total = 0usize;

        winhttp_get_stream(&asset.browser_download_url, "", MAX_BINARY_BYTES, |chunk| {
            for &b in chunk {
                if magic_len < 2 {
                    magic[magic_len] = b;
                    magic_len += 1;
                } else {
                    break;
                }
            }
            total = total.saturating_add(chunk.len());
            hasher.update(chunk)?;
            out_file
                .write_all(chunk)
                .map_err(|e| format!("Gagal menulis unduhan: {}", e))?;
            Ok(())
        })?;

        out_file
            .flush()
            .map_err(|e| format!("Gagal flush file unduhan: {}", e))?;
        drop(out_file);

        if total < MIN_BINARY_BYTES || magic != *b"MZ" {
            return Err("Berkas pembaruan bukan executable Windows yang valid".to_string());
        }

        let actual_sha = hasher.finish_hex()?;
        if !constant_time_eq(&actual_sha, &expected_sha) {
            return Err("Verifikasi SHA-256 pembaruan gagal".to_string());
        }

        fs::rename(&tmp_path, &final_path)
            .map_err(|e| format!("Gagal memindahkan berkas pembaruan: {}", e))?;
        Ok(())
    })();

    if let Err(err) = download_result {
        let _ = fs::remove_file(&tmp_path);
        return UpdateRuntimeState::Failed(err);
    }

    UpdateRuntimeState::Ready(UpdateReadyInfo {
        version: clean_ver,
        staged_path: final_path,
        is_setup: installed,
        sha256_hex: expected_sha,
    })
}

pub fn spawn_apply_update(ready: &UpdateReadyInfo, relaunch: bool) -> bool {
    if !ready.staged_path.exists() {
        return false;
    }
    if let Ok(actual) = sha256_file(&ready.staged_path) {
        if !constant_time_eq(&actual, &ready.sha256_hex) {
            return false;
        }
    } else {
        return false;
    }

    let pid = std::process::id().to_string();
    let mut cmd = Command::new(&ready.staged_path);

    if ready.is_setup {
        cmd.args(["--silent", "--preserve-shortcuts", "--wait-pid", &pid]);
        if !relaunch {
            cmd.arg("--no-launch");
        }
    } else {
        let Ok(cur_exe) = std::env::current_exe() else {
            return false;
        };
        cmd.arg("--apply-update")
            .arg("--wait-pid")
            .arg(&pid)
            .arg("--target")
            .arg(&cur_exe);
        if relaunch {
            cmd.arg("--relaunch");
        }
    }

    cmd.spawn().is_ok()
}

fn wait_for_pid(pid: u32, timeout_ms: u32) {
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            if !handle.is_invalid() {
                let _ = WaitForSingleObject(handle, timeout_ms);
                let _ = CloseHandle(handle);
            }
        }
    }
}

/// Runs when a verified downloaded portable `kurowser.exe` is launched with `--apply-update`.
/// Returns `true` if handled and main process should exit immediately without opening a window.
pub fn maybe_handle_apply_update_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == "--apply-update") {
        return false;
    }

    let mut wait_pid: Option<u32> = None;
    let mut target: Option<PathBuf> = None;
    let mut relaunch = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--wait-pid" if i + 1 < args.len() => {
                wait_pid = args[i + 1].parse::<u32>().ok();
                i += 2;
            }
            "--target" if i + 1 < args.len() => {
                target = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--relaunch" => {
                relaunch = true;
                i += 1;
            }
            _ => i += 1,
        }
    }

    if let Some(pid) = wait_pid {
        if pid != std::process::id() {
            wait_for_pid(pid, 8000);
        }
    }

    let Ok(self_exe) = std::env::current_exe() else {
        return true;
    };
    let Some(target_exe) = target else {
        return true;
    };
    let is_exe = target_exe
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"));
    if !is_exe || self_exe == target_exe {
        return true;
    }

    let mut replaced = false;
    for _ in 0..8 {
        if fs::copy(&self_exe, &target_exe).is_ok() {
            replaced = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }

    if replaced && relaunch {
        let _ = Command::new(&target_exe).spawn();
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_compares_semver_safely() {
        assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver("v1.2.0-beta.1"), None);
        assert_eq!(parse_semver("1.2"), None);

        assert!(is_newer_version("v1.1.1", "1.1.0"));
        assert!(is_newer_version("v1.2.0", "1.1.9"));
        assert!(is_newer_version("v2.0.0", "1.9.9"));
        assert!(!is_newer_version("v1.1.0", "1.1.0"));
        assert!(!is_newer_version("v1.0.9", "1.1.0"));
        assert!(!is_newer_version("v1.2.0-rc1", "1.1.0"));
    }

    #[test]
    fn validates_github_download_urls_and_digests() {
        assert!(is_trusted_download_url(
            "https://github.com/devkecelup/kurowser/releases/download/v1.1.0/KurowserSetup.exe"
        ));
        assert!(!is_trusted_download_url(
            "http://github.com/devkecelup/kurowser/releases/download/v1.1.0/KurowserSetup.exe"
        ));
        assert!(!is_trusted_download_url(
            "https://evil.example.com/KurowserSetup.exe"
        ));

        assert_eq!(
            parse_sha256_digest(
                "sha256:B5C09DC4FD45C006256A8A71A7B2ABA29A226869D4A6ACC2053FA176431C510D"
            ),
            Some("b5c09dc4fd45c006256a8a71a7b2aba29a226869d4a6acc2053fa176431c510d".to_string())
        );
        assert_eq!(parse_sha256_digest("md5:1234"), None);
        assert_eq!(parse_sha256_digest("sha256:short"), None);
    }

    #[test]
    fn sha256_hashes_file_via_windows_cng() {
        let tmp = std::env::temp_dir().join("kurowser_sha256_unit_test.bin");
        fs::write(&tmp, b"hello kurowser").unwrap();
        let digest = sha256_file(&tmp).unwrap();
        let _ = fs::remove_file(&tmp);
        assert_eq!(digest.len(), 64);
        assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
    }
}
