#![windows_subsystem = "windows"]

use std::cell::RefCell;
use std::ffi::OsString;
use std::fs;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::core::{w, HSTRING, Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IPersistFile,
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteTreeW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WRITE, REG_DWORD,
    REG_OPTION_NON_VOLATILE, REG_SZ, REG_VALUE_TYPE,
};
use windows::Win32::System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Programs, IShellLinkW, SHChangeNotify, SHGetKnownFolderPath,
    ShellLink, KNOWN_FOLDER_FLAG, SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const APP_NAME: &str = "Kurowser";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_PUBLISHER: &str = "devkecelup";
const UNINSTALL_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Kurowser";
const APP_PATHS_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\App Paths\kurowser.exe";
const WEBVIEW2_CLIENT_GUID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";

fn exe_size_label() -> String {
    let bytes = KUROWSER_EXE_BYTES.len();
    let mib = bytes as f64 / (1024.0 * 1024.0);
    format!("v{APP_VERSION} ({:.2} MB)", mib)
}

fn is_already_installed() -> bool {
    default_install_dir().join("kurowser.exe").exists()
}

fn read_installed_version() -> Option<String> {
    unsafe {
        let wsub = wide_null(UNINSTALL_REG_KEY);
        let mut hk = HKEY::default();
        if !RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(wsub.as_ptr()), 0, KEY_READ, &mut hk).is_ok() {
            return None;
        }
        let wname = wide_null("DisplayVersion");
        let mut val_type = REG_VALUE_TYPE::default();
        let mut byte_len = 0u32;
        let query1 = RegQueryValueExW(
            hk,
            PCWSTR(wname.as_ptr()),
            None,
            Some(&mut val_type),
            None,
            Some(&mut byte_len),
        );
        if !query1.is_ok() || val_type != REG_SZ || byte_len < 2 || byte_len > 128 {
            let _ = RegCloseKey(hk);
            return None;
        }
        let mut buf = vec![0u8; byte_len as usize];
        let query2 = RegQueryValueExW(
            hk,
            PCWSTR(wname.as_ptr()),
            None,
            Some(&mut val_type),
            Some(buf.as_mut_ptr()),
            Some(&mut byte_len),
        );
        let _ = RegCloseKey(hk);
        if !query2.is_ok() {
            return None;
        }
        let u16s: Vec<u16> = buf
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&ch| ch != 0)
            .collect();
        let ver = String::from_utf16_lossy(&u16s).trim().to_string();
        if ver.is_empty() {
            None
        } else {
            Some(ver)
        }
    }
}

const KUROWSER_EXE_BYTES: &[u8] = include_bytes!("../../kurowser.exe");
const LOGO_PNG_BYTES: &[u8] = include_bytes!("../../assets/kurowser_logo.png");
const FONT_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Poppins-Regular.ttf");
const FONT_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Poppins-Medium.ttf");
const FONT_SEMIBOLD: &[u8] = include_bytes!("../../assets/fonts/Poppins-SemiBold.ttf");

const LOG_W: i32 = 520;
const LOG_H: i32 = 408;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AppMode {
    Install,
    Uninstall,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CliOptions {
    mode: AppMode,
    silent: bool,
    desktop_shortcut: bool,
    preserve_shortcuts: bool,
    launch_after: bool,
    purge_data: bool,
    wait_pid: Option<u32>,
}

fn parse_cli_args<I, S>(args: I, exe_name_hint: Option<&str>) -> CliOptions
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut mode = match exe_name_hint {
        Some(name) if name.to_ascii_lowercase().contains("uninstall") => AppMode::Uninstall,
        _ => AppMode::Install,
    };
    let mut silent = false;
    let mut desktop_shortcut = true;
    let mut preserve_shortcuts = false;
    let mut launch_after = true;
    let mut purge_data = false;
    let mut wait_pid: Option<u32> = None;
    let mut expect_pid = false;

    for raw in args {
        let s = raw.as_ref().trim();
        let a = s.to_ascii_lowercase();
        if expect_pid {
            expect_pid = false;
            if let Ok(pid) = a.parse::<u32>() {
                if pid > 0 {
                    wait_pid = Some(pid);
                }
            }
            continue;
        }
        if let Some(pid_str) = a.strip_prefix("--wait-pid=") {
            if let Ok(pid) = pid_str.parse::<u32>() {
                if pid > 0 {
                    wait_pid = Some(pid);
                }
            }
            continue;
        }
        match a.as_str() {
            "--uninstall" | "/uninstall" | "-u" => mode = AppMode::Uninstall,
            "--install" | "/install" | "-i" => mode = AppMode::Install,
            "--silent" | "/s" | "-s" => silent = true,
            "--no-desktop" => desktop_shortcut = false,
            "--preserve-shortcuts" => preserve_shortcuts = true,
            "--no-launch" => launch_after = false,
            "--purge-data" => purge_data = true,
            "--wait-pid" => expect_pid = true,
            _ => {}
        }
    }

    CliOptions {
        mode,
        silent,
        desktop_shortcut,
        preserve_shortcuts,
        launch_after,
        purge_data,
        wait_pid,
    }
}

fn wide_null(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = OsString::from(s).encode_wide().collect();
    v.push(0);
    v
}

struct FontGuard(Vec<HANDLE>);

impl Drop for FontGuard {
    fn drop(&mut self) {
        unsafe {
            for h in &self.0 {
                let _ = RemoveFontMemResourceEx(*h);
            }
        }
    }
}

fn load_poppins() -> FontGuard {
    let mut handles = Vec::new();
    for data in [FONT_REGULAR, FONT_MEDIUM, FONT_SEMIBOLD] {
        let mut count = 0u32;
        let h = unsafe { AddFontMemResourceEx(data.as_ptr() as _, data.len() as u32, None, &mut count) };
        if !h.is_invalid() && !h.0.is_null() {
            handles.push(h);
        }
    }
    FontGuard(handles)
}

