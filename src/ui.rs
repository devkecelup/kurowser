// ─── ui.rs ─── Native Win32 Chrome: Poppins font, Favicons, Omnibox Popup & Dynamic Themes ───
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Graphics::Dwm::*;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{EM_GETSEL, EM_REPLACESEL, EM_SETSEL, MARGINS, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::engine::{AcceleratorAction, PixelRect};
use crate::omnibox::{Suggestion, SuggestionKind};
use crate::storage::{Rgb, ThemePalette};
use crate::tabs::{FolderId, SidebarEntry, TabId};

// ── Layout constants (logical px at 96 DPI) ──
pub const TOPBAR_HEIGHT: i32 = 36;
pub const SIDEBAR_WIDTH: i32 = 280;
pub const COLLAPSED_RAIL_WIDTH: i32 = 72;
pub const FRAME_PAD: i32 = 8;
pub const FRAME_CORNER_D: i32 = 16;
pub const TAB_ITEM_HEIGHT: i32 = 34;
pub const OMNIBOX_ROW_H: i32 = 46;

#[derive(Clone, Copy, Debug)]
pub struct NavVisualState {
    pub can_back: bool,
    pub can_forward: bool,
    pub is_loading: bool,
    pub is_bookmarked: bool,
    pub sec_icon: SecurityIcon,
    pub shields_active: bool,
    pub blocked_count: u64,
    pub hover: Option<TopbarButton>,
    pub addr_focused: bool,
}

impl NavVisualState {
    pub fn has_shield_badge(&self) -> bool {
        self.blocked_count > 0
            && self.shields_active
            && self.sec_icon != SecurityIcon::Internal
    }
}

const FONT_REGULAR: &[u8] = include_bytes!("../assets/fonts/Poppins-Regular.ttf");
const FONT_MEDIUM: &[u8] = include_bytes!("../assets/fonts/Poppins-Medium.ttf");
const FONT_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/Poppins-SemiBold.ttf");

static SETTING_ADDR_TEXT: AtomicBool = AtomicBool::new(false);

pub fn is_setting_address_text() -> bool {
    SETTING_ADDR_TEXT.load(Ordering::Relaxed)
}

// ── Embedded Poppins Font Loader ──

pub struct FontGuard(Vec<HANDLE>);

impl Drop for FontGuard {
    fn drop(&mut self) {
        unsafe {
            for h in self.0.drain(..) {
                if !h.is_invalid() {
                    let _ = RemoveFontMemResourceEx(h);
                }
            }
        }
    }
}

pub fn load_bundled_poppins() -> FontGuard {
    let mut handles = Vec::new();
    for data in [FONT_REGULAR, FONT_MEDIUM, FONT_SEMIBOLD] {
        let mut count = 0u32;
        let h = unsafe {
            AddFontMemResourceEx(
                data.as_ptr() as *const _,
                data.len() as u32,
                None,
                &mut count,
            )
        };
        if !h.is_invalid() {
            handles.push(h);
        }
    }
    FontGuard(handles)
}

// ── Public UI Types ──

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopbarButton {
    WinMinimize,
    WinMaximize,
    WinClose,
    ToggleSidebar,
    RailSearch,
    Back,
    Forward,
    ReloadOrStop,
    Shield,
    BookmarkStar,
    NewTab,
    RailSettings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DragDropTarget {
    Folder(FolderId),
    Ungrouped,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarHit {
    None,
    Tab(usize, TabId),
    CloseTab(usize, TabId),
    FolderToggle(usize, FolderId),
    FolderAddTab(usize, FolderId),
    FolderMenu(usize, FolderId),
    NewTab,
    NewFolder,
    OpenHistory,
    OpenBookmarks,
    OpenDownloads,
    OpenSettings,
    TrimMemory,
}

pub enum FolderRenameEvent {
    Commit(String),
    Cancel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OmniboxNavKey {
    Up,
    Down,
}

pub enum AddrBarEvent {
    Submit(String),
    Escape,
    OmniboxKey(OmniboxNavKey),
    FocusChanged(bool),
    Shortcut(AcceleratorAction),
    Repaint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityIcon {
    Internal,
    Secure,
    Plain,
}

pub fn security_icon_for_url(url: &str) -> SecurityIcon {
    if url.starts_with("kurowser://") {
        SecurityIcon::Internal
    } else if url.starts_with("https://") {
        SecurityIcon::Secure
    } else {
        SecurityIcon::Plain
    }
}

pub fn rgb(c: Rgb) -> COLORREF {
    COLORREF((c.0 as u32) | ((c.1 as u32) << 8) | ((c.2 as u32) << 16))
}

pub fn scale_i32(v: i32, scale: f64) -> i32 {
    ((v as f64) * scale).round() as i32
}

// ── Crisp Vector Logo Rasterizer, Premultiplied Alpha DIB & Exact-DPI Favicon Cache ──

pub struct DibBitmap {
    pub hbmp: HBITMAP,
    pub width: i32,
    pub height: i32,
}

impl Drop for DibBitmap {
    fn drop(&mut self) {
        unsafe {
            if !self.hbmp.0.is_null() {
                let _ = DeleteObject(self.hbmp);
            }
        }
    }
}

struct CachedSiteIcon {
    premul_rgba: Vec<f32>,
    src_w: u32,
    src_h: u32,
    by_size: HashMap<i32, DibBitmap>,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct ShieldCacheKey {
    size: i32,
    mode: u8,
    fill_rgb: [u8; 3],
    stroke_rgb: [u8; 3],
    glyph_rgb: [u8; 3],
    bg_rgb: [u8; 3],
}

pub struct FaviconCache {
    logos: RefCell<HashMap<i32, DibBitmap>>,
    shields: RefCell<HashMap<ShieldCacheKey, DibBitmap>>,
    icons: RefCell<HashMap<String, CachedSiteIcon>>,
    order: Vec<String>,
}

impl FaviconCache {
    pub fn new() -> Self {
        Self {
            logos: RefCell::new(HashMap::new()),
            shields: RefCell::new(HashMap::new()),
            icons: RefCell::new(HashMap::new()),
            order: Vec::new(),
        }
    }

    pub fn clear_scaled_caches(&self) {
        self.logos.borrow_mut().clear();
        self.shields.borrow_mut().clear();
        for icon in self.icons.borrow_mut().values_mut() {
            icon.by_size.clear();
        }
    }

    pub fn insert_png(&mut self, domain: String, png_bytes: &[u8]) -> bool {
        if domain.is_empty() {
            return false;
        }
        let Some((rgba, w, h)) = decode_png_rgba(png_bytes) else {
            return false;
        };
        let premul = rgba_to_premul_f32(&rgba);
        {
            let mut map = self.icons.borrow_mut();
            if !map.contains_key(&domain) {
                while self.order.len() >= 256 {
                    let oldest = self.order.remove(0);
                    map.remove(&oldest);
                }
                self.order.push(domain.clone());
            }
            map.insert(
                domain,
                CachedSiteIcon {
                    premul_rgba: premul,
                    src_w: w,
                    src_h: h,
                    by_size: HashMap::new(),
                },
            );
        }
        true
    }

    pub fn draw_logo(&self, hdc: HDC, x: i32, y: i32, size: i32) {
        if size <= 0 {
            return;
        }
        let mut logos = self.logos.borrow_mut();
        if !logos.contains_key(&size) {
            let premul = render_kurowser_logo_premul(size as u32);
            if let Some(dib) = create_premul_dib_from_f32(&premul, size, size) {
                if logos.len() >= 12 {
                    logos.clear();
                }
                logos.insert(size, dib);
            }
        }
        if let Some(dib) = logos.get(&size) {
            unsafe {
                draw_dib_alpha(hdc, dib, x, y, size, size);
            }
        }
    }

    pub fn draw_shield(
        &self,
        hdc: HDC,
        x: i32,
        y: i32,
        size: i32,
        active: bool,
        is_internal: bool,
        palette: &ThemePalette,
    ) {
        if size <= 0 {
            return;
        }
        let (mode, fill, stroke, glyph) = if is_internal {
            (
                1u8,
                palette.bg_surface,
                palette.text_muted,
                palette.text_muted,
            )
        } else if active {
            (
                2u8,
                palette.accent_soft,
                palette.accent_text,
                palette.accent_text,
            )
        } else {
            (
                0u8,
                palette.bg_surface,
                palette.text_disabled,
                palette.danger_fg,
            )
        };
        let key = ShieldCacheKey {
            size,
            mode,
            fill_rgb: [fill.0, fill.1, fill.2],
            stroke_rgb: [stroke.0, stroke.1, stroke.2],
            glyph_rgb: [glyph.0, glyph.1, glyph.2],
            bg_rgb: [palette.bg_surface.0, palette.bg_surface.1, palette.bg_surface.2],
        };
        let mut shields = self.shields.borrow_mut();
        if !shields.contains_key(&key) {
            let premul = render_shield_premul(
                size as u32,
                mode,
                fill,
                stroke,
                glyph,
                palette.bg_surface,
            );
            if let Some(dib) = create_premul_dib_from_f32(&premul, size, size) {
                if shields.len() >= 24 {
                    shields.clear();
                }
                shields.insert(key, dib);
            }
        }
        if let Some(dib) = shields.get(&key) {
            unsafe {
                draw_dib_alpha(hdc, dib, x, y, size, size);
            }
        }
    }

    pub fn draw_site(&self, hdc: HDC, domain: &str, x: i32, y: i32, size: i32) -> bool {
        if domain.is_empty() || size <= 0 {
            return false;
        }
        let mut icons = self.icons.borrow_mut();
        let Some(entry) = icons.get_mut(domain) else {
            return false;
        };
        if !entry.by_size.contains_key(&size) {
            let resized = resample_premul_f32(
                &entry.premul_rgba,
                entry.src_w,
                entry.src_h,
                size as u32,
                size as u32,
            );
            if let Some(dib) = create_premul_dib_from_f32(&resized, size, size) {
                if entry.by_size.len() >= 4 {
                    entry.by_size.clear();
                }
                entry.by_size.insert(size, dib);
            }
        }
        if let Some(dib) = entry.by_size.get(&size) {
            unsafe {
                draw_dib_alpha(hdc, dib, x, y, size, size);
            }
            return true;
        }
        false
    }
}

pub fn build_window_icon() -> Option<tao::window::Icon> {
    let size = 64u32;
    let premul = render_kurowser_logo_premul(size);
    let mut straight = vec![0u8; (size * size * 4) as usize];
    for (src, dst) in premul.chunks_exact(4).zip(straight.chunks_exact_mut(4)) {
        let a = src[3].clamp(0.0, 1.0);
        if a > 1e-5 {
            dst[0] = (src[0] / a * 255.0).round().clamp(0.0, 255.0) as u8;
            dst[1] = (src[1] / a * 255.0).round().clamp(0.0, 255.0) as u8;
            dst[2] = (src[2] / a * 255.0).round().clamp(0.0, 255.0) as u8;
            dst[3] = (a * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
    tao::window::Icon::from_rgba(straight, size, size).ok()
}

fn sd_rounded_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - (hx - r);
    let qy = (py - cy).abs() - (hy - r);
    let dx = qx.max(0.0);
    let dy = qy.max(0.0);
    (dx * dx + dy * dy).sqrt() + qx.max(qy).min(0.0) - r
}

fn sd_capsule(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32, r: f32) -> f32 {
    let pax = px - ax;
    let pay = py - ay;
    let bax = bx - ax;
    let bay = by - ay;
    let denom = bax * bax + bay * bay;
    let h = if denom > 0.0 {
        ((pax * bax + pay * bay) / denom).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = pax - bax * h;
    let dy = pay - bay * h;
    (dx * dx + dy * dy).sqrt() - r
}

fn render_kurowser_logo_premul(size: u32) -> Vec<f32> {
    let ss: u32 = if size <= 36 { 8 } else { 4 };
    let scale = 64.0_f32 / (size as f32);
    let inv_samples = 1.0_f32 / ((ss * ss) as f32);
    let mut out = vec![0.0_f32; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let mut ar = 0.0_f32;
            let mut ag = 0.0_f32;
            let mut ab = 0.0_f32;
            let mut aa = 0.0_f32;

            for sy in 0..ss {
                let py = (y as f32 + (sy as f32 + 0.5) / (ss as f32)) * scale;
                for sx in 0..ss {
                    let px = (x as f32 + (sx as f32 + 0.5) / (ss as f32)) * scale;
                    let d_box = sd_rounded_box(px, py, 32.0, 32.0, 29.5, 29.5, 15.5);
                    if d_box > 0.0 {
                        continue;
                    }

                    let t = (((px - 2.5) + (py - 2.5)) / (2.0 * 59.0)).clamp(0.0, 1.0);
                    // Deep Petrol Teal architectural shading: #14857c -> #0f766e -> #0b5d56
                    let (mut r, mut g, mut b) = if t < 0.5 {
                        let u = t * 2.0;
                        (
                            (20.0 + (15.0 - 20.0) * u) / 255.0,
                            (133.0 + (118.0 - 133.0) * u) / 255.0,
                            (124.0 + (110.0 - 124.0) * u) / 255.0,
                        )
                    } else {
                        let u = (t - 0.5) * 2.0;
                        (
                            (15.0 + (11.0 - 15.0) * u) / 255.0,
                            (118.0 + (93.0 - 118.0) * u) / 255.0,
                            (110.0 + (86.0 - 110.0) * u) / 255.0,
                        )
                    };

                    if d_box > -1.8 && py < 30.0 {
                        let rim = ((1.0 - (d_box + 0.8).abs()) * (1.0 - (py - 2.5) / 28.0))
                            .clamp(0.0, 1.0)
                            * 0.22;
                        r += (235.0 / 255.0 - r) * rim;
                        g += (250.0 / 255.0 - g) * rim;
                        b += (248.0 / 255.0 - b) * rim;
                    }

                    let d_k_shad = sd_capsule(px, py - 1.4, 20.5, 18.0, 20.5, 46.0, 3.8)
                        .min(sd_capsule(px, py - 1.4, 24.0, 34.0, 42.5, 18.0, 3.7))
                        .min(sd_capsule(px, py - 1.4, 29.8, 29.0, 43.2, 46.0, 3.7));
                    if d_k_shad < 0.0 {
                        r += (4.0 / 255.0 - r) * 0.18;
                        g += (34.0 / 255.0 - g) * 0.18;
                        b += (32.0 / 255.0 - b) * 0.18;
                    }

                    let d_k = sd_capsule(px, py, 20.5, 18.0, 20.5, 46.0, 3.8)
                        .min(sd_capsule(px, py, 24.0, 34.0, 42.5, 18.0, 3.7))
                        .min(sd_capsule(px, py, 29.8, 29.0, 43.2, 46.0, 3.7));
                    if d_k < 0.0 {
                        r = 1.0;
                        g = 1.0;
                        b = 1.0;
                    }

                    ar += r;
                    ag += g;
                    ab += b;
                    aa += 1.0;
                }
            }

            let idx = ((y * size + x) * 4) as usize;
            out[idx] = ar * inv_samples;
            out[idx + 1] = ag * inv_samples;
            out[idx + 2] = ab * inv_samples;
            out[idx + 3] = aa * inv_samples;
        }
    }
    out
}

fn sd_polygon(px: f32, py: f32, v: &[(f32, f32)]) -> f32 {
    let n = v.len();
    if n < 3 {
        return 1e6;
    }
    let mut d = (px - v[0].0) * (px - v[0].0) + (py - v[0].1) * (py - v[0].1);
    let mut s = 1.0_f32;
    let mut j = n - 1;
    for i in 0..n {
        let ex = v[j].0 - v[i].0;
        let ey = v[j].1 - v[i].1;
        let wx = px - v[i].0;
        let wy = py - v[i].1;
        let denom = ex * ex + ey * ey;
        let t = if denom > 0.0 {
            ((wx * ex + wy * ey) / denom).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let bx = wx - ex * t;
        let by = wy - ey * t;
        d = d.min(bx * bx + by * by);
        let c0 = py >= v[i].1;
        let c1 = py < v[j].1;
        let c2 = ex * wy > ey * wx;
        if (c0 && c1 && c2) || (!c0 && !c1 && !c2) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

fn cubic_bezier_pt(p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let tt = t * t;
    let uu = u * u;
    let uuu = uu * u;
    let ttt = tt * t;
    (
        uuu * p0.0 + 3.0 * uu * t * p1.0 + 3.0 * u * tt * p2.0 + ttt * p3.0,
        uuu * p0.1 + 3.0 * uu * t * p1.1 + 3.0 * u * tt * p2.1 + ttt * p3.1,
    )
}

fn build_shield_crest_polygon() -> Vec<(f32, f32)> {
    let mut pts = Vec::with_capacity(34);
    pts.push((32.0, 6.5));
    for i in 1..=6 {
        let t = i as f32 / 6.0;
        pts.push(cubic_bezier_pt(
            (32.0, 6.5),
            (42.5, 6.5),
            (48.8, 10.6),
            (52.5, 13.5),
            t,
        ));
    }
    for i in 1..=9 {
        let t = i as f32 / 9.0;
        pts.push(cubic_bezier_pt(
            (52.5, 29.0),
            (52.5, 42.8),
            (43.4, 52.6),
            (32.0, 57.5),
            t,
        ));
    }
    for i in 1..=9 {
        let t = i as f32 / 9.0;
        pts.push(cubic_bezier_pt(
            (32.0, 57.5),
            (20.6, 52.6),
            (11.5, 42.8),
            (11.5, 29.0),
            t,
        ));
    }
    for i in 1..=5 {
        let t = i as f32 / 6.0;
        pts.push(cubic_bezier_pt(
            (11.5, 13.5),
            (15.2, 10.6),
            (21.5, 6.5),
            (32.0, 6.5),
            t,
        ));
    }
    pts
}

fn render_shield_premul(
    size: u32,
    mode: u8,
    fill: Rgb,
    stroke: Rgb,
    glyph: Rgb,
    _bg: Rgb,
) -> Vec<f32> {
    let ss: u32 = if size <= 36 { 8 } else { 4 };
    let scale = 64.0_f32 / (size as f32);
    let inv_samples = 1.0_f32 / ((ss * ss) as f32);
    let crest = build_shield_crest_polygon();
    let mut out = vec![0.0_f32; (size * size * 4) as usize];

    let fill_f = (fill.0 as f32 / 255.0, fill.1 as f32 / 255.0, fill.2 as f32 / 255.0);
    let stroke_f = (
        stroke.0 as f32 / 255.0,
        stroke.1 as f32 / 255.0,
        stroke.2 as f32 / 255.0,
    );
    let glyph_f = (glyph.0 as f32 / 255.0, glyph.1 as f32 / 255.0, glyph.2 as f32 / 255.0);

    for y in 0..size {
        for x in 0..size {
            let mut ar = 0.0_f32;
            let mut ag = 0.0_f32;
            let mut ab = 0.0_f32;
            let mut aa = 0.0_f32;

            for sy in 0..ss {
                let py = (y as f32 + (sy as f32 + 0.5) / (ss as f32)) * scale;
                for sx in 0..ss {
                    let px = (x as f32 + (sx as f32 + 0.5) / (ss as f32)) * scale;
                    let d_crest = sd_polygon(px, py, &crest);
                    let d_slash = sd_capsule(px, py, 14.0, 50.0, 50.0, 14.0, 2.5);
                    let d_slash_rim = sd_capsule(px, py, 14.0, 50.0, 50.0, 14.0, 5.0);

                    if mode == 0 {
                        if d_slash < 0.0 {
                            ar += glyph_f.0;
                            ag += glyph_f.1;
                            ab += glyph_f.2;
                            aa += 1.0;
                            continue;
                        }
                        if d_slash_rim < 0.0 {
                            continue;
                        }
                        if d_crest <= 0.0 {
                            if d_crest >= -3.6 {
                                ar += stroke_f.0;
                                ag += stroke_f.1;
                                ab += stroke_f.2;
                                aa += 1.0;
                            } else {
                                ar += fill_f.0 * 0.25;
                                ag += fill_f.1 * 0.25;
                                ab += fill_f.2 * 0.25;
                                aa += 0.25;
                            }
                        }
                        continue;
                    }

                    if d_crest > 0.0 {
                        continue;
                    }

                    let d_check = sd_capsule(px, py, 22.8, 32.2, 29.4, 38.8, 2.5)
                        .min(sd_capsule(px, py, 29.4, 38.8, 42.6, 24.4, 2.5));

                    if d_check < 0.0 {
                        ar += glyph_f.0;
                        ag += glyph_f.1;
                        ab += glyph_f.2;
                        aa += 1.0;
                    } else if d_crest >= -3.6 {
                        ar += stroke_f.0;
                        ag += stroke_f.1;
                        ab += stroke_f.2;
                        aa += 1.0;
                    } else if mode == 2 {
                        let sheen = ((32.0 - py) / 32.0).clamp(0.0, 0.22);
                        let rr = (fill_f.0 + (stroke_f.0 - fill_f.0) * sheen).clamp(0.0, 1.0);
                        let gg = (fill_f.1 + (stroke_f.1 - fill_f.1) * sheen).clamp(0.0, 1.0);
                        let bb = (fill_f.2 + (stroke_f.2 - fill_f.2) * sheen).clamp(0.0, 1.0);
                        ar += rr;
                        ag += gg;
                        ab += bb;
                        aa += 1.0;
                    } else {
                        let a = 0.40_f32;
                        ar += fill_f.0 * a;
                        ag += fill_f.1 * a;
                        ab += fill_f.2 * a;
                        aa += a;
                    }
                }
            }

            let idx = ((y * size + x) * 4) as usize;
            out[idx] = ar * inv_samples;
            out[idx + 1] = ag * inv_samples;
            out[idx + 2] = ab * inv_samples;
            out[idx + 3] = aa * inv_samples;
        }
    }
    out
}

fn decode_png_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    if info.width == 0 || info.height == 0 {
        return None;
    }

    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for chunk in data.chunks_exact(3) {
                out.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
            }
            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for chunk in data.chunks_exact(2) {
                out.extend_from_slice(&[chunk[0], chunk[0], chunk[0], chunk[1]]);
            }
            out
        }
        png::ColorType::Grayscale => {
            let mut out = Vec::with_capacity((info.width * info.height * 4) as usize);
            for &g in data {
                out.extend_from_slice(&[g, g, g, 255]);
            }
            out
        }
        png::ColorType::Indexed => return None,
    };

    Some((rgba, info.width, info.height))
}

fn rgba_to_premul_f32(rgba: &[u8]) -> Vec<f32> {
    let mut out = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        let a = px[3] as f32 / 255.0;
        out.push((px[0] as f32 / 255.0) * a);
        out.push((px[1] as f32 / 255.0) * a);
        out.push((px[2] as f32 / 255.0) * a);
        out.push(a);
    }
    out
}

fn resample_premul_f32(src: &[f32], sw: u32, sh: u32, dw: u32, dh: u32) -> Vec<f32> {
    if sw == dw && sh == dh && src.len() >= (sw * sh * 4) as usize {
        return src[..(sw * sh * 4) as usize].to_vec();
    }
    let mut out = vec![0.0_f32; (dw * dh * 4) as usize];
    if sw >= dw && sh >= dh {
        // Exact area-box average downsampler in premultiplied space (crisp, zero dark halos)
        let sx_scale = sw as f32 / dw as f32;
        let sy_scale = sh as f32 / dh as f32;
        for dy in 0..dh {
            let sy0_f = dy as f32 * sy_scale;
            let sy1_f = (dy + 1) as f32 * sy_scale;
            let sy0 = (sy0_f.floor() as u32).min(sh - 1);
            let sy1 = ((sy1_f.ceil() as u32).saturating_sub(1)).min(sh - 1);
            for dx in 0..dw {
                let sx0_f = dx as f32 * sx_scale;
                let sx1_f = (dx + 1) as f32 * sx_scale;
                let sx0 = (sx0_f.floor() as u32).min(sw - 1);
                let sx1 = ((sx1_f.ceil() as u32).saturating_sub(1)).min(sw - 1);

                let mut sum = [0.0_f32; 4];
                let mut total_w = 0.0_f32;
                for sy in sy0..=sy1 {
                    let wy = (sy1_f.min((sy + 1) as f32) - sy0_f.max(sy as f32)).max(0.0);
                    for sx in sx0..=sx1 {
                        let wx = (sx1_f.min((sx + 1) as f32) - sx0_f.max(sx as f32)).max(0.0);
                        let w = wx * wy;
                        if w > 0.0 {
                            let idx = ((sy * sw + sx) * 4) as usize;
                            for c in 0..4 {
                                sum[c] += src[idx + c] * w;
                            }
                            total_w += w;
                        }
                    }
                }
                let out_i = ((dy * dw + dx) * 4) as usize;
                if total_w > 0.0 {
                    for c in 0..4 {
                        out[out_i + c] = (sum[c] / total_w).clamp(0.0, 1.0);
                    }
                }
            }
        }
        return out;
    }

    // Premultiplied bilinear upsampler
    for dy in 0..dh {
        let sy_f = ((dy as f32 + 0.5) * sh as f32 / dh as f32) - 0.5;
        let sy0 = (sy_f.floor() as i32).clamp(0, sh as i32 - 1) as u32;
        let sy1 = (sy0 + 1).min(sh - 1);
        let fy = (sy_f - sy_f.floor()).clamp(0.0, 1.0);

        for dx in 0..dw {
            let sx_f = ((dx as f32 + 0.5) * sw as f32 / dw as f32) - 0.5;
            let sx0 = (sx_f.floor() as i32).clamp(0, sw as i32 - 1) as u32;
            let sx1 = (sx0 + 1).min(sw - 1);
            let fx = (sx_f - sx_f.floor()).clamp(0.0, 1.0);

            let i00 = ((sy0 * sw + sx0) * 4) as usize;
            let i10 = ((sy0 * sw + sx1) * 4) as usize;
            let i01 = ((sy1 * sw + sx0) * 4) as usize;
            let i11 = ((sy1 * sw + sx1) * 4) as usize;
            let out_i = ((dy * dw + dx) * 4) as usize;
            for c in 0..4 {
                let top = src[i00 + c] * (1.0 - fx) + src[i10 + c] * fx;
                let bot = src[i01 + c] * (1.0 - fx) + src[i11 + c] * fx;
                out[out_i + c] = (top * (1.0 - fy) + bot * fy).clamp(0.0, 1.0);
            }
        }
    }
    out
}

fn create_premul_dib_from_f32(premul: &[f32], dst_w: i32, dst_h: i32) -> Option<DibBitmap> {
    if dst_w <= 0 || dst_h <= 0 || premul.len() < (dst_w * dst_h * 4) as usize {
        return None;
    }
    let mut bmi = BITMAPINFO::default();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: dst_w,
        biHeight: -dst_h, // top-down DIB
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };

    let mut bits_ptr: *mut core::ffi::c_void = std::ptr::null_mut();
    let hbmp = unsafe { CreateDIBSection(None, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0).ok()? };
    if bits_ptr.is_null() {
        unsafe {
            let _ = DeleteObject(hbmp);
        }
        return None;
    }

    let pixel_count = (dst_w * dst_h) as usize;
    let dst_slice = unsafe { std::slice::from_raw_parts_mut(bits_ptr as *mut u8, pixel_count * 4) };
    for (src_px, dst_px) in premul.chunks_exact(4).zip(dst_slice.chunks_exact_mut(4)) {
        let a = (src_px[3] * 255.0).round().clamp(0.0, 255.0) as u8;
        let max_rgb = a as f32;
        dst_px[0] = (src_px[2] * 255.0).round().clamp(0.0, max_rgb) as u8; // B premul
        dst_px[1] = (src_px[1] * 255.0).round().clamp(0.0, max_rgb) as u8; // G premul
        dst_px[2] = (src_px[0] * 255.0).round().clamp(0.0, max_rgb) as u8; // R premul
        dst_px[3] = a;
    }

    Some(DibBitmap {
        hbmp,
        width: dst_w,
        height: dst_h,
    })
}

unsafe fn draw_dib_alpha(hdc_dest: HDC, dib: &DibBitmap, x: i32, y: i32, w: i32, h: i32) {
    let src_dc = CreateCompatibleDC(hdc_dest);
    let old = SelectObject(src_dc, dib.hbmp);
    let bf = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let _ = AlphaBlend(hdc_dest, x, y, w, h, src_dc, 0, 0, dib.width, dib.height, bf);
    SelectObject(src_dc, old);
    let _ = DeleteDC(src_dc);
}

// ── Layout Computation ──

#[derive(Clone, Debug)]
pub struct Layout {
    pub window: RECT,
    pub sidebar: RECT,
    pub topbar: RECT,
    pub content: RECT,
    pub sidebar_expanded: bool,
    pub is_maximized: bool,
    pub dpi_scale: f64,
}

impl Layout {
    pub fn compute_with_metrics(
        w: i32,
        h: i32,
        scale: f64,
        is_maximized: bool,
        sidebar_expanded: bool,
    ) -> Self {
        let w = w.max(1);
        let h = h.max(1);
        let scale = if scale > 0.0 { scale } else { 1.0 };

        let sb_w = if sidebar_expanded {
            scale_i32(SIDEBAR_WIDTH, scale).min((w * 49) / 100)
        } else {
            scale_i32(COLLAPSED_RAIL_WIDTH, scale).min((w * 35) / 100)
        };
        let tb_h = if h > scale_i32(120, scale) {
            scale_i32(TOPBAR_HEIGHT, scale)
        } else {
            scale_i32(28, scale)
        };

        let pad = scale_i32(FRAME_PAD, scale);
        let pad_x = if (w - sb_w) > pad * 4 { pad } else { 0 };
        let pad_bot = if (h - tb_h) > pad * 4 { pad } else { 0 };

        Layout {
            window: RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            },
            sidebar: RECT {
                left: 0,
                top: 0,
                right: sb_w,
                bottom: h,
            },
            topbar: RECT {
                left: sb_w,
                top: 0,
                right: w,
                bottom: tb_h,
            },
            content: RECT {
                left: sb_w + pad_x,
                top: tb_h,
                right: (w - pad_x).max(sb_w + pad_x + 1),
                bottom: (h - pad_bot).max(tb_h + 1),
            },
            sidebar_expanded,
            is_maximized,
            dpi_scale: scale,
        }
    }

    pub fn compute(hwnd: HWND, sidebar_expanded: bool) -> Self {
        let mut rc = RECT::default();
        let is_maximized = unsafe {
            let _ = GetClientRect(hwnd, &mut rc);
            IsZoomed(hwnd).as_bool()
        };
        let w = (rc.right - rc.left).max(1);
        let h = (rc.bottom - rc.top).max(1);

        let dpi = unsafe { GetDpiForWindow(hwnd) } as f64;
        let scale = if dpi > 0.0 { dpi / 96.0 } else { 1.0 };

        Self::compute_with_metrics(w, h, scale, is_maximized, sidebar_expanded)
    }

    pub fn content_bounds(&self) -> PixelRect {
        PixelRect {
            x: self.content.left,
            y: self.content.top,
            width: (self.content.right - self.content.left).max(1),
            height: (self.content.bottom - self.content.top).max(1),
            corner_diameter: scale_i32(FRAME_CORNER_D, self.dpi_scale),
        }
    }
}

pub fn paint_frame_backdrop(
    hdc_screen: HDC,
    layout: &Layout,
    palette: &ThemePalette,
    nav: NavVisualState,
    _has_wv: bool,
) {
    let bx = layout.topbar.left;
    let by = layout.topbar.top;
    let bw = layout.topbar.right - bx;
    let bh = layout.window.bottom - by;
    if bw <= 0 || bh <= 0 {
        return;
    }

    unsafe {
        let mem_dc = CreateCompatibleDC(hdc_screen);
        let mem_bmp = CreateCompatibleBitmap(hdc_screen, bw, bh);
        let old_bmp = SelectObject(mem_dc, mem_bmp);

        let local = RECT {
            left: 0,
            top: 0,
            right: bw,
            bottom: bh,
        };
        let bg_br = CreateSolidBrush(rgb(palette.bg_chrome));
        FillRect(mem_dc, &local, bg_br);
        let _ = DeleteObject(bg_br);

        let btns = topbar_button_rects(layout, nav.has_shield_badge());
        let cap_min = RECT {
            left: btns.win_min.left - bx,
            top: btns.win_min.top - by,
            right: btns.win_min.right - bx,
            bottom: btns.win_min.bottom - by,
        };
        let cap_max = RECT {
            left: btns.win_max.left - bx,
            top: btns.win_max.top - by,
            right: btns.win_max.right - bx,
            bottom: btns.win_max.bottom - by,
        };
        let cap_close = RECT {
            left: btns.win_close.left - bx,
            top: btns.win_close.top - by,
            right: btns.win_close.right - bx,
            bottom: btns.win_close.bottom - by,
        };
        draw_caption_button(
            mem_dc,
            &cap_min,
            layout.dpi_scale,
            palette,
            nav.hover == Some(TopbarButton::WinMinimize),
            CaptionKind::Minimize,
        );
        draw_caption_button(
            mem_dc,
            &cap_max,
            layout.dpi_scale,
            palette,
            nav.hover == Some(TopbarButton::WinMaximize),
            if layout.is_maximized {
                CaptionKind::Restore
            } else {
                CaptionKind::Maximize
            },
        );
        draw_caption_button(
            mem_dc,
            &cap_close,
            layout.dpi_scale,
            palette,
            nav.hover == Some(TopbarButton::WinClose),
            CaptionKind::Close,
        );

        let cr = scale_i32(FRAME_CORNER_D, layout.dpi_scale);
        let loc_c = RECT {
            left: layout.content.left - bx - 1,
            top: layout.content.top - by - 1,
            right: layout.content.right - bx + 1,
            bottom: layout.content.bottom - by + 1,
        };
        let app_br = CreateSolidBrush(rgb(palette.bg_app));
        let border_pen = CreatePen(PS_SOLID, 1, rgb(palette.border_subtle));
        let old_b = SelectObject(mem_dc, app_br);
        let old_p = SelectObject(mem_dc, border_pen);
        let _ = RoundRect(
            mem_dc,
            loc_c.left,
            loc_c.top,
            loc_c.right,
            loc_c.bottom,
            cr + 2,
            cr + 2,
        );
        SelectObject(mem_dc, old_b);
        SelectObject(mem_dc, old_p);
        let _ = DeleteObject(app_br);
        let _ = DeleteObject(border_pen);

        let _ = BitBlt(hdc_screen, bx, by, bw, bh, mem_dc, 0, 0, SRCCOPY);
        SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(mem_bmp);
        let _ = DeleteDC(mem_dc);
    }
}

pub fn partition_sidebar_entries<'a>(
    entries: &'a [SidebarEntry<'a>],
) -> (Vec<(usize, &'a crate::tabs::SidebarTabInfo<'a>)>, Vec<(usize, &'a SidebarEntry<'a>)>) {
    let mut pinned = Vec::new();
    let mut tree = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        match entry {
            SidebarEntry::Tab(tab) if tab.pinned && tab.folder_id.is_none() && pinned.len() < 8 => {
                pinned.push((idx, tab));
            }
            _ => tree.push((idx, entry)),
        }
    }
    (pinned, tree)
}

struct SidebarGeometry {
    url_pill: RECT,
    new_btn: RECT,
    new_folder_btn: RECT,
    pinned_tiles: Vec<(usize, TabId, RECT)>,
    pinned_bottom: i32,
    list_top: i32,
    list_bottom: i32,
    dock_rects: [(SidebarHit, &'static str, RECT); 4],
    dock_top: i32,
    footer_top: i32,
    trim_btn: RECT,
    item_h: i32,
    gap: i32,
}

fn sidebar_geometry(
    rect: &RECT,
    scale: f64,
    pinned_tabs: &[(usize, &crate::tabs::SidebarTabInfo<'_>)],
) -> SidebarGeometry {
    let pad = scale_i32(10, scale);
    let left = rect.left + pad;
    let right = rect.right - pad;
    let item_h = scale_i32(TAB_ITEM_HEIGHT, scale);
    let gap = scale_i32(3, scale);

    let url_top = rect.top + scale_i32(42, scale);
    let url_h = scale_i32(36, scale);
    let url_pill = RECT {
        left,
        top: url_top,
        right,
        bottom: url_top + url_h,
    };

    let new_top = url_pill.bottom + scale_i32(8, scale);
    let new_h = scale_i32(32, scale);
    let cta_gap = scale_i32(6, scale);
    let folder_w = scale_i32(84, scale);
    let avail_cta = (right - left).max(scale_i32(120, scale));
    let tab_w = (avail_cta - folder_w - cta_gap).max(scale_i32(72, scale));

    let new_btn = RECT {
        left,
        top: new_top,
        right: left + tab_w,
        bottom: new_top + new_h,
    };
    let new_folder_btn = RECT {
        left: new_btn.right + cta_gap,
        top: new_top,
        right,
        bottom: new_top + new_h,
    };

    // Optional Pinned Favorites strip (up to 4 per row x 2 rows)
    let mut pinned_tiles = Vec::new();
    let mut pinned_bottom = new_btn.bottom + scale_i32(6, scale);
    if !pinned_tabs.is_empty() {
        let cols = 4i32;
        let t_gap = scale_i32(6, scale);
        let tile_w = ((avail_cta - t_gap * (cols - 1)) / cols).max(scale_i32(36, scale));
        let tile_h = scale_i32(38, scale);
        let p_start_y = new_btn.bottom + scale_i32(10, scale);
        let mut max_row_y = p_start_y;
        for (slot, (orig_idx, tab)) in pinned_tabs.iter().take(8).enumerate() {
            let row = (slot as i32) / cols;
            let col = (slot as i32) % cols;
            let tx = left + col * (tile_w + t_gap);
            let ty = p_start_y + row * (tile_h + t_gap);
            let rc = RECT {
                left: tx,
                top: ty,
                right: if col == cols - 1 { right } else { tx + tile_w },
                bottom: ty + tile_h,
            };
            max_row_y = max_row_y.max(rc.bottom);
            pinned_tiles.push((*orig_idx, tab.id, rc));
        }
        pinned_bottom = max_row_y + scale_i32(8, scale);
    }

    let footer_h = scale_i32(34, scale);
    let footer_top = (rect.bottom - footer_h).max(rect.top + scale_i32(180, scale));

    let trim_btn = RECT {
        left: right - scale_i32(90, scale),
        top: footer_top + scale_i32(5, scale),
        right,
        bottom: rect.bottom - scale_i32(5, scale),
    };

    // Single-row 4-icon calm Utility Strip right above footer
    let util_h = scale_i32(44, scale);
    let util_pad_v = scale_i32(5, scale);
    let util_pad_h = scale_i32(8, scale);
    let util_left = rect.left + util_pad_h;
    let util_right = rect.right - util_pad_h;
    let dock_top = footer_top - util_h - util_pad_v * 2;
    let util_gap = scale_i32(2, scale);
    let col_w = ((util_right - util_left) - util_gap * 3).max(40) / 4;
    let u_y = dock_top + util_pad_v;

    let dock_rects = [
        (
            SidebarHit::OpenHistory,
            "History",
            RECT {
                left: util_left,
                top: u_y,
                right: util_left + col_w,
                bottom: u_y + util_h,
            },
        ),
        (
            SidebarHit::OpenBookmarks,
            "Bookmark",
            RECT {
                left: util_left + col_w + util_gap,
                top: u_y,
                right: util_left + col_w * 2 + util_gap,
                bottom: u_y + util_h,
            },
        ),
        (
            SidebarHit::OpenDownloads,
            "Unduhan",
            RECT {
                left: util_left + (col_w + util_gap) * 2,
                top: u_y,
                right: util_left + col_w * 3 + util_gap * 2,
                bottom: u_y + util_h,
            },
        ),
        (
            SidebarHit::OpenSettings,
            "Setelan",
            RECT {
                left: util_left + (col_w + util_gap) * 3,
                top: u_y,
                right: util_right,
                bottom: u_y + util_h,
            },
        ),
    ];

    let list_top = pinned_bottom + scale_i32(4, scale);
    let list_bottom = (dock_top - scale_i32(4, scale)).max(list_top);

    SidebarGeometry {
        url_pill,
        new_btn,
        new_folder_btn,
        pinned_tiles,
        pinned_bottom,
        list_top,
        list_bottom,
        dock_rects,
        dock_top,
        footer_top,
        trim_btn,
        item_h,
        gap,
    }
}

pub fn max_sidebar_scroll(
    rect: &RECT,
    scale: f64,
    entries: &[SidebarEntry<'_>],
) -> i32 {
    let (pinned, tree) = partition_sidebar_entries(entries);
    let g = sidebar_geometry(rect, scale, &pinned);
    let total_h = (tree.len() as i32) * (g.item_h + g.gap);
    let avail = (g.list_bottom - g.list_top).max(0);
    (total_h - avail).max(0)
}

pub fn folder_rename_rect(
    rect: &RECT,
    scale: f64,
    entries: &[SidebarEntry<'_>],
    scroll_offset: i32,
    target_fid: FolderId,
) -> Option<RECT> {
    let (pinned, tree) = partition_sidebar_entries(entries);
    let g = sidebar_geometry(rect, scale, &pinned);
    let max_scroll = max_sidebar_scroll(rect, scale, entries);
    let scroll = scroll_offset.clamp(0, max_scroll);
    for (tree_i, (_, entry)) in tree.iter().enumerate() {
        if let SidebarEntry::FolderHeader { folder_id, .. } = entry {
            if *folder_id == target_fid {
                let y = g.list_top + (tree_i as i32) * (g.item_h + g.gap) - scroll;
                if y + g.item_h <= g.list_top || y >= g.list_bottom {
                    return None;
                }
                let edit_h = scale_i32(22, scale).max(16);
                let ey = y + (g.item_h - edit_h) / 2;
                return Some(RECT {
                    left: rect.left + scale_i32(46, scale),
                    top: ey,
                    right: rect.right - scale_i32(18, scale),
                    bottom: ey + edit_h,
                });
            }
        }
    }
    None
}

// ── Double-Buffered Sidebar Render ──

pub fn paint_sidebar(
    hdc_screen: HDC,
    layout: &Layout,
    palette: &ThemePalette,
    nav: NavVisualState,
    entries: &[SidebarEntry<'_>],
    total_tab_count: usize,
    active_url: &str,
    live_count: usize,
    max_live: usize,
    hover: SidebarHit,
    scroll_offset: i32,
    renaming_folder: Option<FolderId>,
    drag_target: Option<DragDropTarget>,
    favicons: &FaviconCache,
) {
    let rect = &layout.sidebar;
    let scale = layout.dpi_scale;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return;
    }

    let (pinned_tabs, tree_entries) = partition_sidebar_entries(entries);

    unsafe {
        let mem_dc = CreateCompatibleDC(hdc_screen);
        let mem_bmp = CreateCompatibleBitmap(hdc_screen, width, height);
        let old_bmp = SelectObject(mem_dc, mem_bmp);

        let local = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };

        let bg_brush = CreateSolidBrush(rgb(palette.bg_chrome));
        FillRect(mem_dc, &local, bg_brush);
        let _ = DeleteObject(bg_brush);

        SetBkMode(mem_dc, TRANSPARENT);

        let font_title = create_font(
            scale,
            if width < scale_i32(252, scale) { 13 } else { 15 },
            FW_SEMIBOLD.0 as i32,
        );
        let font_tab = create_font(scale, 13, FW_NORMAL.0 as i32);
        let font_tab_active = create_font(scale, 13, FW_MEDIUM.0 as i32);
        let font_dock = create_font(scale, 12, FW_MEDIUM.0 as i32);
        let font_small = create_font(scale, 10, FW_MEDIUM.0 as i32);
        let font_meta = create_font(scale, 11, FW_MEDIUM.0 as i32);
        let old_font = SelectObject(mem_dc, font_title);

        let hdr_btns = topbar_button_rects(layout, nav.has_shield_badge());

        // Sidebar header row: [K] Kurowser brand on left, nav controls right-aligned inside sidebar
        let logo_sz = scale_i32(18, scale);
        let logo_x = scale_i32(10, scale);
        let logo_y = scale_i32(9, scale);
        favicons.draw_logo(mem_dc, logo_x, logo_y, logo_sz);

        let title_left = logo_x + logo_sz + scale_i32(7, scale);
        let title_right = (hdr_btns.toggle.left - scale_i32(6, scale)).max(title_left);
        if title_right > title_left + scale_i32(28, scale) {
            SelectObject(mem_dc, font_title);
            SetTextColor(mem_dc, rgb(palette.text_primary));
            let mut brand = wide("Kurowser");
            let mut brand_rc = RECT {
                left: title_left,
                top: scale_i32(5, scale),
                right: title_right,
                bottom: scale_i32(31, scale),
            };
            DrawTextW(
                mem_dc,
                &mut brand,
                &mut brand_rc,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
            );
        }

        // Nav controls inside sidebar header (right cluster)
        draw_nav_button(
            mem_dc,
            &hdr_btns.toggle,
            scale,
            palette,
            true,
            nav.hover == Some(TopbarButton::ToggleSidebar),
            |dc, cx, cy| {
                draw_sidebar_toggle_icon(dc, cx, cy, scale, true, rgb(palette.accent_text))
            },
        );
        draw_nav_button(
            mem_dc,
            &hdr_btns.back,
            scale,
            palette,
            nav.can_back,
            nav.hover == Some(TopbarButton::Back),
            |dc, cx, cy| {
                let col = if nav.can_back {
                    palette.text_primary
                } else {
                    palette.text_disabled
                };
                draw_arrow_icon(dc, cx, cy, scale, true, rgb(col));
            },
        );
        draw_nav_button(
            mem_dc,
            &hdr_btns.forward,
            scale,
            palette,
            nav.can_forward,
            nav.hover == Some(TopbarButton::Forward),
            |dc, cx, cy| {
                let col = if nav.can_forward {
                    palette.text_primary
                } else {
                    palette.text_disabled
                };
                draw_arrow_icon(dc, cx, cy, scale, false, rgb(col));
            },
        );
        draw_nav_button(
            mem_dc,
            &hdr_btns.reload,
            scale,
            palette,
            true,
            nav.hover == Some(TopbarButton::ReloadOrStop),
            |dc, cx, cy| {
                if nav.is_loading {
                    draw_close_x(
                        dc,
                        cx,
                        cy,
                        scale_i32(5, scale),
                        rgb(palette.danger_fg),
                        scale,
                    );
                } else {
                    draw_reload_icon(dc, cx, cy, scale, rgb(palette.text_primary));
                }
            },
        );

        let g = sidebar_geometry(&local, scale, &pinned_tabs);

        // Integrated full-width URL / Search Pill inside sidebar
        paint_url_pill(mem_dc, &g.url_pill, &hdr_btns, scale, palette, nav, favicons);

        // Split CTA Row: "+ Tab Baru" (primary subtle accent) and "+ Folder" (neutral)
        let new_hover = matches!(hover, SidebarHit::NewTab);
        let (nb_fill, nb_border, nb_txt) = if new_hover {
            (palette.accent_soft, palette.accent, palette.text_primary)
        } else {
            (palette.bg_surface, palette.border, palette.accent_text)
        };
        draw_rounded_fill(
            mem_dc,
            &g.new_btn,
            scale_i32(8, scale),
            rgb(nb_fill),
            rgb(nb_border),
        );
        draw_plus_icon(
            mem_dc,
            g.new_btn.left + scale_i32(15, scale),
            (g.new_btn.top + g.new_btn.bottom) / 2,
            scale_i32(4, scale),
            rgb(nb_txt),
            scale,
        );
        SelectObject(mem_dc, font_tab_active);
        SetTextColor(mem_dc, rgb(nb_txt));
        let mut new_lbl = wide("Tab Baru");
        let mut new_lbl_rc = RECT {
            left: g.new_btn.left + scale_i32(26, scale),
            top: g.new_btn.top,
            right: g.new_btn.right - scale_i32(6, scale),
            bottom: g.new_btn.bottom,
        };
        DrawTextW(
            mem_dc,
            &mut new_lbl,
            &mut new_lbl_rc,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );

        let folder_hover = matches!(hover, SidebarHit::NewFolder);
        let (fb_fill, fb_border, fb_txt) = if folder_hover {
            (palette.bg_hover, palette.accent, palette.text_primary)
        } else {
            (palette.bg_surface, palette.border, palette.text_muted)
        };
        draw_rounded_fill(
            mem_dc,
            &g.new_folder_btn,
            scale_i32(8, scale),
            rgb(fb_fill),
            rgb(fb_border),
        );
        draw_folder_glyph(
            mem_dc,
            g.new_folder_btn.left + scale_i32(11, scale),
            (g.new_folder_btn.top + g.new_folder_btn.bottom) / 2 - scale_i32(6, scale),
            scale_i32(13, scale),
            rgb(if folder_hover { palette.accent_text } else { fb_txt }),
            false,
        );
        SelectObject(mem_dc, font_dock);
        SetTextColor(mem_dc, rgb(fb_txt));
        let mut fld_lbl = wide("Folder");
        let mut fld_lbl_rc = RECT {
            left: g.new_folder_btn.left + scale_i32(28, scale),
            top: g.new_folder_btn.top,
            right: g.new_folder_btn.right - scale_i32(6, scale),
            bottom: g.new_folder_btn.bottom,
        };
        DrawTextW(
            mem_dc,
            &mut fld_lbl,
            &mut fld_lbl_rc,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );

        // Optional Pinned Favorites strip (Arc-style icon tiles)
        if !pinned_tabs.is_empty() {
            for ((orig_idx, tab_id, tile_rc), (_, tab)) in
                g.pinned_tiles.iter().zip(pinned_tabs.iter())
            {
                let is_hov = matches!(hover, SidebarHit::Tab(idx, id) if idx == *orig_idx && id == *tab_id);
                let (fill, border) = if tab.is_active {
                    (palette.bg_active, palette.accent)
                } else if is_hov {
                    (palette.bg_hover, palette.border)
                } else {
                    (palette.bg_surface, palette.border_subtle)
                };
                draw_rounded_fill(
                    mem_dc,
                    tile_rc,
                    scale_i32(8, scale),
                    rgb(fill),
                    rgb(border),
                );
                let ico_sz = scale_i32(18, scale);
                let ix = (tile_rc.left + tile_rc.right - ico_sz) / 2;
                let iy = (tile_rc.top + tile_rc.bottom - ico_sz) / 2;
                draw_site_icon(
                    mem_dc,
                    favicons,
                    palette,
                    tab.domain,
                    false,
                    ix,
                    iy,
                    ico_sz,
                    scale,
                    font_small,
                );
                if tab.is_active {
                    let bar = RECT {
                        left: tile_rc.left + scale_i32(14, scale),
                        top: tile_rc.bottom - scale_i32(3, scale),
                        right: tile_rc.right - scale_i32(14, scale),
                        bottom: tile_rc.bottom - 1,
                    };
                    let abr = CreateSolidBrush(rgb(palette.accent));
                    FillRect(mem_dc, &bar, abr);
                    let _ = DeleteObject(abr);
                }
            }

            let pdiv_br = CreateSolidBrush(rgb(palette.border_subtle));
            let pdiv = RECT {
                left: scale_i32(12, scale),
                top: g.pinned_bottom - scale_i32(2, scale),
                right: width - scale_i32(12, scale),
                bottom: g.pinned_bottom - scale_i32(1, scale),
            };
            FillRect(mem_dc, &pdiv, pdiv_br);
            let _ = DeleteObject(pdiv_br);
        }

        // Clipped scrollable tab & folder list
        let max_scroll = max_sidebar_scroll(&local, scale, entries);
        let scroll = scroll_offset.clamp(0, max_scroll);
        let saved = SaveDC(mem_dc);
        IntersectClipRect(mem_dc, 0, g.list_top, width, g.list_bottom);

        let mut last_item_bottom = g.list_top;

        for (tree_i, (orig_i, entry)) in tree_entries.iter().enumerate() {
            let y = g.list_top + (tree_i as i32) * (g.item_h + g.gap) - scroll;
            last_item_bottom = last_item_bottom.max(y + g.item_h);
            if y + g.item_h < g.list_top || y > g.list_bottom {
                continue;
            }

            match entry {
                SidebarEntry::FolderHeader {
                    folder_id,
                    name,
                    color,
                    collapsed,
                    tab_count,
                    has_active_child,
                } => {
                    let item_rect = RECT {
                        left: scale_i32(10, scale),
                        top: y,
                        right: width - scale_i32(10, scale),
                        bottom: y + g.item_h,
                    };
                    let is_hovered = matches!(
                        hover,
                        SidebarHit::FolderToggle(idx, _)
                            | SidebarHit::FolderAddTab(idx, _)
                            | SidebarHit::FolderMenu(idx, _)
                            if idx == *orig_i
                    );
                    let is_drop = drag_target == Some(DragDropTarget::Folder(*folder_id));
                    let is_renaming = renaming_folder == Some(*folder_id);

                    if is_drop {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(8, scale),
                            rgb(color.mix(palette.bg_surface, 0.72)),
                            rgb(palette.accent),
                        );
                    } else if is_hovered {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(8, scale),
                            rgb(palette.bg_hover),
                            rgb(palette.border_subtle),
                        );
                    } else if *collapsed && *has_active_child {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(8, scale),
                            rgb(palette.bg_surface),
                            rgb(palette.border_subtle),
                        );
                    }

                    let chev_cx = item_rect.left + scale_i32(10, scale);
                    let chev_cy = item_rect.top + g.item_h / 2;
                    draw_chevron(
                        mem_dc,
                        chev_cx,
                        chev_cy,
                        scale,
                        *collapsed,
                        rgb(if is_hovered {
                            palette.text_primary
                        } else {
                            palette.text_muted
                        }),
                    );

                    let fld_sz = scale_i32(15, scale);
                    let fld_x = item_rect.left + scale_i32(18, scale);
                    let fld_y = item_rect.top + (g.item_h - fld_sz) / 2;
                    draw_folder_glyph(mem_dc, fld_x, fld_y, fld_sz, rgb(*color), !*collapsed);

                    if !is_renaming {
                        SelectObject(mem_dc, font_tab_active);
                        let title_col = if is_hovered || !*collapsed {
                            palette.text_primary
                        } else {
                            palette.text_inactive_tab
                        };
                        SetTextColor(mem_dc, rgb(title_col));

                        let right_reserve = if is_hovered {
                            scale_i32(52, scale)
                        } else {
                            scale_i32(28, scale)
                        };
                        let mut text = wide(if name.is_empty() { "Folder" } else { name });
                        let mut text_rect = RECT {
                            left: fld_x + fld_sz + scale_i32(7, scale),
                            top: item_rect.top,
                            right: item_rect.right - right_reserve,
                            bottom: item_rect.bottom,
                        };
                        DrawTextW(
                            mem_dc,
                            &mut text,
                            &mut text_rect,
                            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                        );
                    }

                    if !is_renaming && is_hovered {
                        let add_box = RECT {
                            left: item_rect.right - scale_i32(46, scale),
                            top: item_rect.top + scale_i32(6, scale),
                            right: item_rect.right - scale_i32(26, scale),
                            bottom: item_rect.bottom - scale_i32(6, scale),
                        };
                        let menu_box = RECT {
                            left: item_rect.right - scale_i32(24, scale),
                            top: item_rect.top + scale_i32(6, scale),
                            right: item_rect.right - scale_i32(4, scale),
                            bottom: item_rect.bottom - scale_i32(6, scale),
                        };
                        let add_hov = matches!(hover, SidebarHit::FolderAddTab(idx, _) if idx == *orig_i);
                        let menu_hov = matches!(hover, SidebarHit::FolderMenu(idx, _) if idx == *orig_i);
                        if add_hov {
                            draw_rounded_fill(
                                mem_dc,
                                &add_box,
                                scale_i32(6, scale),
                                rgb(palette.accent_soft),
                                rgb(palette.accent),
                            );
                        }
                        draw_plus_icon(
                            mem_dc,
                            (add_box.left + add_box.right) / 2,
                            (add_box.top + add_box.bottom) / 2,
                            scale_i32(4, scale),
                            rgb(if add_hov {
                                palette.accent_text
                            } else {
                                palette.text_muted
                            }),
                            scale,
                        );
                        if menu_hov {
                            draw_rounded_fill(
                                mem_dc,
                                &menu_box,
                                scale_i32(6, scale),
                                rgb(palette.bg_surface),
                                rgb(palette.border),
                            );
                        }
                        draw_dots_icon(
                            mem_dc,
                            (menu_box.left + menu_box.right) / 2,
                            (menu_box.top + menu_box.bottom) / 2,
                            scale,
                            rgb(if menu_hov {
                                palette.text_primary
                            } else {
                                palette.text_muted
                            }),
                        );
                    } else if !is_renaming && *tab_count > 0 {
                        // Quiet tabular count without boxed pill clutter
                        SelectObject(mem_dc, font_meta);
                        SetTextColor(
                            mem_dc,
                            rgb(if *collapsed && *has_active_child {
                                palette.accent_text
                            } else {
                                palette.text_muted
                            }),
                        );
                        let mut cnt_w = wide(&tab_count.to_string());
                        let mut cnt_rc = RECT {
                            left: item_rect.right - scale_i32(26, scale),
                            top: item_rect.top,
                            right: item_rect.right - scale_i32(8, scale),
                            bottom: item_rect.bottom,
                        };
                        DrawTextW(
                            mem_dc,
                            &mut cnt_w,
                            &mut cnt_rc,
                            DT_RIGHT | DT_SINGLELINE | DT_VCENTER,
                        );
                    }
                }

                SidebarEntry::Tab(tab) => {
                    let in_folder = tab.folder_id.is_some();
                    if in_folder {
                        if let Some(fcol) = tab.folder_color {
                            let guide_col = fcol.mix(
                                palette.bg_chrome,
                                if tab.is_active { 0.15 } else { 0.55 },
                            );
                            let gbr = CreateSolidBrush(rgb(guide_col));
                            let guide = RECT {
                                left: scale_i32(17, scale),
                                top: y - (g.gap / 2),
                                right: scale_i32(19, scale),
                                bottom: y + g.item_h + (g.gap / 2) + 1,
                            };
                            FillRect(mem_dc, &guide, gbr);
                            let _ = DeleteObject(gbr);
                        }
                    }

                    let item_rect = RECT {
                        left: if in_folder {
                            scale_i32(25, scale)
                        } else {
                            scale_i32(10, scale)
                        },
                        top: y,
                        right: width - scale_i32(10, scale),
                        bottom: y + g.item_h,
                    };

                    let is_hovered = matches!(
                        hover,
                        SidebarHit::Tab(idx, _) | SidebarHit::CloseTab(idx, _) if idx == *orig_i
                    );
                    let close_hovered = matches!(hover, SidebarHit::CloseTab(idx, _) if idx == *orig_i);
                    let folder_drop_hl = tab
                        .folder_id
                        .is_some_and(|fid| drag_target == Some(DragDropTarget::Folder(fid)));

                    if tab.is_active {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(8, scale),
                            rgb(palette.bg_active),
                            rgb(palette.border_subtle),
                        );
                        let bar_col = tab.folder_color.unwrap_or(palette.accent);
                        let accent_br = CreateSolidBrush(rgb(bar_col));
                        let bar = RECT {
                            left: item_rect.left + scale_i32(2, scale),
                            top: item_rect.top + scale_i32(8, scale),
                            right: item_rect.left + scale_i32(5, scale),
                            bottom: item_rect.bottom - scale_i32(8, scale),
                        };
                        FillRect(mem_dc, &bar, accent_br);
                        let _ = DeleteObject(accent_br);
                    } else if is_hovered || folder_drop_hl {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(8, scale),
                            rgb(palette.bg_hover),
                            rgb(if folder_drop_hl {
                                palette.accent
                            } else {
                                palette.bg_hover
                            }),
                        );
                    }

                    let ico_sz = scale_i32(18, scale);
                    let ico_x = item_rect.left + scale_i32(10, scale);
                    let ico_y = item_rect.top + (g.item_h - ico_sz) / 2;

                    draw_site_icon(
                        mem_dc,
                        favicons,
                        palette,
                        tab.domain,
                        false,
                        ico_x,
                        ico_y,
                        ico_sz,
                        scale,
                        font_small,
                    );

                    if tab.is_loading || tab.is_suspended || tab.is_audio {
                        let dot_sz = scale_i32(6, scale).max(4);
                        let dot_color = if tab.is_audio {
                            rgb(palette.accent_text)
                        } else if tab.is_loading {
                            rgb(palette.accent)
                        } else {
                            rgb(palette.warning_fg)
                        };
                        draw_dot(
                            mem_dc,
                            ico_x + ico_sz - dot_sz + 1,
                            ico_y + ico_sz - dot_sz + 1,
                            dot_sz,
                            dot_color,
                            rgb(palette.bg_chrome),
                        );
                    }

                    SelectObject(
                        mem_dc,
                        if tab.is_active {
                            font_tab_active
                        } else {
                            font_tab
                        },
                    );
                    let title_col = if tab.is_active || is_hovered {
                        palette.text_primary
                    } else {
                        palette.text_inactive_tab
                    };
                    SetTextColor(mem_dc, rgb(title_col));

                    let right_reserve = if tab.is_active || is_hovered {
                        scale_i32(28, scale)
                    } else if tab.pinned || tab.is_live {
                        scale_i32(20, scale)
                    } else {
                        scale_i32(10, scale)
                    };

                    let raw_title = if tab.title.is_empty() {
                        "Tab Baru"
                    } else {
                        tab.title
                    };
                    let mut text = wide(raw_title);
                    let mut text_rect = RECT {
                        left: ico_x + ico_sz + scale_i32(9, scale),
                        top: item_rect.top,
                        right: item_rect.right - right_reserve,
                        bottom: item_rect.bottom,
                    };
                    DrawTextW(
                        mem_dc,
                        &mut text,
                        &mut text_rect,
                        DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
                    );

                    if tab.is_active || is_hovered {
                        let close_box = RECT {
                            left: item_rect.right - scale_i32(25, scale),
                            top: item_rect.top + scale_i32(6, scale),
                            right: item_rect.right - scale_i32(5, scale),
                            bottom: item_rect.bottom - scale_i32(6, scale),
                        };
                        if close_hovered {
                            draw_rounded_fill(
                                mem_dc,
                                &close_box,
                                scale_i32(6, scale),
                                rgb(palette.danger_bg),
                                rgb(palette.danger_border),
                            );
                        }
                        let x_col = if close_hovered {
                            rgb(palette.danger_fg)
                        } else {
                            rgb(palette.text_muted)
                        };
                        draw_close_x(
                            mem_dc,
                            (close_box.left + close_box.right) / 2,
                            (close_box.top + close_box.bottom) / 2,
                            scale_i32(4, scale),
                            x_col,
                            scale,
                        );
                    } else if tab.pinned {
                        draw_pin_dot(
                            mem_dc,
                            item_rect.right - scale_i32(13, scale),
                            (item_rect.top + item_rect.bottom) / 2,
                            scale,
                            rgb(palette.accent_text),
                        );
                    } else if tab.is_live && !tab.is_suspended {
                        let r = scale_i32(3, scale).max(2);
                        draw_dot(
                            mem_dc,
                            item_rect.right - scale_i32(14, scale),
                            (item_rect.top + item_rect.bottom) / 2 - r,
                            r * 2,
                            rgb(palette.success_fg),
                            rgb(palette.bg_chrome),
                        );
                    }
                }
            }
        }

        if drag_target == Some(DragDropTarget::Ungrouped) {
            let line_y = (last_item_bottom + scale_i32(3, scale))
                .min(g.list_bottom - scale_i32(4, scale));
            let drop_rc = RECT {
                left: scale_i32(12, scale),
                top: line_y,
                right: width - scale_i32(12, scale),
                bottom: line_y + scale_i32(2, scale).max(2),
            };
            let abr = CreateSolidBrush(rgb(palette.accent));
            FillRect(mem_dc, &drop_rc, abr);
            let _ = DeleteObject(abr);
        }

        if max_scroll > 0 {
            let avail_h = (g.list_bottom - g.list_top).max(1);
            let total_h = avail_h + max_scroll;
            let thumb_h = ((avail_h as f64 / total_h as f64) * avail_h as f64)
                .round()
                .max(scale_i32(20, scale) as f64) as i32;
            let track_avail = (avail_h - thumb_h).max(1);
            let thumb_y =
                g.list_top + ((scroll as f64 / max_scroll as f64) * track_avail as f64).round() as i32;
            let sb_rect = RECT {
                left: width - scale_i32(5, scale),
                top: thumb_y,
                right: width - scale_i32(2, scale),
                bottom: thumb_y + thumb_h,
            };
            draw_rounded_fill(
                mem_dc,
                &sb_rect,
                3,
                rgb(palette.border),
                rgb(palette.border),
            );
        }

        let _ = RestoreDC(mem_dc, saved);

        // Calm 1-row 4-icon Utility Strip
        let dock_div = RECT {
            left: scale_i32(12, scale),
            top: g.dock_top,
            right: width - scale_i32(12, scale),
            bottom: g.dock_top + 1,
        };
        let ddiv_brush = CreateSolidBrush(rgb(palette.border_subtle));
        FillRect(mem_dc, &dock_div, ddiv_brush);
        let _ = DeleteObject(ddiv_brush);

        SelectObject(mem_dc, font_small);
        for (hit, label, rc) in g.dock_rects {
            let hovered = hover == hit;
            let is_current = match hit {
                SidebarHit::OpenHistory => active_url == "kurowser://history",
                SidebarHit::OpenBookmarks => active_url == "kurowser://bookmarks",
                SidebarHit::OpenDownloads => active_url == "kurowser://downloads",
                SidebarHit::OpenSettings => active_url == "kurowser://settings",
                _ => false,
            };
            if is_current {
                draw_rounded_fill(
                    mem_dc,
                    &rc,
                    scale_i32(8, scale),
                    rgb(palette.accent_soft),
                    rgb(palette.accent),
                );
            } else if hovered {
                draw_rounded_fill(
                    mem_dc,
                    &rc,
                    scale_i32(8, scale),
                    rgb(palette.bg_hover),
                    rgb(palette.border_subtle),
                );
            }

            let fg = if is_current {
                palette.accent_text
            } else if hovered {
                palette.text_primary
            } else {
                palette.text_muted
            };

            let icon_cx = (rc.left + rc.right) / 2;
            let icon_cy = rc.top + scale_i32(15, scale);
            draw_dock_icon(mem_dc, hit, icon_cx, icon_cy, scale, rgb(fg));

            SetTextColor(mem_dc, rgb(fg));
            let mut lbl = wide(label);
            let mut lbl_rc = RECT {
                left: rc.left + 1,
                top: rc.top + scale_i32(25, scale),
                right: rc.right - 1,
                bottom: rc.bottom - scale_i32(2, scale),
            };
            DrawTextW(
                mem_dc,
                &mut lbl,
                &mut lbl_rc,
                DT_CENTER | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
            );
        }

        // Footer separator, live counter & "Tidurkan Tab" button
        let fdiv = RECT {
            left: scale_i32(10, scale),
            top: g.footer_top,
            right: width - scale_i32(10, scale),
            bottom: g.footer_top + 1,
        };
        let fdiv_brush = CreateSolidBrush(rgb(palette.border_subtle));
        FillRect(mem_dc, &fdiv, fdiv_brush);
        let _ = DeleteObject(fdiv_brush);

        SelectObject(mem_dc, font_meta);
        SetTextColor(mem_dc, rgb(palette.text_muted));
        let footer = format!("Aktif {}/{} · {} tab", live_count, max_live, total_tab_count);
        let mut footer_text = wide(&footer);
        let mut footer_rect = RECT {
            left: scale_i32(12, scale),
            top: g.footer_top + 2,
            right: g.trim_btn.left - scale_i32(6, scale),
            bottom: height - scale_i32(2, scale),
        };
        DrawTextW(
            mem_dc,
            &mut footer_text,
            &mut footer_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
        );

        let trim_hovered = hover == SidebarHit::TrimMemory;
        let (tr_fill, tr_border, tr_fg) = if trim_hovered {
            (palette.accent, palette.accent, palette.on_accent)
        } else {
            (palette.bg_surface, palette.border, palette.accent_text)
        };
        draw_rounded_fill(
            mem_dc,
            &g.trim_btn,
            scale_i32(7, scale),
            rgb(tr_fill),
            rgb(tr_border),
        );
        SetTextColor(mem_dc, rgb(tr_fg));
        let mut trim_lbl = wide("Tidurkan Tab");
        let mut trim_rc = g.trim_btn;
        DrawTextW(
            mem_dc,
            &mut trim_lbl,
            &mut trim_rc,
            DT_CENTER | DT_SINGLELINE | DT_VCENTER,
        );

        let _ = BitBlt(
            hdc_screen,
            rect.left,
            rect.top,
            width,
            height,
            mem_dc,
            0,
            0,
            SRCCOPY,
        );

        SelectObject(mem_dc, old_font);
        SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(font_title);
        let _ = DeleteObject(font_tab);
        let _ = DeleteObject(font_tab_active);
        let _ = DeleteObject(font_dock);
        let _ = DeleteObject(font_small);
        let _ = DeleteObject(font_meta);
        let _ = DeleteObject(mem_bmp);
        let _ = DeleteDC(mem_dc);
    }
}

unsafe fn paint_url_pill(
    hdc: HDC,
    frame: &RECT,
    btns: &TopbarButtons,
    scale: f64,
    palette: &ThemePalette,
    nav: NavVisualState,
    favicons: &FaviconCache,
) {
    if frame.right <= frame.left {
        return;
    }

    let rad = scale_i32(10, scale);
    let border_col = if nav.addr_focused {
        palette.focus_ring
    } else {
        palette.border
    };
    draw_rounded_fill(hdc, frame, rad, rgb(palette.bg_surface), rgb(border_col));

    // Crisp 2px focus ring when address bar is focused
    if nav.addr_focused {
        let pen_w = scale_i32(2, scale).clamp(2, 3);
        let pen = CreatePen(PS_SOLID, pen_w, rgb(palette.focus_ring));
        let old_p = SelectObject(hdc, pen);
        let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let _ = RoundRect(
            hdc,
            frame.left,
            frame.top,
            frame.right,
            frame.bottom,
            rad,
            rad,
        );
        SelectObject(hdc, old_b);
        SelectObject(hdc, old_p);
        let _ = DeleteObject(pen);
    }

    let sec_cx = frame.left + scale_i32(14, scale);
    let sec_cy = (frame.top + frame.bottom) / 2;
    draw_security_icon(hdc, sec_cx, sec_cy, scale, nav.sec_icon, palette);

    let shield_rc = btns.shield;
    if nav.hover == Some(TopbarButton::Shield) {
        draw_rounded_fill(
            hdc,
            &shield_rc,
            scale_i32(7, scale),
            rgb(palette.bg_hover),
            rgb(palette.border_subtle),
        );
    }
    let font_badge = create_font(scale, 10, FW_SEMIBOLD.0 as i32);
    draw_shield_button_glyph(
        hdc,
        &shield_rc,
        scale,
        palette,
        nav.shields_active,
        nav.blocked_count,
        nav.sec_icon == SecurityIcon::Internal,
        font_badge,
        favicons,
    );
    let _ = DeleteObject(font_badge);

    let star_rc = btns.star;
    if nav.hover == Some(TopbarButton::BookmarkStar) {
        draw_rounded_fill(
            hdc,
            &star_rc,
            scale_i32(7, scale),
            rgb(palette.bg_hover),
            rgb(palette.border_subtle),
        );
    }
    let star_cx = (star_rc.left + star_rc.right) / 2;
    let star_cy = (star_rc.top + star_rc.bottom) / 2;
    let star_col = if nav.is_bookmarked {
        rgb(palette.warning_fg)
    } else if nav.hover == Some(TopbarButton::BookmarkStar) {
        rgb(palette.text_primary)
    } else {
        rgb(palette.text_muted)
    };
    draw_star_icon(
        hdc,
        star_cx,
        star_cy,
        scale_i32(6, scale).max(5) as f64,
        nav.is_bookmarked,
        star_col,
        scale,
    );
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaptionKind {
    Minimize,
    Maximize,
    Restore,
    Close,
}

unsafe fn draw_caption_button(
    hdc: HDC,
    r: &RECT,
    scale: f64,
    palette: &ThemePalette,
    hovered: bool,
    kind: CaptionKind,
) {
    let fg = if kind == CaptionKind::Close && hovered {
        let close_red = Rgb(0xc4, 0x2b, 0x1c);
        draw_rounded_fill(hdc, r, scale_i32(6, scale), rgb(close_red), rgb(close_red));
        rgb(Rgb(0xff, 0xff, 0xff))
    } else if hovered {
        draw_rounded_fill(
            hdc,
            r,
            scale_i32(6, scale),
            rgb(palette.bg_hover),
            rgb(palette.border_subtle),
        );
        rgb(palette.text_primary)
    } else {
        rgb(palette.text_muted)
    };

    let cx = (r.left + r.right) / 2;
    let cy = (r.top + r.bottom) / 2;
    let stroke_w = (1.3 * scale).round().clamp(1.0, 2.0) as i32;
    match kind {
        CaptionKind::Minimize => {
            let hw = scale_i32(4, scale).max(3);
            let pen = CreatePen(PS_SOLID, stroke_w, fg);
            let old = SelectObject(hdc, pen);
            let _ = MoveToEx(hdc, cx - hw, cy + 1, None);
            let _ = LineTo(hdc, cx + hw + 1, cy + 1);
            SelectObject(hdc, old);
            let _ = DeleteObject(pen);
        }
        CaptionKind::Maximize => {
            let hs = scale_i32(4, scale).max(3);
            let pen = CreatePen(PS_SOLID, stroke_w, fg);
            let old_p = SelectObject(hdc, pen);
            let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
            let _ = Rectangle(hdc, cx - hs, cy - hs, cx + hs + 1, cy + hs + 1);
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            let _ = DeleteObject(pen);
        }
        CaptionKind::Restore => {
            let pen = CreatePen(PS_SOLID, stroke_w, fg);
            let old_p = SelectObject(hdc, pen);
            let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
            let off = scale_i32(2, scale).max(1);
            let hs = scale_i32(3, scale).max(2);
            let _ = MoveToEx(hdc, cx - hs + off, cy - hs - off, None);
            let _ = LineTo(hdc, cx + hs + off, cy - hs - off);
            let _ = LineTo(hdc, cx + hs + off, cy + hs - off);
            let _ = MoveToEx(hdc, cx - hs + off, cy - hs - off, None);
            let _ = LineTo(hdc, cx - hs + off, cy - hs + off);
            let _ = LineTo(hdc, cx + hs - off, cy - hs + off);
            let _ = Rectangle(hdc, cx - hs - off, cy - hs + off, cx + hs - off + 1, cy + hs + off + 1);
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            let _ = DeleteObject(pen);
        }
        CaptionKind::Close => {
            draw_close_x(hdc, cx, cy, scale_i32(4, scale).max(3), fg, scale);
        }
    }
}

// ── Double-Buffered 72px Collapsed Icon Rail ──

fn rail_list_bounds(rect: &RECT, scale: f64, btns: &TopbarButtons) -> (i32, i32) {
    let list_top = (btns.new_tab.bottom + scale_i32(8, scale)).max(rect.top);
    let list_bottom = (btns.star.top - scale_i32(8, scale)).max(list_top);
    (list_top, list_bottom)
}

pub fn max_rail_scroll(
    layout: &Layout,
    entries: &[SidebarEntry<'_>],
) -> i32 {
    if layout.sidebar_expanded {
        return 0;
    }
    let scale = layout.dpi_scale;
    let btns = topbar_button_rects(layout, false);
    let (list_top, list_bottom) = rail_list_bounds(&layout.sidebar, scale, &btns);
    let tile_h = scale_i32(34, scale);
    let gap = scale_i32(4, scale);
    let total_h = (entries.len() as i32) * (tile_h + gap);
    let avail = (list_bottom - list_top).max(0);
    (total_h - avail).max(0)
}

pub fn hit_test_rail_tabs(
    layout: &Layout,
    entries: &[SidebarEntry<'_>],
    scroll_offset: i32,
    x: i32,
    y: i32,
) -> SidebarHit {
    if layout.sidebar_expanded || !point_in_rect(x, y, &layout.sidebar) {
        return SidebarHit::None;
    }
    let scale = layout.dpi_scale;
    let btns = topbar_button_rects(layout, false);
    let (list_top, list_bottom) = rail_list_bounds(&layout.sidebar, scale, &btns);
    if y < list_top || y >= list_bottom {
        return SidebarHit::None;
    }

    let tile_w = scale_i32(48, scale).min((layout.sidebar.right - layout.sidebar.left - 8).max(24));
    let tile_h = scale_i32(34, scale);
    let gap = scale_i32(4, scale);
    let tile_left = layout.sidebar.left + ((layout.sidebar.right - layout.sidebar.left - tile_w) / 2);
    let max_scroll = max_rail_scroll(layout, entries);
    let scroll = scroll_offset.clamp(0, max_scroll);

    for (i, entry) in entries.iter().enumerate() {
        let top = list_top + (i as i32) * (tile_h + gap) - scroll;
        let rc = RECT {
            left: tile_left,
            top,
            right: tile_left + tile_w,
            bottom: top + tile_h,
        };
        if rc.bottom <= list_top || rc.top >= list_bottom {
            continue;
        }
        if point_in_rect(x, y, &rc) {
            return match entry {
                SidebarEntry::FolderHeader { folder_id, .. } => {
                    SidebarHit::FolderToggle(i, *folder_id)
                }
                SidebarEntry::Tab(tab) => SidebarHit::Tab(i, tab.id),
            };
        }
    }
    SidebarHit::None
}

pub fn paint_rail(
    hdc_screen: HDC,
    layout: &Layout,
    palette: &ThemePalette,
    nav: NavVisualState,
    entries: &[SidebarEntry<'_>],
    scroll_offset: i32,
    hover: SidebarHit,
    favicons: &FaviconCache,
) {
    if layout.sidebar_expanded {
        return;
    }
    let rect = &layout.sidebar;
    let scale = layout.dpi_scale;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return;
    }

    unsafe {
        let mem_dc = CreateCompatibleDC(hdc_screen);
        let mem_bmp = CreateCompatibleBitmap(hdc_screen, width, height);
        let old_bmp = SelectObject(mem_dc, mem_bmp);

        let local = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };

        let bg_brush = CreateSolidBrush(rgb(palette.bg_chrome));
        FillRect(mem_dc, &local, bg_brush);
        let _ = DeleteObject(bg_brush);

        let btns = topbar_button_rects(layout, nav.has_shield_badge());

        draw_nav_button(
            mem_dc,
            &btns.toggle,
            scale,
            palette,
            true,
            nav.hover == Some(TopbarButton::ToggleSidebar),
            |dc, cx, cy| {
                draw_sidebar_toggle_icon(dc, cx, cy, scale, false, rgb(palette.accent_text))
            },
        );

        // Mini Search Pill trigger
        let srch_hov = nav.hover == Some(TopbarButton::RailSearch);
        draw_rounded_fill(
            mem_dc,
            &btns.rail_search,
            scale_i32(8, scale),
            rgb(if srch_hov { palette.bg_hover } else { palette.bg_surface }),
            rgb(if srch_hov { palette.accent } else { palette.border }),
        );
        draw_search_magnifier(
            mem_dc,
            (btns.rail_search.left + btns.rail_search.right) / 2,
            (btns.rail_search.top + btns.rail_search.bottom) / 2,
            scale,
            rgb(if srch_hov { palette.accent_text } else { palette.text_muted }),
        );

        draw_nav_button(
            mem_dc,
            &btns.back,
            scale,
            palette,
            nav.can_back,
            nav.hover == Some(TopbarButton::Back),
            |dc, cx, cy| {
                let col = if nav.can_back {
                    palette.text_primary
                } else {
                    palette.text_disabled
                };
                draw_arrow_icon(dc, cx, cy, scale, true, rgb(col));
            },
        );
        draw_nav_button(
            mem_dc,
            &btns.forward,
            scale,
            palette,
            nav.can_forward,
            nav.hover == Some(TopbarButton::Forward),
            |dc, cx, cy| {
                let col = if nav.can_forward {
                    palette.text_primary
                } else {
                    palette.text_disabled
                };
                draw_arrow_icon(dc, cx, cy, scale, false, rgb(col));
            },
        );
        draw_nav_button(
            mem_dc,
            &btns.reload,
            scale,
            palette,
            true,
            nav.hover == Some(TopbarButton::ReloadOrStop),
            |dc, cx, cy| {
                if nav.is_loading {
                    draw_close_x(
                        dc,
                        cx,
                        cy,
                        scale_i32(5, scale),
                        rgb(palette.danger_fg),
                        scale,
                    );
                } else {
                    draw_reload_icon(dc, cx, cy, scale, rgb(palette.text_primary));
                }
            },
        );

        if nav.hover == Some(TopbarButton::Shield) {
            draw_rounded_fill(
                mem_dc,
                &btns.shield,
                scale_i32(7, scale),
                rgb(palette.bg_hover),
                rgb(palette.border_subtle),
            );
        }
        let font_badge = create_font(scale, 10, FW_SEMIBOLD.0 as i32);
        draw_shield_button_glyph(
            mem_dc,
            &btns.shield,
            scale,
            palette,
            nav.shields_active,
            nav.blocked_count,
            nav.sec_icon == SecurityIcon::Internal,
            font_badge,
            favicons,
        );
        let _ = DeleteObject(font_badge);

        // Divider above "+ New Tab"
        let div_y = btns.shield.bottom + scale_i32(7, scale);
        let div_rc = RECT {
            left: scale_i32(14, scale),
            top: div_y,
            right: width - scale_i32(14, scale),
            bottom: div_y + 1,
        };
        let div_br = CreateSolidBrush(rgb(palette.border_subtle));
        FillRect(mem_dc, &div_rc, div_br);
        let _ = DeleteObject(div_br);

        let nt_hov = nav.hover == Some(TopbarButton::NewTab);
        draw_rounded_fill(
            mem_dc,
            &btns.new_tab,
            scale_i32(8, scale),
            rgb(if nt_hov { palette.accent_soft } else { palette.bg_surface }),
            rgb(if nt_hov { palette.accent } else { palette.border }),
        );
        draw_plus_icon(
            mem_dc,
            (btns.new_tab.left + btns.new_tab.right) / 2,
            (btns.new_tab.top + btns.new_tab.bottom) / 2,
            scale_i32(5, scale),
            rgb(palette.accent_text),
            scale,
        );

        // Scrollable favicon & folder strip
        let (list_top, list_bottom) = rail_list_bounds(&local, scale, &btns);
        let tile_w = scale_i32(48, scale).min((width - 8).max(24));
        let tile_h = scale_i32(34, scale);
        let gap = scale_i32(4, scale);
        let tile_left = (width - tile_w) / 2;
        let max_scroll = max_rail_scroll(layout, entries);
        let scroll = scroll_offset.clamp(0, max_scroll);

        let font_small = create_font(scale, 10, FW_MEDIUM.0 as i32);
        let saved = SaveDC(mem_dc);
        IntersectClipRect(mem_dc, 0, list_top, width, list_bottom);

        for (i, entry) in entries.iter().enumerate() {
            let y = list_top + (i as i32) * (tile_h + gap) - scroll;
            if y + tile_h < list_top || y > list_bottom {
                continue;
            }
            let tile_rc = RECT {
                left: tile_left,
                top: y,
                right: tile_left + tile_w,
                bottom: y + tile_h,
            };

            match entry {
                SidebarEntry::FolderHeader {
                    folder_id: _,
                    name: _,
                    color,
                    collapsed,
                    tab_count: _,
                    has_active_child,
                } => {
                    let is_hov = matches!(hover, SidebarHit::FolderToggle(idx, _) if idx == i);
                    if is_hov {
                        draw_rounded_fill(
                            mem_dc,
                            &tile_rc,
                            scale_i32(8, scale),
                            rgb(palette.bg_hover),
                            rgb(palette.border_subtle),
                        );
                    } else if *collapsed && *has_active_child {
                        draw_rounded_fill(
                            mem_dc,
                            &tile_rc,
                            scale_i32(8, scale),
                            rgb(palette.bg_surface),
                            rgb(palette.border_subtle),
                        );
                    }
                    let fld_sz = scale_i32(15, scale);
                    let fld_x = (tile_rc.left + tile_rc.right - fld_sz) / 2 - scale_i32(3, scale);
                    let fld_y = tile_rc.top + (tile_h - fld_sz) / 2;
                    draw_folder_glyph(mem_dc, fld_x, fld_y, fld_sz, rgb(*color), !*collapsed);
                    draw_chevron(
                        mem_dc,
                        fld_x + fld_sz + scale_i32(5, scale),
                        tile_rc.top + tile_h / 2,
                        scale * 0.85,
                        *collapsed,
                        rgb(if is_hov { palette.text_primary } else { palette.text_muted }),
                    );
                }

                SidebarEntry::Tab(tab) => {
                    if let Some(fcol) = tab.folder_color {
                        let guide_col = fcol.mix(
                            palette.bg_chrome,
                            if tab.is_active { 0.15 } else { 0.55 },
                        );
                        let gbr = CreateSolidBrush(rgb(guide_col));
                        let guide = RECT {
                            left: scale_i32(5, scale),
                            top: y - (gap / 2),
                            right: scale_i32(7, scale),
                            bottom: y + tile_h + (gap / 2) + 1,
                        };
                        FillRect(mem_dc, &guide, gbr);
                        let _ = DeleteObject(gbr);
                    }

                    let is_hov = matches!(hover, SidebarHit::Tab(idx, _) if idx == i);
                    if tab.is_active {
                        draw_rounded_fill(
                            mem_dc,
                            &tile_rc,
                            scale_i32(8, scale),
                            rgb(palette.bg_active),
                            rgb(palette.border_subtle),
                        );
                        let bar_col = tab.folder_color.unwrap_or(palette.accent);
                        let abr = CreateSolidBrush(rgb(bar_col));
                        let bar = RECT {
                            left: tile_rc.left + scale_i32(2, scale),
                            top: tile_rc.top + scale_i32(7, scale),
                            right: tile_rc.left + scale_i32(5, scale),
                            bottom: tile_rc.bottom - scale_i32(7, scale),
                        };
                        FillRect(mem_dc, &bar, abr);
                        let _ = DeleteObject(abr);
                    } else if is_hov {
                        draw_rounded_fill(
                            mem_dc,
                            &tile_rc,
                            scale_i32(8, scale),
                            rgb(palette.bg_hover),
                            rgb(palette.bg_hover),
                        );
                    }

                    let ico_sz = scale_i32(18, scale);
                    let ix = (tile_rc.left + tile_rc.right - ico_sz) / 2;
                    let iy = (tile_rc.top + tile_rc.bottom - ico_sz) / 2;
                    draw_site_icon(
                        mem_dc,
                        favicons,
                        palette,
                        tab.domain,
                        false,
                        ix,
                        iy,
                        ico_sz,
                        scale,
                        font_small,
                    );

                    if tab.is_loading || tab.is_suspended || tab.is_audio {
                        let dot_sz = scale_i32(5, scale).max(4);
                        let dot_color = if tab.is_audio {
                            rgb(palette.accent_text)
                        } else if tab.is_loading {
                            rgb(palette.accent)
                        } else {
                            rgb(palette.warning_fg)
                        };
                        draw_dot(
                            mem_dc,
                            ix + ico_sz - dot_sz + 1,
                            iy + ico_sz - dot_sz + 1,
                            dot_sz,
                            dot_color,
                            rgb(palette.bg_chrome),
                        );
                    }
                }
            }
        }

        if max_scroll > 0 {
            let avail_h = (list_bottom - list_top).max(1);
            let total_h = avail_h + max_scroll;
            let thumb_h = ((avail_h as f64 / total_h as f64) * avail_h as f64)
                .round()
                .max(scale_i32(18, scale) as f64) as i32;
            let track_avail = (avail_h - thumb_h).max(1);
            let thumb_y =
                list_top + ((scroll as f64 / max_scroll as f64) * track_avail as f64).round() as i32;
            let sb_rect = RECT {
                left: width - scale_i32(4, scale),
                top: thumb_y,
                right: width - scale_i32(1, scale),
                bottom: thumb_y + thumb_h,
            };
            draw_rounded_fill(mem_dc, &sb_rect, 3, rgb(palette.border), rgb(palette.border));
        }

        let _ = RestoreDC(mem_dc, saved);
        let _ = DeleteObject(font_small);

        // Bottom quick controls (Bookmark Star + Settings)
        let bot_div = RECT {
            left: scale_i32(14, scale),
            top: btns.star.top - scale_i32(6, scale),
            right: width - scale_i32(14, scale),
            bottom: btns.star.top - scale_i32(5, scale),
        };
        let bdiv_br = CreateSolidBrush(rgb(palette.border_subtle));
        FillRect(mem_dc, &bot_div, bdiv_br);
        let _ = DeleteObject(bdiv_br);

        draw_nav_button(
            mem_dc,
            &btns.star,
            scale,
            palette,
            true,
            nav.hover == Some(TopbarButton::BookmarkStar),
            |dc, cx, cy| {
                let col = if nav.is_bookmarked {
                    rgb(palette.warning_fg)
                } else if nav.hover == Some(TopbarButton::BookmarkStar) {
                    rgb(palette.text_primary)
                } else {
                    rgb(palette.text_muted)
                };
                draw_star_icon(
                    dc,
                    cx,
                    cy,
                    scale_i32(6, scale).max(5) as f64,
                    nav.is_bookmarked,
                    col,
                    scale,
                );
            },
        );
        draw_nav_button(
            mem_dc,
            &btns.rail_settings,
            scale,
            palette,
            true,
            nav.hover == Some(TopbarButton::RailSettings),
            |dc, cx, cy| {
                let col = if nav.hover == Some(TopbarButton::RailSettings) {
                    palette.text_primary
                } else {
                    palette.text_muted
                };
                draw_dock_icon(dc, SidebarHit::OpenSettings, cx, cy, scale, rgb(col));
            },
        );

        let _ = BitBlt(
            hdc_screen,
            rect.left,
            rect.top,
            width,
            height,
            mem_dc,
            0,
            0,
            SRCCOPY,
        );

        SelectObject(mem_dc, old_bmp);
        let _ = DeleteObject(mem_bmp);
        let _ = DeleteDC(mem_dc);
    }
}

unsafe fn draw_nav_button(
    hdc: HDC,
    r: &RECT,
    scale: f64,
    palette: &ThemePalette,
    enabled: bool,
    hovered: bool,
    draw_glyph: impl FnOnce(HDC, i32, i32),
) {
    if hovered && enabled {
        draw_rounded_fill(
            hdc,
            r,
            scale_i32(7, scale),
            rgb(palette.bg_hover),
            rgb(palette.border_subtle),
        );
    }
    let cx = (r.left + r.right) / 2;
    let cy = (r.top + r.bottom) / 2;
    draw_glyph(hdc, cx, cy);
}

// ── Clean Vector GDI Icon Primitives ──

unsafe fn draw_rounded_fill(hdc: HDC, r: &RECT, radius: i32, fill: COLORREF, border: COLORREF) {
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(PS_SOLID, 1, border);
    let old_brush = SelectObject(hdc, brush);
    let old_pen = SelectObject(hdc, pen);
    let _ = RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

unsafe fn draw_dot(hdc: HDC, x: i32, y: i32, size: i32, fill: COLORREF, border: COLORREF) {
    let brush = CreateSolidBrush(fill);
    let pen = CreatePen(PS_SOLID, 1, border);
    let old_b = SelectObject(hdc, brush);
    let old_p = SelectObject(hdc, pen);
    let _ = Ellipse(hdc, x, y, x + size, y + size);
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

unsafe fn draw_plus_icon(hdc: HDC, cx: i32, cy: i32, half: i32, color: COLORREF, scale: f64) {
    let w = scale_i32(2, scale).clamp(1, 3);
    let pen = CreatePen(PS_SOLID, w, color);
    let old = SelectObject(hdc, pen);
    let _ = MoveToEx(hdc, cx - half, cy, None);
    let _ = LineTo(hdc, cx + half + 1, cy);
    let _ = MoveToEx(hdc, cx, cy - half, None);
    let _ = LineTo(hdc, cx, cy + half + 1);
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn draw_close_x(hdc: HDC, cx: i32, cy: i32, half: i32, color: COLORREF, scale: f64) {
    let w = (1.5 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, w.clamp(1, 3), color);
    let old = SelectObject(hdc, pen);
    let _ = MoveToEx(hdc, cx - half, cy - half, None);
    let _ = LineTo(hdc, cx + half + 1, cy + half + 1);
    let _ = MoveToEx(hdc, cx + half, cy - half, None);
    let _ = LineTo(hdc, cx - half - 1, cy + half + 1);
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn draw_sidebar_toggle_icon(
    hdc: HDC,
    cx: i32,
    cy: i32,
    scale: f64,
    expanded: bool,
    color: COLORREF,
) {
    let hw = scale_i32(7, scale);
    let hh = scale_i32(6, scale);
    let w = (1.5 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, w.clamp(1, 2), color);
    let old_p = SelectObject(hdc, pen);
    let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = RoundRect(hdc, cx - hw, cy - hh, cx + hw, cy + hh, 3, 3);
    let split_x = cx - hw + scale_i32(5, scale);
    let _ = MoveToEx(hdc, split_x, cy - hh + 1, None);
    let _ = LineTo(hdc, split_x, cy + hh - 1);
    if expanded {
        let fill_rc = RECT {
            left: cx - hw + 2,
            top: cy - hh + 2,
            right: split_x,
            bottom: cy + hh - 2,
        };
        let fill = CreateSolidBrush(color);
        FillRect(hdc, &fill_rc, fill);
        let _ = DeleteObject(fill);
    }
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(pen);
}

unsafe fn draw_arrow_icon(hdc: HDC, cx: i32, cy: i32, scale: f64, left: bool, color: COLORREF) {
    let hw = scale_i32(6, scale);
    let ch = scale_i32(5, scale);
    let w = (1.6 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, w.clamp(1, 3), color);
    let old = SelectObject(hdc, pen);
    let _ = MoveToEx(hdc, cx - hw, cy, None);
    let _ = LineTo(hdc, cx + hw + 1, cy);
    if left {
        let _ = MoveToEx(hdc, cx - hw + ch, cy - ch, None);
        let _ = LineTo(hdc, cx - hw, cy);
        let _ = LineTo(hdc, cx - hw + ch + 1, cy + ch + 1);
    } else {
        let _ = MoveToEx(hdc, cx + hw - ch, cy - ch, None);
        let _ = LineTo(hdc, cx + hw, cy);
        let _ = LineTo(hdc, cx + hw - ch - 1, cy + ch + 1);
    }
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn draw_reload_icon(hdc: HDC, cx: i32, cy: i32, scale: f64, color: COLORREF) {
    let r = (5.8 * scale).max(4.0);
    let w = (1.6 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, w.clamp(1, 3), color);
    let old = SelectObject(hdc, pen);

    const STEPS: usize = 16;
    let mut pts = [POINT::default(); STEPS + 1];
    let start_deg = 38.0_f64;
    let end_deg = 325.0_f64;
    for (i, pt) in pts.iter_mut().enumerate() {
        let deg = start_deg + (end_deg - start_deg) * (i as f64 / STEPS as f64);
        let rad = deg.to_radians();
        *pt = POINT {
            x: cx + (r * rad.cos()).round() as i32,
            y: cy + (r * rad.sin()).round() as i32,
        };
    }
    let _ = Polyline(hdc, &pts);

    let tip = pts[STEPS];
    let arm = scale_i32(4, scale).max(3);
    let _ = MoveToEx(hdc, tip.x - arm, tip.y, None);
    let _ = LineTo(hdc, tip.x, tip.y);
    let _ = LineTo(hdc, tip.x, tip.y - arm);

    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn draw_star_icon(
    hdc: HDC,
    cx: i32,
    cy: i32,
    outer_r: f64,
    filled: bool,
    color: COLORREF,
    scale: f64,
) {
    let inner_r = outer_r * 0.44;
    let mut pts = [POINT::default(); 10];
    for i in 0..10 {
        let r = if i % 2 == 0 { outer_r } else { inner_r };
        let angle = -std::f64::consts::FRAC_PI_2 + (i as f64) * std::f64::consts::PI / 5.0;
        pts[i] = POINT {
            x: cx + (r * angle.cos()).round() as i32,
            y: cy + (r * angle.sin()).round() as i32,
        };
    }
    let pen_w = (1.3 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, pen_w.clamp(1, 2), color);
    let old_p = SelectObject(hdc, pen);
    if filled {
        let brush = CreateSolidBrush(color);
        let old_b = SelectObject(hdc, brush);
        let _ = Polygon(hdc, &pts);
        SelectObject(hdc, old_b);
        let _ = DeleteObject(brush);
    } else {
        let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
        let _ = Polygon(hdc, &pts);
        SelectObject(hdc, old_b);
    }
    SelectObject(hdc, old_p);
    let _ = DeleteObject(pen);
}

unsafe fn draw_security_icon(
    hdc: HDC,
    cx: i32,
    cy: i32,
    scale: f64,
    kind: SecurityIcon,
    palette: &ThemePalette,
) {
    match kind {
        SecurityIcon::Internal => {
            let r = scale_i32(4, scale);
            draw_dot(
                hdc,
                cx - r,
                cy - r,
                r * 2,
                rgb(palette.accent_text),
                rgb(palette.accent_text),
            );
        }
        SecurityIcon::Secure => {
            let col = rgb(palette.success_fg);
            let bw = scale_i32(4, scale);
            let bh = scale_i32(3, scale);
            let pen = CreatePen(PS_SOLID, 1, col);
            let old_p = SelectObject(hdc, pen);
            let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
            let _ = Arc(
                hdc,
                cx - bw + 1,
                cy - bh - scale_i32(4, scale),
                cx + bw - 1,
                cy + 2,
                cx + bw - 1,
                cy - 1,
                cx - bw + 1,
                cy - 1,
            );
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            let _ = DeleteObject(pen);
            let body = RECT {
                left: cx - bw,
                top: cy - 1,
                right: cx + bw,
                bottom: cy + bh + scale_i32(2, scale),
            };
            draw_rounded_fill(hdc, &body, 2, col, col);
        }
        SecurityIcon::Plain => {
            let r = scale_i32(5, scale);
            let col = rgb(palette.text_muted);
            let pen = CreatePen(PS_SOLID, 1, col);
            let old_p = SelectObject(hdc, pen);
            let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
            let _ = Ellipse(hdc, cx - r, cy - r, cx + r, cy + r);
            let _ = MoveToEx(hdc, cx - r + 1, cy, None);
            let _ = LineTo(hdc, cx + r, cy);
            let _ = MoveToEx(hdc, cx, cy - r + 1, None);
            let _ = LineTo(hdc, cx, cy + r);
            SelectObject(hdc, old_b);
            SelectObject(hdc, old_p);
            let _ = DeleteObject(pen);
        }
    }
}

unsafe fn draw_folder_glyph(
    hdc: HDC,
    x: i32,
    y: i32,
    size: i32,
    color: COLORREF,
    open: bool,
) {
    let tab_w = (size * 44) / 100;
    let tab_h = (size * 22) / 100;
    let top_y = y + (size / 7);
    let bot_y = y + size - 1;
    let body_top = top_y + tab_h;

    let pts = [
        POINT { x, y: top_y },
        POINT { x: x + tab_w, y: top_y },
        POINT { x: x + tab_w + (size / 7), y: body_top },
        POINT { x: x + size - 1, y: body_top },
        POINT { x: x + size - 1, y: bot_y },
        POINT { x, y: bot_y },
    ];

    let brush = CreateSolidBrush(color);
    let pen = CreatePen(PS_SOLID, 1, color);
    let old_b = SelectObject(hdc, if open { HGDIOBJ(brush.0) } else { GetStockObject(NULL_BRUSH) });
    let old_p = SelectObject(hdc, pen);
    let _ = Polygon(hdc, &pts);
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

unsafe fn draw_chevron(hdc: HDC, cx: i32, cy: i32, scale: f64, collapsed: bool, color: COLORREF) {
    let d = scale_i32(3, scale).max(2);
    let w = (1.4 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, w.clamp(1, 2), color);
    let old = SelectObject(hdc, pen);
    if collapsed {
        let _ = MoveToEx(hdc, cx - d / 2, cy - d, None);
        let _ = LineTo(hdc, cx + d / 2 + 1, cy);
        let _ = LineTo(hdc, cx - d / 2, cy + d + 1);
    } else {
        let _ = MoveToEx(hdc, cx - d, cy - d / 2, None);
        let _ = LineTo(hdc, cx, cy + d / 2 + 1);
        let _ = LineTo(hdc, cx + d + 1, cy - d / 2);
    }
    SelectObject(hdc, old);
    let _ = DeleteObject(pen);
}

unsafe fn draw_dots_icon(hdc: HDC, cx: i32, cy: i32, scale: f64, color: COLORREF) {
    let sp = scale_i32(4, scale).max(3);
    let sz = scale_i32(3, scale).max(2);
    let r = sz / 2;
    for dx in [-sp, 0, sp] {
        draw_dot(hdc, cx + dx - r, cy - r, sz, color, color);
    }
}

unsafe fn draw_shield_button_glyph(
    hdc: HDC,
    rc: &RECT,
    scale: f64,
    palette: &ThemePalette,
    shields_active: bool,
    blocked_count: u64,
    is_internal: bool,
    font_badge: HGDIOBJ,
    favicons: &FaviconCache,
) {
    let w = rc.right - rc.left;
    let compact = w < scale_i32(34, scale);
    let has_badge = blocked_count > 0 && shields_active && !is_internal;
    let ico_sz = scale_i32(16, scale).max(12);
    let ico_x = if !compact && has_badge {
        rc.left + scale_i32(3, scale)
    } else {
        (rc.left + rc.right - ico_sz) / 2
    };
    let ico_y = (rc.top + rc.bottom - ico_sz) / 2;

    favicons.draw_shield(hdc, ico_x, ico_y, ico_sz, shields_active, is_internal, palette);

    if has_badge {
        if compact {
            let dot_sz = scale_i32(6, scale).max(5);
            draw_dot(
                hdc,
                ico_x + ico_sz - dot_sz + 2,
                ico_y - 1,
                dot_sz,
                rgb(palette.accent),
                rgb(palette.bg_chrome),
            );
        } else {
            let label = if blocked_count > 99 {
                "99+".to_string()
            } else {
                blocked_count.to_string()
            };
            let cy = (rc.top + rc.bottom) / 2;
            let pill = RECT {
                left: rc.right - scale_i32(20, scale),
                top: cy - scale_i32(7, scale),
                right: rc.right - scale_i32(2, scale),
                bottom: cy + scale_i32(7, scale),
            };
            draw_rounded_fill(
                hdc,
                &pill,
                scale_i32(6, scale),
                rgb(palette.accent),
                rgb(palette.accent),
            );
            let prev = SelectObject(hdc, font_badge);
            SetTextColor(hdc, rgb(palette.on_accent));
            let mut txt = wide(&label);
            let mut trc = pill;
            DrawTextW(hdc, &mut txt, &mut trc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
            SelectObject(hdc, prev);
        }
    }
}

unsafe fn draw_pin_dot(hdc: HDC, cx: i32, cy: i32, scale: f64, color: COLORREF) {
    let r = scale_i32(4, scale);
    let pen = CreatePen(PS_SOLID, 1, color);
    let brush = CreateSolidBrush(color);
    let old_p = SelectObject(hdc, pen);
    let old_b = SelectObject(hdc, brush);
    let _ = Ellipse(hdc, cx - r / 2, cy - r, cx + r / 2 + 1, cy);
    let _ = MoveToEx(hdc, cx, cy, None);
    let _ = LineTo(hdc, cx, cy + r);
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);
}

unsafe fn draw_dock_icon(
    hdc: HDC,
    hit: SidebarHit,
    cx: i32,
    cy: i32,
    scale: f64,
    color: COLORREF,
) {
    let pen_w = (1.3 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, pen_w.clamp(1, 2), color);
    let old_p = SelectObject(hdc, pen);
    let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));

    match hit {
        SidebarHit::OpenHistory => {
            let r = scale_i32(5, scale);
            let _ = Ellipse(hdc, cx - r, cy - r, cx + r, cy + r);
            let _ = MoveToEx(hdc, cx, cy - scale_i32(3, scale), None);
            let _ = LineTo(hdc, cx, cy);
            let _ = LineTo(hdc, cx + scale_i32(3, scale), cy + scale_i32(1, scale));
        }
        SidebarHit::OpenBookmarks => {
            draw_star_icon(hdc, cx, cy, scale_i32(6, scale) as f64, false, color, scale);
        }
        SidebarHit::OpenDownloads => {
            let h = scale_i32(4, scale);
            let _ = MoveToEx(hdc, cx, cy - h, None);
            let _ = LineTo(hdc, cx, cy + h - 1);
            let _ = MoveToEx(hdc, cx - scale_i32(3, scale), cy, None);
            let _ = LineTo(hdc, cx, cy + h - 1);
            let _ = LineTo(hdc, cx + scale_i32(4, scale), cy - 1);
            let _ = MoveToEx(hdc, cx - scale_i32(5, scale), cy + h + 1, None);
            let _ = LineTo(hdc, cx + scale_i32(5, scale) + 1, cy + h + 1);
        }
        SidebarHit::OpenSettings => {
            let hw = scale_i32(5, scale);
            let dy = scale_i32(3, scale);
            let _ = MoveToEx(hdc, cx - hw, cy - dy, None);
            let _ = LineTo(hdc, cx + hw + 1, cy - dy);
            let _ = MoveToEx(hdc, cx - hw, cy + dy, None);
            let _ = LineTo(hdc, cx + hw + 1, cy + dy);
            let kr = scale_i32(2, scale).max(2);
            draw_dot(hdc, cx - scale_i32(2, scale) - kr / 2, cy - dy - kr / 2, kr + 1, color, color);
            draw_dot(hdc, cx + scale_i32(2, scale) - kr / 2, cy + dy - kr / 2, kr + 1, color, color);
        }
        _ => {}
    }

    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(pen);
}

unsafe fn draw_search_magnifier(hdc: HDC, cx: i32, cy: i32, scale: f64, color: COLORREF) {
    let r = scale_i32(5, scale);
    let w = (1.6 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, w.clamp(1, 2), color);
    let old_p = SelectObject(hdc, pen);
    let old_b = SelectObject(hdc, GetStockObject(NULL_BRUSH));
    let _ = Ellipse(hdc, cx - r - 1, cy - r - 1, cx + r - 1, cy + r - 1);
    let _ = MoveToEx(hdc, cx + scale_i32(2, scale), cy + scale_i32(2, scale), None);
    let _ = LineTo(hdc, cx + scale_i32(6, scale), cy + scale_i32(6, scale));
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(pen);
}

unsafe fn draw_site_icon(
    hdc: HDC,
    favicons: &FaviconCache,
    palette: &ThemePalette,
    domain: &str,
    is_search: bool,
    x: i32,
    y: i32,
    size: i32,
    scale: f64,
    font_small: HGDIOBJ,
) {
    if is_search {
        let bg = RECT {
            left: x,
            top: y,
            right: x + size,
            bottom: y + size,
        };
        draw_rounded_fill(
            hdc,
            &bg,
            scale_i32(6, scale),
            rgb(palette.accent_soft),
            rgb(palette.border_subtle),
        );
        draw_search_magnifier(hdc, x + size / 2, y + size / 2, scale, rgb(palette.accent_text));
        return;
    }

    if domain.starts_with("kurowser.") || domain == "kurowser" {
        favicons.draw_logo(hdc, x, y, size);
        return;
    }

    if favicons.draw_site(hdc, domain, x, y, size) {
        return;
    }

    // Muted, cohesive monogram tiles all verified >= 5.1:1 with #ffffff
    const MONO_COLORS: [Rgb; 6] = [
        Rgb(0x0f, 0x76, 0x6e),
        Rgb(0x03, 0x69, 0xa1),
        Rgb(0x04, 0x78, 0x57),
        Rgb(0x9a, 0x34, 0x12),
        Rgb(0x33, 0x41, 0x55),
        Rgb(0x47, 0x55, 0x69),
    ];
    let hash: usize = domain.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    let (badge_col, letter_col) = if domain.is_empty() {
        (palette.accent, palette.on_accent)
    } else {
        (MONO_COLORS[hash % MONO_COLORS.len()], Rgb(0xff, 0xff, 0xff))
    };

    let bg = RECT {
        left: x,
        top: y,
        right: x + size,
        bottom: y + size,
    };
    draw_rounded_fill(hdc, &bg, scale_i32(6, scale), rgb(badge_col), rgb(badge_col));

    let letter = domain
        .chars()
        .find(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('K');
    let mut s = [letter as u16, 0u16];
    let prev = SelectObject(hdc, font_small);
    SetTextColor(hdc, rgb(letter_col));
    let mut rc = bg;
    DrawTextW(hdc, &mut s, &mut rc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
    SelectObject(hdc, prev);
}

// ── Topbar / Nav Cluster Geometry & Hit-Testing ──

pub struct TopbarButtons {
    pub win_min: RECT,
    pub win_max: RECT,
    pub win_close: RECT,
    pub toggle: RECT,
    pub rail_search: RECT,
    pub back: RECT,
    pub forward: RECT,
    pub reload: RECT,
    pub shield: RECT,
    pub star: RECT,
    pub new_tab: RECT,
    pub rail_settings: RECT,
}

pub fn topbar_button_rects(layout: &Layout, has_shield_badge: bool) -> TopbarButtons {
    let scale = layout.dpi_scale;

    // Window caption buttons (Minimize / Maximize / Close) always anchored at top-right
    let win_w = scale_i32(34, scale);
    let win_h = scale_i32(26, scale);
    let win_gap = scale_i32(2, scale);
    let cap_y = scale_i32(5, scale);
    let cap_right = (layout.window.right - scale_i32(8, scale)).max(layout.sidebar.right + 12);
    let win_close = RECT {
        left: cap_right - win_w,
        top: cap_y,
        right: cap_right,
        bottom: cap_y + win_h,
    };
    let win_max = RECT {
        left: win_close.left - win_gap - win_w,
        top: cap_y,
        right: win_close.left - win_gap,
        bottom: cap_y + win_h,
    };
    let win_min = RECT {
        left: win_max.left - win_gap - win_w,
        top: cap_y,
        right: win_max.left - win_gap,
        bottom: cap_y + win_h,
    };

    if layout.sidebar_expanded {
        let sb = &layout.sidebar;
        let pad = scale_i32(10, scale);
        let nav_sz = scale_i32(25, scale);
        let nav_gap = scale_i32(3, scale);
        let btn_y = sb.top + scale_i32(6, scale);
        let r_right = sb.right - pad;

        let reload = RECT {
            left: r_right - nav_sz,
            top: btn_y,
            right: r_right,
            bottom: btn_y + nav_sz,
        };
        let forward = RECT {
            left: reload.left - nav_gap - nav_sz,
            top: btn_y,
            right: reload.left - nav_gap,
            bottom: btn_y + nav_sz,
        };
        let back = RECT {
            left: forward.left - nav_gap - nav_sz,
            top: btn_y,
            right: forward.left - nav_gap,
            bottom: btn_y + nav_sz,
        };
        let toggle = RECT {
            left: back.left - nav_gap - nav_sz,
            top: btn_y,
            right: back.left - nav_gap,
            bottom: btn_y + nav_sz,
        };

        let url_top = sb.top + scale_i32(42, scale);
        let url_h = scale_i32(36, scale);
        let star_w = scale_i32(24, scale);
        let star_h = scale_i32(26, scale);
        let star_y = url_top + (url_h - star_h) / 2;
        let frame_right = sb.right - pad;
        let star = RECT {
            left: frame_right - star_w - scale_i32(5, scale),
            top: star_y,
            right: frame_right - scale_i32(5, scale),
            bottom: star_y + star_h,
        };
        let shield_w = scale_i32(if has_shield_badge { 40 } else { 26 }, scale);
        let shield = RECT {
            left: star.left - shield_w - scale_i32(2, scale),
            top: star_y,
            right: star.left - scale_i32(2, scale),
            bottom: star_y + star_h,
        };

        return TopbarButtons {
            win_min,
            win_max,
            win_close,
            toggle,
            rail_search: RECT::default(),
            back,
            forward,
            reload,
            shield,
            star,
            new_tab: RECT::default(),
            rail_settings: RECT::default(),
        };
    }

    let sb = &layout.sidebar;
    let rail_w = (sb.right - sb.left).max(52);
    let full_pad = scale_i32(8, scale).min((rail_w - 24) / 2).max(2);
    let full_l = sb.left + full_pad;
    let full_r = sb.right - full_pad;

    let toggle = RECT {
        left: full_l,
        top: sb.top + scale_i32(8, scale),
        right: full_r,
        bottom: sb.top + scale_i32(36, scale),
    };
    let rail_search = RECT {
        left: full_l,
        top: toggle.bottom + scale_i32(5, scale),
        right: full_r,
        bottom: toggle.bottom + scale_i32(33, scale),
    };

    let pair_sz = scale_i32(26, scale);
    let pair_gap = scale_i32(4, scale);
    let pair_x0 = sb.left + (rail_w - (pair_sz * 2 + pair_gap)) / 2;
    let row_d_y = rail_search.bottom + scale_i32(8, scale);
    let back = RECT {
        left: pair_x0,
        top: row_d_y,
        right: pair_x0 + pair_sz,
        bottom: row_d_y + pair_sz,
    };
    let forward = RECT {
        left: back.right + pair_gap,
        top: row_d_y,
        right: back.right + pair_gap + pair_sz,
        bottom: row_d_y + pair_sz,
    };

    let row_e_y = back.bottom + scale_i32(4, scale);
    let reload = RECT {
        left: pair_x0,
        top: row_e_y,
        right: pair_x0 + pair_sz,
        bottom: row_e_y + pair_sz,
    };
    let shield = RECT {
        left: reload.right + pair_gap,
        top: row_e_y,
        right: reload.right + pair_gap + pair_sz,
        bottom: row_e_y + pair_sz,
    };

    let new_tab = RECT {
        left: full_l,
        top: shield.bottom + scale_i32(15, scale),
        right: full_r,
        bottom: shield.bottom + scale_i32(45, scale),
    };

    let bot_y = (sb.bottom - scale_i32(38, scale)).max(new_tab.bottom + scale_i32(16, scale));
    let star = RECT {
        left: pair_x0,
        top: bot_y,
        right: pair_x0 + pair_sz,
        bottom: bot_y + pair_sz,
    };
    let rail_settings = RECT {
        left: star.right + pair_gap,
        top: bot_y,
        right: star.right + pair_gap + pair_sz,
        bottom: bot_y + pair_sz,
    };

    TopbarButtons {
        win_min,
        win_max,
        win_close,
        toggle,
        rail_search,
        back,
        forward,
        reload,
        shield,
        star,
        new_tab,
        rail_settings,
    }
}

pub fn address_bar_frame_rect(layout: &Layout) -> RECT {
    if !layout.sidebar_expanded {
        return RECT::default();
    }
    let scale = layout.dpi_scale;
    let sb = &layout.sidebar;
    let pad = scale_i32(10, scale);
    let url_top = sb.top + scale_i32(42, scale);
    RECT {
        left: sb.left + pad,
        top: url_top,
        right: (sb.right - pad).max(sb.left + pad + 40),
        bottom: url_top + scale_i32(36, scale),
    }
}

pub fn hit_test_topbar(layout: &Layout, has_shield_badge: bool, x: i32, y: i32) -> Option<TopbarButton> {
    let btns = topbar_button_rects(layout, has_shield_badge);
    if point_in_rect(x, y, &btns.win_close) {
        Some(TopbarButton::WinClose)
    } else if point_in_rect(x, y, &btns.win_max) {
        Some(TopbarButton::WinMaximize)
    } else if point_in_rect(x, y, &btns.win_min) {
        Some(TopbarButton::WinMinimize)
    } else if point_in_rect(x, y, &btns.toggle) {
        Some(TopbarButton::ToggleSidebar)
    } else if point_in_rect(x, y, &btns.rail_search) {
        Some(TopbarButton::RailSearch)
    } else if point_in_rect(x, y, &btns.back) {
        Some(TopbarButton::Back)
    } else if point_in_rect(x, y, &btns.forward) {
        Some(TopbarButton::Forward)
    } else if point_in_rect(x, y, &btns.reload) {
        Some(TopbarButton::ReloadOrStop)
    } else if point_in_rect(x, y, &btns.shield) {
        Some(TopbarButton::Shield)
    } else if point_in_rect(x, y, &btns.star) {
        Some(TopbarButton::BookmarkStar)
    } else if point_in_rect(x, y, &btns.new_tab) {
        Some(TopbarButton::NewTab)
    } else if point_in_rect(x, y, &btns.rail_settings) {
        Some(TopbarButton::RailSettings)
    } else {
        None
    }
}

pub fn window_drag_area_hit(layout: &Layout, has_shield_badge: bool, x: i32, y: i32) -> bool {
    if hit_test_topbar(layout, has_shield_badge, x, y).is_some() {
        return false;
    }
    if point_in_rect(x, y, &layout.content) {
        return false;
    }
    if point_in_rect(x, y, &layout.topbar) {
        return true;
    }
    if layout.sidebar_expanded {
        let header_h = scale_i32(42, layout.dpi_scale);
        return x >= layout.sidebar.left && x < layout.sidebar.right && y >= 0 && y < header_h;
    }
    let btns = topbar_button_rects(layout, has_shield_badge);
    x >= layout.sidebar.left && x < layout.sidebar.right && y >= 0 && y < btns.toggle.top
}

pub fn hit_test_sidebar(
    rect: &RECT,
    scale: f64,
    entries: &[SidebarEntry<'_>],
    scroll_offset: i32,
    x: i32,
    y: i32,
) -> SidebarHit {
    if !point_in_rect(x, y, rect) {
        return SidebarHit::None;
    }

    let (pinned_tabs, tree_entries) = partition_sidebar_entries(entries);
    let g = sidebar_geometry(rect, scale, &pinned_tabs);

    if point_in_rect(x, y, &g.new_btn) {
        return SidebarHit::NewTab;
    }
    if point_in_rect(x, y, &g.new_folder_btn) {
        return SidebarHit::NewFolder;
    }

    for &(orig_idx, tab_id, rc) in &g.pinned_tiles {
        if point_in_rect(x, y, &rc) {
            return SidebarHit::Tab(orig_idx, tab_id);
        }
    }

    if point_in_rect(x, y, &g.trim_btn) {
        return SidebarHit::TrimMemory;
    }

    for (hit, _, rc) in g.dock_rects {
        if point_in_rect(x, y, &rc) {
            return hit;
        }
    }

    if y >= g.list_top && y < g.list_bottom {
        let max_scroll = max_sidebar_scroll(rect, scale, entries);
        let scroll = scroll_offset.clamp(0, max_scroll);
        for (tree_i, &(orig_i, entry)) in tree_entries.iter().enumerate() {
            let item_top = g.list_top + (tree_i as i32) * (g.item_h + g.gap) - scroll;
            let in_folder = matches!(entry, SidebarEntry::Tab(t) if t.folder_id.is_some());
            let item_rect = RECT {
                left: rect.left
                    + if in_folder {
                        scale_i32(25, scale)
                    } else {
                        scale_i32(10, scale)
                    },
                top: item_top,
                right: rect.right - scale_i32(10, scale),
                bottom: item_top + g.item_h,
            };
            if item_rect.bottom <= g.list_top || item_rect.top >= g.list_bottom {
                continue;
            }
            match entry {
                SidebarEntry::FolderHeader { folder_id, .. } => {
                    let add_rect = RECT {
                        left: item_rect.right - scale_i32(47, scale),
                        top: item_rect.top,
                        right: item_rect.right - scale_i32(26, scale),
                        bottom: item_rect.bottom,
                    };
                    let menu_rect = RECT {
                        left: item_rect.right - scale_i32(25, scale),
                        top: item_rect.top,
                        right: item_rect.right - scale_i32(3, scale),
                        bottom: item_rect.bottom,
                    };
                    if point_in_rect(x, y, &add_rect) {
                        return SidebarHit::FolderAddTab(orig_i, *folder_id);
                    }
                    if point_in_rect(x, y, &menu_rect) {
                        return SidebarHit::FolderMenu(orig_i, *folder_id);
                    }
                    if point_in_rect(x, y, &item_rect) {
                        return SidebarHit::FolderToggle(orig_i, *folder_id);
                    }
                }
                SidebarEntry::Tab(tab) => {
                    let close_rect = RECT {
                        left: item_rect.right - scale_i32(27, scale),
                        top: item_rect.top,
                        right: item_rect.right - scale_i32(3, scale),
                        bottom: item_rect.bottom,
                    };
                    if point_in_rect(x, y, &close_rect) {
                        return SidebarHit::CloseTab(orig_i, tab.id);
                    }
                    if point_in_rect(x, y, &item_rect) {
                        return SidebarHit::Tab(orig_i, tab.id);
                    }
                }
            }
        }
    }

    SidebarHit::None
}

pub fn hit_test_sidebar_drop_target(
    rect: &RECT,
    scale: f64,
    entries: &[SidebarEntry<'_>],
    scroll_offset: i32,
    x: i32,
    y: i32,
) -> Option<DragDropTarget> {
    if !point_in_rect(x, y, rect) {
        return None;
    }
    let (pinned_tabs, tree_entries) = partition_sidebar_entries(entries);
    let g = sidebar_geometry(rect, scale, &pinned_tabs);
    if y < g.list_top || y >= g.list_bottom {
        return None;
    }
    let max_scroll = max_sidebar_scroll(rect, scale, entries);
    let scroll = scroll_offset.clamp(0, max_scroll);
    for (tree_i, &(_, entry)) in tree_entries.iter().enumerate() {
        let item_top = g.list_top + (tree_i as i32) * (g.item_h + g.gap) - scroll;
        let item_bottom = item_top + g.item_h;
        if y >= item_top && y < item_bottom + g.gap {
            match entry {
                SidebarEntry::FolderHeader { folder_id, .. } => {
                    return Some(DragDropTarget::Folder(*folder_id));
                }
                SidebarEntry::Tab(tab) => {
                    if let Some(fid) = tab.folder_id {
                        return Some(DragDropTarget::Folder(fid));
                    }
                    return Some(DragDropTarget::Ungrouped);
                }
            }
        }
    }
    Some(DragDropTarget::Ungrouped)
}

pub fn point_in_rect(x: i32, y: i32, r: &RECT) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}

// ── Address Bar Subclass (EDIT Control) ──

pub const ADDR_CTRL_ID: u16 = 100;
const ADDR_SUBCLASS_ID: usize = 200;

struct AddrSubclassData {
    on_event: Box<dyn Fn(AddrBarEvent) + 'static>,
    select_all_on_up: bool,
}

thread_local! {
    static ADDR_FONT_HANDLE: RefCell<Option<HGDIOBJ>> = const { RefCell::new(None) };
}

pub fn create_address_bar(
    parent: HWND,
    scale: f64,
    on_event: impl Fn(AddrBarEvent) + 'static,
) -> HWND {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let style = WS_CHILD | WS_VISIBLE | WINDOW_STYLE(ES_AUTOHSCROLL as u32);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            style,
            0,
            0,
            100,
            24,
            parent,
            HMENU(ADDR_CTRL_ID as usize as _),
            hinstance,
            None,
        )
        .unwrap_or_default();

        update_address_bar_font(hwnd, scale);

        const EM_SETCUEBANNER: u32 = 0x1501;
        let cue = wide("Cari atau ketik URL…");
        SendMessageW(
            hwnd,
            EM_SETCUEBANNER,
            WPARAM(1),
            LPARAM(cue.as_ptr() as isize),
        );

        let data = Box::new(AddrSubclassData {
            on_event: Box::new(on_event),
            select_all_on_up: false,
        });
        let raw = Box::into_raw(data) as usize;
        let _ = SetWindowSubclass(hwnd, Some(address_bar_subclass_proc), ADDR_SUBCLASS_ID, raw);

        hwnd
    }
}

pub fn update_address_bar_font(addr_hwnd: HWND, scale: f64) {
    let font = create_font(scale, 14, FW_NORMAL.0 as i32);
    unsafe {
        SendMessageW(addr_hwnd, WM_SETFONT, WPARAM(font.0 as _), LPARAM(1));
    }
    ADDR_FONT_HANDLE.with(|slot| {
        if let Some(old) = slot.borrow_mut().replace(font) {
            unsafe {
                let _ = DeleteObject(old);
            }
        }
    });
}

unsafe extern "system" fn address_bar_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    ref_data: usize,
) -> LRESULT {
    let data = &mut *(ref_data as *mut AddrSubclassData);

    match msg {
        WM_LBUTTONDOWN => {
            if GetFocus() != hwnd {
                data.select_all_on_up = true;
            }
        }

        WM_LBUTTONUP => {
            let select = data.select_all_on_up;
            data.select_all_on_up = false;
            let res = DefSubclassProc(hwnd, msg, wparam, lparam);
            if select {
                SendMessageW(hwnd, EM_SETSEL, WPARAM(0), LPARAM(-1));
            }
            return res;
        }

        WM_KEYDOWN => {
            let vkey = wparam.0 as u16;
            let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
            let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;

            if vkey == VK_RETURN.0 {
                let text = get_address_bar_text(hwnd);
                (data.on_event)(AddrBarEvent::Submit(text));
                return LRESULT(0);
            }
            if vkey == VK_ESCAPE.0 {
                (data.on_event)(AddrBarEvent::Escape);
                return LRESULT(0);
            }
            if vkey == VK_DOWN.0 && !ctrl {
                (data.on_event)(AddrBarEvent::OmniboxKey(OmniboxNavKey::Down));
                return LRESULT(0);
            }
            if vkey == VK_UP.0 && !ctrl {
                (data.on_event)(AddrBarEvent::OmniboxKey(OmniboxNavKey::Up));
                return LRESULT(0);
            }

            if vkey == VK_BACK.0 && ctrl && !shift {
                let text = get_address_bar_text(hwnd);
                let mut sel_start: u32 = 0;
                let mut sel_end: u32 = 0;
                SendMessageW(
                    hwnd,
                    EM_GETSEL,
                    WPARAM(&mut sel_start as *mut u32 as usize),
                    LPARAM(&mut sel_end as *mut u32 as isize),
                );
                if sel_start == sel_end {
                    let utf16: Vec<u16> = text.encode_utf16().collect();
                    let cur = (sel_end as usize).min(utf16.len());
                    let mut start = cur;
                    let is_sep = |u: u16| -> bool {
                        let c = u as u32;
                        c <= 0x20
                            || matches!(
                                c,
                                0x2F | 0x3A | 0x2E | 0x3F | 0x26 | 0x3D | 0x2D | 0x5F
                            )
                    };
                    while start > 0 && is_sep(utf16[start - 1]) {
                        start -= 1;
                    }
                    while start > 0 && !is_sep(utf16[start - 1]) {
                        start -= 1;
                    }
                    SendMessageW(hwnd, EM_SETSEL, WPARAM(start), LPARAM(cur as isize));
                }
                let empty = [0u16];
                SendMessageW(hwnd, EM_REPLACESEL, WPARAM(1), LPARAM(empty.as_ptr() as isize));
                return LRESULT(0);
            }

            if ctrl {
                match vkey {
                    k if k == VK_A.0 || k == VK_L.0 => {
                        SendMessageW(hwnd, EM_SETSEL, WPARAM(0), LPARAM(-1));
                        return LRESULT(0);
                    }
                    k if k == VK_T.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::NewTab));
                        return LRESULT(0);
                    }
                    k if k == VK_W.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::CloseTab));
                        return LRESULT(0);
                    }
                    k if k == VK_B.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::ToggleSidebar));
                        return LRESULT(0);
                    }
                    k if k == VK_R.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::Reload));
                        return LRESULT(0);
                    }
                    k if k == VK_D.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::ToggleBookmark));
                        return LRESULT(0);
                    }
                    k if k == VK_H.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::OpenHistory));
                        return LRESULT(0);
                    }
                    k if k == VK_J.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::OpenDownloads));
                        return LRESULT(0);
                    }
                    k if k == VK_O.0 && shift => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::OpenBookmarks));
                        return LRESULT(0);
                    }
                    k if k == VK_OEM_COMMA.0 => {
                        (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::OpenSettings));
                        return LRESULT(0);
                    }
                    _ => {}
                }
            } else if vkey == VK_F5.0 {
                (data.on_event)(AddrBarEvent::Shortcut(AcceleratorAction::Reload));
                return LRESULT(0);
            }
        }

        // Prevent Windows system beep on Enter / Escape / browser Ctrl shortcuts / Ctrl+Backspace (0x7F),
        // while keeping Backspace (0x08 without Ctrl), Ctrl+C/V/X/Z, and Delete working normally.
        WM_CHAR => {
            let ch = wparam.0 as u32;
            let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
            let is_app_ctrl = ctrl
                && matches!(
                    ch,
                    0x01 | 0x02 | 0x04 | 0x08 | 0x0A | 0x0C | 0x0F | 0x12 | 0x14 | 0x17
                );
            if ch == 0x0D || ch == 0x1B || ch == 0x7F || is_app_ctrl {
                return LRESULT(0);
            }
        }

        WM_SETFOCUS => {
            (data.on_event)(AddrBarEvent::FocusChanged(true));
            (data.on_event)(AddrBarEvent::Repaint);
        }

        WM_KILLFOCUS => {
            data.select_all_on_up = false;
            (data.on_event)(AddrBarEvent::FocusChanged(false));
            (data.on_event)(AddrBarEvent::Repaint);
        }

        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(hwnd, Some(address_bar_subclass_proc), ADDR_SUBCLASS_ID);
            drop(Box::from_raw(ref_data as *mut AddrSubclassData));
        }

        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

