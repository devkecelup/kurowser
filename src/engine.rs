// ─── engine.rs ─── Native WebView2 engine: kurowser:// custom scheme, Security & Shields ───
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{mpsc, Arc, Mutex, Once};

use webview2_com::Microsoft::Web::WebView2::Win32::{
    CreateCoreWebView2EnvironmentWithOptions, ICoreWebView2, ICoreWebView2Controller,
    ICoreWebView2Controller2, ICoreWebView2CustomSchemeRegistration, ICoreWebView2Environment,
    ICoreWebView2EnvironmentOptions, ICoreWebView2NavigationStartingEventArgs3,
    ICoreWebView2Settings3, ICoreWebView2Settings4, ICoreWebView2Settings5,
    ICoreWebView2Settings6, ICoreWebView2_13, ICoreWebView2_15, ICoreWebView2_19, ICoreWebView2_3,
    ICoreWebView2_4, ICoreWebView2_8, COREWEBVIEW2_COLOR, COREWEBVIEW2_DOWNLOAD_STATE,
    COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED, COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS,
    COREWEBVIEW2_KEY_EVENT_KIND, COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN,
    COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL,
    COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC, COREWEBVIEW2_NAVIGATION_KIND,
    COREWEBVIEW2_NAVIGATION_KIND_BACK_OR_FORWARD, COREWEBVIEW2_NAVIGATION_KIND_RELOAD,
    COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_CAMERA,
    COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ, COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE,
    COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION, COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS,
    COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
    COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES,
    COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS,
    COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS, COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS,
    COREWEBVIEW2_PERMISSION_STATE_DEFAULT, COREWEBVIEW2_PERMISSION_STATE_DENY,
    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK, COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT,
    COREWEBVIEW2_SCROLLBAR_STYLE_FLUENT_OVERLAY, COREWEBVIEW2_WEB_RESOURCE_CONTEXT,
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT,
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT,
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET,
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST,
};
use webview2_com::{
    take_pwstr, AcceleratorKeyPressedEventHandler,
    AddScriptToExecuteOnDocumentCreatedCompletedHandler, ContentLoadingEventHandler,
    CoreWebView2CustomSchemeRegistration, CoreWebView2EnvironmentOptions,
    CreateCoreWebView2ControllerCompletedHandler, CreateCoreWebView2EnvironmentCompletedHandler,
    DocumentTitleChangedEventHandler, DownloadStartingEventHandler, FaviconChangedEventHandler,
    GetFaviconCompletedHandler, NavigationCompletedEventHandler, NavigationStartingEventHandler,
    NewWindowRequestedEventHandler, PermissionRequestedEventHandler, SourceChangedEventHandler,
    StateChangedEventHandler, TrySuspendCompletedHandler, WebResourceRequestedEventHandler,
};
use windows::core::{w, HSTRING, Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::{BOOL, E_POINTER, E_UNEXPECTED, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateRoundRectRgn, SetWindowRgn};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED, IStream, STREAM_SEEK_SET};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::WinRT::EventRegistrationToken;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VK_1, VK_9, VK_B, VK_CONTROL, VK_D, VK_ESCAPE, VK_F5, VK_H, VK_J, VK_L,
    VK_LEFT, VK_MENU, VK_O, VK_OEM_COMMA, VK_R, VK_RIGHT, VK_SHIFT, VK_T, VK_TAB, VK_W,
};
use windows::Win32::UI::Shell::SHCreateMemStream;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, SetWindowPos, ShowWindow,
    CS_HREDRAW, CS_VREDRAW, HCURSOR, HICON, HMENU, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
    SWP_NOZORDER, SW_HIDE, SW_SHOW, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_VISIBLE,
    WINDOW_EX_STYLE,
};

use crate::adblock::{self, normalize_host};
use crate::pages;
use crate::storage::{
    constant_time_eq, domain_from_url, generate_csprng_hex, BrowserData, QuickLinkItem,
    SearchEngine, ThemePreset,
};

pub const START_URL: &str = "kurowser://start";
pub const HISTORY_URL: &str = "kurowser://history";
pub const BOOKMARKS_URL: &str = "kurowser://bookmarks";
pub const DOWNLOADS_URL: &str = "kurowser://downloads";
pub const SETTINGS_URL: &str = "kurowser://settings";

const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/Poppins-Regular.ttf");
const FONT_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Poppins-Medium.ttf");
const FONT_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Poppins-SemiBold.ttf");
const LOGO_PNG: &[u8] = include_bytes!("../assets/kurowser_logo.png");
const EMPTY_PNG_1X1: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

const BROWSER_ARGS: &str = "\
    --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
    --autoplay-policy=user-gesture-required \
    --process-per-site \
    --disable-background-networking \
    --disable-component-update \
    --disable-domain-reliability \
    --no-pings";

static REGISTER_CONTAINER_CLASS: Once = Once::new();

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PixelRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub corner_diameter: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryLevel {
    Normal,
    Low,
}

#[derive(Clone, Copy, Debug)]
pub enum AcceleratorAction {
    NewTab,
    CloseTab,
    FocusAddressBar,
    ToggleSidebar,
    Reload,
    ToggleBookmark,
    OpenHistory,
    OpenBookmarks,
    OpenDownloads,
    OpenSettings,
    NextTab,
    PrevTab,
    SelectTabIndex(usize),
    GoBack,
    GoForward,
    StopLoading,
}

#[derive(Clone, Debug)]
pub enum InternalApiEvent {
    SettingsUpdated,
    DataUpdated,
    ShieldsUpdated,
}

pub struct WebViewCallbacks {
    pub on_title: Box<dyn Fn(String) + 'static>,
    pub on_load: Box<dyn Fn(bool, String) + 'static>,
    pub on_url_changed: Box<dyn Fn(String) + 'static>,
    pub on_new_window: Box<dyn Fn(String) + 'static>,
    pub on_shortcut: Box<dyn Fn(AcceleratorAction) + 'static>,
    pub on_favicon: Box<dyn Fn(String, Vec<u8>) + 'static>,
    pub on_download_started: Box<dyn Fn(String, PathBuf) + 'static>,
    pub on_download_completed: Box<dyn Fn(String, Option<PathBuf>, bool) + 'static>,
    pub on_internal_api: Box<dyn Fn(InternalApiEvent) + 'static>,
    pub on_ad_blocked: Box<dyn Fn(String) + 'static>,
}

pub struct WebContext {
    env: ICoreWebView2Environment,
}

pub struct KurowserWebView {
    hwnd: HWND,
    controller: ICoreWebView2Controller,
    webview: ICoreWebView2,
    expected_internal_nav: Rc<RefCell<Option<String>>>,
    shields_script_id: Rc<RefCell<Option<String>>>,
}