fn detect_webview2_installed() -> bool {
    let sub_hklm = format!(r"SOFTWARE\Microsoft\EdgeUpdate\Clients\{}", WEBVIEW2_CLIENT_GUID);
    let sub_hkcu = format!(r"Software\Microsoft\EdgeUpdate\Clients\{}", WEBVIEW2_CLIENT_GUID);
    unsafe {
        if reg_key_exists(HKEY_LOCAL_MACHINE, &sub_hklm, KEY_READ | KEY_WOW64_32KEY) {
            return true;
        }
        if reg_key_exists(HKEY_LOCAL_MACHINE, &sub_hklm, KEY_READ) {
            return true;
        }
        if reg_key_exists(HKEY_CURRENT_USER, &sub_hkcu, KEY_READ) {
            return true;
        }
    }
    false
}

unsafe fn reg_key_exists(root: HKEY, subkey: &str, sam: windows::Win32::System::Registry::REG_SAM_FLAGS) -> bool {
    let wsub = wide_null(subkey);
    let mut hk = HKEY::default();
    if RegOpenKeyExW(root, PCWSTR(wsub.as_ptr()), 0, sam, &mut hk).is_ok() {
        let _ = RegCloseKey(hk);
        true
    } else {
        false
    }
}

unsafe fn reg_set_sz(hk: HKEY, name: Option<&str>, value: &str) -> Result<(), String> {
    let wname = name.map(wide_null);
    let wval = wide_null(value);
    let bytes: Vec<u8> = wval.iter().flat_map(|u| u.to_le_bytes()).collect();
    let pname = wname
        .as_ref()
        .map(|v| PCWSTR(v.as_ptr()))
        .unwrap_or(PCWSTR::null());
    let err = RegSetValueExW(hk, pname, 0, REG_SZ, Some(&bytes));
    if err.is_ok() {
        Ok(())
    } else {
        Err(format!("RegSetValueExW failed: {:?}", err))
    }
}

unsafe fn reg_set_dword(hk: HKEY, name: &str, value: u32) -> Result<(), String> {
    let wname = wide_null(name);
    let bytes = value.to_le_bytes();
    let err = RegSetValueExW(hk, PCWSTR(wname.as_ptr()), 0, REG_DWORD, Some(&bytes));
    if err.is_ok() {
        Ok(())
    } else {
        Err(format!("RegSetValueExW DWORD failed: {:?}", err))
    }
}

fn default_install_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Programs").join("Kurowser")
}

fn user_profile_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Kurowser")
}

fn known_folder_path(id: &windows::core::GUID) -> Option<PathBuf> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(pw): windows::core::Result<PWSTR> =
            SHGetKnownFolderPath(id, KNOWN_FOLDER_FLAG(0), None)
        else {
            return None;
        };
        if pw.is_null() {
            return None;
        }
        let s = pw.to_string().ok();
        CoTaskMemFree(Some(pw.0 as _));
        s.map(PathBuf::from)
    }
}

fn start_menu_shortcut_path() -> Option<PathBuf> {
    known_folder_path(&FOLDERID_Programs).map(|p| p.join("Kurowser.lnk"))
}

fn desktop_shortcut_path() -> Option<PathBuf> {
    known_folder_path(&FOLDERID_Desktop).map(|p| p.join("Kurowser.lnk"))
}

fn create_shortcut(lnk_path: &Path, target: &Path, workdir: &Path, desc: &str) -> Result<(), String> {
    if let Some(parent) = lnk_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("CoCreateInstance ShellLink failed: {:?}", e))?;
        link.SetPath(&HSTRING::from(target.as_os_str()))
            .map_err(|e| format!("SetPath failed: {:?}", e))?;
        link.SetWorkingDirectory(&HSTRING::from(workdir.as_os_str()))
            .map_err(|e| format!("SetWorkingDirectory failed: {:?}", e))?;
        link.SetDescription(&HSTRING::from(desc))
            .map_err(|e| format!("SetDescription failed: {:?}", e))?;
        link.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)
            .map_err(|e| format!("SetIconLocation failed: {:?}", e))?;
        let pf: IPersistFile = link
            .cast()
            .map_err(|e| format!("IPersistFile cast failed: {:?}", e))?;
        pf.Save(&HSTRING::from(lnk_path.as_os_str()), true)
            .map_err(|e| format!("Shortcut Save failed: {:?}", e))?;
    }
    Ok(())
}

fn write_uninstall_registry(install_dir: &Path, exe_path: &Path, uninstall_exe: &Path) -> Result<(), String> {
    unsafe {
        let wsub = wide_null(UNINSTALL_REG_KEY);
        let mut hk = HKEY::default();
        let err = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wsub.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hk,
            None,
        );
        if !err.is_ok() {
            return Err(format!("Gagal menulis Uninstall registry: {:?}", err));
        }

        let icon_val = format!("{},0", exe_path.display());
        let uninst_cmd = format!("\"{}\" --uninstall", uninstall_exe.display());
        let quiet_uninst_cmd = format!("\"{}\" --uninstall --silent", uninstall_exe.display());
        let est_kb = ((KUROWSER_EXE_BYTES.len() / 1024).max(1)) as u32;

        let res: Result<(), String> = (|| {
            reg_set_sz(hk, Some("DisplayName"), APP_NAME)?;
            reg_set_sz(hk, Some("DisplayVersion"), APP_VERSION)?;
            reg_set_sz(hk, Some("Publisher"), APP_PUBLISHER)?;
            reg_set_sz(hk, Some("DisplayIcon"), &icon_val)?;
            reg_set_sz(hk, Some("InstallLocation"), &install_dir.to_string_lossy())?;
            reg_set_sz(hk, Some("UninstallString"), &uninst_cmd)?;
            reg_set_sz(hk, Some("QuietUninstallString"), &quiet_uninst_cmd)?;
            reg_set_sz(
                hk,
                Some("URLInfoAbout"),
                "https://github.com/devkecelup/kurowser",
            )?;
            reg_set_dword(hk, "EstimatedSize", est_kb)?;
            reg_set_dword(hk, "NoModify", 1)?;
            reg_set_dword(hk, "NoRepair", 1)?;
            Ok(())
        })();
        let _ = RegCloseKey(hk);
        res?;

        // App Paths (allows Win+R -> kurowser)
        let wapp = wide_null(APP_PATHS_REG_KEY);
        let mut hk_app = HKEY::default();
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(wapp.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hk_app,
            None,
        )
        .is_ok()
        {
            let _ = reg_set_sz(hk_app, None, &exe_path.to_string_lossy());
            let _ = reg_set_sz(hk_app, Some("Path"), &install_dir.to_string_lossy());
            let _ = RegCloseKey(hk_app);
        }
    }
    Ok(())
}