pub fn position_address_bar(addr_hwnd: HWND, layout: &Layout, has_shield_badge: bool) {
    if !layout.sidebar_expanded {
        unsafe {
            let _ = ShowWindow(addr_hwnd, SW_HIDE);
        }
        return;
    }
    let scale = layout.dpi_scale;
    let frame = address_bar_frame_rect(layout);
    let btns = topbar_button_rects(layout, has_shield_badge);
    let edit_left = frame.left + scale_i32(27, scale);
    let edit_right = (btns.shield.left - scale_i32(4, scale)).max(edit_left + 20);
    let width = (edit_right - edit_left).max(20);
    let edit_h = scale_i32(22, scale).max(16);
    let y = frame.top + ((frame.bottom - frame.top - edit_h) / 2);

    unsafe {
        let _ = SetWindowPos(
            addr_hwnd,
            None,
            edit_left,
            y,
            width,
            edit_h,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn set_address_bar_text(addr_hwnd: HWND, text: &str) {
    SETTING_ADDR_TEXT.store(true, Ordering::Relaxed);
    unsafe {
        let wtext = wide(text);
        let _ = SetWindowTextW(addr_hwnd, PCWSTR(wtext.as_ptr()));
    }
    SETTING_ADDR_TEXT.store(false, Ordering::Relaxed);
}

pub fn set_address_bar_preview(addr_hwnd: HWND, text: &str) {
    set_address_bar_text(addr_hwnd, text);
    unsafe {
        let len = text.encode_utf16().count() as isize;
        SendMessageW(addr_hwnd, EM_SETSEL, WPARAM(len as usize), LPARAM(len));
    }
}

pub fn get_edit_text(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let copied = GetWindowTextW(hwnd, &mut buf) as usize;
        String::from_utf16_lossy(&buf[..copied.min(buf.len())])
    }
}

pub fn get_address_bar_text(addr_hwnd: HWND) -> String {
    get_edit_text(addr_hwnd)
}

pub fn is_address_bar_focused(addr_hwnd: HWND) -> bool {
    unsafe { GetFocus() == addr_hwnd }
}

// ── Inline Folder Rename Subclass (EDIT Control) ──

pub const FOLDER_RENAME_CTRL_ID: u16 = 101;
const FOLDER_RENAME_SUBCLASS_ID: usize = 201;

struct FolderRenameSubclassData {
    on_event: Box<dyn Fn(FolderRenameEvent) + 'static>,
}

thread_local! {
    static FOLDER_RENAME_FONT: RefCell<Option<HGDIOBJ>> = const { RefCell::new(None) };
}

pub fn create_folder_rename_edit(
    parent: HWND,
    scale: f64,
    on_event: impl Fn(FolderRenameEvent) + 'static,
) -> HWND {
    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap_or_default();
        let style = WS_CHILD | WS_BORDER | WINDOW_STYLE(ES_AUTOHSCROLL as u32);

        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            style,
            0,
            0,
            120,
            22,
            parent,
            HMENU(FOLDER_RENAME_CTRL_ID as usize as _),
            hinstance,
            None,
        )
        .unwrap_or_default();

        update_folder_rename_font(hwnd, scale);

        let data = Box::new(FolderRenameSubclassData {
            on_event: Box::new(on_event),
        });
        let raw = Box::into_raw(data) as usize;
        let _ = SetWindowSubclass(
            hwnd,
            Some(folder_rename_subclass_proc),
            FOLDER_RENAME_SUBCLASS_ID,
            raw,
        );
        hwnd
    }
}

