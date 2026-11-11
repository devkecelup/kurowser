// ─── main.rs ─── Kurowser: Lightweight Native Win32 + WebView2 Browser ───
#![windows_subsystem = "windows"]

mod adblock;
mod engine;
mod omnibox;
mod pages;
mod storage;
mod tabs;
mod ui;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use tao::dpi::LogicalSize;
use tao::event::{ElementState, Event, StartCause, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop, EventLoopBuilder, EventLoopProxy};
use tao::keyboard::KeyCode;
use tao::platform::windows::WindowExtWindows;
use tao::window::{Theme, WindowBuilder};

use windows::core::PCWSTR;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::{EM_SETSEL, WM_MOUSELEAVE};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use engine::{AcceleratorAction, InternalApiEvent, WebViewCallbacks};
use storage::{BrowserData, Rgb, ThemePalette, FOLDER_PALETTE};
use tabs::{FolderId, TabId, TabStore};
use ui::{
    AddrBarEvent, DragDropTarget, FaviconCache, FolderRenameEvent, Layout, OmniboxNavKey,
    OmniboxPopup, SidebarHit, TopbarButton,
};

#[derive(Debug)]
enum AppEvent {
    NewTab(Option<String>),
    NewTabInFolder(FolderId),
    OpenInternalPage(&'static str),
    SwitchTab(TabId),
    CloseTab(Option<TabId>),
    TogglePin(TabId),
    NewFolder,
    CreateFolderFromTab(TabId),
    ToggleFolderCollapsed(FolderId),
    BeginFolderRename(FolderId),
    CommitFolderRename(String),
    CancelFolderRename,
    SetFolderColor(FolderId, Rgb),
    MoveTabToFolder(TabId, Option<FolderId>),
    DeleteFolderKeepTabs(FolderId),
    DeleteFolderWithTabs(FolderId),
    OpenFolderContextMenu {
        fid: FolderId,
        client_x: i32,
        client_y: i32,
    },
    OpenTabContextMenu {
        tab_id: TabId,
        client_x: i32,
        client_y: i32,
    },
    OpenShieldsMenu {
        client_x: i32,
        client_y: i32,
    },
    ToggleGlobalShields,
    ToggleSiteShields(String),
    ToggleCosmeticFilter,
    AdBlocked {
        tab_id: TabId,
        page_host: String,
    },
    ToggleSidebar,
    ToggleBookmark,
    TrimMemory,
    SubmitAddressBar(String),
    PickSuggestion(usize),
    AddressTextChanged,
    AddressFocusChanged(bool),
    OmniboxKey(OmniboxNavKey),
    CancelAddressEdit,
    GoBack,
    GoForward,
    ReloadOrStop,
    TitleChanged(TabId, String),
    FaviconLoaded {
        domain: String,
        png_bytes: Vec<u8>,
    },
    UrlChanged {
        tab_id: TabId,
        url: String,
    },
    PageLoad {
        tab_id: TabId,
        finished: bool,
        url: String,
    },
    DownloadStarted {
        url: String,
        path: PathBuf,
    },
    DownloadCompleted {
        url: String,
        path: Option<PathBuf>,
        ok: bool,
    },
    InternalApi(InternalApiEvent),
    Shortcut(AcceleratorAction),
    Repaint,
}

#[derive(Clone, Copy, Debug)]
struct DragState {
    tab_id: TabId,
    start_x: i32,
    start_y: i32,
    active: bool,
    target: Option<DragDropTarget>,
}

struct AppState {
    tab_store: TabStore,
    layout: Layout,
    palette: ThemePalette,
    browser_data: Arc<Mutex<BrowserData>>,
    favicons: Rc<RefCell<FaviconCache>>,
    omnibox: OmniboxPopup,
    max_live_tabs: usize,
    sidebar_expanded: bool,
    sidebar_hover: SidebarHit,
    sidebar_scroll: i32,
    renaming_folder: Option<FolderId>,
    sidebar_drag: Option<DragState>,
    topbar_hover: Option<TopbarButton>,
    active_bookmarked: bool,
    addr_user_editing: bool,
    tracking_mouse: bool,
    addr_hwnd: HWND,
    rename_hwnd: HWND,
    parent_hwnd: HWND,
}

struct SubclassContext {
    state: Rc<RefCell<AppState>>,
    proxy: EventLoopProxy<AppEvent>,
    hand_cursor: HCURSOR,
}

fn save_folders_state(st: &AppState) {
    let exported = st.tab_store.export_saved_folders();
    st.browser_data
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .save_folders(exported);
}

fn reload_active_tab(st: &AppState) {
    if let Some(tab) = st.tab_store.active_tab() {
        if !tab.url.starts_with("kurowser://") {
            if let Some(ref wv) = tab.webview {
                engine::reload(wv);
            }
        }
    }
}

fn main() {
    let _font_guard = ui::load_bundled_poppins();

    let initial_data = BrowserData::load();
    let initial_palette = initial_data.palette();
    let initial_sidebar = initial_data.settings.sidebar_expanded;
    let initial_max_live = initial_data.settings.max_live_tabs.clamp(2, 4);
    let saved_folders = initial_data.folders.clone();
    let browser_data = Arc::new(Mutex::new(initial_data));

    let event_loop: EventLoop<AppEvent> = EventLoopBuilder::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Kurowser")
        .with_window_icon(ui::build_window_icon())
        .with_inner_size(LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(LogicalSize::new(560.0, 380.0))
        .with_theme(Some(Theme::Dark))
        .with_background_color((
            initial_palette.bg_app.0,
            initial_palette.bg_app.1,
            initial_palette.bg_app.2,
            255,
        ))
        .build(&event_loop)
        .expect("Failed to create main window");

    let parent_hwnd = HWND(window.hwnd() as _);
    ui::apply_dark_titlebar(parent_hwnd);

    let layout = Layout::compute(parent_hwnd, initial_sidebar);
    let mut web_context = engine::create_web_context();

    let proxy_addr = proxy.clone();
    let addr_hwnd = ui::create_address_bar(parent_hwnd, layout.dpi_scale, move |evt| {
        let app_evt = match evt {
            AddrBarEvent::Submit(text) => AppEvent::SubmitAddressBar(text),
            AddrBarEvent::Escape => AppEvent::CancelAddressEdit,
            AddrBarEvent::OmniboxKey(k) => AppEvent::OmniboxKey(k),
            AddrBarEvent::FocusChanged(focused) => AppEvent::AddressFocusChanged(focused),
            AddrBarEvent::Shortcut(act) => AppEvent::Shortcut(act),
            AddrBarEvent::Repaint => AppEvent::Repaint,
        };
        let _ = proxy_addr.send_event(app_evt);
    });
    ui::position_address_bar(addr_hwnd, &layout.topbar, layout.dpi_scale);
    ui::set_address_bar_text(addr_hwnd, engine::START_URL);

    let proxy_rename = proxy.clone();
    let rename_hwnd = ui::create_folder_rename_edit(parent_hwnd, layout.dpi_scale, move |evt| {
        let app_evt = match evt {
            FolderRenameEvent::Commit(name) => AppEvent::CommitFolderRename(name),
            FolderRenameEvent::Cancel => AppEvent::CancelFolderRename,
        };
        let _ = proxy_rename.send_event(app_evt);
    });

    let favicons = Rc::new(RefCell::new(FaviconCache::new()));
    {
        let cached = browser_data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .load_all_cached_favicons(128);
        let mut fc = favicons.borrow_mut();
        for (domain, png_bytes) in cached {
            fc.insert_png(domain, &png_bytes);
        }
    }
    let proxy_pick = proxy.clone();
    let omnibox = OmniboxPopup::create(
        parent_hwnd,
        layout.dpi_scale,
        initial_palette.clone(),
        Rc::clone(&favicons),
        move |idx| {
            let _ = proxy_pick.send_event(AppEvent::PickSuggestion(idx));
        },
    );

    let mut initial_tabs = TabStore::new();
    initial_tabs.load_saved_folders(&saved_folders);
    let first_tab_id = initial_tabs.add_tab("Kurowser", engine::START_URL);
    initial_tabs.set_active(first_tab_id);

    let state = Rc::new(RefCell::new(AppState {
        tab_store: initial_tabs,
        layout: layout.clone(),
        palette: initial_palette,
        browser_data: Arc::clone(&browser_data),
        favicons,
        omnibox,
        max_live_tabs: initial_max_live,
        sidebar_expanded: initial_sidebar,
        sidebar_hover: SidebarHit::None,
        sidebar_scroll: 0,
        renaming_folder: None,
        sidebar_drag: None,
        topbar_hover: None,
        active_bookmarked: false,
        addr_user_editing: false,
        tracking_mouse: false,
        addr_hwnd,
        rename_hwnd,
        parent_hwnd,
    }));

    if let Ok(wv) = engine::create_webview(
        parent_hwnd,
        &mut web_context,
        Arc::clone(&browser_data),
        layout.content_bounds(),
        None,
        make_callbacks(first_tab_id, &proxy),
    ) {
        let mut st = state.borrow_mut();
        if let Some(tab) = st.tab_store.get_tab_mut(first_tab_id) {
            tab.webview = Some(wv);
        }
    }

    install_parent_subclass(parent_hwnd, Rc::clone(&state), proxy.clone());

    let web_context = RefCell::new(web_context);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                invalidate_chrome(parent_hwnd);
            }

            Event::WindowEvent {
                event: WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                recompute_layout_and_resize(&state, parent_hwnd);
            }

            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                {
                    let st = state.borrow();
                    save_folders_state(&st);
                    st.browser_data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .flush_counters();
                }
                *control_flow = ControlFlow::Exit;
            }

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event, .. },
                ..
            } => {
                if event.state == ElementState::Pressed
                    && !ui::is_address_bar_focused(addr_hwnd)
                    && state.borrow().renaming_folder.is_none()
                {
                    if let Some(act) = map_tao_shortcut(event.physical_key) {
                        handle_shortcut(&act, &state, &web_context, &proxy);
                    }
                }
            }

            Event::UserEvent(app_event) => match app_event {
                AppEvent::NewTab(opt_url) => {
                    open_new_tab(&state, &web_context, &proxy, opt_url.as_deref(), None);
                }

                AppEvent::NewTabInFolder(fid) => {
                    open_new_tab(&state, &web_context, &proxy, None, Some(fid));
                }

                AppEvent::OpenInternalPage(url) => {
                    open_internal_page(&state, &web_context, &proxy, url);
                }

                AppEvent::SwitchTab(target_id) => {
                    switch_to_tab(&state, &web_context, &proxy, target_id);
                }

                AppEvent::CloseTab(opt_id) => {
                    close_tab(&state, &web_context, &proxy, opt_id);
                }

                AppEvent::TogglePin(tab_id) => {
                    {
                        let mut st = state.borrow_mut();
                        let had_folder = st
                            .tab_store
                            .get_tab(tab_id)
                            .and_then(|t| t.folder_id)
                            .is_some();
                        st.tab_store.toggle_pin(tab_id);
                        if had_folder {
                            save_folders_state(&st);
                        }
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::NewFolder => {
                    let fid = {
                        let mut st = state.borrow_mut();
                        let fid = st.tab_store.create_folder("Folder Baru", None);
                        save_folders_state(&st);
                        fid
                    };
                    begin_folder_rename(&state, fid);
                }

                AppEvent::CreateFolderFromTab(tab_id) => {
                    let opt_fid = {
                        let mut st = state.borrow_mut();
                        let res = st.tab_store.create_folder_from_tab(tab_id, "Folder Baru");
                        if res.is_some() {
                            save_folders_state(&st);
                        }
                        res
                    };
                    if let Some(fid) = opt_fid {
                        begin_folder_rename(&state, fid);
                    }
                }

                AppEvent::ToggleFolderCollapsed(fid) => {
                    {
                        let mut st = state.borrow_mut();
                        st.tab_store.toggle_folder_collapsed(fid);
                        let max_scroll = ui::max_sidebar_scroll(
                            &st.layout.sidebar,
                            st.layout.dpi_scale,
                            st.tab_store.sidebar_entries().len(),
                        );
                        st.sidebar_scroll = st.sidebar_scroll.clamp(0, max_scroll);
                        save_folders_state(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::BeginFolderRename(fid) => {
                    begin_folder_rename(&state, fid);
                }

                AppEvent::CommitFolderRename(name) => {
                    {
                        let mut st = state.borrow_mut();
                        if let Some(fid) = st.renaming_folder.take() {
                            ui::hide_folder_rename_edit(st.rename_hwnd);
                            if st.tab_store.rename_folder(fid, &name) {
                                save_folders_state(&st);
                            }
                        } else {
                            ui::hide_folder_rename_edit(st.rename_hwnd);
                        }
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::CancelFolderRename => {
                    {
                        let mut st = state.borrow_mut();
                        st.renaming_folder = None;
                        ui::hide_folder_rename_edit(st.rename_hwnd);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::SetFolderColor(fid, color) => {
                    {
                        let mut st = state.borrow_mut();
                        st.tab_store.set_folder_color(fid, color);
                        save_folders_state(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::MoveTabToFolder(tab_id, opt_fid) => {
                    {
                        let mut st = state.borrow_mut();
                        st.tab_store.move_tab_to_folder(tab_id, opt_fid);
                        save_folders_state(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::DeleteFolderKeepTabs(fid) => {
                    {
                        let mut st = state.borrow_mut();
                        if st.renaming_folder == Some(fid) {
                            st.renaming_folder = None;
                            ui::hide_folder_rename_edit(st.rename_hwnd);
                        }
                        st.tab_store.delete_folder_keep_tabs(fid);
                        save_folders_state(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::DeleteFolderWithTabs(fid) => {
                    let (next_active, is_empty) = {
                        let mut st = state.borrow_mut();
                        if st.renaming_folder == Some(fid) {
                            st.renaming_folder = None;
                            ui::hide_folder_rename_edit(st.rename_hwnd);
                        }
                        let closed = st.tab_store.delete_folder_with_tabs(fid);
                        drop(closed);
                        save_folders_state(&st);
                        (st.tab_store.active_id, st.tab_store.tabs.is_empty())
                    };
                    if is_empty {
                        open_new_tab(&state, &web_context, &proxy, None, None);
                    } else if let Some(id) = next_active {
                        switch_to_tab(&state, &web_context, &proxy, id);
                    } else {
                        invalidate_chrome(parent_hwnd);
                    }
                }

                AppEvent::OpenFolderContextMenu {
                    fid,
                    client_x,
                    client_y,
                } => {
                    show_folder_context_menu(&state, &proxy, fid, client_x, client_y);
                }

                AppEvent::OpenTabContextMenu {
                    tab_id,
                    client_x,
                    client_y,
                } => {
                    show_tab_context_menu(&state, &proxy, tab_id, client_x, client_y);
                }

                AppEvent::OpenShieldsMenu { client_x, client_y } => {
                    show_shields_menu(&state, &proxy, client_x, client_y);
                }

                AppEvent::ToggleGlobalShields => {
                    {
                        let st = state.borrow();
                        st.browser_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .toggle_global_adblock();
                        reload_active_tab(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::ToggleSiteShields(host) => {
                    {
                        let st = state.borrow();
                        st.browser_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .toggle_site_allowlist(&host);
                        reload_active_tab(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::ToggleCosmeticFilter => {
                    {
                        let st = state.borrow();
                        let mut d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
                        d.settings.adblock_cosmetic = !d.settings.adblock_cosmetic;
                        d.save_settings();
                        drop(d);
                        reload_active_tab(&st);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::AdBlocked { tab_id, page_host } => {
                    let _ = page_host;
                    {
                        let mut st = state.borrow_mut();
                        st.tab_store.increment_blocked(tab_id);
                        st.browser_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .increment_blocked_ads();
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::ToggleSidebar => {
                    toggle_sidebar(&state, parent_hwnd);
                }

                AppEvent::ToggleBookmark => {
                    {
                        let mut st = state.borrow_mut();
                        if let Some(tab) = st.tab_store.active_tab() {
                            let url = tab.url.clone();
                            let title = tab.title.clone();
                            let bookmarked = st
                                .browser_data
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .toggle_bookmark(&url, &title);
                            st.active_bookmarked = bookmarked;
                        }
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::TrimMemory => {
                    trim_inactive_memory(&state);
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::SubmitAddressBar(raw_input) => {
                    let target = {
                        let st = state.borrow();
                        let picked = st.omnibox.selected_url();
                        st.omnibox.hide();
                        picked.unwrap_or(raw_input)
                    };
                    navigate_active_tab(&state, &web_context, &proxy, &target);
                }

                AppEvent::PickSuggestion(idx) => {
                    let target = {
                        let st = state.borrow();
                        let url = st
                            .omnibox
                            .state
                            .borrow()
                            .suggestions
                            .get(idx)
                            .map(|s| s.url.clone());
                        st.omnibox.hide();
                        url
                    };
                    if let Some(url) = target {
                        navigate_active_tab(&state, &web_context, &proxy, &url);
                    }
                }

                AppEvent::AddressTextChanged => {
                    if !ui::is_setting_address_text() && ui::is_address_bar_focused(addr_hwnd) {
                        let query = ui::get_address_bar_text(addr_hwnd);
                        let mut st = state.borrow_mut();
                        st.addr_user_editing = true;
                        let suggestions = {
                            let d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
                            omnibox::suggest(
                                &query,
                                &d.history,
                                &d.bookmarks,
                                d.settings.search_engine,
                            )
                        };
                        st.omnibox.update_and_show(
                            st.parent_hwnd,
                            &st.layout.topbar,
                            st.layout.dpi_scale,
                            suggestions,
                        );
                    }
                }

                AppEvent::AddressFocusChanged(focused) => {
                    if !focused {
                        let mut st = state.borrow_mut();
                        st.addr_user_editing = false;
                        st.omnibox.hide();
                    } else {
                        let query = ui::get_address_bar_text(addr_hwnd);
                        if query.trim().is_empty() {
                            let st = state.borrow();
                            let suggestions = {
                                let d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
                                omnibox::suggest(
                                    "",
                                    &d.history,
                                    &d.bookmarks,
                                    d.settings.search_engine,
                                )
                            };
                            st.omnibox.update_and_show(
                                st.parent_hwnd,
                                &st.layout.topbar,
                                st.layout.dpi_scale,
                                suggestions,
                            );
                        }
                    }
                }

                AppEvent::OmniboxKey(key) => {
                    let mut st = state.borrow_mut();
                    st.addr_user_editing = true;
                    if !st.omnibox.is_visible() {
                        let query = ui::get_address_bar_text(st.addr_hwnd);
                        let suggestions = {
                            let d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
                            omnibox::suggest(
                                &query,
                                &d.history,
                                &d.bookmarks,
                                d.settings.search_engine,
                            )
                        };
                        st.omnibox.update_and_show(
                            st.parent_hwnd,
                            &st.layout.topbar,
                            st.layout.dpi_scale,
                            suggestions,
                        );
                    }
                    if let Some(s) = st.omnibox.step_selection(key) {
                        if s.kind != omnibox::SuggestionKind::Search {
                            ui::set_address_bar_preview(st.addr_hwnd, &s.url);
                        }
                    }
                }

                AppEvent::CancelAddressEdit => {
                    let mut st = state.borrow_mut();
                    st.addr_user_editing = false;
                    if st.omnibox.is_visible() {
                        st.omnibox.hide();
                    } else if let Some(tab) = st.tab_store.active_tab() {
                        ui::set_address_bar_text(st.addr_hwnd, &tab.url);
                        if let Some(ref wv) = tab.webview {
                            wv.focus();
                        }
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::GoBack => {
                    let st = state.borrow();
                    if let Some(tab) = st.tab_store.active_tab() {
                        if let Some(ref wv) = tab.webview {
                            engine::go_back(wv);
                        }
                    }
                }

                AppEvent::GoForward => {
                    let st = state.borrow();
                    if let Some(tab) = st.tab_store.active_tab() {
                        if let Some(ref wv) = tab.webview {
                            engine::go_forward(wv);
                        }
                    }
                }

                AppEvent::ReloadOrStop => {
                    let st = state.borrow();
                    if let Some(tab) = st.tab_store.active_tab() {
                        if let Some(ref wv) = tab.webview {
                            if tab.is_loading {
                                engine::stop(wv);
                            } else {
                                engine::reload(wv);
                            }
                        }
                    }
                }

                AppEvent::TitleChanged(tab_id, title) => {
                    let is_active = {
                        let mut st = state.borrow_mut();
                        st.tab_store.update_title(tab_id, &title);
                        if let Some(tab) = st.tab_store.get_tab(tab_id) {
                            let url = tab.url.clone();
                            let in_folder = tab.folder_id.is_some();
                            st.browser_data
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .update_history_title(&url, &title);
                            if in_folder {
                                save_folders_state(&st);
                            }
                        }
                        st.tab_store.active_id == Some(tab_id)
                    };
                    if is_active && !title.is_empty() {
                        set_window_title(parent_hwnd, &title);
                    }
                    invalidate_chrome(parent_hwnd);
                }

                AppEvent::FaviconLoaded { domain, png_bytes } => {
                    let (inserted, repaint_popup) = {
                        let st = state.borrow();
                        st.browser_data
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .save_favicon_png(&domain, &png_bytes);
                        let ok = st.favicons.borrow_mut().insert_png(domain, &png_bytes);
                        (ok, st.omnibox.is_visible())
                    };
                    if inserted {
                        invalidate_chrome(parent_hwnd);
                        if repaint_popup {
                            state.borrow().omnibox.repaint_if_visible();
                        }
                    }
                }

                AppEvent::UrlChanged { tab_id, url } => {
                    sync_tab_navigation(&state, tab_id, &url, None, parent_hwnd);
                }

                AppEvent::PageLoad {
                    tab_id,
                    finished,
                    url,
                } => {
                    sync_tab_navigation(&state, tab_id, &url, Some(finished), parent_hwnd);
                }

                AppEvent::DownloadStarted { url, path } => {
                    state
                        .borrow()
                        .browser_data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .record_download_start(&url, &path);
                }

                AppEvent::DownloadCompleted { url, path, ok } => {
                    state
                        .borrow()
                        .browser_data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .update_download_status(&url, path.as_deref(), ok);
                }

                AppEvent::InternalApi(api_evt) => match api_evt {
                    InternalApiEvent::SettingsUpdated => {
                        let mut st = state.borrow_mut();
                        let (new_palette, new_max) = {
                            let d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
                            (d.palette(), d.settings.max_live_tabs.clamp(2, 4))
                        };
                        let bg = (
                            new_palette.bg_app.0,
                            new_palette.bg_app.1,
                            new_palette.bg_app.2,
                        );
                        st.palette = new_palette.clone();
                        st.max_live_tabs = new_max;
                        st.omnibox.set_palette(new_palette);
                        for tab in &st.tab_store.tabs {
                            if let Some(ref wv) = tab.webview {
                                engine::apply_theme_bg(wv, bg);
                            }
                        }
                        drop(st);
                        unsafe {
                            let _ = RedrawWindow(
                                addr_hwnd,
                                None,
                                None,
                                RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
                            );
                        }
                        invalidate_chrome(parent_hwnd);
                    }
                    InternalApiEvent::ShieldsUpdated => {
                        invalidate_chrome(parent_hwnd);
                    }
                    InternalApiEvent::DataUpdated => {
                        let mut st = state.borrow_mut();
                        if let Some(url) = st.tab_store.active_tab().map(|t| t.url.clone()) {
                            let bm = st
                                .browser_data
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .is_bookmarked(&url);
                            st.active_bookmarked = bm;
                        }
                        drop(st);
                        invalidate_chrome(parent_hwnd);
                    }
                },

                AppEvent::Shortcut(act) => {
                    handle_shortcut(&act, &state, &web_context, &proxy);
                }

                AppEvent::Repaint => {
                    invalidate_chrome(parent_hwnd);
                }
            },

            _ => {}
        }
    });
}

fn begin_folder_rename(state: &Rc<RefCell<AppState>>, fid: FolderId) {
    let setup = {
        let mut st = state.borrow_mut();
        if let Some(f) = st.tab_store.folders.iter_mut().find(|f| f.id == fid) {
            f.collapsed = false;
        }
        let entries = st.tab_store.sidebar_entries();
        let entry_idx = entries
            .iter()
            .position(|e| matches!(e, tabs::SidebarEntry::FolderHeader { folder_id, .. } if *folder_id == fid));
        if let Some(idx) = entry_idx {
            let max_scroll =
                ui::max_sidebar_scroll(&st.layout.sidebar, st.layout.dpi_scale, entries.len());
            let row_step = ui::scale_i32(ui::TAB_ITEM_HEIGHT + 4, st.layout.dpi_scale);
            let desired = (idx as i32) * row_step;
            st.sidebar_scroll = desired.clamp(0, max_scroll);
        }

        let name = st
            .tab_store
            .get_folder(fid)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "Folder".to_string());
        let rect_opt = {
            let entries = st.tab_store.sidebar_entries();
            ui::folder_rename_rect(
                &st.layout.sidebar,
                st.layout.dpi_scale,
                &entries,
                st.sidebar_scroll,
                fid,
            )
        };
        if rect_opt.is_some() {
            st.renaming_folder = Some(fid);
        }
        (
            st.rename_hwnd,
            st.parent_hwnd,
            rect_opt,
            name,
            st.sidebar_expanded,
        )
    };

    if let (rename_hwnd, parent_hwnd, Some(rc), name, true) = setup {
        invalidate_chrome(parent_hwnd);
        ui::show_folder_rename_edit(rename_hwnd, &rc, &name);
    }
}

fn show_folder_context_menu(
    state: &Rc<RefCell<AppState>>,
    proxy: &EventLoopProxy<AppEvent>,
    fid: FolderId,
    client_x: i32,
    client_y: i32,
) {
    let (parent_hwnd, current_color_hex) = {
        let st = state.borrow();
        let Some(f) = st.tab_store.get_folder(fid) else {
            return;
        };
        (st.parent_hwnd, f.color.hex())
    };

    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let Ok(color_sub) = CreatePopupMenu() else {
            let _ = DestroyMenu(menu);
            return;
        };

        const CMD_ADD_TAB: u32 = 1;
        const CMD_RENAME: u32 = 2;
        const CMD_DEL_KEEP: u32 = 3;
        const CMD_DEL_ALL: u32 = 4;
        const CMD_COLOR_BASE: u32 = 100;

        let w_add = ui::wide("+ Tab Baru di Folder Ini");
        let _ = AppendMenuW(menu, MF_STRING, CMD_ADD_TAB as usize, PCWSTR(w_add.as_ptr()));

        let w_ren = ui::wide("Ganti Nama Folder");
        let _ = AppendMenuW(menu, MF_STRING, CMD_RENAME as usize, PCWSTR(w_ren.as_ptr()));

        for (idx, (label, hex)) in FOLDER_PALETTE.iter().enumerate() {
            let flags = if hex.eq_ignore_ascii_case(&current_color_hex) {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let wl = ui::wide(label);
            let _ = AppendMenuW(
                color_sub,
                flags,
                (CMD_COLOR_BASE + idx as u32) as usize,
                PCWSTR(wl.as_ptr()),
            );
        }
        let w_color = ui::wide("Warna Folder");
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            color_sub.0 as usize,
            PCWSTR(w_color.as_ptr()),
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

        let w_dk = ui::wide("Hapus Folder (Simpan Tab)");
        let _ = AppendMenuW(menu, MF_STRING, CMD_DEL_KEEP as usize, PCWSTR(w_dk.as_ptr()));

        let w_da = ui::wide("Hapus Folder & Tutup Semua Tab");
        let _ = AppendMenuW(menu, MF_STRING, CMD_DEL_ALL as usize, PCWSTR(w_da.as_ptr()));

        let mut pt = POINT {
            x: client_x,
            y: client_y,
        };
        let _ = ClientToScreen(parent_hwnd, &mut pt);
        let _ = SetForegroundWindow(parent_hwnd);
        let picked = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            parent_hwnd,
            None,
        );
        let _ = DestroyMenu(menu);

        let cmd = picked.0 as u32;
        match cmd {
            CMD_ADD_TAB => {
                let _ = proxy.send_event(AppEvent::NewTabInFolder(fid));
            }
            CMD_RENAME => {
                let _ = proxy.send_event(AppEvent::BeginFolderRename(fid));
            }
            CMD_DEL_KEEP => {
                let _ = proxy.send_event(AppEvent::DeleteFolderKeepTabs(fid));
            }
            CMD_DEL_ALL => {
                let _ = proxy.send_event(AppEvent::DeleteFolderWithTabs(fid));
            }
            c if c >= CMD_COLOR_BASE && c < CMD_COLOR_BASE + FOLDER_PALETTE.len() as u32 => {
                let idx = (c - CMD_COLOR_BASE) as usize;
                if let Some(rgb) = Rgb::parse_hex(FOLDER_PALETTE[idx].1) {
                    let _ = proxy.send_event(AppEvent::SetFolderColor(fid, rgb));
                }
            }
            _ => {}
        }
    }
}

fn show_tab_context_menu(
    state: &Rc<RefCell<AppState>>,
    proxy: &EventLoopProxy<AppEvent>,
    tab_id: TabId,
    client_x: i32,
    client_y: i32,
) {
    let (parent_hwnd, pinned, current_fid, folders) = {
        let st = state.borrow();
        let Some(tab) = st.tab_store.get_tab(tab_id) else {
            return;
        };
        let flist: Vec<(FolderId, String)> = st
            .tab_store
            .folders
            .iter()
            .map(|f| (f.id, f.name.clone()))
            .collect();
        (st.parent_hwnd, tab.pinned, tab.folder_id, flist)
    };

    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let Ok(move_sub) = CreatePopupMenu() else {
            let _ = DestroyMenu(menu);
            return;
        };

        const CMD_PIN: u32 = 1;
        const CMD_NEW_FOLDER_FROM_TAB: u32 = 2;
        const CMD_UNGROUP: u32 = 3;
        const CMD_CLOSE: u32 = 4;
        const CMD_MOVE_BASE: u32 = 100;

        let w_pin = ui::wide(if pinned { "Lepas Sematan (Unpin)" } else { "Sematkan Tab (Pin)" });
        let _ = AppendMenuW(menu, MF_STRING, CMD_PIN as usize, PCWSTR(w_pin.as_ptr()));

        let w_new_fld = ui::wide("+ Folder Baru dari Tab Ini");
        let _ = AppendMenuW(
            move_sub,
            MF_STRING,
            CMD_NEW_FOLDER_FROM_TAB as usize,
            PCWSTR(w_new_fld.as_ptr()),
        );
        if current_fid.is_some() {
            let w_out = ui::wide("Keluarkan dari Folder");
            let _ = AppendMenuW(move_sub, MF_STRING, CMD_UNGROUP as usize, PCWSTR(w_out.as_ptr()));
        }
        if !folders.is_empty() {
            let _ = AppendMenuW(move_sub, MF_SEPARATOR, 0, PCWSTR::null());
            for (idx, (fid, name)) in folders.iter().enumerate() {
                let flags = if current_fid == Some(*fid) {
                    MF_STRING | MF_CHECKED
                } else {
                    MF_STRING
                };
                let wn = ui::wide(name);
                let _ = AppendMenuW(
                    move_sub,
                    flags,
                    (CMD_MOVE_BASE + idx as u32) as usize,
                    PCWSTR(wn.as_ptr()),
                );
            }
        }
        let w_move = ui::wide("Pindahkan ke Folder");
        let _ = AppendMenuW(
            menu,
            MF_POPUP,
            move_sub.0 as usize,
            PCWSTR(w_move.as_ptr()),
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let w_close = ui::wide("Tutup Tab");
        let _ = AppendMenuW(menu, MF_STRING, CMD_CLOSE as usize, PCWSTR(w_close.as_ptr()));

        let mut pt = POINT {
            x: client_x,
            y: client_y,
        };
        let _ = ClientToScreen(parent_hwnd, &mut pt);
        let _ = SetForegroundWindow(parent_hwnd);
        let picked = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            parent_hwnd,
            None,
        );
        let _ = DestroyMenu(menu);

        let cmd = picked.0 as u32;
        match cmd {
            CMD_PIN => {
                let _ = proxy.send_event(AppEvent::TogglePin(tab_id));
            }
            CMD_NEW_FOLDER_FROM_TAB => {
                let _ = proxy.send_event(AppEvent::CreateFolderFromTab(tab_id));
            }
            CMD_UNGROUP => {
                let _ = proxy.send_event(AppEvent::MoveTabToFolder(tab_id, None));
            }
            CMD_CLOSE => {
                let _ = proxy.send_event(AppEvent::CloseTab(Some(tab_id)));
            }
            c if c >= CMD_MOVE_BASE && c < CMD_MOVE_BASE + folders.len() as u32 => {
                let fid = folders[(c - CMD_MOVE_BASE) as usize].0;
                let _ = proxy.send_event(AppEvent::MoveTabToFolder(tab_id, Some(fid)));
            }
            _ => {}
        }
    }
}

fn show_shields_menu(
    state: &Rc<RefCell<AppState>>,
    proxy: &EventLoopProxy<AppEvent>,
    client_x: i32,
    client_y: i32,
) {
    let (parent_hwnd, cur_url, blocked_count, global_on, cosmetic_on, site_active, total_blocked) = {
        let st = state.borrow();
        let active = st.tab_store.active_tab();
        let url = active.map(|t| t.url.clone()).unwrap_or_default();
        let blocked = active.map(|t| t.blocked_count).unwrap_or(0);
        let d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
        let policy = d.shield_policy();
        let host = adblock::normalize_host(&url);
        let site_on = if url.starts_with("http://") || url.starts_with("https://") {
            policy.is_active_for_site(&host)
        } else {
            policy.global_enabled
        };
        (
            st.parent_hwnd,
            url,
            blocked,
            policy.global_enabled,
            policy.cosmetic_enabled,
            site_on,
            d.settings.total_blocked_ads,
        )
    };

    let host = adblock::normalize_host(&cur_url);
    let is_http = (cur_url.starts_with("http://") || cur_url.starts_with("https://")) && !host.is_empty();

    unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };

        const CMD_SITE: u32 = 1;
        const CMD_GLOBAL: u32 = 2;
        const CMD_COSMETIC: u32 = 3;
        const CMD_SETTINGS: u32 = 4;

        let hdr_text = if is_http {
            format!(
                "Kurowser Shields ({}): {} diblokir (Total {})",
                if site_active { "AKTIF" } else { "MATI" },
                blocked_count,
                total_blocked
            )
        } else {
            format!("Kurowser Shields: Total {} iklan diblokir", total_blocked)
        };
        let w_hdr = ui::wide(&hdr_text);
        let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(w_hdr.as_ptr()));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());

        if is_http {
            let site_lbl = if site_active {
                format!("Shields Aktif di {}", host)
            } else {
                format!("Aktifkan Shields di {}", host)
            };
            let w_site = ui::wide(&site_lbl);
            let flags = if site_active {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let _ = AppendMenuW(menu, flags, CMD_SITE as usize, PCWSTR(w_site.as_ptr()));
        }

        let w_global = ui::wide("Pemblokir Iklan & Tracker Global");
        let _ = AppendMenuW(
            menu,
            if global_on {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            },
            CMD_GLOBAL as usize,
            PCWSTR(w_global.as_ptr()),
        );

        let w_cos = ui::wide("Sembunyikan Elemen Iklan (Cosmetic)");
        let _ = AppendMenuW(
            menu,
            if cosmetic_on {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            },
            CMD_COSMETIC as usize,
            PCWSTR(w_cos.as_ptr()),
        );

        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let w_set = ui::wide("Kelola Allowlist & Shields...");
        let _ = AppendMenuW(menu, MF_STRING, CMD_SETTINGS as usize, PCWSTR(w_set.as_ptr()));

        let mut pt = POINT {
            x: client_x,
            y: client_y,
        };
        let _ = ClientToScreen(parent_hwnd, &mut pt);
        let _ = SetForegroundWindow(parent_hwnd);
        let picked = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
            pt.x,
            pt.y,
            0,
            parent_hwnd,
            None,
        );
        let _ = DestroyMenu(menu);

        match picked.0 as u32 {
            CMD_SITE if is_http => {
                let _ = proxy.send_event(AppEvent::ToggleSiteShields(host));
            }
            CMD_GLOBAL => {
                let _ = proxy.send_event(AppEvent::ToggleGlobalShields);
            }
            CMD_COSMETIC => {
                let _ = proxy.send_event(AppEvent::ToggleCosmeticFilter);
            }
            CMD_SETTINGS => {
                let _ = proxy.send_event(AppEvent::OpenInternalPage(engine::SETTINGS_URL));
            }
            _ => {}
        }
    }
}

fn sync_tab_navigation(
    state: &Rc<RefCell<AppState>>,
    tab_id: TabId,
    url: &str,
    finished_opt: Option<bool>,
    parent_hwnd: HWND,
) {
    let addr_update = {
        let mut st = state.borrow_mut();
        let prev_url = st
            .tab_store
            .get_tab(tab_id)
            .map(|t| t.url.clone())
            .unwrap_or_default();

        match finished_opt {
            Some(finished) => {
                if !finished && !url.is_empty() {
                    let prev_host = adblock::normalize_host(&prev_url);
                    let new_host = adblock::normalize_host(url);
                    if prev_host != new_host {
                        st.tab_store.reset_blocked(tab_id);
                    }
                }
                st.tab_store.update_page_state(tab_id, url, finished);
            }
            None => {
                st.tab_store.update_url(tab_id, url);
            }
        }

        if let Some(tab) = st.tab_store.get_tab_mut(tab_id) {
            if let Some(ref wv) = tab.webview {
                tab.can_go_back = engine::can_go_back(wv);
                tab.can_go_forward = engine::can_go_forward(wv);
                if finished_opt == Some(true) {
                    tab.is_audio = engine::is_playing_audio(wv);
                }
            }
        }

        if finished_opt == Some(true) {
            let (tab_url, tab_title, in_folder) = st
                .tab_store
                .get_tab(tab_id)
                .map(|t| (t.url.clone(), t.title.clone(), t.folder_id.is_some()))
                .unwrap_or_default();
            st.browser_data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record_visit(&tab_url, &tab_title);
            if in_folder {
                save_folders_state(&st);
            }
        }

        let is_active = st.tab_store.active_id == Some(tab_id);
        let cur_url = st
            .tab_store
            .get_tab(tab_id)
            .map(|t| t.url.clone())
            .unwrap_or_default();
        let url_changed = cur_url != prev_url;
        let popup_open = st.omnibox.is_visible();
        let should_sync_addr = is_active
            && !cur_url.is_empty()
            && (!st.addr_user_editing || (url_changed && !popup_open));

        if is_active {
            let bm = st
                .browser_data
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_bookmarked(&cur_url);
            st.active_bookmarked = bm;
            if should_sync_addr {
                st.addr_user_editing = false;
            }
        }

        if should_sync_addr {
            Some((st.addr_hwnd, cur_url))
        } else {
            None
        }
    };

    if let Some((addr_hwnd, target_url)) = addr_update {
        if ui::get_address_bar_text(addr_hwnd) != target_url {
            ui::set_address_bar_text(addr_hwnd, &target_url);
        }
    }
    invalidate_chrome(parent_hwnd);
}

// ── Callbacks & Shortcuts ──

fn make_callbacks(tab_id: TabId, proxy: &EventLoopProxy<AppEvent>) -> WebViewCallbacks {
    let p_title = proxy.clone();
    let p_load = proxy.clone();
    let p_url = proxy.clone();
    let p_new_win = proxy.clone();
    let p_key = proxy.clone();
    let p_fav = proxy.clone();
    let p_dl_start = proxy.clone();
    let p_dl_done = proxy.clone();
    let p_api = proxy.clone();
    let p_ad = proxy.clone();

    WebViewCallbacks {
        on_title: Box::new(move |title| {
            let _ = p_title.send_event(AppEvent::TitleChanged(tab_id, title));
        }),
        on_load: Box::new(move |finished, url| {
            let _ = p_load.send_event(AppEvent::PageLoad {
                tab_id,
                finished,
                url,
            });
        }),
        on_url_changed: Box::new(move |url| {
            let _ = p_url.send_event(AppEvent::UrlChanged { tab_id, url });
        }),
        on_new_window: Box::new(move |uri| {
            let _ = p_new_win.send_event(AppEvent::NewTab(Some(uri)));
        }),
        on_shortcut: Box::new(move |act| {
            let _ = p_key.send_event(AppEvent::Shortcut(act));
        }),
        on_favicon: Box::new(move |domain, png_bytes| {
            let _ = p_fav.send_event(AppEvent::FaviconLoaded { domain, png_bytes });
        }),
        on_download_started: Box::new(move |url, path| {
            let _ = p_dl_start.send_event(AppEvent::DownloadStarted { url, path });
        }),
        on_download_completed: Box::new(move |url, path, ok| {
            let _ = p_dl_done.send_event(AppEvent::DownloadCompleted { url, path, ok });
        }),
        on_internal_api: Box::new(move |evt| {
            let _ = p_api.send_event(AppEvent::InternalApi(evt));
        }),
        on_ad_blocked: Box::new(move |page_host| {
            let _ = p_ad.send_event(AppEvent::AdBlocked { tab_id, page_host });
        }),
    }
}

fn map_tao_shortcut(key: KeyCode) -> Option<AcceleratorAction> {
    let ctrl = unsafe { GetKeyState(VK_CONTROL.0 as i32) < 0 };
    let shift = unsafe { GetKeyState(VK_SHIFT.0 as i32) < 0 };
    let alt = unsafe { GetKeyState(VK_MENU.0 as i32) < 0 };

    if ctrl && !alt {
        match key {
            KeyCode::KeyT => Some(AcceleratorAction::NewTab),
            KeyCode::KeyW => Some(AcceleratorAction::CloseTab),
            KeyCode::KeyL => Some(AcceleratorAction::FocusAddressBar),
            KeyCode::KeyB => Some(AcceleratorAction::ToggleSidebar),
            KeyCode::KeyR => Some(AcceleratorAction::Reload),
            KeyCode::KeyD => Some(AcceleratorAction::ToggleBookmark),
            KeyCode::KeyH => Some(AcceleratorAction::OpenHistory),
            KeyCode::KeyJ => Some(AcceleratorAction::OpenDownloads),
            KeyCode::KeyO if shift => Some(AcceleratorAction::OpenBookmarks),
            KeyCode::Comma => Some(AcceleratorAction::OpenSettings),
            KeyCode::Tab if shift => Some(AcceleratorAction::PrevTab),
            KeyCode::Tab => Some(AcceleratorAction::NextTab),
            KeyCode::Digit1 => Some(AcceleratorAction::SelectTabIndex(0)),
            KeyCode::Digit2 => Some(AcceleratorAction::SelectTabIndex(1)),
            KeyCode::Digit3 => Some(AcceleratorAction::SelectTabIndex(2)),
            KeyCode::Digit4 => Some(AcceleratorAction::SelectTabIndex(3)),
            KeyCode::Digit5 => Some(AcceleratorAction::SelectTabIndex(4)),
            KeyCode::Digit6 => Some(AcceleratorAction::SelectTabIndex(5)),
            KeyCode::Digit7 => Some(AcceleratorAction::SelectTabIndex(6)),
            KeyCode::Digit8 => Some(AcceleratorAction::SelectTabIndex(7)),
            KeyCode::Digit9 => Some(AcceleratorAction::SelectTabIndex(8)),
            _ => None,
        }
    } else if alt && !ctrl {
        match key {
            KeyCode::ArrowLeft => Some(AcceleratorAction::GoBack),
            KeyCode::ArrowRight => Some(AcceleratorAction::GoForward),
            _ => None,
        }
    } else if !ctrl && !alt && key == KeyCode::F5 {
        Some(AcceleratorAction::Reload)
    } else {
        None
    }
}

fn handle_shortcut(
    act: &AcceleratorAction,
    state: &Rc<RefCell<AppState>>,
    web_context: &RefCell<engine::WebContext>,
    proxy: &EventLoopProxy<AppEvent>,
) {
    match act {
        AcceleratorAction::NewTab => open_new_tab(state, web_context, proxy, None, None),
        AcceleratorAction::CloseTab => close_tab(state, web_context, proxy, None),
        AcceleratorAction::FocusAddressBar => focus_address_bar(state),
        AcceleratorAction::ToggleSidebar => {
            let parent_hwnd = state.borrow().parent_hwnd;
            toggle_sidebar(state, parent_hwnd);
        }
        AcceleratorAction::ToggleBookmark => {
            {
                let mut st = state.borrow_mut();
                if let Some((url, title)) = st
                    .tab_store
                    .active_tab()
                    .map(|t| (t.url.clone(), t.title.clone()))
                {
                    let bm = st
                        .browser_data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .toggle_bookmark(&url, &title);
                    st.active_bookmarked = bm;
                }
            }
            let parent_hwnd = state.borrow().parent_hwnd;
            invalidate_chrome(parent_hwnd);
        }
        AcceleratorAction::OpenHistory => {
            open_internal_page(state, web_context, proxy, engine::HISTORY_URL);
        }
        AcceleratorAction::OpenBookmarks => {
            open_internal_page(state, web_context, proxy, engine::BOOKMARKS_URL);
        }
        AcceleratorAction::OpenDownloads => {
            open_internal_page(state, web_context, proxy, engine::DOWNLOADS_URL);
        }
        AcceleratorAction::OpenSettings => {
            open_internal_page(state, web_context, proxy, engine::SETTINGS_URL);
        }
        AcceleratorAction::Reload => {
            let st = state.borrow();
            if let Some(tab) = st.tab_store.active_tab() {
                if let Some(ref wv) = tab.webview {
                    engine::reload(wv);
                }
            }
        }
        AcceleratorAction::StopLoading => {
            let st = state.borrow();
            if let Some(tab) = st.tab_store.active_tab() {
                if tab.is_loading {
                    if let Some(ref wv) = tab.webview {
                        engine::stop(wv);
                    }
                }
            }
        }
        AcceleratorAction::NextTab => {
            let next = state.borrow().tab_store.next_tab_id();
            if let Some(id) = next {
                switch_to_tab(state, web_context, proxy, id);
            }
        }
        AcceleratorAction::PrevTab => {
            let prev = state.borrow().tab_store.prev_tab_id();
            if let Some(id) = prev {
                switch_to_tab(state, web_context, proxy, id);
            }
        }
        AcceleratorAction::SelectTabIndex(idx) => {
            let target = state.borrow().tab_store.nth_tab_id(*idx);
            if let Some(id) = target {
                switch_to_tab(state, web_context, proxy, id);
            }
        }
        AcceleratorAction::GoBack => {
            let st = state.borrow();
            if let Some(tab) = st.tab_store.active_tab() {
                if let Some(ref wv) = tab.webview {
                    engine::go_back(wv);
                }
            }
        }
        AcceleratorAction::GoForward => {
            let st = state.borrow();
            if let Some(tab) = st.tab_store.active_tab() {
                if let Some(ref wv) = tab.webview {
                    engine::go_forward(wv);
                }
            }
        }
    }
}

// ── Tab Lifecycle & Memory Control ──

fn toggle_sidebar(state: &Rc<RefCell<AppState>>, parent_hwnd: HWND) {
    {
        let mut st = state.borrow_mut();
        st.sidebar_expanded = !st.sidebar_expanded;
        if !st.sidebar_expanded && st.renaming_folder.is_some() {
            st.renaming_folder = None;
            ui::hide_folder_rename_edit(st.rename_hwnd);
        }
        let mut d = st.browser_data.lock().unwrap_or_else(|e| e.into_inner());
        d.settings.sidebar_expanded = st.sidebar_expanded;
        d.save_settings();
    }
    recompute_layout_and_resize(state, parent_hwnd);
}

fn trim_inactive_memory(state: &Rc<RefCell<AppState>>) {
    let mut st = state.borrow_mut();
    let active_id = st.tab_store.active_id;

    let evict_ids: Vec<TabId> = st
        .tab_store
        .tabs
        .iter()
        .filter(|t| {
            t.webview.is_some()
                && Some(t.id) != active_id
                && !t.pinned
                && !t.is_audio
        })
        .map(|t| t.id)
        .collect();

    for id in evict_ids {
        if let Some(tab) = st.tab_store.get_tab_mut(id) {
            tab.is_suspended = false;
            tab.is_loading = false;
            let old = tab.webview.take();
            drop(old);
        }
    }

    for tab in &mut st.tab_store.tabs {
        if Some(tab.id) != active_id {
            if let Some(ref wv) = tab.webview {
                let (audio, suspended) = engine::suspend_background(wv);
                tab.is_audio = audio;
                tab.is_suspended = suspended;
            }
        }
    }
}

fn background_current_active(st: &mut AppState) {
    if let Some(active_id) = st.tab_store.active_id {
        if let Some(tab) = st.tab_store.get_tab_mut(active_id) {
            if let Some(ref wv) = tab.webview {
                let (audio, suspended) = engine::suspend_background(wv);
                tab.is_audio = audio;
                tab.is_suspended = suspended;
            }
        }
    }
}

fn ensure_webview_budget(st: &mut AppState) {
    let budget = st.max_live_tabs.clamp(2, 4);
    while st.tab_store.live_count() >= budget {
        if let Some(evicted) = st.tab_store.evict_lru() {
            drop(evicted);
        } else {
            break;
        }
    }
}

fn open_internal_page(
    state: &Rc<RefCell<AppState>>,
    web_context: &RefCell<engine::WebContext>,
    proxy: &EventLoopProxy<AppEvent>,
    target_url: &str,
) {
    let existing = state.borrow().tab_store.find_by_url(target_url);
    if let Some(id) = existing {
        switch_to_tab(state, web_context, proxy, id);
        let st = state.borrow();
        if let Some(tab) = st.tab_store.get_tab(id) {
            if let Some(ref wv) = tab.webview {
                engine::reload(wv);
            }
        }
    } else {
        open_new_tab(state, web_context, proxy, Some(target_url), None);
    }
}

fn open_new_tab(
    state: &Rc<RefCell<AppState>>,
    web_context: &RefCell<engine::WebContext>,
    proxy: &EventLoopProxy<AppEvent>,
    initial_url: Option<&str>,
    folder_id: Option<FolderId>,
) {
    let is_start = initial_url.is_none();
    let url_str = initial_url.unwrap_or(engine::START_URL);
    let init_title = match url_str {
        engine::START_URL => "Kurowser",
        engine::HISTORY_URL => "History",
        engine::BOOKMARKS_URL => "Bookmarks",
        engine::DOWNLOADS_URL => "Downloads",
        engine::SETTINGS_URL => "Tema & Settings",
        _ => "Loading…",
    };

    let (id, bounds, browser_data, addr_hwnd, parent_hwnd) = {
        let mut st = state.borrow_mut();
        st.addr_user_editing = false;
        st.omnibox.hide();
        background_current_active(&mut st);
        ensure_webview_budget(&mut st);

        let id = st
            .tab_store
            .add_tab_in_folder(init_title, url_str, folder_id);
        st.tab_store.set_active(id);
        if folder_id.is_some() {
            save_folders_state(&st);
        }
        let bm = st
            .browser_data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_bookmarked(url_str);
        st.active_bookmarked = bm;
        if !url_str.starts_with("kurowser://") {
            if let Some(t) = st.tab_store.get_tab_mut(id) {
                t.is_loading = true;
            }
        }
        (
            id,
            st.layout.content_bounds(),
            Arc::clone(&st.browser_data),
            st.addr_hwnd,
            st.parent_hwnd,
        )
    };

    let mut ctx = web_context.borrow_mut();
    let maybe_wv = engine::create_webview(
        parent_hwnd,
        &mut ctx,
        browser_data,
        bounds,
        initial_url,
        make_callbacks(id, proxy),
    );
    drop(ctx);

    if let Ok(wv) = maybe_wv {
        if !is_start {
            wv.focus();
        }
        let mut st = state.borrow_mut();
        if let Some(tab) = st.tab_store.get_tab_mut(id) {
            tab.webview = Some(wv);
        }
    }

    ui::set_address_bar_text(addr_hwnd, url_str);
    set_window_title(parent_hwnd, init_title);
    if is_start {
        focus_address_bar(state);
    }
    invalidate_chrome(parent_hwnd);
}

fn switch_to_tab(
    state: &Rc<RefCell<AppState>>,
    web_context: &RefCell<engine::WebContext>,
    proxy: &EventLoopProxy<AppEvent>,
    target_id: TabId,
) {
    let (already_active, has_webview, url, title, bounds, browser_data, addr_hwnd, parent_hwnd) = {
        let mut st = state.borrow_mut();
        st.addr_user_editing = false;
        st.omnibox.hide();
        if st.tab_store.get_tab(target_id).is_none() {
            return;
        }
        let already = st.tab_store.active_id == Some(target_id);
        if !already {
            background_current_active(&mut st);
            st.tab_store.set_active(target_id);
        }

        let bounds = st.layout.content_bounds();
        let addr_hwnd = st.addr_hwnd;
        let parent_hwnd = st.parent_hwnd;
        let browser_data = Arc::clone(&st.browser_data);

        let (has_wv, url, title) = {
            let tab = st.tab_store.get_tab_mut(target_id).unwrap();
            let has_wv = tab.webview.is_some();
            if let Some(ref wv) = tab.webview {
                engine::resume_foreground(wv, bounds);
                tab.is_suspended = false;
                tab.can_go_back = engine::can_go_back(wv);
                tab.can_go_forward = engine::can_go_forward(wv);
            }
            (has_wv, tab.url.clone(), tab.title.clone())
        };
        let bm = st
            .browser_data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_bookmarked(&url);
        st.active_bookmarked = bm;

        (
            already,
            has_wv,
            url,
            title,
            bounds,
            browser_data,
            addr_hwnd,
            parent_hwnd,
        )
    };

    if already_active && has_webview {
        invalidate_chrome(parent_hwnd);
        return;
    }

    if !has_webview {
        {
            let mut st = state.borrow_mut();
            ensure_webview_budget(&mut st);
            if let Some(t) = st.tab_store.get_tab_mut(target_id) {
                if !url.starts_with("kurowser://") {
                    t.is_loading = true;
                }
            }
        }

        let mut ctx = web_context.borrow_mut();
        let maybe_wv = engine::create_webview(
            parent_hwnd,
            &mut ctx,
            browser_data,
            bounds,
            Some(url.as_str()),
            make_callbacks(target_id, proxy),
        );
        drop(ctx);

        if let Ok(wv) = maybe_wv {
            wv.focus();
            let mut st = state.borrow_mut();
            if let Some(tab) = st.tab_store.get_tab_mut(target_id) {
                tab.webview = Some(wv);
            }
        }
    }

    ui::set_address_bar_text(addr_hwnd, &url);
    set_window_title(parent_hwnd, &title);
    invalidate_chrome(parent_hwnd);
}

fn close_tab(
    state: &Rc<RefCell<AppState>>,
    web_context: &RefCell<engine::WebContext>,
    proxy: &EventLoopProxy<AppEvent>,
    target_opt: Option<TabId>,
) {
    let (next_active, is_empty) = {
        let mut st = state.borrow_mut();
        let Some(target_id) = target_opt.or(st.tab_store.active_id) else {
            return;
        };
        let had_folder = st
            .tab_store
            .get_tab(target_id)
            .and_then(|t| t.folder_id)
            .is_some();
        let old_wv = st.tab_store.close_tab(target_id);
        drop(old_wv);
        if had_folder {
            save_folders_state(&st);
        }
        (st.tab_store.active_id, st.tab_store.tabs.is_empty())
    };

    if is_empty {
        open_new_tab(state, web_context, proxy, None, None);
    } else if let Some(new_id) = next_active {
        switch_to_tab(state, web_context, proxy, new_id);
    }
}

fn navigate_active_tab(
    state: &Rc<RefCell<AppState>>,
    web_context: &RefCell<engine::WebContext>,
    proxy: &EventLoopProxy<AppEvent>,
    raw_input: &str,
) {
    let (active_id, has_wv, normalized, bounds, browser_data, addr_hwnd, parent_hwnd) = {
        let mut st = state.borrow_mut();
        st.addr_user_editing = false;
        let search_engine = st
            .browser_data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .settings
            .search_engine;
        let normalized = engine::normalize_input(raw_input, search_engine);
        let Some(id) = st.tab_store.active_id else {
            return;
        };
        let bounds = st.layout.content_bounds();
        let addr_hwnd = st.addr_hwnd;
        let parent_hwnd = st.parent_hwnd;
        let browser_data = Arc::clone(&st.browser_data);
        let bm = st
            .browser_data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_bookmarked(&normalized);
        st.active_bookmarked = bm;

        let tab = st.tab_store.get_tab_mut(id).unwrap();
        tab.url = normalized.clone();
        tab.domain = storage::domain_from_url(&normalized);
        tab.blocked_count = 0;
        if normalized == engine::START_URL {
            tab.title = "Kurowser".to_string();
            tab.is_loading = false;
        } else {
            tab.is_loading = !normalized.starts_with("kurowser://");
        }

        let has_wv = if let Some(ref wv) = tab.webview {
            engine::navigate(wv, &normalized, search_engine);
            wv.focus();
            true
        } else {
            false
        };
        (
            id,
            has_wv,
            normalized,
            bounds,
            browser_data,
            addr_hwnd,
            parent_hwnd,
        )
    };

    if !has_wv {
        let mut ctx = web_context.borrow_mut();
        if let Ok(wv) = engine::create_webview(
            parent_hwnd,
            &mut ctx,
            browser_data,
            bounds,
            Some(normalized.as_str()),
            make_callbacks(active_id, proxy),
        ) {
            wv.focus();
            let mut st = state.borrow_mut();
            if let Some(tab) = st.tab_store.get_tab_mut(active_id) {
                tab.webview = Some(wv);
            }
        }
    }

    ui::set_address_bar_text(addr_hwnd, &normalized);
    invalidate_chrome(parent_hwnd);
}

fn focus_address_bar(state: &Rc<RefCell<AppState>>) {
    let addr_hwnd = state.borrow().addr_hwnd;
    unsafe {
        let _ = SetFocus(addr_hwnd);
        SendMessageW(addr_hwnd, EM_SETSEL, WPARAM(0), LPARAM(-1));
    }
    invalidate_chrome(state.borrow().parent_hwnd);
}

fn recompute_layout_and_resize(state: &Rc<RefCell<AppState>>, parent_hwnd: HWND) {
    let mut st = state.borrow_mut();
    let old_scale = st.layout.dpi_scale;
    st.layout = Layout::compute(parent_hwnd, st.sidebar_expanded);
    if (st.layout.dpi_scale - old_scale).abs() > 0.01 {
        ui::update_address_bar_font(st.addr_hwnd, st.layout.dpi_scale);
        ui::update_folder_rename_font(st.rename_hwnd, st.layout.dpi_scale);
        st.favicons.borrow().clear_scaled_caches();
    }
    ui::position_address_bar(st.addr_hwnd, &st.layout.topbar, st.layout.dpi_scale);

    if let Some(fid) = st.renaming_folder {
        st.renaming_folder = None;
        ui::hide_folder_rename_edit(st.rename_hwnd);
        let _ = fid;
    }

    let entry_count = st.tab_store.sidebar_entries().len();
    let max_scroll =
        ui::max_sidebar_scroll(&st.layout.sidebar, st.layout.dpi_scale, entry_count);
    st.sidebar_scroll = st.sidebar_scroll.clamp(0, max_scroll);

    if st.omnibox.is_visible() {
        st.omnibox
            .sync_geometry(parent_hwnd, &st.layout.topbar, st.layout.dpi_scale);
    }

    let bounds = st.layout.content_bounds();
    if let Some(tab) = st.tab_store.active_tab() {
        if let Some(ref wv) = tab.webview {
            engine::set_bounds(wv, bounds);
        }
    }
    drop(st);
    invalidate_chrome(parent_hwnd);
}

// ── Parent Win32 Subclass ──

const SUBCLASS_ID: usize = 1;

fn install_parent_subclass(
    hwnd: HWND,
    state: Rc<RefCell<AppState>>,
    proxy: EventLoopProxy<AppEvent>,
) {
    let hand_cursor = unsafe { LoadCursorW(None, IDC_HAND).unwrap_or_default() };
    let ctx = Box::new(SubclassContext {
        state,
        proxy,
        hand_cursor,
    });
    let raw = Box::into_raw(ctx) as usize;

    unsafe {
        let _ = SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, raw);
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    ref_data: usize,
) -> LRESULT {
    let ctx = &*(ref_data as *const SubclassContext);
    let state = &ctx.state;
    let proxy = &ctx.proxy;

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);

            if let Ok(st) = state.try_borrow() {
                let fav_cache = st.favicons.borrow();
                let active = st.tab_store.active_tab();
                let active_url = active.map(|t| t.url.as_str()).unwrap_or("");

                if st.sidebar_expanded {
                    let entries = st.tab_store.sidebar_entries();
                    let drag_target = st
                        .sidebar_drag
                        .and_then(|d| if d.active { d.target } else { None });
                    ui::paint_sidebar(
                        hdc,
                        &st.layout.sidebar,
                        st.layout.dpi_scale,
                        &st.palette,
                        &entries,
                        st.tab_store.tabs.len(),
                        active_url,
                        st.tab_store.live_count(),
                        st.max_live_tabs,
                        st.sidebar_hover,
                        st.sidebar_scroll,
                        st.renaming_folder,
                        drag_target,
                        &fav_cache,
                    );
                }

                let can_back = active.map(|t| t.can_go_back).unwrap_or(false);
                let can_forward = active.map(|t| t.can_go_forward).unwrap_or(false);
                let is_loading = active.map(|t| t.is_loading).unwrap_or(false);
                let blocked_count = active.map(|t| t.blocked_count).unwrap_or(0);
                let has_wv = active.and_then(|t| t.webview.as_ref()).is_some();
                let sec_icon = active
                    .map(|t| ui::security_icon_for_url(&t.url))
                    .unwrap_or(ui::SecurityIcon::Internal);
                let shields_active = {
                    let policy = st
                        .browser_data
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .shield_policy();
                    if sec_icon == ui::SecurityIcon::Internal {
                        policy.global_enabled
                    } else {
                        let host = adblock::normalize_host(active_url);
                        policy.is_active_for_site(&host)
                    }
                };

                ui::paint_topbar(
                    hdc,
                    &st.layout.topbar,
                    st.layout.dpi_scale,
                    &st.palette,
                    st.sidebar_expanded,
                    can_back,
                    can_forward,
                    is_loading,
                    st.active_bookmarked,
                    sec_icon,
                    shields_active,
                    blocked_count,
                    st.topbar_hover,
                    ui::is_address_bar_focused(st.addr_hwnd),
                );

                if !has_wv {
                    ui::paint_content_fallback(hdc, &st.layout.content, &st.palette);
                }
            }

            let _ = EndPaint(hwnd, &ps);
            return LRESULT(0);
        }

        WM_ERASEBKGND => return LRESULT(1),

        WM_CTLCOLOREDIT => {
            if let Ok(st) = state.try_borrow() {
                if let Some(result) = ui::handle_ctlcoloredit(wparam, &st.palette) {
                    return result;
                }
            }
        }

        WM_COMMAND => {
            let hiword = ((wparam.0 >> 16) & 0xFFFF) as u32;
            let ctrl_id = (wparam.0 & 0xFFFF) as u16;
            if ctrl_id == ui::ADDR_CTRL_ID && hiword == EN_CHANGE && !ui::is_setting_address_text()
            {
                let _ = proxy.send_event(AppEvent::AddressTextChanged);
            }
        }

        WM_MOVE => {
            if let Ok(st) = state.try_borrow() {
                if st.omnibox.is_visible() {
                    st.omnibox
                        .sync_geometry(hwnd, &st.layout.topbar, st.layout.dpi_scale);
                }
            }
        }

        WM_ACTIVATE => {
            let active_state = (wparam.0 & 0xFFFF) as u32;
            if active_state == WA_INACTIVE {
                if let Ok(mut st) = state.try_borrow_mut() {
                    st.omnibox.hide();
                    if st.sidebar_drag.take().is_some() {
                        let _ = ReleaseCapture();
                        drop(st);
                        invalidate_chrome(hwnd);
                    }
                }
            }
        }

        WM_CAPTURECHANGED => {
            if let Ok(mut st) = state.try_borrow_mut() {
                if st.sidebar_drag.take().is_some() {
                    drop(st);
                    invalidate_chrome(hwnd);
                }
            }
        }

        WM_MOUSEWHEEL => {
            let mut pt = POINT {
                x: (lparam.0 & 0xFFFF) as i16 as i32,
                y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
            };
            let _ = ScreenToClient(hwnd, &mut pt);
            let delta = ((wparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Ok(mut st) = state.try_borrow_mut() {
                if st.sidebar_expanded && ui::point_in_rect(pt.x, pt.y, &st.layout.sidebar) {
                    if st.renaming_folder.take().is_some() {
                        ui::hide_folder_rename_edit(st.rename_hwnd);
                    }
                    let entries = st.tab_store.sidebar_entries();
                    let max_scroll = ui::max_sidebar_scroll(
                        &st.layout.sidebar,
                        st.layout.dpi_scale,
                        entries.len(),
                    );
                    let step = ui::scale_i32(38, st.layout.dpi_scale);
                    let steps = -(delta as f64 / WHEEL_DELTA as f64);
                    let new_scroll =
                        (st.sidebar_scroll + (steps * step as f64).round() as i32)
                            .clamp(0, max_scroll);
                    if new_scroll != st.sidebar_scroll {
                        let layout_sidebar = st.layout.sidebar;
                        let dpi_scale = st.layout.dpi_scale;
                        drop(entries);
                        st.sidebar_scroll = new_scroll;
                        let entries_after = st.tab_store.sidebar_entries();
                        let hover_hit = ui::hit_test_sidebar(
                            &layout_sidebar,
                            dpi_scale,
                            &entries_after,
                            new_scroll,
                            pt.x,
                            pt.y,
                        );
                        drop(entries_after);
                        st.sidebar_hover = hover_hit;
                        drop(st);
                        invalidate_chrome(hwnd);
                    }
                    return LRESULT(0);
                }
            }
        }

        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Ok(mut st) = state.try_borrow_mut() {
                st.omnibox.hide();

                if let Some(btn) = ui::hit_test_topbar(&st.layout.topbar, st.layout.dpi_scale, x, y)
                {
                    match btn {
                        TopbarButton::ToggleSidebar => {
                            let _ = proxy.send_event(AppEvent::ToggleSidebar);
                        }
                        TopbarButton::Back => {
                            let _ = proxy.send_event(AppEvent::GoBack);
                        }
                        TopbarButton::Forward => {
                            let _ = proxy.send_event(AppEvent::GoForward);
                        }
                        TopbarButton::ReloadOrStop => {
                            let _ = proxy.send_event(AppEvent::ReloadOrStop);
                        }
                        TopbarButton::Shield => {
                            let btns = ui::topbar_button_rects(&st.layout.topbar, st.layout.dpi_scale);
                            let _ = proxy.send_event(AppEvent::OpenShieldsMenu {
                                client_x: btns.shield.left,
                                client_y: btns.shield.bottom + 4,
                            });
                        }
                        TopbarButton::BookmarkStar => {
                            let _ = proxy.send_event(AppEvent::ToggleBookmark);
                        }
                        TopbarButton::NewTab => {
                            let _ = proxy.send_event(AppEvent::NewTab(None));
                        }
                    }
                    return LRESULT(0);
                }

                if st.sidebar_expanded {
                    let hit = {
                        let entries = st.tab_store.sidebar_entries();
                        ui::hit_test_sidebar(
                            &st.layout.sidebar,
                            st.layout.dpi_scale,
                            &entries,
                            st.sidebar_scroll,
                            x,
                            y,
                        )
                    };
                    match hit {
                        SidebarHit::CloseTab(_, id) => {
                            let _ = proxy.send_event(AppEvent::CloseTab(Some(id)));
                            return LRESULT(0);
                        }
                        SidebarHit::Tab(_, id) => {
                            st.sidebar_drag = Some(DragState {
                                tab_id: id,
                                start_x: x,
                                start_y: y,
                                active: false,
                                target: None,
                            });
                            SetCapture(hwnd);
                            return LRESULT(0);
                        }
                        SidebarHit::FolderToggle(_, fid) => {
                            let _ = proxy.send_event(AppEvent::ToggleFolderCollapsed(fid));
                            return LRESULT(0);
                        }
                        SidebarHit::FolderAddTab(_, fid) => {
                            let _ = proxy.send_event(AppEvent::NewTabInFolder(fid));
                            return LRESULT(0);
                        }
                        SidebarHit::FolderMenu(_, fid) => {
                            let _ = proxy.send_event(AppEvent::OpenFolderContextMenu {
                                fid,
                                client_x: x,
                                client_y: y,
                            });
                            return LRESULT(0);
                        }
                        SidebarHit::NewTab => {
                            let _ = proxy.send_event(AppEvent::NewTab(None));
                            return LRESULT(0);
                        }
                        SidebarHit::NewFolder => {
                            let _ = proxy.send_event(AppEvent::NewFolder);
                            return LRESULT(0);
                        }
                        SidebarHit::OpenHistory => {
                            let _ =
                                proxy.send_event(AppEvent::OpenInternalPage(engine::HISTORY_URL));
                            return LRESULT(0);
                        }
                        SidebarHit::OpenBookmarks => {
                            let _ =
                                proxy.send_event(AppEvent::OpenInternalPage(engine::BOOKMARKS_URL));
                            return LRESULT(0);
                        }
                        SidebarHit::OpenDownloads => {
                            let _ =
                                proxy.send_event(AppEvent::OpenInternalPage(engine::DOWNLOADS_URL));
                            return LRESULT(0);
                        }
                        SidebarHit::OpenSettings => {
                            let _ =
                                proxy.send_event(AppEvent::OpenInternalPage(engine::SETTINGS_URL));
                            return LRESULT(0);
                        }
                        SidebarHit::TrimMemory => {
                            let _ = proxy.send_event(AppEvent::TrimMemory);
                            return LRESULT(0);
                        }
                        SidebarHit::None => {}
                    }
                }
            }
        }

        WM_LBUTTONUP => {
            if let Ok(mut st) = state.try_borrow_mut() {
                if let Some(drag) = st.sidebar_drag.take() {
                    let _ = ReleaseCapture();
                    if !drag.active {
                        let _ = proxy.send_event(AppEvent::SwitchTab(drag.tab_id));
                    } else if let Some(target) = drag.target {
                        match target {
                            DragDropTarget::Folder(fid) => {
                                let _ = proxy
                                    .send_event(AppEvent::MoveTabToFolder(drag.tab_id, Some(fid)));
                            }
                            DragDropTarget::Ungrouped => {
                                let _ = proxy
                                    .send_event(AppEvent::MoveTabToFolder(drag.tab_id, None));
                            }
                        }
                    }
                    drop(st);
                    invalidate_chrome(hwnd);
                    return LRESULT(0);
                }
            }
        }

        WM_MBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Ok(st) = state.try_borrow() {
                if st.sidebar_expanded {
                    let entries = st.tab_store.sidebar_entries();
                    match ui::hit_test_sidebar(
                        &st.layout.sidebar,
                        st.layout.dpi_scale,
                        &entries,
                        st.sidebar_scroll,
                        x,
                        y,
                    ) {
                        SidebarHit::Tab(_, id) | SidebarHit::CloseTab(_, id) => {
                            let _ = proxy.send_event(AppEvent::CloseTab(Some(id)));
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
            }
        }

        WM_RBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Ok(st) = state.try_borrow() {
                if let Some(TopbarButton::Shield) =
                    ui::hit_test_topbar(&st.layout.topbar, st.layout.dpi_scale, x, y)
                {
                    let _ = proxy.send_event(AppEvent::OpenShieldsMenu {
                        client_x: x,
                        client_y: y,
                    });
                    return LRESULT(0);
                }

                if st.sidebar_expanded {
                    let entries = st.tab_store.sidebar_entries();
                    match ui::hit_test_sidebar(
                        &st.layout.sidebar,
                        st.layout.dpi_scale,
                        &entries,
                        st.sidebar_scroll,
                        x,
                        y,
                    ) {
                        SidebarHit::Tab(_, id) => {
                            let _ = proxy.send_event(AppEvent::OpenTabContextMenu {
                                tab_id: id,
                                client_x: x,
                                client_y: y,
                            });
                            return LRESULT(0);
                        }
                        SidebarHit::FolderToggle(_, fid)
                        | SidebarHit::FolderAddTab(_, fid)
                        | SidebarHit::FolderMenu(_, fid) => {
                            let _ = proxy.send_event(AppEvent::OpenFolderContextMenu {
                                fid,
                                client_x: x,
                                client_y: y,
                            });
                            return LRESULT(0);
                        }
                        _ => {}
                    }
                }
            }
        }

        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;

            if let Ok(mut st) = state.try_borrow_mut() {
                if !st.tracking_mouse {
                    let mut tme = TRACKMOUSEEVENT {
                        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    if TrackMouseEvent(&mut tme).is_ok() {
                        st.tracking_mouse = true;
                    }
                }

                if let Some(mut drag) = st.sidebar_drag {
                    let dx = (x - drag.start_x).abs();
                    let dy = (y - drag.start_y).abs();
                    let mut changed = false;
                    if !drag.active && (dx > 6 || dy > 6) {
                        drag.active = true;
                        changed = true;
                    }
                    if drag.active {
                        let new_target = {
                            let entries = st.tab_store.sidebar_entries();
                            ui::hit_test_sidebar_drop_target(
                                &st.layout.sidebar,
                                st.layout.dpi_scale,
                                &entries,
                                st.sidebar_scroll,
                                x,
                                y,
                            )
                        };
                        if new_target != drag.target {
                            drag.target = new_target;
                            changed = true;
                        }
                    }
                    st.sidebar_drag = Some(drag);
                    if changed {
                        drop(st);
                        invalidate_chrome(hwnd);
                    }
                    return LRESULT(0);
                }

                let new_topbar =
                    ui::hit_test_topbar(&st.layout.topbar, st.layout.dpi_scale, x, y);
                let new_sidebar = if st.sidebar_expanded {
                    let entries = st.tab_store.sidebar_entries();
                    ui::hit_test_sidebar(
                        &st.layout.sidebar,
                        st.layout.dpi_scale,
                        &entries,
                        st.sidebar_scroll,
                        x,
                        y,
                    )
                } else {
                    SidebarHit::None
                };

                if st.topbar_hover != new_topbar || st.sidebar_hover != new_sidebar {
                    st.topbar_hover = new_topbar;
                    st.sidebar_hover = new_sidebar;
                    drop(st);
                    invalidate_chrome(hwnd);
                }
            }
        }

        WM_MOUSELEAVE => {
            if let Ok(mut st) = state.try_borrow_mut() {
                st.tracking_mouse = false;
                if st.topbar_hover.is_some() || st.sidebar_hover != SidebarHit::None {
                    st.topbar_hover = None;
                    st.sidebar_hover = SidebarHit::None;
                    drop(st);
                    invalidate_chrome(hwnd);
                }
            }
        }

        WM_SETCURSOR => {
            let target_hwnd = HWND(wparam.0 as _);
            let hit_code = (lparam.0 & 0xFFFF) as u32;
            if target_hwnd == hwnd && hit_code == HTCLIENT {
                if let Ok(st) = state.try_borrow() {
                    if st.topbar_hover.is_some()
                        || st.sidebar_hover != SidebarHit::None
                        || st.sidebar_drag.is_some_and(|d| d.active)
                    {
                        SetCursor(ctx.hand_cursor);
                        return LRESULT(1);
                    }
                }
            }
        }

        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
            drop(Box::from_raw(ref_data as *mut SubclassContext));
        }

        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

fn invalidate_chrome(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(hwnd, None, false);
    }
}

fn set_window_title(hwnd: HWND, page_title: &str) {
    let full = if page_title.is_empty() || page_title == "Kurowser" {
        "Kurowser".to_string()
    } else if page_title.ends_with("— Kurowser") || page_title.ends_with("- Kurowser") {
        page_title.to_string()
    } else {
        format!("{} — Kurowser", page_title)
    };
    let wide_t = ui::wide(&full);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(wide_t.as_ptr()));
    }
}