impl Drop for KurowserWebView {
    fn drop(&mut self) {
        unsafe {
            let _ = self.controller.Close();
            if !self.hwnd.0.is_null() {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

impl KurowserWebView {
    pub fn focus(&self) {
        unsafe {
            let _ = self
                .controller
                .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC);
        }
    }

    pub fn set_bounds(&self, bounds: PixelRect) {
        let w = bounds.width.max(1);
        let h = bounds.height.max(1);
        unsafe {
            let _ = self.controller.SetBounds(RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            });
            let _ = SetWindowPos(
                self.hwnd,
                HWND::default(),
                bounds.x,
                bounds.y,
                w,
                h,
                SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER,
            );
            apply_container_region(self.hwnd, w, h, bounds.corner_diameter);
        }
    }

    pub fn set_visible(&self, visible: bool) {
        unsafe {
            let _ = ShowWindow(self.hwnd, if visible { SW_SHOW } else { SW_HIDE });
            let _ = self.controller.SetIsVisible(visible);
        }
    }

    pub fn set_background_color(&self, rgba: (u8, u8, u8, u8)) {
        unsafe {
            if let Ok(c2) = self.controller.cast::<ICoreWebView2Controller2>() {
                let _ = c2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                    R: rgba.0,
                    G: rgba.1,
                    B: rgba.2,
                    A: rgba.3,
                });
            }
        }
    }

    pub fn set_preferred_color_scheme(&self, is_light: bool) {
        unsafe {
            if let Ok(wv13) = self.webview.cast::<ICoreWebView2_13>() {
                if let Ok(profile) = wv13.Profile() {
                    let scheme = if is_light {
                        COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT
                    } else {
                        COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK
                    };
                    let _ = profile.SetPreferredColorScheme(scheme);
                }
            }
        }
    }

    pub fn set_memory_usage_level(&self, level: MemoryLevel) {
        if let Ok(wv19) = self.webview.cast::<ICoreWebView2_19>() {
            let lv = match level {
                MemoryLevel::Normal => COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(0),
                MemoryLevel::Low => COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL(1),
            };
            unsafe {
                let _ = wv19.SetMemoryUsageTargetLevel(lv);
            }
        }
    }

    pub fn load_url(&self, raw_url: &str) {
        let url = to_webview_url(raw_url);
        if is_kurowser_scheme(&url) {
            *self.expected_internal_nav.borrow_mut() = Some(to_display_url(&url));
        }
        let h = HSTRING::from(url);
        unsafe {
            let _ = self.webview.Navigate(&h);
        }
    }
}

fn web_context_dir() -> PathBuf {
    let local_app = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(local_app)
        .join("Kurowser")
        .join("WebView2Data")
}

pub fn create_web_context() -> WebContext {
    let dir = web_context_dir();
    let _ = std::fs::create_dir_all(&dir);

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let scheme = CoreWebView2CustomSchemeRegistration::new("kurowser".to_string());
    unsafe {
        scheme.set_treat_as_secure(true);
        scheme.set_has_authority_component(true);
        scheme.set_allowed_origins(vec!["kurowser://*".to_string()]);
    }
    let scheme_iface: ICoreWebView2CustomSchemeRegistration = scheme.into();

    let options = CoreWebView2EnvironmentOptions::default();
    unsafe {
        options.set_additional_browser_arguments(BROWSER_ARGS.to_string());
        options.set_enable_tracking_prevention(true);
        options.set_are_browser_extensions_enabled(false);
        options.set_is_custom_crash_reporting_enabled(false);
        options.set_scroll_bar_style(COREWEBVIEW2_SCROLLBAR_STYLE_FLUENT_OVERLAY);
        options.set_scheme_registrations(vec![Some(scheme_iface)]);
    }
    let options_iface: ICoreWebView2EnvironmentOptions = options.into();
    let data_dir = HSTRING::from(dir.as_os_str());

    let (tx, rx) = mpsc::channel();
    unsafe {
        let handler = CreateCoreWebView2EnvironmentCompletedHandler::create(Box::new(
            move |error_code, environment| {
                error_code?;
                tx.send(environment.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                    .map_err(|_| windows::core::Error::from(E_UNEXPECTED))
            },
        ));
        CreateCoreWebView2EnvironmentWithOptions(
            PCWSTR::null(),
            &data_dir,
            &options_iface,
            &handler,
        )
        .expect("Failed to initialize WebView2 environment");
    }

    let env = webview2_com::wait_with_pump(rx)
        .expect("WebView2 environment pump canceled")
        .expect("WebView2 environment creation failed");

    WebContext { env }
}

pub fn is_kurowser_scheme(url: &str) -> bool {
    let b = url.as_bytes();
    b.len() >= 11 && b[..11].eq_ignore_ascii_case(b"kurowser://")
}

pub fn is_dangerous_scheme(url: &str) -> bool {
    let low = url.trim().to_ascii_lowercase();
    low.starts_with("javascript:")
        || low.starts_with("vbscript:")
        || low.starts_with("data:text/html")
        || low.starts_with("data:application/xhtml+xml")
        || low.starts_with("data:image/svg+xml")
}

pub fn to_webview_url(display_url: &str) -> String {
    if is_kurowser_scheme(display_url) {
        let clean = canonical_kurowser_url(display_url);
        return clean;
    }
    display_url.to_string()
}

pub fn to_display_url(raw_url: &str) -> String {
    if is_kurowser_scheme(raw_url) {
        return canonical_kurowser_url(raw_url);
    }
    raw_url.to_string()
}

fn canonical_kurowser_url(raw: &str) -> String {
    let trimmed = raw.trim();
    let rest = if trimmed.len() >= 11 {
        &trimmed[11..]
    } else {
        "start"
    };
    let clean_slug = rest
        .trim_start_matches('/')
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("start")
        .to_ascii_lowercase();
    let slug = match clean_slug.as_str() {
        "" | "start" => "start",
        "history" => "history",
        "bookmarks" => "bookmarks",
        "downloads" => "downloads",
        "settings" => "settings",
        _ => "start",
    };
    format!("kurowser://{}", slug)
}

fn parse_kurowser_uri(raw: &str) -> Option<(String, String)> {
    if !is_kurowser_scheme(raw) {
        return None;
    }
    let rest = &raw[11..];
    let no_frag = rest.split('#').next().unwrap_or("");
    let no_query = no_frag.split('?').next().unwrap_or("");
    let no_leading = no_query.trim_start_matches('/');
    let (host, path) = match no_leading.split_once('/') {
        Some((h, p)) => (h.to_ascii_lowercase(), p.to_string()),
        None => (no_leading.to_ascii_lowercase(), String::new()),
    };
    Some((host, path))
}

unsafe fn default_container_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn create_container_hwnd(parent: HWND, bounds: PixelRect) -> Result<HWND, String> {
    let class_name = w!("KUROWSER_WEBVIEW");
    let hinstance = unsafe { GetModuleHandleW(PCWSTR::null()).unwrap_or_default() };

    REGISTER_CONTAINER_CLASS.call_once(|| {
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(default_container_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance.into(),
            hIcon: HICON::default(),
            hCursor: HCURSOR::default(),
            hbrBackground: Default::default(),
            lpszMenuName: PCWSTR::null(),
            lpszClassName: class_name,
            hIconSm: HICON::default(),
        };
        unsafe {
            let _ = RegisterClassExW(&wc);
        }
    });

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class_name,
            PCWSTR::null(),
            WS_CHILD | WS_CLIPCHILDREN | WS_VISIBLE,
            bounds.x,
            bounds.y,
            bounds.width.max(1),
            bounds.height.max(1),
            parent,
            HMENU::default(),
            hinstance,
            None,
        )
        .map_err(|e| format!("CreateWindowExW failed: {:?}", e))?
    };

    unsafe {
        apply_container_region(hwnd, bounds.width.max(1), bounds.height.max(1), bounds.corner_diameter);
    }

    Ok(hwnd)
}