pub fn update_folder_rename_font(edit_hwnd: HWND, scale: f64) {
    let font = create_font(scale, 13, FW_MEDIUM.0 as i32);
    unsafe {
        SendMessageW(edit_hwnd, WM_SETFONT, WPARAM(font.0 as _), LPARAM(1));
    }
    FOLDER_RENAME_FONT.with(|slot| {
        if let Some(old) = slot.borrow_mut().replace(font) {
            unsafe {
                let _ = DeleteObject(old);
            }
        }
    });
}

pub fn show_folder_rename_edit(edit_hwnd: HWND, rect: &RECT, initial_name: &str) {
    unsafe {
        let wtext = wide(initial_name);
        let _ = SetWindowTextW(edit_hwnd, PCWSTR(wtext.as_ptr()));
        let _ = SetWindowPos(
            edit_hwnd,
            HWND_TOP,
            rect.left,
            rect.top,
            (rect.right - rect.left).max(40),
            (rect.bottom - rect.top).max(18),
            SWP_SHOWWINDOW,
        );
        let _ = SetFocus(edit_hwnd);
        SendMessageW(edit_hwnd, EM_SETSEL, WPARAM(0), LPARAM(-1));
    }
}

pub fn hide_folder_rename_edit(edit_hwnd: HWND) {
    if !edit_hwnd.0.is_null() {
        unsafe {
            let _ = ShowWindow(edit_hwnd, SW_HIDE);
        }
    }
}