fn wait_for_pid_exit(pid: u32, timeout_ms: u32) {
    unsafe {
        if let Ok(handle) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            if !handle.is_invalid() {
                let _ = WaitForSingleObject(handle, timeout_ms);
                let _ = CloseHandle(handle);
            }
        }
    }
}

fn perform_install(opts: &CliOptions) -> Result<PathBuf, String> {
    if let Some(pid) = opts.wait_pid {
        if pid != std::process::id() {
            wait_for_pid_exit(pid, 8000);
        }
    }

    let install_dir = default_install_dir();
    fs::create_dir_all(&install_dir)
        .map_err(|e| format!("Gagal membuat folder instalasi: {}", e))?;

    let target_exe = install_dir.join("kurowser.exe");

    // If a previous install exists and the file is locked (Kurowser is open),
    // wait briefly for graceful exit, then terminate only if still locked.
    if let Err(e) = fs::write(&target_exe, KUROWSER_EXE_BYTES) {
        if target_exe.exists() {
            let mut last_err = e.to_string();
            let mut written = false;

            for attempt in 0..4 {
                if attempt == 1 {
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    let _ = Command::new("taskkill")
                        .args(["/F", "/IM", "kurowser.exe"])
                        .creation_flags(CREATE_NO_WINDOW)
                        .output();
                }
                std::thread::sleep(std::time::Duration::from_millis(500));
                match fs::write(&target_exe, KUROWSER_EXE_BYTES) {
                    Ok(()) => {
                        written = true;
                        break;
                    }
                    Err(e2) => last_err = e2.to_string(),
                }
            }
            if !written {
                return Err(format!(
                    "Gagal menyalin kurowser.exe (tutup Kurowser dan coba lagi): {}",
                    last_err
                ));
            }
        } else {
            return Err(format!("Gagal menyalin kurowser.exe: {}", e));
        }
    }

    let uninstall_exe = install_dir.join("Uninstall.exe");
    if let Ok(self_exe) = std::env::current_exe() {
        if self_exe != uninstall_exe {
            let _ = fs::copy(&self_exe, &uninstall_exe);
        }
    }

    if let Some(sm) = start_menu_shortcut_path() {
        create_shortcut(&sm, &target_exe, &install_dir, "Kurowser Web Browser")?;
    }
    let write_desktop = if opts.preserve_shortcuts {
        desktop_shortcut_path().is_some_and(|p| p.exists())
    } else {
        opts.desktop_shortcut
    };
    if write_desktop {
        if let Some(dk) = desktop_shortcut_path() {
            create_shortcut(&dk, &target_exe, &install_dir, "Kurowser Web Browser")?;
        }
    }

    write_uninstall_registry(&install_dir, &target_exe, &uninstall_exe)?;
    unsafe {
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }
    Ok(target_exe)
}

fn perform_uninstall(purge_data: bool) -> Result<(), String> {
    let install_dir = default_install_dir();
    let target_exe = install_dir.join("kurowser.exe");
    let uninstall_exe = install_dir.join("Uninstall.exe");

    if target_exe.exists() {
        fs::remove_file(&target_exe).map_err(|e| {
            format!(
                "Gagal menghapus kurowser.exe (tutup Kurowser yang sedang berjalan dulu): {}",
                e
            )
        })?;
    }

    if let Some(sm) = start_menu_shortcut_path() {
        let _ = fs::remove_file(sm);
    }
    if let Some(dk) = desktop_shortcut_path() {
        let _ = fs::remove_file(dk);
    }

    unsafe {
        let wu = wide_null(UNINSTALL_REG_KEY);
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wu.as_ptr()));
        let wa = wide_null(APP_PATHS_REG_KEY);
        let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(wa.as_ptr()));
        SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None);
    }

    if purge_data {
        let prof = user_profile_dir();
        if prof.exists() {
            let _ = fs::remove_dir_all(prof);
        }
    }

    // Handle Uninstall.exe self-deletion safely (deferred until process exit when running from install_dir).
    let mut deferred_self_delete: Option<(PathBuf, PathBuf)> = None;
    if let Ok(self_exe) = std::env::current_exe() {
        let install_str = install_dir.to_string_lossy();
        let same_dir = self_exe
            .parent()
            .is_some_and(|p| p.to_string_lossy().eq_ignore_ascii_case(&install_str));
        if same_dir && uninstall_exe.exists() {
            deferred_self_delete = Some((uninstall_exe, install_dir));
        } else {
            let _ = fs::remove_file(&uninstall_exe);
            let _ = fs::remove_dir(&install_dir);
        }
    }

    if let Some(pair) = deferred_self_delete {
        PENDING_SELF_DELETE.with(|c| {
            *c.borrow_mut() = Some(pair);
        });
    }

    Ok(())
}

// ── Native Win32 Dark Mode GUI ──