unsafe fn apply_container_region(hwnd: HWND, w: i32, h: i32, corner_diameter: i32) {
    if corner_diameter > 2 {
        let rgn = CreateRoundRectRgn(0, 0, w + 1, h + 1, corner_diameter, corner_diameter);
        if !rgn.is_invalid() {
            let _ = SetWindowRgn(hwnd, rgn, BOOL(1));
        }
    } else {
        let _ = SetWindowRgn(hwnd, None, BOOL(1));
    }
}

unsafe extern "system" fn default_container_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    default_container_proc(hwnd, msg, wparam, lparam)
}

pub fn create_webview(
    parent_hwnd: HWND,
    web_context: &mut WebContext,
    browser_data: Arc<Mutex<BrowserData>>,
    bounds: PixelRect,
    initial_url: Option<&str>,
    callbacks: WebViewCallbacks,
) -> Result<KurowserWebView, String> {
    let WebViewCallbacks {
        on_title,
        on_load,
        on_url_changed,
        on_new_window,
        on_shortcut,
        on_favicon,
        on_download_started,
        on_download_completed,
        on_internal_api,
        on_ad_blocked,
    } = callbacks;

    let (bg_color, is_light) = {
        let d = browser_data.lock().unwrap_or_else(|e| e.into_inner());
        let p = d.palette();
        ((p.bg_app.0, p.bg_app.1, p.bg_app.2, 255), p.is_light)
    };

    let container_hwnd = create_container_hwnd(parent_hwnd, bounds)?;
    let env = web_context.env.clone();

    let (tx, rx) = mpsc::channel();
    unsafe {
        let handler = CreateCoreWebView2ControllerCompletedHandler::create(Box::new(
            move |error_code, controller| {
                error_code?;
                tx.send(controller.ok_or_else(|| windows::core::Error::from(E_POINTER)))
                    .map_err(|_| windows::core::Error::from(E_UNEXPECTED))
            },
        ));
        env.CreateCoreWebView2Controller(container_hwnd, &handler)
            .map_err(|e| {
                let _ = DestroyWindow(container_hwnd);
                format!("CreateCoreWebView2Controller failed: {:?}", e)
            })?;
    }

    let controller = webview2_com::wait_with_pump(rx)
        .map_err(|e| format!("Controller pump canceled: {:?}", e))?
        .map_err(|e| format!("Controller error: {:?}", e))?;

    let webview = unsafe {
        controller
            .CoreWebView2()
            .map_err(|e| format!("CoreWebView2 failed: {:?}", e))?
    };

    // Sync WebView2 preferred color scheme + matching app background
    unsafe {
        if let Ok(wv13) = webview.cast::<ICoreWebView2_13>() {
            if let Ok(profile) = wv13.Profile() {
                let scheme = if is_light {
                    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_LIGHT
                } else {
                    COREWEBVIEW2_PREFERRED_COLOR_SCHEME_DARK
                };
                let _ = profile.SetPreferredColorScheme(scheme);
            }
        }
        if let Ok(c2) = controller.cast::<ICoreWebView2Controller2>() {
            let _ = c2.SetDefaultBackgroundColor(COREWEBVIEW2_COLOR {
                R: bg_color.0,
                G: bg_color.1,
                B: bg_color.2,
                A: bg_color.3,
            });
        }
    }

    let target_url = to_webview_url(initial_url.unwrap_or(START_URL));
    let initial_is_internal = is_kurowser_scheme(&target_url);

    // Hardened WebView2 Settings (disable host COM objects, web messaging, status bar)
    unsafe {
        if let Ok(settings) = webview.Settings() {
            let _ = settings.SetIsStatusBarEnabled(false);
            let _ = settings.SetAreDefaultContextMenusEnabled(true);
            let _ = settings.SetIsZoomControlEnabled(true);
            let _ = settings.SetAreHostObjectsAllowed(false);
            let _ = settings.SetIsWebMessageEnabled(false);
            let _ = settings.SetAreDevToolsEnabled(!initial_is_internal || cfg!(debug_assertions));
            if let Ok(s3) = settings.cast::<ICoreWebView2Settings3>() {
                let _ = s3.SetAreBrowserAcceleratorKeysEnabled(true);
            }
            if let Ok(s4) = settings.cast::<ICoreWebView2Settings4>() {
                let _ = s4.SetIsGeneralAutofillEnabled(true);
                let _ = s4.SetIsPasswordAutosaveEnabled(false);
            }
            if let Ok(s5) = settings.cast::<ICoreWebView2Settings5>() {
                let _ = s5.SetIsPinchZoomEnabled(true);
            }
            if let Ok(s6) = settings.cast::<ICoreWebView2Settings6>() {
                let _ = s6.SetIsSwipeNavigationEnabled(false);
            }
        }
    }

    let expected_internal_nav = Rc::new(RefCell::new(if initial_is_internal {
        Some(to_display_url(&target_url))
    } else {
        None
    }));

    let mut token = EventRegistrationToken::default();

    // 1. WebResourceRequested: native kurowser:// server + Brave-style Shields Ad & Tracker blocker
    unsafe {
        let _ = webview.AddWebResourceRequestedFilter(
            w!("*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        );
    }
    let env_for_req = env.clone();
    let req_data = Arc::clone(&browser_data);
    let req_api_cb = Rc::new(on_internal_api);
    let req_ad_cb = Rc::new(on_ad_blocked);
    unsafe {
        let _ = webview.add_WebResourceRequested(
            &WebResourceRequestedEventHandler::create(Box::new(move |sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let webview_req = args.Request()?;
                let mut uri_pw = PWSTR::null();
                webview_req.Uri(&mut uri_pw)?;
                let uri = take_pwstr(uri_pw);

                if is_kurowser_scheme(&uri) {
                    let page_source = sender
                        .as_ref()
                        .and_then(|wv| {
                            let mut s = PWSTR::null();
                            wv.Source(&mut s).ok()?;
                            Some(take_pwstr(s))
                        })
                        .unwrap_or_default();

                    let mut res_ctx = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                    let _ = args.ResourceContext(&mut res_ctx);

                    let response = handle_kurowser_request(
                        &env_for_req,
                        &req_data,
                        &req_api_cb,
                        &webview_req,
                        &uri,
                        &page_source,
                        res_ctx,
                    );
                    if let Ok(resp) = response {
                        let _ = args.SetResponse(&resp);
                    }
                    return Ok(());
                }

                // Shields Ad & Tracker interception for HTTP/HTTPS
                if uri.starts_with("https://") || uri.starts_with("http://") {
                    let policy = {
                        req_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .shield_policy()
                    };
                    if policy.global_enabled {
                        let page_url = sender
                            .as_ref()
                            .and_then(|wv| {
                                let mut s = PWSTR::null();
                                wv.Source(&mut s).ok()?;
                                Some(take_pwstr(s))
                            })
                            .unwrap_or_default();

                        let mut res_ctx = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
                        let _ = args.ResourceContext(&mut res_ctx);
                        let is_top_doc = res_ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT;

                        if adblock::should_block_request(&uri, &page_url, is_top_doc, &policy) {
                            let (ctype, body): (&str, &[u8]) =
                                if res_ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_SCRIPT {
                                    ("application/javascript; charset=utf-8", b"/* blocked */")
                                } else if res_ctx
                                    == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST
                                {
                                    ("application/json; charset=utf-8", b"{}")
                                } else if res_ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_STYLESHEET {
                                    ("text/css; charset=utf-8", b"")
                                } else if res_ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE {
                                    ("image/png", EMPTY_PNG_1X1)
                                } else if res_ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT {
                                    ("text/html; charset=utf-8", b"")
                                } else {
                                    ("text/plain; charset=utf-8", b"")
                                };
                            let headers = format!(
                                "Content-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nCache-Control: no-store\r\n",
                                ctype
                            );
                            if let Ok(resp) =
                                make_response(&env_for_req, 200, "OK", &headers, body)
                            {
                                let _ = args.SetResponse(&resp);
                            }
                            req_ad_cb(normalize_host(&page_url));
                        }
                    }
                }

                Ok(())
            })),
            &mut token,
        );
    }

    // 2. NavigationStarting guard (blocks dangerous schemes & unauthorized external->kurowser://)
    let expected_for_nav = Rc::clone(&expected_internal_nav);
    unsafe {
        let _ = webview.add_NavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut uri_pw = PWSTR::null();
                args.Uri(&mut uri_pw)?;
                let uri = take_pwstr(uri_pw);

                if is_dangerous_scheme(&uri) {
                    let _ = args.SetCancel(true);
                    return Ok(());
                }

                let target_is_internal = is_kurowser_scheme(&uri);
                if target_is_internal {
                    let mut is_redirected = BOOL(0);
                    let _ = args.IsRedirected(&mut is_redirected);
                    if is_redirected.as_bool() {
                        let _ = args.SetCancel(true);
                        return Ok(());
                    }

                    let src_is_internal = sender
                        .as_ref()
                        .and_then(|wv| {
                            let mut s = PWSTR::null();
                            wv.Source(&mut s).ok()?;
                            Some(is_kurowser_scheme(&take_pwstr(s)))
                        })
                        .unwrap_or(false);

                    let is_back_forward_or_reload = args
                        .cast::<ICoreWebView2NavigationStartingEventArgs3>()
                        .ok()
                        .and_then(|a3| {
                            let mut k = COREWEBVIEW2_NAVIGATION_KIND::default();
                            a3.NavigationKind(&mut k).ok()?;
                            Some(
                                k == COREWEBVIEW2_NAVIGATION_KIND_BACK_OR_FORWARD
                                    || k == COREWEBVIEW2_NAVIGATION_KIND_RELOAD,
                            )
                        })
                        .unwrap_or(false);

                    let display = to_display_url(&uri);
                    let matches_expected = {
                        let mut slot = expected_for_nav.borrow_mut();
                        if slot.as_deref() == Some(display.as_str()) {
                            *slot = None;
                            true
                        } else {
                            false
                        }
                    };

                    if !src_is_internal && !matches_expected && !is_back_forward_or_reload {
                        let _ = args.SetCancel(true);
                        return Ok(());
                    }
                } else {
                    *expected_for_nav.borrow_mut() = None;
                }

                // Dynamically lock DevTools on internal pages in release builds
                if let Some(wv) = sender.as_ref() {
                    if let Ok(settings) = wv.Settings() {
                        let _ = settings
                            .SetAreDevToolsEnabled(!target_is_internal || cfg!(debug_assertions));
                    }
                }

                Ok(())
            })),
            &mut token,
        );
    }

    // 3. FrameNavigationStarting guard (external iframes can never embed kurowser:// or dangerous URIs)
    unsafe {
        let _ = webview.add_FrameNavigationStarting(
            &NavigationStartingEventHandler::create(Box::new(move |_, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut uri_pw = PWSTR::null();
                args.Uri(&mut uri_pw)?;
                let uri = take_pwstr(uri_pw);
                if is_kurowser_scheme(&uri) || is_dangerous_scheme(&uri) {
                    let _ = args.SetCancel(true);
                }
                Ok(())
            })),
            &mut token,
        );
    }

    // 4. NewWindowRequested: open safe targets inside a Kurowser tab, block dangerous & spoofing
    unsafe {
        let _ = webview.add_NewWindowRequested(
            &NewWindowRequestedEventHandler::create(Box::new(move |sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let mut uri_pw = PWSTR::null();
                args.Uri(&mut uri_pw)?;
                let uri = take_pwstr(uri_pw);
                let _ = args.SetHandled(true);

                if uri.is_empty() || is_dangerous_scheme(&uri) {
                    return Ok(());
                }

                if is_kurowser_scheme(&uri) {
                    let src_is_internal = sender
                        .as_ref()
                        .and_then(|wv| {
                            let mut s = PWSTR::null();
                            wv.Source(&mut s).ok()?;
                            Some(is_kurowser_scheme(&take_pwstr(s)))
                        })
                        .unwrap_or(false);
                    if !src_is_internal {
                        return Ok(());
                    }
                }

                on_new_window(to_display_url(&uri));
                Ok(())
            })),
            &mut token,
        );
    }

    // 5. Hardened PermissionRequested (never auto-allow clipboard/sensors/local-fonts)
    unsafe {
        let _ = webview.add_PermissionRequested(
            &PermissionRequestedEventHandler::create(Box::new(move |sender, args| {
                let Some(args) = args else {
                    return Ok(());
                };
                let src_is_internal = sender
                    .as_ref()
                    .and_then(|wv| {
                        let mut s = PWSTR::null();
                        wv.Source(&mut s).ok()?;
                        Some(is_kurowser_scheme(&take_pwstr(s)))
                    })
                    .unwrap_or(false);
                if src_is_internal {
                    let _ = args.SetState(COREWEBVIEW2_PERMISSION_STATE_DENY);
                    return Ok(());
                }

                let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                args.PermissionKind(&mut kind)?;
                let mut user_init = BOOL(0);
                let _ = args.IsUserInitiated(&mut user_init);

                let state = match kind {
                    k if k == COREWEBVIEW2_PERMISSION_KIND_CLIPBOARD_READ
                        || k == COREWEBVIEW2_PERMISSION_KIND_CAMERA
                        || k == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE
                        || k == COREWEBVIEW2_PERMISSION_KIND_GEOLOCATION =>
                    {
                        if user_init.as_bool() {
                            COREWEBVIEW2_PERMISSION_STATE_DEFAULT
                        } else {
                            COREWEBVIEW2_PERMISSION_STATE_DENY
                        }
                    }
                    k if k == COREWEBVIEW2_PERMISSION_KIND_NOTIFICATIONS
                        || k == COREWEBVIEW2_PERMISSION_KIND_OTHER_SENSORS
                        || k == COREWEBVIEW2_PERMISSION_KIND_LOCAL_FONTS
                        || k == COREWEBVIEW2_PERMISSION_KIND_MIDI_SYSTEM_EXCLUSIVE_MESSAGES
                        || k == COREWEBVIEW2_PERMISSION_KIND_FILE_READ_WRITE
                        || k == COREWEBVIEW2_PERMISSION_KIND_MULTIPLE_AUTOMATIC_DOWNLOADS =>
                    {
                        COREWEBVIEW2_PERMISSION_STATE_DENY
                    }
                    _ => COREWEBVIEW2_PERMISSION_STATE_DEFAULT,
                };
                let _ = args.SetState(state);
                Ok(())
            })),
            &mut token,
        );
    }

    // 6. DocumentTitleChanged
    unsafe {
        let _ = webview.add_DocumentTitleChanged(
            &DocumentTitleChangedEventHandler::create(Box::new(move |wv, _| {
                if let Some(wv) = wv {
                    let mut pw = PWSTR::null();
                    wv.DocumentTitle(&mut pw)?;
                    on_title(take_pwstr(pw));
                }
                Ok(())
            })),
            &mut token,
        );
    }

    // 7. ContentLoading & NavigationCompleted
    let on_load_rc = Rc::new(on_load);
    let on_load_start = Rc::clone(&on_load_rc);
    unsafe {
        let _ = webview.add_ContentLoading(
            &ContentLoadingEventHandler::create(Box::new(move |wv, _| {
                if let Some(wv) = wv {
                    let mut pw = PWSTR::null();
                    wv.Source(&mut pw)?;
                    let url = take_pwstr(pw);
                    on_load_start(false, to_display_url(&url));
                }
                Ok(())
            })),
            &mut token,
        );
        let _ = webview.add_NavigationCompleted(
            &NavigationCompletedEventHandler::create(Box::new(move |wv, _| {
                if let Some(wv) = wv {
                    let mut pw = PWSTR::null();
                    wv.Source(&mut pw)?;
                    let url = take_pwstr(pw);
                    on_load_rc(true, to_display_url(&url));
                }
                Ok(())
            })),
            &mut token,
        );
    }

    // 8. Early document-start AdBlock & YouTube ad pruner registration (before first navigation)
    let shields_script_id = Rc::new(RefCell::new(None));
    let initial_policy = {
        browser_data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .shield_policy()
    };
    install_shields_script_blocking(&webview, &shields_script_id, &initial_policy);

    // 9. SourceChanged (live address bar sync including pushState SPAs)
    unsafe {
        let _ = webview.add_SourceChanged(
            &SourceChangedEventHandler::create(Box::new(move |wv, _| {
                if let Some(wv) = wv {
                    let mut pw = PWSTR::null();
                    wv.Source(&mut pw)?;
                    let raw = take_pwstr(pw);
                    if !raw.is_empty()
                        && !raw.starts_with("about:blank")
                        && !raw.starts_with("data:")
                    {
                        on_url_changed(to_display_url(&raw));
                    }
                }
                Ok(())
            })),
            &mut token,
        );
    }

    // 10. Download handler
    if let Ok(wv4) = webview.cast::<ICoreWebView2_4>() {
        let on_dl_done = Rc::new(on_download_completed);
        let on_dl_start = Rc::new(on_download_started);
        unsafe {
            let _ = wv4.add_DownloadStarting(
                &DownloadStartingEventHandler::create(Box::new(move |_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let op = args.DownloadOperation()?;
                    let mut uri_pw = PWSTR::null();
                    op.Uri(&mut uri_pw)?;
                    let uri = take_pwstr(uri_pw);

                    if uri.is_empty() || is_dangerous_scheme(&uri) || is_kurowser_scheme(&uri) {
                        let _ = args.SetCancel(true);
                        return Ok(());
                    }

                    let mut path_pw = PWSTR::null();
                    args.ResultFilePath(&mut path_pw)?;
                    let path = PathBuf::from(take_pwstr(path_pw));
                    on_dl_start(uri, path);

                    let done_cb = Rc::clone(&on_dl_done);
                    let _ = op.add_StateChanged(
                        &StateChangedEventHandler::create(Box::new(move |dl_op, _| {
                            let Some(dl_op) = dl_op else {
                                return Ok(());
                            };
                            let mut state = COREWEBVIEW2_DOWNLOAD_STATE::default();
                            dl_op.State(&mut state)?;
                            if state != COREWEBVIEW2_DOWNLOAD_STATE_IN_PROGRESS {
                                let mut u = PWSTR::null();
                                dl_op.Uri(&mut u)?;
                                let dl_uri = take_pwstr(u);
                                let ok = state == COREWEBVIEW2_DOWNLOAD_STATE_COMPLETED;
                                let dl_path = if ok {
                                    let mut p = PWSTR::null();
                                    dl_op.ResultFilePath(&mut p)?;
                                    Some(PathBuf::from(take_pwstr(p)))
                                } else {
                                    None
                                };
                                done_cb(dl_uri, dl_path, ok);
                            }
                            Ok(())
                        })),
                        &mut EventRegistrationToken::default(),
                    );

                    Ok(())
                })),
                &mut token,
            );
        }
    }

    // 11. Accelerators & Favicons
    attach_accelerator_handler(&controller, on_shortcut);
    attach_favicon_handler(&webview, on_favicon);

    let kwv = KurowserWebView {
        hwnd: container_hwnd,
        controller,
        webview,
        expected_internal_nav,
        shields_script_id,
    };

    kwv.set_bounds(bounds);
    kwv.set_visible(true);
    kwv.set_memory_usage_level(MemoryLevel::Normal);
    kwv.load_url(&target_url);

    Ok(kwv)
}