unsafe extern "system" fn folder_rename_subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _uid: usize,
    ref_data: usize,
) -> LRESULT {
    let data = &*(ref_data as *const FolderRenameSubclassData);

    match msg {
        WM_KEYDOWN => {
            let vkey = wparam.0 as u16;
            if vkey == VK_RETURN.0 {
                (data.on_event)(FolderRenameEvent::Commit(get_edit_text(hwnd)));
                return LRESULT(0);
            }
            if vkey == VK_ESCAPE.0 {
                (data.on_event)(FolderRenameEvent::Cancel);
                return LRESULT(0);
            }
        }
        WM_CHAR => {
            let ch = wparam.0 as u32;
            if ch == 0x0D || ch == 0x1B {
                return LRESULT(0);
            }
        }
        WM_KILLFOCUS => {
            if IsWindowVisible(hwnd).as_bool() {
                (data.on_event)(FolderRenameEvent::Commit(get_edit_text(hwnd)));
            }
        }
        WM_NCDESTROY => {
            let _ = RemoveWindowSubclass(
                hwnd,
                Some(folder_rename_subclass_proc),
                FOLDER_RENAME_SUBCLASS_ID,
            );
            drop(Box::from_raw(ref_data as *mut FolderRenameSubclassData));
        }
        _ => {}
    }

    DefSubclassProc(hwnd, msg, wparam, lparam)
}