#[derive(Clone, Copy, PartialEq, Eq)]
enum UiStep {
    Ready,
    Done,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum HitItem {
    None,
    CheckDesktopOrPurge,
    CheckLaunch,
    PrimaryBtn,
    SecondaryBtn,
}

struct LogoBitmap {
    w: i32,
    h: i32,
    bgra: Vec<u8>,
}

fn decode_logo_on_bg(bg: (u8, u8, u8)) -> Option<LogoBitmap> {
    let decoder = png::Decoder::new(std::io::Cursor::new(LOGO_PNG_BYTES));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let w = info.width as i32;
    let h = info.height as i32;
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    for i in 0..(w * h) as usize {
        let (r, g, b, a) = match info.color_type {
            png::ColorType::Rgba => (buf[i * 4], buf[i * 4 + 1], buf[i * 4 + 2], buf[i * 4 + 3]),
            png::ColorType::Rgb => (buf[i * 3], buf[i * 3 + 1], buf[i * 3 + 2], 255),
            _ => (bg.0, bg.1, bg.2, 0),
        };
        let alpha = a as f32 / 255.0;
        let out_r = (r as f32 * alpha + bg.0 as f32 * (1.0 - alpha)).round() as u8;
        let out_g = (g as f32 * alpha + bg.1 as f32 * (1.0 - alpha)).round() as u8;
        let out_b = (b as f32 * alpha + bg.2 as f32 * (1.0 - alpha)).round() as u8;
        bgra[i * 4] = out_b;
        bgra[i * 4 + 1] = out_g;
        bgra[i * 4 + 2] = out_r;
        bgra[i * 4 + 3] = 255;
    }
    Some(LogoBitmap { w, h, bgra })
}

struct GuiState {
    opts: CliOptions,
    webview2_ok: bool,
    is_update: bool,
    installed_ver: Option<String>,
    step: UiStep,
    status_msg: String,
    installed_exe: Option<PathBuf>,
    hover: HitItem,
    scale: f64,
    logo: Option<LogoBitmap>,
}

thread_local! {
    static STATE: RefCell<Option<GuiState>> = const { RefCell::new(None) };
    static PENDING_SELF_DELETE: RefCell<Option<(PathBuf, PathBuf)>> = const { RefCell::new(None) };
}

fn spawn_pending_self_delete() {
    let pair = PENDING_SELF_DELETE.with(|c| c.borrow_mut().take());
    if let Some((uninstall_exe, install_dir)) = pair {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let _ = std::env::set_current_dir(std::env::temp_dir());
        let cmd_line = format!(
            "/D /S /C \"ping -n 2 127.0.0.1 >nul & del /F /Q \"{0}\" 2>nul & rmdir \"{1}\" 2>nul & ping -n 2 127.0.0.1 >nul & del /F /Q \"{0}\" 2>nul & rmdir \"{1}\" 2>nul\"",
            uninstall_exe.display(),
            install_dir.display()
        );
        let _ = Command::new("cmd.exe")
            .current_dir(std::env::temp_dir())
            .raw_arg(cmd_line)
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}

fn sc(v: i32, s: f64) -> i32 {
    ((v as f64) * s).round() as i32
}

fn c_ref(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

fn pt_in(x: i32, y: i32, rc: &RECT) -> bool {
    x >= rc.left && x < rc.right && y >= rc.top && y < rc.bottom
}

struct LayoutRects {
    chk1: RECT,
    chk2: RECT,
    primary_btn: RECT,
    secondary_btn: RECT,
}

fn compute_rects(w: i32, h: i32, scale: f64, show_two_btns: bool) -> LayoutRects {
    let pad = sc(28, scale);
    let chk1 = RECT {
        left: pad,
        top: sc(238, scale),
        right: w - pad,
        bottom: sc(268, scale),
    };
    let chk2 = RECT {
        left: pad,
        top: sc(274, scale),
        right: w - pad,
        bottom: sc(304, scale),
    };
    let btn_h = sc(42, scale);
    let btn_y = h - pad - btn_h;
    if show_two_btns {
        let sec_w = sc(116, scale);
        let gap = sc(10, scale);
        let secondary_btn = RECT {
            left: w - pad - sec_w,
            top: btn_y,
            right: w - pad,
            bottom: btn_y + btn_h,
        };
        let primary_btn = RECT {
            left: pad,
            top: btn_y,
            right: secondary_btn.left - gap,
            bottom: btn_y + btn_h,
        };
        LayoutRects {
            chk1,
            chk2,
            primary_btn,
            secondary_btn,
        }
    } else {
        let primary_btn = RECT {
            left: pad,
            top: btn_y,
            right: w - pad,
            bottom: btn_y + btn_h,
        };
        LayoutRects {
            chk1,
            chk2,
            primary_btn,
            secondary_btn: RECT::default(),
        }
    }
}

unsafe fn mk_font(scale: f64, px: i32, weight: i32) -> HFONT {
    let face = wide_null("Poppins");
    let mut lf = LOGFONTW::default();
    lf.lfHeight = -sc(px, scale);
    lf.lfWeight = weight;
    lf.lfCharSet = FONT_CHARSET(DEFAULT_CHARSET.0 as u8);
    lf.lfQuality = FONT_QUALITY(CLEARTYPE_QUALITY.0 as u8);
    for (i, ch) in face.iter().enumerate().take(31) {
        lf.lfFaceName[i] = *ch;
    }
    CreateFontIndirectW(&lf)
}

unsafe fn draw_round_box(hdc: HDC, rc: &RECT, radius: i32, fill: COLORREF, border: COLORREF) {
    let br = CreateSolidBrush(fill);
    let pen = CreatePen(PS_SOLID, 1, border);
    let old_b = SelectObject(hdc, br);
    let old_p = SelectObject(hdc, pen);
    let _ = RoundRect(hdc, rc.left, rc.top, rc.right, rc.bottom, radius, radius);
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(br);
    let _ = DeleteObject(pen);
}

unsafe fn draw_text(hdc: HDC, text: &str, rc: &RECT, color: COLORREF, font: HFONT, flags: DRAW_TEXT_FORMAT) {
    SetTextColor(hdc, color);
    let old_f = SelectObject(hdc, font);
    let mut wide: Vec<u16> = OsString::from(text).encode_wide().collect();
    let mut r = *rc;
    let _ = DrawTextW(hdc, &mut wide, &mut r, flags | DT_SINGLELINE | DT_NOPREFIX);
    SelectObject(hdc, old_f);
}

unsafe fn draw_checkbox(
    hdc: HDC,
    rc: &RECT,
    scale: f64,
    checked: bool,
    hovered: bool,
    label: &str,
    font: HFONT,
) {
    let box_sz = sc(18, scale);
    let bx = rc.left;
    let by = rc.top + (rc.bottom - rc.top - box_sz) / 2;
    let b_rc = RECT {
        left: bx,
        top: by,
        right: bx + box_sz,
        bottom: by + box_sz,
    };
    let fill = if checked {
        c_ref(15, 118, 110)
    } else {
        c_ref(30, 36, 45)
    };
    let border = if checked {
        c_ref(20, 184, 166)
    } else if hovered {
        c_ref(94, 234, 212)
    } else {
        c_ref(117, 126, 140)
    };
    draw_round_box(hdc, &b_rc, sc(5, scale), fill, border);

    if checked {
        let pen = CreatePen(PS_SOLID, sc(2, scale).max(1), c_ref(255, 255, 255));
        let old = SelectObject(hdc, pen);
        let _ = MoveToEx(hdc, bx + sc(4, scale), by + sc(9, scale), None);
        let _ = LineTo(hdc, bx + sc(8, scale), by + sc(13, scale));
        let _ = LineTo(hdc, bx + sc(14, scale), by + sc(5, scale));
        SelectObject(hdc, old);
        let _ = DeleteObject(pen);
    }

    let lbl_rc = RECT {
        left: bx + box_sz + sc(10, scale),
        top: rc.top,
        right: rc.right,
        bottom: rc.bottom,
    };
    draw_text(
        hdc,
        label,
        &lbl_rc,
        if hovered {
            c_ref(242, 245, 249)
        } else {
            c_ref(193, 201, 212)
        },
        font,
        DT_LEFT | DT_VCENTER,
    );
}

unsafe fn paint_window(hwnd: HWND, hdc_screen: HDC) {
    let mut cli = RECT::default();
    let _ = GetClientRect(hwnd, &mut cli);
    let w = (cli.right - cli.left).max(1);
    let h = (cli.bottom - cli.top).max(1);

    let mem = CreateCompatibleDC(hdc_screen);
    let bmp = CreateCompatibleBitmap(hdc_screen, w, h);
    let old_bmp = SelectObject(mem, bmp);
    SetBkMode(mem, TRANSPARENT);

    let bg = CreateSolidBrush(c_ref(15, 18, 23));
    FillRect(mem, &cli, bg);
    let _ = DeleteObject(bg);

    STATE.with(|cell| {
        let guard = cell.borrow();
        let Some(st) = guard.as_ref() else {
            return;
        };
        let s = st.scale;
        let pad = sc(28, s);

        let f_title = mk_font(s, 22, FW_SEMIBOLD.0 as i32);
        let f_sub = mk_font(s, 13, FW_REGULAR.0 as i32);
        let f_body = mk_font(s, 13, FW_MEDIUM.0 as i32);
        let f_btn = mk_font(s, 14, FW_SEMIBOLD.0 as i32);
        let f_small = mk_font(s, 12, FW_REGULAR.0 as i32);

        // Logo
        let logo_sz = sc(46, s);
        if let Some(ref logo) = st.logo {
            let mut bmi = BITMAPINFO::default();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = logo.w;
            bmi.bmiHeader.biHeight = -logo.h;
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB.0;
            SetStretchBltMode(mem, HALFTONE);
            let _ = StretchDIBits(
                mem,
                pad,
                sc(24, s),
                logo_sz,
                logo_sz,
                0,
                0,
                logo.w,
                logo.h,
                Some(logo.bgra.as_ptr() as _),
                &bmi,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
        }

        let title_str = match (st.opts.mode, st.step) {
            (AppMode::Install, UiStep::Done) if st.is_update => "Kurowser Diperbarui!",
            (AppMode::Install, UiStep::Done) => "Kurowser Terpasang!",
            (AppMode::Install, _) if st.is_update => "Perbarui Kurowser",
            (AppMode::Install, _) => "Install Kurowser",
            (AppMode::Uninstall, UiStep::Done) => "Kurowser Dihapus",
            (AppMode::Uninstall, _) => "Uninstall Kurowser",
        };
        let size_label = exe_size_label();
        let sub_str = match st.opts.mode {
            AppMode::Install if st.is_update => {
                if let Some(ref prev) = st.installed_ver {
                    format!("Memperbarui v{prev} -> {size_label} · Windows x64")
                } else {
                    format!("Memperbarui ke {size_label} · Windows x64")
                }
            }
            AppMode::Install => format!("Browser Windows · {size_label}"),
            AppMode::Uninstall => "Menghapus aplikasi, shortcut, dan entri sistem".to_string(),
        };
        let sub_str = sub_str.as_str();
        draw_text(
            mem,
            title_str,
            &RECT {
                left: pad + logo_sz + sc(14, s),
                top: sc(22, s),
                right: w - pad,
                bottom: sc(50, s),
            },
            c_ref(242, 245, 249),
            f_title,
            DT_LEFT | DT_VCENTER,
        );
        draw_text(
            mem,
            sub_str,
            &RECT {
                left: pad + logo_sz + sc(14, s),
                top: sc(49, s),
                right: w - pad,
                bottom: sc(72, s),
            },
            c_ref(163, 173, 185),
            f_sub,
            DT_LEFT | DT_VCENTER,
        );

        // Info card (Graphite chrome + border_subtle)
        let card_rc = RECT {
            left: pad,
            top: sc(92, s),
            right: w - pad,
            bottom: sc(222, s),
        };
        draw_round_box(
            mem,
            &card_rc,
            sc(12, s),
            c_ref(22, 26, 33),
            c_ref(47, 55, 68),
        );

        let dir_lbl = if st.opts.mode == AppMode::Install {
            "Lokasi Instalasi:"
        } else {
            "Folder Aplikasi yang Dihapus:"
        };
        draw_text(
            mem,
            dir_lbl,
            &RECT {
                left: card_rc.left + sc(16, s),
                top: card_rc.top + sc(12, s),
                right: card_rc.right - sc(16, s),
                bottom: card_rc.top + sc(32, s),
            },
            c_ref(163, 173, 185),
            f_small,
            DT_LEFT | DT_VCENTER,
        );
        let dir_str = default_install_dir().to_string_lossy().to_string();
        draw_text(
            mem,
            &dir_str,
            &RECT {
                left: card_rc.left + sc(16, s),
                top: card_rc.top + sc(32, s),
                right: card_rc.right - sc(16, s),
                bottom: card_rc.top + sc(54, s),
            },
            c_ref(242, 245, 249),
            f_body,
            DT_LEFT | DT_VCENTER | DT_PATH_ELLIPSIS,
        );

        let line2_rc = RECT {
            left: card_rc.left + sc(16, s),
            top: card_rc.top + sc(64, s),
            right: card_rc.right - sc(16, s),
            bottom: card_rc.top + sc(118, s),
        };
        if !st.status_msg.is_empty() {
            let col = if st.step == UiStep::Error {
                c_ref(253, 164, 175)
            } else {
                c_ref(110, 231, 183)
            };
            let mut msg_w: Vec<u16> = OsString::from(&st.status_msg).encode_wide().collect();
            let old_f = SelectObject(mem, f_small);
            SetTextColor(mem, col);
            let mut r = line2_rc;
            let _ = DrawTextW(mem, &mut msg_w, &mut r, DT_LEFT | DT_WORDBREAK | DT_NOPREFIX);
            SelectObject(mem, old_f);
        } else if st.opts.mode == AppMode::Install {
            let wv_txt = if st.webview2_ok {
                "WebView2 Runtime: Terdeteksi di Windows (Siap)"
            } else {
                "WebView2 Runtime belum terdeteksi; disarankan pasang WebView2."
            };
            let wv_col = if st.webview2_ok {
                c_ref(110, 231, 183)
            } else {
                c_ref(252, 211, 77)
            };
            let mut wv_w: Vec<u16> = OsString::from(wv_txt).encode_wide().collect();
            let old_f = SelectObject(mem, f_small);
            SetTextColor(mem, wv_col);
            let mut r = line2_rc;
            let _ = DrawTextW(mem, &mut wv_w, &mut r, DT_LEFT | DT_WORDBREAK | DT_NOPREFIX);
            SelectObject(mem, old_f);
        } else {
            let note = "Riwayat & bookmark di %LOCALAPPDATA%\\Kurowser tetap aman kecuali dicentang.";
            let mut note_w: Vec<u16> = OsString::from(note).encode_wide().collect();
            let old_f = SelectObject(mem, f_small);
            SetTextColor(mem, c_ref(163, 173, 185));
            let mut r = line2_rc;
            let _ = DrawTextW(mem, &mut note_w, &mut r, DT_LEFT | DT_WORDBREAK | DT_NOPREFIX);
            SelectObject(mem, old_f);
        }

        let show_two = st.step == UiStep::Ready || st.step == UiStep::Error;
        let rects = compute_rects(w, h, s, show_two);

        if st.step == UiStep::Ready {
            match st.opts.mode {
                AppMode::Install => {
                    draw_checkbox(
                        mem,
                        &rects.chk1,
                        s,
                        st.opts.desktop_shortcut,
                        st.hover == HitItem::CheckDesktopOrPurge,
                        "Buat shortcut Kurowser di Desktop (+ Start Menu)",
                        f_body,
                    );
                    draw_checkbox(
                        mem,
                        &rects.chk2,
                        s,
                        st.opts.launch_after,
                        st.hover == HitItem::CheckLaunch,
                        "Jalankan Kurowser setelah instalasi selesai",
                        f_body,
                    );
                }
                AppMode::Uninstall => {
                    draw_checkbox(
                        mem,
                        &rects.chk1,
                        s,
                        st.opts.purge_data,
                        st.hover == HitItem::CheckDesktopOrPurge,
                        "Hapus juga profil, riwayat, bookmark & folder",
                        f_body,
                    );
                }
            }
        }

        let primary_lbl = match (st.opts.mode, st.step) {
            (AppMode::Install, UiStep::Done) => "Buka Kurowser Sekarang",
            (AppMode::Install, _) if st.is_update => "Perbarui Kurowser",
            (AppMode::Install, _) => "Install Kurowser",
            (AppMode::Uninstall, UiStep::Done) => "Tutup",
            (AppMode::Uninstall, _) => "Hapus Kurowser",
        };
        let p_fill = match (st.opts.mode, st.step) {
            (AppMode::Uninstall, UiStep::Ready) | (AppMode::Uninstall, UiStep::Error) => {
                if st.hover == HitItem::PrimaryBtn {
                    c_ref(185, 28, 28)
                } else {
                    c_ref(220, 38, 38)
                }
            }
            _ => {
                if st.hover == HitItem::PrimaryBtn {
                    c_ref(11, 93, 86)
                } else {
                    c_ref(15, 118, 110)
                }
            }
        };
        let p_txt = c_ref(255, 255, 255);
        let p_border = if matches!(
            (st.opts.mode, st.step),
            (AppMode::Uninstall, UiStep::Ready) | (AppMode::Uninstall, UiStep::Error)
        ) {
            c_ref(251, 113, 133)
        } else {
            c_ref(20, 184, 166)
        };
        draw_round_box(mem, &rects.primary_btn, sc(10, s), p_fill, p_border);
        draw_text(
            mem,
            primary_lbl,
            &rects.primary_btn,
            p_txt,
            f_btn,
            DT_CENTER | DT_VCENTER,
        );

        if show_two {
            let s_fill = if st.hover == HitItem::SecondaryBtn {
                c_ref(38, 45, 56)
            } else {
                c_ref(30, 36, 45)
            };
            draw_round_box(
                mem,
                &rects.secondary_btn,
                sc(10, s),
                s_fill,
                c_ref(117, 126, 140),
            );
            draw_text(
                mem,
                "Batal",
                &rects.secondary_btn,
                c_ref(242, 245, 249),
                f_body,
                DT_CENTER | DT_VCENTER,
            );
        }

        let _ = DeleteObject(f_title);
        let _ = DeleteObject(f_sub);
        let _ = DeleteObject(f_body);
        let _ = DeleteObject(f_btn);
        let _ = DeleteObject(f_small);
    });

    let _ = BitBlt(hdc_screen, 0, 0, w, h, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old_bmp);
    let _ = DeleteObject(bmp);
    let _ = DeleteDC(mem);
}

fn hit_test_gui(hwnd: HWND, x: i32, y: i32) -> HitItem {
    let mut cli = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut cli);
    }
    let w = (cli.right - cli.left).max(1);
    let h = (cli.bottom - cli.top).max(1);

    STATE.with(|cell| {
        let guard = cell.borrow();
        let Some(st) = guard.as_ref() else {
            return HitItem::None;
        };
        let show_two = st.step == UiStep::Ready || st.step == UiStep::Error;
        let rects = compute_rects(w, h, st.scale, show_two);
        if pt_in(x, y, &rects.primary_btn) {
            return HitItem::PrimaryBtn;
        }
        if show_two && pt_in(x, y, &rects.secondary_btn) {
            return HitItem::SecondaryBtn;
        }
        if st.step == UiStep::Ready {
            if pt_in(x, y, &rects.chk1) {
                return HitItem::CheckDesktopOrPurge;
            }
            if st.opts.mode == AppMode::Install && pt_in(x, y, &rects.chk2) {
                return HitItem::CheckLaunch;
            }
        }
        HitItem::None
    })
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            paint_window(hwnd, hdc);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let hit = hit_test_gui(hwnd, x, y);
            let changed = STATE.with(|cell| {
                let mut g = cell.borrow_mut();
                if let Some(st) = g.as_mut() {
                    if st.hover != hit {
                        st.hover = hit;
                        return true;
                    }
                }
                false
            });
            if changed {
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let hit_code = (lparam.0 & 0xFFFF) as u32;
            if hit_code == HTCLIENT {
                let hovered = STATE.with(|c| {
                    c.borrow()
                        .as_ref()
                        .is_some_and(|st| st.hover != HitItem::None)
                });
                if hovered {
                    if let Ok(cur) = LoadCursorW(None, IDC_HAND) {
                        SetCursor(cur);
                        return LRESULT(1);
                    }
                }
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let hit = hit_test_gui(hwnd, x, y);
            match hit {
                HitItem::CheckDesktopOrPurge => {
                    STATE.with(|c| {
                        if let Some(st) = c.borrow_mut().as_mut() {
                            match st.opts.mode {
                                AppMode::Install => st.opts.desktop_shortcut = !st.opts.desktop_shortcut,
                                AppMode::Uninstall => st.opts.purge_data = !st.opts.purge_data,
                            }
                        }
                    });
                    let _ = InvalidateRect(hwnd, None, false);
                }
                HitItem::CheckLaunch => {
                    STATE.with(|c| {
                        if let Some(st) = c.borrow_mut().as_mut() {
                            st.opts.launch_after = !st.opts.launch_after;
                        }
                    });
                    let _ = InvalidateRect(hwnd, None, false);
                }
                HitItem::SecondaryBtn => {
                    let _ = DestroyWindow(hwnd);
                }
                HitItem::PrimaryBtn => {
                    let action = STATE.with(|c| {
                        let g = c.borrow();
                        g.as_ref().map(|st| (st.opts.clone(), st.step, st.installed_exe.clone()))
                    });
                    if let Some((opts, step, exe_opt)) = action {
                        match (opts.mode, step) {
                            (AppMode::Install, UiStep::Done) => {
                                if let Some(exe) = exe_opt {
                                    let _ = Command::new(exe).spawn();
                                }
                                let _ = DestroyWindow(hwnd);
                            }
                            (AppMode::Uninstall, UiStep::Done) => {
                                let _ = DestroyWindow(hwnd);
                            }
                            (AppMode::Install, _) => {
                                let launch_now = opts.launch_after;
                                let was_update = STATE.with(|c| {
                                    c.borrow().as_ref().map(|st| st.is_update).unwrap_or(false)
                                });
                                match perform_install(&opts) {
                                    Ok(exe) => {
                                        if launch_now {
                                            let _ = Command::new(&exe).spawn();
                                            let _ = DestroyWindow(hwnd);
                                        } else {
                                            STATE.with(|c| {
                                                if let Some(st) = c.borrow_mut().as_mut() {
                                                    st.step = UiStep::Done;
                                                    st.installed_exe = Some(exe);
                                                    st.status_msg = if was_update {
                                                        format!("Kurowser v{APP_VERSION} berhasil diperbarui.")
                                                    } else {
                                                        "Kurowser, shortcut Start Menu, dan entri Apps & Features berhasil dipasang.".to_string()
                                                    };
                                                }
                                            });
                                            let _ = InvalidateRect(hwnd, None, false);
                                        }
                                    }
                                    Err(err) => {
                                        STATE.with(|c| {
                                            if let Some(st) = c.borrow_mut().as_mut() {
                                                st.step = UiStep::Error;
                                                st.status_msg = err;
                                            }
                                        });
                                        let _ = InvalidateRect(hwnd, None, false);
                                    }
                                }
                            }
                            (AppMode::Uninstall, _) => match perform_uninstall(opts.purge_data) {
                                Ok(()) => {
                                    STATE.with(|c| {
                                        if let Some(st) = c.borrow_mut().as_mut() {
                                            st.step = UiStep::Done;
                                            st.status_msg = "Kurowser dan seluruh shortcut berhasil dihapus.".to_string();
                                        }
                                    });
                                    let _ = InvalidateRect(hwnd, None, false);
                                }
                                Err(err) => {
                                    STATE.with(|c| {
                                        if let Some(st) = c.borrow_mut().as_mut() {
                                            st.step = UiStep::Error;
                                            st.status_msg = err;
                                        }
                                    });
                                    let _ = InvalidateRect(hwnd, None, false);
                                }
                            },
                        }
                    }
                }
                HitItem::None => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn run_gui(opts: CliOptions) {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    let _font_guard = load_poppins();
    let webview2_ok = detect_webview2_installed();
    let logo = decode_logo_on_bg((15, 18, 23));
    let is_update = opts.mode == AppMode::Install && is_already_installed();
    let installed_ver = if is_update { read_installed_version() } else { None };

    // Window title changes dynamically based on update vs fresh install.
    // Using a stack-allocated wide string to avoid static lifetime issues.
    let win_title_str = match opts.mode {
        AppMode::Install if is_update => format!("Perbarui Kurowser v{APP_VERSION}\0"),
        AppMode::Install => format!("Kurowser Installer v{APP_VERSION}\0"),
        AppMode::Uninstall => format!("Uninstall Kurowser v{APP_VERSION}\0"),
    };
    let win_title_wide: Vec<u16> = win_title_str.encode_utf16().collect();

    STATE.with(|cell| {
        *cell.borrow_mut() = Some(GuiState {
            opts,
            webview2_ok,
            is_update,
            installed_ver,
            step: UiStep::Ready,
            status_msg: String::new(),
            installed_exe: None,
            hover: HitItem::None,
            scale: 1.0,
            logo,
        });
    });

    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let cls = w!("KUROWSER_SETUP_WND");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hIcon: LoadIconW(hinst, PCWSTR(1 as _)).unwrap_or_default(),
            lpszClassName: cls,
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            cls,
            PCWSTR(win_title_wide.as_ptr()),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            LOG_W,
            LOG_H,
            None,
            None,
            hinst,
            None,
        )
        .expect("Failed to create installer window");

        let dpi = GetDpiForWindow(hwnd) as f64;
        let scale = if dpi > 0.0 { dpi / 96.0 } else { 1.0 };
        STATE.with(|cell| {
            if let Some(st) = cell.borrow_mut().as_mut() {
                st.scale = scale;
            }
        });

        let mut win_rc = RECT {
            left: 0,
            top: 0,
            right: sc(LOG_W, scale),
            bottom: sc(LOG_H, scale),
        };
        let _ = AdjustWindowRectExForDpi(&mut win_rc, style, false, WINDOW_EX_STYLE(0), dpi as u32);
        let ww = win_rc.right - win_rc.left;
        let wh = win_rc.bottom - win_rc.top;
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        let x = ((sw - ww) / 2).max(0);
        let y = ((sh - wh) / 2).max(0);
        let _ = SetWindowPos(hwnd, None, x, y, ww, wh, SWP_NOZORDER);

        let dark: BOOL = TRUE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const _ as _,
            std::mem::size_of::<BOOL>() as u32,
        );

        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        CoUninitialize();
    }
}

fn main() {
    let exe_hint = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));
    let opts = parse_cli_args(std::env::args().skip(1), exe_hint.as_deref());

    if opts.silent {
        let exit_code = match opts.mode {
            AppMode::Install => match perform_install(&opts) {
                Ok(exe) => {
                    if opts.launch_after {
                        let _ = Command::new(exe).spawn();
                    }
                    0
                }
                Err(_) => 1,
            },
            AppMode::Uninstall => match perform_uninstall(opts.purge_data) {
                Ok(()) => {
                    spawn_pending_self_delete();
                    0
                }
                Err(_) => 1,
            },
        };
        std::process::exit(exit_code);
    }

    run_gui(opts);
    spawn_pending_self_delete();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_install_and_uninstall_flags() {
        let def = parse_cli_args(Vec::<String>::new(), Some("KurowserSetup.exe"));
        assert_eq!(def.mode, AppMode::Install);
        assert!(!def.silent);
        assert!(def.desktop_shortcut);
        assert!(!def.preserve_shortcuts);
        assert!(def.launch_after);
        assert!(!def.purge_data);
        assert_eq!(def.wait_pid, None);

        let silent_inst = parse_cli_args(
            ["--silent", "--no-desktop", "--no-launch"],
            Some("KurowserSetup.exe"),
        );
        assert_eq!(silent_inst.mode, AppMode::Install);
        assert!(silent_inst.silent);
        assert!(!silent_inst.desktop_shortcut);
        assert!(!silent_inst.launch_after);

        let auto_upd = parse_cli_args(
            ["--silent", "--preserve-shortcuts", "--wait-pid", "4242", "--no-launch"],
            Some("KurowserSetup.exe"),
        );
        assert_eq!(auto_upd.mode, AppMode::Install);
        assert!(auto_upd.silent);
        assert!(auto_upd.preserve_shortcuts);
        assert!(!auto_upd.launch_after);
        assert_eq!(auto_upd.wait_pid, Some(4242));

        let eq_pid = parse_cli_args(["--wait-pid=999"], None);
        assert_eq!(eq_pid.wait_pid, Some(999));

        let uninst = parse_cli_args(["/uninstall", "--silent", "--purge-data"], None);
        assert_eq!(uninst.mode, AppMode::Uninstall);
        assert!(uninst.silent);
        assert!(uninst.purge_data);

        let uninst_by_name = parse_cli_args(Vec::<String>::new(), Some("Uninstall.exe"));
        assert_eq!(uninst_by_name.mode, AppMode::Uninstall);
    }

    #[test]
    fn computes_dynamic_version_size_label() {
        let lbl = exe_size_label();
        assert!(lbl.starts_with(&format!("v{}", APP_VERSION)));
        assert!(lbl.contains("MB)"));
    }

    #[test]
    fn resolves_per_user_install_dir() {
        let dir = default_install_dir();
        assert!(dir.ends_with(Path::new("Programs").join("Kurowser")));
    }
}