fn handle_kurowser_request(
    env: &ICoreWebView2Environment,
    browser_data: &Arc<Mutex<BrowserData>>,
    on_api: &Rc<Box<dyn Fn(InternalApiEvent) + 'static>>,
    req: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceRequest,
    uri: &str,
    page_source: &str,
    res_ctx: COREWEBVIEW2_WEB_RESOURCE_CONTEXT,
) -> windows::core::Result<
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceResponse,
> {
    let Some((slug, sub_path)) = parse_kurowser_uri(uri) else {
        return make_response(env, 404, "Not Found", "Content-Type: text/plain\r\n", b"");
    };

    let mut method_pw = PWSTR::null();
    unsafe { req.Method(&mut method_pw)? };
    let method = take_pwstr(method_pw).to_ascii_uppercase();
    let source_is_internal = is_kurowser_scheme(page_source);

    // Static embedded fonts and brand logo
    if method == "GET" {
        match sub_path.as_str() {
            "fonts/Poppins-Regular.ttf" => {
                return make_response(
                    env,
                    200,
                    "OK",
                    "Content-Type: font/ttf\r\nCache-Control: max-age=86400\r\n",
                    FONT_REGULAR,
                );
            }
            "fonts/Poppins-Medium.ttf" => {
                return make_response(
                    env,
                    200,
                    "OK",
                    "Content-Type: font/ttf\r\nCache-Control: max-age=86400\r\n",
                    FONT_MEDIUM,
                );
            }
            "fonts/Poppins-SemiBold.ttf" => {
                return make_response(
                    env,
                    200,
                    "OK",
                    "Content-Type: font/ttf\r\nCache-Control: max-age=86400\r\n",
                    FONT_SEMIBOLD,
                );
            }
            "logo.png" => {
                return make_response(
                    env,
                    200,
                    "OK",
                    "Content-Type: image/png\r\nCache-Control: max-age=86400\r\n",
                    LOGO_PNG,
                );
            }
            _ => {}
        }

        if let Some(raw_domain) = sub_path.strip_prefix("favicon/") {
            if !source_is_internal {
                return make_response(env, 403, "Forbidden", "Content-Type: text/plain\r\n", b"");
            }
            if let Some(png_bytes) = browser_data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .load_favicon_png(raw_domain)
            {
                return make_response(
                    env,
                    200,
                    "OK",
                    "Content-Type: image/png\r\nCache-Control: max-age=3600\r\n",
                    &png_bytes,
                );
            }
            return make_response(env, 404, "Not Found", "Content-Type: text/plain\r\n", b"");
        }
    }

    // Mutating internal JSON APIs
    if let Some(api_path) = sub_path.strip_prefix("api/") {
        if method != "POST"
            || !source_is_internal
            || res_ctx == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT
        {
            return make_response(
                env,
                403,
                "Forbidden",
                "Content-Type: text/plain\r\nX-Content-Type-Options: nosniff\r\n",
                b"Forbidden",
            );
        }

        let headers_map = unsafe { read_request_headers(req)? };
        let origin_ok = headers_map
            .get("origin")
            .map(|o| is_kurowser_scheme(o) || o == "null")
            .unwrap_or(true);
        let referer_ok = headers_map
            .get("referer")
            .map(|r| is_kurowser_scheme(r))
            .unwrap_or(true);
        let token_ok = {
            let d = browser_data.lock().unwrap_or_else(|e| e.into_inner());
            headers_map
                .get("x-kurowser-token")
                .map(|tok| constant_time_eq(tok, &d.api_token))
                .unwrap_or(false)
        };

        if !token_ok || !origin_ok || !referer_ok {
            return make_response(
                env,
                403,
                "Forbidden",
                "Content-Type: text/plain\r\nX-Content-Type-Options: nosniff\r\n",
                b"Forbidden",
            );
        }

        let body_bytes = unsafe { read_request_body(req) };
        let body_str = String::from_utf8_lossy(&body_bytes);
        let mut response_json = b"{\"ok\":true}".to_vec();
        let mut api_event: Option<InternalApiEvent> = None;

        match api_path {
            "settings" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    let mut d = browser_data.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(p) = v.get("theme_preset").and_then(|x| x.as_str()) {
                        d.settings.theme_preset = ThemePreset::from_id(p);
                    }
                    if let Some(acc) = v.get("custom_accent_hex") {
                        d.settings.custom_accent_hex = acc.as_str().map(|s| s.to_string());
                    }
                    if let Some(se) = v.get("search_engine").and_then(|x| x.as_str()) {
                        d.settings.search_engine = if se == "duckduckgo" {
                            SearchEngine::DuckDuckGo
                        } else {
                            SearchEngine::Google
                        };
                    }
                    if let Some(m) = v.get("max_live_tabs").and_then(|x| x.as_u64()) {
                        d.settings.max_live_tabs = (m as usize).clamp(2, 4);
                    }
                    if let Some(ad_on) = v.get("adblock_enabled").and_then(|x| x.as_bool()) {
                        d.settings.adblock_enabled = ad_on;
                    }
                    if let Some(cosmetic) = v.get("adblock_cosmetic").and_then(|x| x.as_bool()) {
                        d.settings.adblock_cosmetic = cosmetic;
                    }
                    d.save_settings();
                    api_event = Some(InternalApiEvent::SettingsUpdated);
                }
            }
            "shields/allowlist/add" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    if let Some(host) = v.get("host").and_then(|x| x.as_str()) {
                        let mut d = browser_data.lock().unwrap_or_else(|e| e.into_inner());
                        d.add_allowlist_host(host);
                        let payload = serde_json::json!({
                            "ok": true,
                            "allowlist": d.settings.adblock_allowlist
                        });
                        if let Ok(bytes) = serde_json::to_vec(&payload) {
                            response_json = bytes;
                        }
                        api_event = Some(InternalApiEvent::ShieldsUpdated);
                    }
                }
            }
            "shields/allowlist/remove" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    if let Some(host) = v.get("host").and_then(|x| x.as_str()) {
                        let mut d = browser_data.lock().unwrap_or_else(|e| e.into_inner());
                        d.remove_allowlist_host(host);
                        let payload = serde_json::json!({
                            "ok": true,
                            "allowlist": d.settings.adblock_allowlist
                        });
                        if let Ok(bytes) = serde_json::to_vec(&payload) {
                            response_json = bytes;
                        }
                        api_event = Some(InternalApiEvent::ShieldsUpdated);
                    }
                }
            }
            "quick-links/save" => {
                #[derive(serde::Deserialize)]
                struct QuickLinksReq {
                    items: Vec<QuickLinkItem>,
                }
                if let Ok(req_body) = serde_json::from_str::<QuickLinksReq>(&body_str) {
                    let saved = browser_data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .set_quick_links(req_body.items);
                    let payload = serde_json::json!({ "ok": true, "items": saved });
                    if let Ok(bytes) = serde_json::to_vec(&payload) {
                        response_json = bytes;
                    }
                    api_event = Some(InternalApiEvent::DataUpdated);
                }
            }
            "quick-links/reset" => {
                let saved = browser_data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .reset_quick_links();
                let payload = serde_json::json!({ "ok": true, "items": saved });
                if let Ok(bytes) = serde_json::to_vec(&payload) {
                    response_json = bytes;
                }
                api_event = Some(InternalApiEvent::DataUpdated);
            }
            "history/delete" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    if let Some(u) = v.get("url").and_then(|x| x.as_str()) {
                        browser_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .delete_history(u);
                    }
                }
                api_event = Some(InternalApiEvent::DataUpdated);
            }
            "history/clear" => {
                browser_data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clear_history();
                api_event = Some(InternalApiEvent::DataUpdated);
            }
            "bookmarks/delete" => {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&body_str) {
                    if let Some(u) = v.get("url").and_then(|x| x.as_str()) {
                        browser_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .delete_bookmark(u);
                    }
                }
                api_event = Some(InternalApiEvent::DataUpdated);
            }
            "downloads/clear" => {
                browser_data
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clear_downloads();
                api_event = Some(InternalApiEvent::DataUpdated);
            }
            "downloads/open-folder" => {
                if let Ok(profile) = std::env::var("USERPROFILE") {
                    let dl_dir = PathBuf::from(profile).join("Downloads");
                    let _ = std::process::Command::new("explorer.exe").arg(dl_dir).spawn();
                }
            }
            _ => {
                return make_response(
                    env,
                    404,
                    "Not Found",
                    "Content-Type: application/json\r\n",
                    b"{\"ok\":false}",
                );
            }
        }

        if let Some(evt) = api_event {
            on_api(evt);
        }

        return make_response(
            env,
            200,
            "OK",
            "Content-Type: application/json\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n",
            &response_json,
        );
    }

    // Block framing / non-GET requests for internal HTML pages
    if method != "GET" {
        return make_response(env, 405, "Method Not Allowed", "Content-Type: text/plain\r\n", b"");
    }

    let csp_nonce = generate_csprng_hex(16);
    let html = {
        let data = browser_data.lock().unwrap_or_else(|e| e.into_inner());
        match slug.as_str() {
            "history" => pages::render_history_page(&data, &csp_nonce),
            "bookmarks" => pages::render_bookmarks_page(&data, &csp_nonce),
            "downloads" => pages::render_downloads_page(&data, &csp_nonce),
            "settings" => pages::render_settings_page(&data, &csp_nonce),
            _ => pages::render_start_page(&data, &csp_nonce),
        }
    };

    let headers = format!(
        "Content-Type: text/html; charset=utf-8\r\n\
         Content-Security-Policy: default-src 'none'; script-src 'nonce-{nonce}'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\
         X-Frame-Options: DENY\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Referrer-Policy: no-referrer\r\n\
         Cache-Control: no-store\r\n",
        nonce = csp_nonce
    );

    make_response(env, 200, "OK", &headers, html.as_bytes())
}