struct EditBrushState {
    bg_ref: COLORREF,
    brush: HBRUSH,
}

thread_local! {
    static EDIT_BRUSH: RefCell<Option<EditBrushState>> = const { RefCell::new(None) };
}

pub fn handle_ctlcoloredit(wparam: WPARAM, palette: &ThemePalette) -> Option<LRESULT> {
    let text_ref = rgb(palette.text_primary);
    let bg_ref = rgb(palette.bg_surface);

    unsafe {
        let hdc = HDC(wparam.0 as _);
        SetTextColor(hdc, text_ref);
        SetBkColor(hdc, bg_ref);

        EDIT_BRUSH.with(|slot| {
            let mut guard = slot.borrow_mut();
            let needs_rebuild = match *guard {
                Some(ref st) => st.bg_ref != bg_ref,
                None => true,
            };
            if needs_rebuild {
                if let Some(old) = guard.take() {
                    let _ = DeleteObject(old.brush);
                }
                let brush = CreateSolidBrush(bg_ref);
                *guard = Some(EditBrushState { bg_ref, brush });
            }
            guard.as_ref().map(|st| LRESULT(st.brush.0 as _))
        })
    }
}

// ── Omnibox Autocomplete Popup Window ──

pub struct OmniboxPopupState {
    pub suggestions: Vec<Suggestion>,
    pub selected_idx: Option<usize>,
    pub hover_idx: Option<usize>,
    pub palette: ThemePalette,
    pub dpi_scale: f64,
    pub favicons: Rc<RefCell<FaviconCache>>,
    pub on_pick: Box<dyn Fn(usize) + 'static>,
    pub tracking_mouse: bool,
    pub visible: bool,
}

pub struct OmniboxPopup {
    pub hwnd: HWND,
    pub state: Rc<RefCell<OmniboxPopupState>>,
    class_registered: bool,
}

impl OmniboxPopup {
    pub fn create(
        parent: HWND,
        scale: f64,
        palette: ThemePalette,
        favicons: Rc<RefCell<FaviconCache>>,
        on_pick: impl Fn(usize) + 'static,
    ) -> Self {
        let class_name = wide("KurowserOmniboxPopup");
        let hinstance = unsafe { GetModuleHandleW(None).unwrap_or_default() };

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW | CS_DROPSHADOW,
            lpfnWndProc: Some(omnibox_wnd_proc),
            hInstance: hinstance.into(),
            hCursor: unsafe { LoadCursorW(None, IDC_HAND).unwrap_or_default() },
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        let atom = unsafe { RegisterClassExW(&wc) };

        let state = Rc::new(RefCell::new(OmniboxPopupState {
            suggestions: Vec::new(),
            selected_idx: None,
            hover_idx: None,
            palette,
            dpi_scale: scale,
            favicons,
            on_pick: Box::new(on_pick),
            tracking_mouse: false,
            visible: false,
        }));

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                PCWSTR(class_name.as_ptr()),
                w!(""),
                WS_POPUP,
                0,
                0,
                400,
                100,
                parent,
                None,
                hinstance,
                None,
            )
            .unwrap_or_default()
        };

        if !hwnd.0.is_null() {
            let ptr = Rc::into_raw(Rc::clone(&state)) as isize;
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr);
            }
        }

        Self {
            hwnd,
            state,
            class_registered: atom != 0,
        }
    }

    pub fn update_and_show(
        &self,
        parent: HWND,
        layout: &Layout,
        suggestions: Vec<Suggestion>,
    ) {
        let is_empty = suggestions.is_empty();
        {
            let mut st = self.state.borrow_mut();
            st.suggestions = suggestions;
            st.selected_idx = None;
            st.hover_idx = None;
            st.dpi_scale = layout.dpi_scale;
            st.visible = !is_empty;
        }
        self.sync_geometry(parent, layout);
    }

    pub fn sync_geometry(&self, parent: HWND, layout: &Layout) {
        if self.hwnd.0.is_null() {
            return;
        }
        let scale = layout.dpi_scale;
        let (visible, count) = {
            let mut st = self.state.borrow_mut();
            st.dpi_scale = scale;
            (st.visible && !st.suggestions.is_empty(), st.suggestions.len())
        };

        if !visible {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
            return;
        }

        let frame = address_bar_frame_rect(layout);
        let max_win_w = (layout.window.right - layout.window.left - scale_i32(16, scale)).max(200);
        let width = if layout.sidebar_expanded {
            scale_i32(440, scale).min(max_win_w)
        } else {
            (frame.right - frame.left)
                .max(scale_i32(340, scale))
                .min(max_win_w)
        };
        let clamped_x = frame
            .left
            .min((layout.window.right - width - scale_i32(8, scale)).max(scale_i32(8, scale)));
        let mut pt = POINT {
            x: clamped_x,
            y: frame.bottom + scale_i32(4, scale),
        };
        unsafe {
            let _ = ClientToScreen(parent, &mut pt);
        }
        let pad_v = scale_i32(6, scale);
        let row_h = scale_i32(OMNIBOX_ROW_H, scale);
        let height = (count as i32) * row_h + pad_v * 2;

        unsafe {
            let _ = SetWindowPos(
                self.hwnd,
                HWND_TOP,
                pt.x,
                pt.y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let _ = InvalidateRect(self.hwnd, None, false);
        }
    }

    pub fn hide(&self) {
        {
            let mut st = self.state.borrow_mut();
            st.visible = false;
            st.selected_idx = None;
            st.hover_idx = None;
        }
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_HIDE);
            }
        }
    }

    pub fn is_visible(&self) -> bool {
        let st = self.state.borrow();
        st.visible && !st.suggestions.is_empty()
    }

    pub fn step_selection(&self, key: OmniboxNavKey) -> Option<Suggestion> {
        let picked = {
            let mut st = self.state.borrow_mut();
            let len = st.suggestions.len();
            if len == 0 || !st.visible {
                return None;
            }
            let next = match (key, st.selected_idx) {
                (OmniboxNavKey::Down, None) => 0,
                (OmniboxNavKey::Down, Some(i)) => (i + 1) % len,
                (OmniboxNavKey::Up, None) => len - 1,
                (OmniboxNavKey::Up, Some(i)) => {
                    if i == 0 {
                        len - 1
                    } else {
                        i - 1
                    }
                }
            };
            st.selected_idx = Some(next);
            st.suggestions.get(next).cloned()
        };
        unsafe {
            let _ = InvalidateRect(self.hwnd, None, false);
        }
        picked
    }

    pub fn selected_url(&self) -> Option<String> {
        let st = self.state.borrow();
        if !st.visible {
            return None;
        }
        let idx = st.selected_idx?;
        st.suggestions.get(idx).map(|s| s.url.clone())
    }

    pub fn set_palette(&self, palette: ThemePalette) {
        self.state.borrow_mut().palette = palette;
        if self.is_visible() {
            unsafe {
                let _ = InvalidateRect(self.hwnd, None, false);
            }
        }
    }

    pub fn repaint_if_visible(&self) {
        if self.is_visible() {
            unsafe {
                let _ = InvalidateRect(self.hwnd, None, false);
            }
        }
    }
}