fn make_response(
    env: &ICoreWebView2Environment,
    status: i32,
    reason: &str,
    headers: &str,
    body: &[u8],
) -> windows::core::Result<
    webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceResponse,
> {
    let stream = if body.is_empty() {
        None
    } else {
        unsafe { SHCreateMemStream(Some(body)) }
    };
    let reason_h = HSTRING::from(reason);
    let headers_h = HSTRING::from(headers);
    unsafe { env.CreateWebResourceResponse(stream.as_ref(), status, &reason_h, &headers_h) }
}

unsafe fn read_request_headers(
    req: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceRequest,
) -> windows::core::Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    let iter = req.Headers()?.GetIterator()?;
    let mut has_cur = BOOL(0);
    iter.HasCurrentHeader(&mut has_cur)?;
    while has_cur.as_bool() {
        let mut k = PWSTR::null();
        let mut v = PWSTR::null();
        iter.GetCurrentHeader(&mut k, &mut v)?;
        map.insert(take_pwstr(k).to_ascii_lowercase(), take_pwstr(v));
        iter.MoveNext(&mut has_cur)?;
    }
    Ok(map)
}

unsafe fn read_request_body(
    req: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceRequest,
) -> Vec<u8> {
    let mut body = Vec::new();
    let Ok(content) = req.Content() else {
        return body;
    };
    let Ok(stream) = content.cast::<IStream>() else {
        return body;
    };
    let _ = stream.Seek(0, STREAM_SEEK_SET, None);
    let mut buf = [0u8; 4096];
    loop {
        let mut read = 0u32;
        if stream
            .Read(buf.as_mut_ptr() as *mut _, buf.len() as u32, Some(&mut read))
            .is_err()
            || read == 0
        {
            break;
        }
        body.extend_from_slice(&buf[..read as usize]);
        if body.len() >= 1024 * 1024 {
            break;
        }
    }
    body
}