impl Drop for OmniboxPopup {
    fn drop(&mut self) {
        let _ = self.class_registered;
        if !self.hwnd.0.is_null() {
            unsafe {
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

unsafe extern "system" fn omnibox_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<OmniboxPopupState>;
    if ptr.is_null() {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    match msg {
        WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),

        WM_ERASEBKGND => return LRESULT(1),

        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Ok(st) = (&*ptr).try_borrow() {
                paint_omnibox_popup(hwnd, hdc, &st);
            }
            let _ = EndPaint(hwnd, &ps);
            return LRESULT(0);
        }

        WM_MOUSEMOVE => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let mut changed = false;
            if let Ok(mut st) = (&*ptr).try_borrow_mut() {
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
                let pad_v = scale_i32(6, st.dpi_scale);
                let row_h = scale_i32(OMNIBOX_ROW_H, st.dpi_scale).max(1);
                let idx = if y >= pad_v {
                    let i = ((y - pad_v) / row_h) as usize;
                    if i < st.suggestions.len() {
                        Some(i)
                    } else {
                        None
                    }
                } else {
                    None
                };
                if st.hover_idx != idx {
                    st.hover_idx = idx;
                    changed = true;
                }
            }
            if changed {
                let _ = InvalidateRect(hwnd, None, false);
            }
            return LRESULT(0);
        }

        WM_MOUSELEAVE => {
            let mut changed = false;
            if let Ok(mut st) = (&*ptr).try_borrow_mut() {
                st.tracking_mouse = false;
                if st.hover_idx.is_some() {
                    st.hover_idx = None;
                    changed = true;
                }
            }
            if changed {
                let _ = InvalidateRect(hwnd, None, false);
            }
            return LRESULT(0);
        }

        WM_LBUTTONDOWN => {
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            if let Ok(st) = (&*ptr).try_borrow() {
                let pad_v = scale_i32(6, st.dpi_scale);
                let row_h = scale_i32(OMNIBOX_ROW_H, st.dpi_scale).max(1);
                if y >= pad_v {
                    let i = ((y - pad_v) / row_h) as usize;
                    if i < st.suggestions.len() {
                        (st.on_pick)(i);
                    }
                }
            }
            return LRESULT(0);
        }

        WM_NCDESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            drop(Rc::from_raw(ptr));
            return LRESULT(0);
        }

        _ => {}
    }