fn attach_favicon_handler(
    webview: &ICoreWebView2,
    on_favicon: Box<dyn Fn(String, Vec<u8>) + 'static>,
) {
    let Ok(core15) = webview.cast::<ICoreWebView2_15>() else {
        return;
    };

    let cb = Rc::new(on_favicon);
    let core15_for_event = core15.clone();

    let handler = FaviconChangedEventHandler::create(Box::new(move |sender, _| {
        let domain = sender
            .and_then(|s| {
                let mut src = PWSTR::null();
                unsafe { s.Source(&mut src).ok()? };
                Some(domain_from_url(&to_display_url(&take_pwstr(src))))
            })
            .unwrap_or_default();

        if domain.is_empty() || domain.starts_with("kurowser.") {
            return Ok(());
        }

        let cb_done = Rc::clone(&cb);
        let done_handler = GetFaviconCompletedHandler::create(Box::new(move |hr, stream| {
            if hr.is_ok() {
                if let Some(stream) = stream {
                    let bytes = unsafe { read_stream_bytes(&stream) };
                    if !bytes.is_empty() {
                        cb_done(domain.clone(), bytes);
                    }
                }
            }
            Ok(())
        }));

        unsafe {
            let _ = core15_for_event.GetFavicon(COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG, &done_handler);
        }
        Ok(())
    }));

    let mut token = EventRegistrationToken::default();
    unsafe {
        let _ = core15.add_FaviconChanged(&handler, &mut token);
    }
}

unsafe fn read_stream_bytes(stream: &IStream) -> Vec<u8> {
    let _ = stream.Seek(0, STREAM_SEEK_SET, None);
    let mut out = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let mut read = 0u32;
        let hr = stream.Read(buf.as_mut_ptr() as *mut _, buf.len() as u32, Some(&mut read));
        if hr.is_err() || read == 0 {
            break;
        }
        out.extend_from_slice(&buf[..read as usize]);
        if out.len() >= 512 * 1024 {
            break;
        }
    }
    out
}

fn attach_accelerator_handler(
    controller: &ICoreWebView2Controller,
    on_shortcut: Box<dyn Fn(AcceleratorAction) + 'static>,
) {
    let handler = AcceleratorKeyPressedEventHandler::create(Box::new(move |_, args| {
        let Some(args) = args else {
            return Ok(());
        };

        let mut kind = COREWEBVIEW2_KEY_EVENT_KIND::default();
        unsafe { args.KeyEventKind(&mut kind)? };
        if kind != COREWEBVIEW2_KEY_EVENT_KIND_KEY_DOWN
            && kind != COREWEBVIEW2_KEY_EVENT_KIND_SYSTEM_KEY_DOWN
        {
            return Ok(());
        }

        let mut vkey = 0u32;
        unsafe { args.VirtualKey(&mut vkey)? };

        let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 };
        let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
        let alt = unsafe { GetKeyState(VK_MENU.0 as i32) < 0 };

        let mut handled = true;
        let action = if ctrl && !alt {
            match vkey as u16 {
                k if k == VK_T.0 => Some(AcceleratorAction::NewTab),
                k if k == VK_W.0 => Some(AcceleratorAction::CloseTab),
                k if k == VK_L.0 => Some(AcceleratorAction::FocusAddressBar),
                k if k == VK_B.0 => Some(AcceleratorAction::ToggleSidebar),
                k if k == VK_R.0 => Some(AcceleratorAction::Reload),
                k if k == VK_D.0 => Some(AcceleratorAction::ToggleBookmark),
                k if k == VK_H.0 => Some(AcceleratorAction::OpenHistory),
                k if k == VK_J.0 => Some(AcceleratorAction::OpenDownloads),
                k if k == VK_O.0 && shift => Some(AcceleratorAction::OpenBookmarks),
                k if k == VK_OEM_COMMA.0 => Some(AcceleratorAction::OpenSettings),
                k if k == VK_TAB.0 && shift => Some(AcceleratorAction::PrevTab),
                k if k == VK_TAB.0 => Some(AcceleratorAction::NextTab),
                k if (VK_1.0..=VK_9.0).contains(&k) => {
                    Some(AcceleratorAction::SelectTabIndex((k - VK_1.0) as usize))
                }
                _ => {
                    handled = false;
                    None
                }
            }
        } else if alt && !ctrl {
            match vkey as u16 {
                k if k == VK_LEFT.0 => Some(AcceleratorAction::GoBack),
                k if k == VK_RIGHT.0 => Some(AcceleratorAction::GoForward),
                _ => {
                    handled = false;
                    None
                }
            }
        } else if !ctrl && !alt {
            match vkey as u16 {
                k if k == VK_F5.0 => Some(AcceleratorAction::Reload),
                k if k == VK_ESCAPE.0 => {
                    handled = false;
                    Some(AcceleratorAction::StopLoading)
                }
                _ => {
                    handled = false;
                    None
                }
            }
        } else {
            handled = false;
            None
        };

        if let Some(act) = action {
            on_shortcut(act);
        }
        if handled {
            unsafe { args.SetHandled(true)? };
        }

        Ok(())
    }));

    let mut token = EventRegistrationToken::default();
    unsafe {
        let _ = controller.add_AcceleratorKeyPressed(&handler, &mut token);
    }
}