    DefWindowProcW(hwnd, msg, wparam, lparam)
}

unsafe fn paint_omnibox_popup(hwnd: HWND, hdc_screen: HDC, st: &OmniboxPopupState) {
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    let width = rc.right - rc.left;
    let height = rc.bottom - rc.top;
    if width <= 0 || height <= 0 {
        return;
    }

    let scale = st.dpi_scale;
    let p = &st.palette;
    let mem_dc = CreateCompatibleDC(hdc_screen);
    let mem_bmp = CreateCompatibleBitmap(hdc_screen, width, height);
    let old_bmp = SelectObject(mem_dc, mem_bmp);

    let bg_br = CreateSolidBrush(rgb(p.bg_chrome));
    FillRect(mem_dc, &rc, bg_br);
    let _ = DeleteObject(bg_br);

    // Outer frame border
    let frame_br = CreateSolidBrush(rgb(p.border));
    FrameRect(mem_dc, &rc, frame_br);
    let _ = DeleteObject(frame_br);

    SetBkMode(mem_dc, TRANSPARENT);
    let font_title = create_font(scale, 13, FW_MEDIUM.0 as i32);
    let font_sub = create_font(scale, 11, FW_NORMAL.0 as i32);
    let font_badge = create_font(scale, 10, FW_MEDIUM.0 as i32);
    let old_font = SelectObject(mem_dc, font_title);

    let pad_v = scale_i32(6, scale);
    let pad_x = scale_i32(8, scale);
    let row_h = scale_i32(OMNIBOX_ROW_H, scale);
    let fav_sz = scale_i32(20, scale);
    let fav_cache = st.favicons.borrow();

    for (i, item) in st.suggestions.iter().enumerate() {
        let top = pad_v + (i as i32) * row_h;
        let row_rc = RECT {
            left: pad_x,
            top,
            right: width - pad_x,
            bottom: top + row_h - scale_i32(2, scale),
        };

        let is_sel = st.selected_idx == Some(i);
        let is_hov = st.hover_idx == Some(i);

        if is_sel {
            draw_rounded_fill(
                mem_dc,
                &row_rc,
                scale_i32(8, scale),
                rgb(p.bg_active),
                rgb(p.border_subtle),
            );
            let bar = RECT {
                left: row_rc.left + scale_i32(2, scale),
                top: row_rc.top + scale_i32(9, scale),
                right: row_rc.left + scale_i32(5, scale),
                bottom: row_rc.bottom - scale_i32(9, scale),
            };
            let abr = CreateSolidBrush(rgb(p.accent));
            FillRect(mem_dc, &bar, abr);
            let _ = DeleteObject(abr);
        } else if is_hov {
            draw_rounded_fill(
                mem_dc,
                &row_rc,
                scale_i32(8, scale),
                rgb(p.bg_hover),
                rgb(p.bg_hover),
            );
        }

        // Icon
        let ico_x = row_rc.left + scale_i32(12, scale);
        let ico_y = row_rc.top + (row_rc.bottom - row_rc.top - fav_sz) / 2;
        draw_site_icon(
            mem_dc,
            &fav_cache,
            p,
            &item.domain,
            item.kind == SuggestionKind::Search,
            ico_x,
            ico_y,
            fav_sz,
            scale,
            font_badge,
        );

        // Quiet right-aligned tabular metadata label (no boxy capsule pill overload)
        let badge_text = match item.kind {
            SuggestionKind::Bookmark => "Bookmark",
            SuggestionKind::History => "History",
            SuggestionKind::TopSite => "Situs",
            SuggestionKind::Internal => "Kurowser",
            SuggestionKind::DirectUrl => "Buka URL",
            SuggestionKind::Search => "Cari",
        };
        let badge_w = scale_i32(62, scale);
        let badge_rc = RECT {
            left: row_rc.right - badge_w - scale_i32(10, scale),
            top: row_rc.top,
            right: row_rc.right - scale_i32(10, scale),
            bottom: row_rc.bottom,
        };
        let bfg = if item.kind == SuggestionKind::Bookmark {
            p.warning_fg
        } else if is_sel || is_hov {
            p.accent_text
        } else {
            p.text_muted
        };
        SelectObject(mem_dc, font_badge);
        SetTextColor(mem_dc, rgb(bfg));
        let mut b_w = wide(badge_text);
        let mut b_rc = badge_rc;
        DrawTextW(mem_dc, &mut b_w, &mut b_rc, DT_RIGHT | DT_SINGLELINE | DT_VCENTER);

        // Title line
        let txt_left = ico_x + fav_sz + scale_i32(12, scale);
        let txt_right = badge_rc.left - scale_i32(10, scale);
        SelectObject(mem_dc, font_title);
        SetTextColor(mem_dc, rgb(p.text_primary));
        let mut t_w = wide(&item.title);
        let mut t_rc = RECT {
            left: txt_left,
            top: row_rc.top + scale_i32(4, scale),
            right: txt_right,
            bottom: row_rc.top + scale_i32(24, scale),
        };
        DrawTextW(
            mem_dc,
            &mut t_w,
            &mut t_rc,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );

        // Subtitle / URL line
        SelectObject(mem_dc, font_sub);
        SetTextColor(
            mem_dc,
            rgb(if is_sel || is_hov {
                p.accent_text
            } else {
                p.text_muted
            }),
        );
        let mut s_w = wide(&item.subtitle);
        let mut s_rc = RECT {
            left: txt_left,
            top: row_rc.top + scale_i32(22, scale),
            right: txt_right,
            bottom: row_rc.bottom - scale_i32(2, scale),
        };
        DrawTextW(
            mem_dc,
            &mut s_w,
            &mut s_rc,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }

    let _ = BitBlt(hdc_screen, 0, 0, width, height, mem_dc, 0, 0, SRCCOPY);

    SelectObject(mem_dc, old_font);
    SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(font_title);
    let _ = DeleteObject(font_sub);
    let _ = DeleteObject(font_badge);
    let _ = DeleteObject(mem_bmp);
    let _ = DeleteDC(mem_dc);
}

// ── Win32 Helpers ──

fn create_font(scale: f64, size: i32, weight: i32) -> HGDIOBJ {
    unsafe {
        let h = -scale_i32(size, scale);
        let name = wide("Poppins");
        let hfont = CreateFontW(
            h,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET.0 as u32,
            OUT_DEFAULT_PRECIS.0 as u32,
            CLIP_DEFAULT_PRECIS.0 as u32,
            CLEARTYPE_QUALITY.0 as u32,
            (DEFAULT_PITCH.0 | FF_SWISS.0) as u32,
            PCWSTR(name.as_ptr()),
        );
        HGDIOBJ(hfont.0)
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn apply_window_theme(hwnd: HWND, is_light: bool) {
    unsafe {
        let dark: BOOL = if is_light { FALSE } else { TRUE };
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const BOOL as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
}

pub fn configure_borderless_frame(hwnd: HWND, is_light: bool) {
    apply_window_theme(hwnd, is_light);
    unsafe {
        let margins = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 1,
            cyBottomHeight: 0,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let corner = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rects_overlap(a: &RECT, b: &RECT) -> bool {
        a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
    }

    #[test]
    fn expanded_layout_places_caption_buttons_at_top_right_without_overlap() {
        for &(w, h, scale) in &[(1180, 760, 1.0), (560, 380, 1.0), (1475, 950, 1.25)] {
            let layout = Layout::compute_with_metrics(w, h, scale, false, true);
            assert_eq!(layout.topbar.bottom, scale_i32(TOPBAR_HEIGHT, scale));
            assert_eq!(layout.content.top, layout.topbar.bottom);
            for &badge in &[false, true] {
                let b = topbar_button_rects(&layout, badge);
                let row1 = [
                    b.toggle, b.back, b.forward, b.reload, b.win_min, b.win_max, b.win_close,
                ];
                for i in 0..row1.len() {
                    assert!(row1[i].right > row1[i].left);
                    assert!(row1[i].bottom <= scale_i32(42, scale));
                    for j in (i + 1)..row1.len() {
                        assert!(!rects_overlap(&row1[i], &row1[j]));
                    }
                }
                assert!(b.toggle.left >= scale_i32(28, scale));
                assert!(b.reload.right <= layout.sidebar.right);
                assert!(b.win_min.left >= layout.sidebar.right);
                assert!(b.win_close.right <= layout.window.right);
                assert!(b.win_close.bottom <= layout.content.top);
                let frame = address_bar_frame_rect(&layout);
                assert!(b.shield.left >= frame.left + scale_i32(60, scale));
                assert!(b.shield.right <= b.star.left);
                assert!(b.star.right <= frame.right);
            }
        }
    }

    #[test]
    fn collapsed_rail_is_72px_with_top_right_captions_and_clean_tiles() {
        for &(w, h) in &[(1180, 760), (560, 380)] {
            let layout = Layout::compute_with_metrics(w, h, 1.0, false, false);
            assert_eq!(layout.topbar.bottom, scale_i32(TOPBAR_HEIGHT, 1.0));
            assert_eq!(layout.sidebar.right, COLLAPSED_RAIL_WIDTH);
            assert_eq!(layout.content.left, COLLAPSED_RAIL_WIDTH + FRAME_PAD);
            assert!(address_bar_frame_rect(&layout).right == 0);

            let b = topbar_button_rects(&layout, true);
            for cap in [b.win_min, b.win_max, b.win_close] {
                assert!(cap.left >= COLLAPSED_RAIL_WIDTH);
                assert!(cap.right <= layout.window.right);
                assert!(cap.bottom <= layout.content.top);
            }
            let rail_btns = [
                b.toggle,
                b.rail_search,
                b.back,
                b.forward,
                b.reload,
                b.shield,
                b.new_tab,
                b.star,
                b.rail_settings,
            ];
            for i in 0..rail_btns.len() {
                assert!(rail_btns[i].left >= 0 && rail_btns[i].right <= COLLAPSED_RAIL_WIDTH);
                assert!(rail_btns[i].top >= 0 && rail_btns[i].bottom <= h);
                for j in (i + 1)..rail_btns.len() {
                    assert!(!rects_overlap(&rail_btns[i], &rail_btns[j]));
                }
            }
            let (ltop, lbot) = rail_list_bounds(&layout.sidebar, 1.0, &b);
            assert!(lbot > ltop + 80);
        }
    }

    #[test]
    fn drag_zones_exclude_buttons_and_content() {
        let layout = Layout::compute_with_metrics(1180, 760, 1.0, false, true);
        let b = topbar_button_rects(&layout, false);
        assert!(window_drag_area_hit(
            &layout,
            false,
            (layout.topbar.left + b.win_min.left) / 2,
            16
        ));
        assert!(window_drag_area_hit(&layout, false, (40 + b.toggle.left) / 2, 18));
        assert!(!window_drag_area_hit(&layout, false, b.toggle.left + 4, 18));
        assert!(!window_drag_area_hit(&layout, false, b.win_close.left + 2, 16));
        assert!(!window_drag_area_hit(&layout, false, 600, 200));
    }

    #[test]
    fn supersampled_shield_keeps_premultiplied_invariants() {
        let fill = Rgb(0x18, 0x38, 0x36);
        let stroke = Rgb(0x5e, 0xea, 0xd4);
        let glyph = Rgb(0xfd, 0xa4, 0xaf);
        let bg = Rgb(0x1e, 0x24, 0x2d);
        for &mode in &[0u8, 1u8, 2u8] {
            let px = render_shield_premul(16, mode, fill, stroke, glyph, bg);
            assert_eq!(px.len(), 16 * 16 * 4);
            let mut non_zero_alpha = 0usize;
            for c in px.chunks_exact(4) {
                let a = c[3];
                assert!(a >= 0.0 && a <= 1.0001);
                assert!(c[0] <= a + 0.001 && c[1] <= a + 0.001 && c[2] <= a + 0.001);
                if a > 0.05 {
                    non_zero_alpha += 1;
                }
            }
            assert!(non_zero_alpha > 40);
        }
    }
}