use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_FAVICON_IMAGE_FORMAT_PNG;

pub fn navigate(
    webview: &KurowserWebView,
    input: &str,
    search_engine: SearchEngine,
) -> String {
    let display_url = normalize_input(input, search_engine);
    webview.load_url(&display_url);
    display_url
}

pub fn go_back(webview: &KurowserWebView) {
    let _ = unsafe { webview.webview.GoBack() };
}

pub fn go_forward(webview: &KurowserWebView) {
    let _ = unsafe { webview.webview.GoForward() };
}

pub fn reload(webview: &KurowserWebView) {
    let _ = unsafe { webview.webview.Reload() };
}

pub fn stop(webview: &KurowserWebView) {
    let _ = unsafe { webview.webview.Stop() };
}

pub fn can_go_back(webview: &KurowserWebView) -> bool {
    let mut can = BOOL(0);
    if unsafe { webview.webview.CanGoBack(&mut can) }.is_ok() {
        return can.as_bool();
    }
    false
}

pub fn can_go_forward(webview: &KurowserWebView) -> bool {
    let mut can = BOOL(0);
    if unsafe { webview.webview.CanGoForward(&mut can) }.is_ok() {
        return can.as_bool();
    }
    false
}

pub fn is_playing_audio(webview: &KurowserWebView) -> bool {
    let mut playing = BOOL(0);
    if let Ok(core8) = webview.webview.cast::<ICoreWebView2_8>() {
        if unsafe { core8.IsDocumentPlayingAudio(&mut playing) }.is_ok() {
            return playing.as_bool();
        }
    }
    false
}

pub fn suspend_background(webview: &KurowserWebView) -> (bool, bool) {
    let audio = is_playing_audio(webview);
    webview.set_visible(false);
    webview.set_memory_usage_level(MemoryLevel::Low);

    if !audio {
        if let Ok(core3) = webview.webview.cast::<ICoreWebView2_3>() {
            let handler = TrySuspendCompletedHandler::create(Box::new(|_, _| Ok(())));
            if unsafe { core3.TrySuspend(&handler) }.is_ok() {
                return (false, true);
            }
        }
    }
    (audio, false)
}

pub fn resume_foreground(webview: &KurowserWebView, bounds: PixelRect) {
    if let Ok(core3) = webview.webview.cast::<ICoreWebView2_3>() {
        let _ = unsafe { core3.Resume() };
    }
    webview.set_memory_usage_level(MemoryLevel::Normal);
    webview.set_bounds(bounds);
    webview.set_visible(true);
    webview.focus();
}

pub fn set_bounds(webview: &KurowserWebView, bounds: PixelRect) {
    webview.set_bounds(bounds);
}

pub fn apply_theme(webview: &KurowserWebView, bg: (u8, u8, u8), is_light: bool) {
    webview.set_background_color((bg.0, bg.1, bg.2, 255));
    webview.set_preferred_color_scheme(is_light);
}

fn install_shields_script_blocking(
    webview: &ICoreWebView2,
    script_id_slot: &Rc<RefCell<Option<String>>>,
    policy: &adblock::ShieldPolicy,
) {
    if let Some(old_id) = script_id_slot.borrow_mut().take() {
        unsafe {
            let _ = webview.RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(old_id));
        }
    }
    let Some(script) = adblock::build_document_script(policy, adblock::process_salt()) else {
        return;
    };
    let (tx, rx) = mpsc::channel();
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |error_code, id_str| {
            let id_opt = if error_code.is_ok() {
                Some(id_str)
            } else {
                None
            };
            let _ = tx.send(id_opt);
            Ok(())
        },
    ));
    let js = HSTRING::from(script);
    if unsafe { webview.AddScriptToExecuteOnDocumentCreated(&js, &handler) }.is_ok() {
        if let Ok(Some(id)) = webview2_com::wait_with_pump(rx) {
            if !id.is_empty() {
                *script_id_slot.borrow_mut() = Some(id);
            }
        }
    }
}

pub fn sync_shields_script(
    kwv: &KurowserWebView,
    policy: &adblock::ShieldPolicy,
    reload_after: bool,
) {
    if let Some(old_id) = kwv.shields_script_id.borrow_mut().take() {
        unsafe {
            let _ = kwv
                .webview
                .RemoveScriptToExecuteOnDocumentCreated(&HSTRING::from(old_id));
        }
    }
    let Some(script) = adblock::build_document_script(policy, adblock::process_salt()) else {
        if reload_after {
            reload(kwv);
        }
        return;
    };

    let slot = Rc::clone(&kwv.shields_script_id);
    let wv_clone = kwv.webview.clone();
    let handler = AddScriptToExecuteOnDocumentCreatedCompletedHandler::create(Box::new(
        move |error_code, id_str| {
            if error_code.is_ok() && !id_str.is_empty() {
                *slot.borrow_mut() = Some(id_str);
            }
            if reload_after {
                let _ = unsafe { wv_clone.Reload() };
            }
            Ok(())
        },
    ));
    let js = HSTRING::from(script);
    if unsafe { kwv.webview.AddScriptToExecuteOnDocumentCreated(&js, &handler) }.is_err()
        && reload_after
    {
        reload(kwv);
    }
}

pub fn normalize_input(input: &str, search_engine: SearchEngine) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return START_URL.to_string();
    }
    if is_dangerous_scheme(trimmed) {
        return START_URL.to_string();
    }
    if is_kurowser_scheme(trimmed) {
        return canonical_kurowser_url(trimmed);
    }
    if trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("file://")
    {
        return trimmed.to_string();
    }
    if trimmed.eq_ignore_ascii_case("localhost") || trimmed.starts_with("localhost:") {
        return format!("http://{}", trimmed);
    }
    if trimmed.contains('.') && !trimmed.contains(' ') {
        return format!("https://{}", trimmed);
    }
    search_engine.search_url(&encode_query_component(trimmed))
}

fn encode_query_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            other => {
                out.push('%');
                out.push(hex_digit(other >> 4));
                out.push(hex_digit(other & 0x0F));
            }
        }
    }
    out
}

fn hex_digit(v: u8) -> char {
    match v {
        0..=9 => (b'0' + v) as char,
        _ => (b'A' + (v - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_kurowser_scheme_without_localhost() {
        assert_eq!(to_display_url("kurowser://start/"), "kurowser://start");
        assert_eq!(to_display_url("kurowser://SETTINGS/"), "kurowser://settings");
        assert_eq!(
            parse_kurowser_uri("kurowser://settings/api/settings"),
            Some(("settings".to_string(), "api/settings".to_string()))
        );
        assert_eq!(
            parse_kurowser_uri("kurowser://start/fonts/Poppins-Regular.ttf"),
            Some(("start".to_string(), "fonts/Poppins-Regular.ttf".to_string()))
        );
    }

    #[test]
    fn blocks_dangerous_input_schemes() {
        assert_eq!(
            normalize_input("javascript:alert(1)", SearchEngine::Google),
            START_URL
        );
        assert!(is_dangerous_scheme("JaVaScRiPt:alert(1)"));
        assert!(is_dangerous_scheme("data:text/html,<script>1</script>"));
    }
}
