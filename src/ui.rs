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
use windows::Win32::UI::Controls::{EM_GETSEL, EM_REPLACESEL, EM_SETSEL, WM_MOUSELEAVE};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::Shell::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::engine::{AcceleratorAction, PixelRect};
use crate::omnibox::{Suggestion, SuggestionKind};
use crate::storage::{Rgb, ThemePalette};
use crate::tabs::{FolderId, SidebarEntry, TabId};

// ── Layout constants (logical px at 96 DPI) ──
pub const TOPBAR_HEIGHT: i32 = 48;
pub const SIDEBAR_WIDTH: i32 = 248;
pub const TAB_ITEM_HEIGHT: i32 = 36;
pub const BUTTON_SIZE: i32 = 32;
pub const OMNIBOX_ROW_H: i32 = 46;

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
    ToggleSidebar,
    Back,
    Forward,
    ReloadOrStop,
    Shield,
    BookmarkStar,
    NewTab,
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

pub struct FaviconCache {
    logos: RefCell<HashMap<i32, DibBitmap>>,
    icons: RefCell<HashMap<String, CachedSiteIcon>>,
    order: Vec<String>,
}

impl FaviconCache {
    pub fn new() -> Self {
        Self {
            logos: RefCell::new(HashMap::new()),
            icons: RefCell::new(HashMap::new()),
            order: Vec::new(),
        }
    }

    pub fn clear_scaled_caches(&self) {
        self.logos.borrow_mut().clear();
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
                    let (mut r, mut g, mut b) = if t < 0.5 {
                        let u = t * 2.0;
                        (
                            (99.0 + (124.0 - 99.0) * u) / 255.0,
                            (102.0 + (58.0 - 102.0) * u) / 255.0,
                            (241.0 + (237.0 - 241.0) * u) / 255.0,
                        )
                    } else {
                        let u = (t - 0.5) * 2.0;
                        (
                            (124.0 + (147.0 - 124.0) * u) / 255.0,
                            (58.0 + (51.0 - 58.0) * u) / 255.0,
                            (237.0 + (234.0 - 237.0) * u) / 255.0,
                        )
                    };

                    if d_box > -1.8 && py < 30.0 {
                        let rim = ((1.0 - (d_box + 0.8).abs()) * (1.0 - (py - 2.5) / 28.0))
                            .clamp(0.0, 1.0)
                            * 0.24;
                        r += (215.0 / 255.0 - r) * rim;
                        g += (210.0 / 255.0 - g) * rim;
                        b += (1.0 - b) * rim;
                    }

                    let d_k_shad = sd_capsule(px, py - 1.4, 20.5, 18.0, 20.5, 46.0, 3.8)
                        .min(sd_capsule(px, py - 1.4, 24.0, 34.0, 42.5, 18.0, 3.7))
                        .min(sd_capsule(px, py - 1.4, 29.8, 29.0, 43.2, 46.0, 3.7));
                    if d_k_shad < 0.0 {
                        r += (15.0 / 255.0 - r) * 0.16;
                        g += (10.0 / 255.0 - g) * 0.16;
                        b += (35.0 / 255.0 - b) * 0.16;
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
    pub sidebar: RECT,
    pub topbar: RECT,
    pub content: RECT,
    pub dpi_scale: f64,
}

impl Layout {
    pub fn compute(hwnd: HWND, sidebar_expanded: bool) -> Self {
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rc);
        }
        let w = (rc.right - rc.left).max(1);
        let h = (rc.bottom - rc.top).max(1);

        let dpi = unsafe { GetDpiForWindow(hwnd) } as f64;
        let scale = if dpi > 0.0 { dpi / 96.0 } else { 1.0 };

        let sb_w = if sidebar_expanded {
            scale_i32(SIDEBAR_WIDTH, scale).min(w / 2)
        } else {
            0
        };
        let tb_h = scale_i32(TOPBAR_HEIGHT, scale).min(h);

        Layout {
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
                left: sb_w,
                top: tb_h,
                right: w,
                bottom: h,
            },
            dpi_scale: scale,
        }
    }

    pub fn content_bounds(&self) -> PixelRect {
        PixelRect {
            x: self.content.left,
            y: self.content.top,
            width: (self.content.right - self.content.left).max(1),
            height: (self.content.bottom - self.content.top).max(1),
        }
    }
}

struct SidebarGeometry {
    new_btn: RECT,
    new_folder_btn: RECT,
    list_top: i32,
    list_bottom: i32,
    dock_rects: [(SidebarHit, &'static str, RECT); 4],
    footer_top: i32,
    trim_btn: RECT,
    item_h: i32,
    gap: i32,
}

fn sidebar_geometry(rect: &RECT, scale: f64) -> SidebarGeometry {
    let left = rect.left + scale_i32(10, scale);
    let right = rect.right - scale_i32(11, scale);
    let item_h = scale_i32(TAB_ITEM_HEIGHT, scale);
    let gap = scale_i32(4, scale);

    let new_top = rect.top + scale_i32(58, scale);
    let new_h = scale_i32(34, scale);
    let cta_gap = scale_i32(6, scale);
    let folder_w = scale_i32(82, scale);
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

    let footer_h = scale_i32(36, scale);
    let footer_top = (rect.bottom - footer_h).max(rect.top + scale_i32(140, scale));

    let trim_btn = RECT {
        left: right - scale_i32(66, scale),
        top: footer_top + scale_i32(6, scale),
        right,
        bottom: rect.bottom - scale_i32(6, scale),
    };

    // 2x2 Quick Dock right above footer
    let tile_h = scale_i32(30, scale);
    let tile_gap = scale_i32(6, scale);
    let dock_h = tile_h * 2 + tile_gap + scale_i32(14, scale);
    let dock_top = footer_top - dock_h;
    let avail_w = (right - left).max(40);
    let half_w = (avail_w - tile_gap) / 2;
    let r0_y = dock_top + scale_i32(7, scale);
    let r1_y = r0_y + tile_h + tile_gap;

    let dock_rects = [
        (
            SidebarHit::OpenHistory,
            "History",
            RECT {
                left,
                top: r0_y,
                right: left + half_w,
                bottom: r0_y + tile_h,
            },
        ),
        (
            SidebarHit::OpenBookmarks,
            "Bookmarks",
            RECT {
                left: left + half_w + tile_gap,
                top: r0_y,
                right,
                bottom: r0_y + tile_h,
            },
        ),
        (
            SidebarHit::OpenDownloads,
            "Downloads",
            RECT {
                left,
                top: r1_y,
                right: left + half_w,
                bottom: r1_y + tile_h,
            },
        ),
        (
            SidebarHit::OpenSettings,
            "Tema",
            RECT {
                left: left + half_w + tile_gap,
                top: r1_y,
                right,
                bottom: r1_y + tile_h,
            },
        ),
    ];

    let list_top = new_btn.bottom + scale_i32(10, scale);
    let list_bottom = (dock_top - scale_i32(6, scale)).max(list_top);

    SidebarGeometry {
        new_btn,
        new_folder_btn,
        list_top,
        list_bottom,
        dock_rects,
        footer_top,
        trim_btn,
        item_h,
        gap,
    }
}

pub fn max_sidebar_scroll(rect: &RECT, scale: f64, entry_count: usize) -> i32 {
    let g = sidebar_geometry(rect, scale);
    let total_h = (entry_count as i32) * (g.item_h + g.gap);
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
    let g = sidebar_geometry(rect, scale);
    let max_scroll = max_sidebar_scroll(rect, scale, entries.len());
    let scroll = scroll_offset.clamp(0, max_scroll);
    for (i, entry) in entries.iter().enumerate() {
        if let SidebarEntry::FolderHeader { folder_id, .. } = entry {
            if *folder_id == target_fid {
                let y = g.list_top + (i as i32) * (g.item_h + g.gap) - scroll;
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

pub fn paint_content_fallback(hdc: HDC, rect: &RECT, palette: &ThemePalette) {
    if rect.right <= rect.left || rect.bottom <= rect.top {
        return;
    }
    unsafe {
        let brush = CreateSolidBrush(rgb(palette.bg_app));
        FillRect(hdc, rect, brush);
        let _ = DeleteObject(brush);
    }
}

// ── Double-Buffered Sidebar Render ──

pub fn paint_sidebar(
    hdc_screen: HDC,
    rect: &RECT,
    scale: f64,
    palette: &ThemePalette,
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

        // Background + right divider
        let bg_brush = CreateSolidBrush(rgb(palette.bg_chrome));
        FillRect(mem_dc, &local, bg_brush);
        let _ = DeleteObject(bg_brush);

        let div_brush = CreateSolidBrush(rgb(palette.border));
        let div_rect = RECT {
            left: width - 1,
            top: 0,
            right: width,
            bottom: height,
        };
        FillRect(mem_dc, &div_rect, div_brush);
        let _ = DeleteObject(div_brush);

        SetBkMode(mem_dc, TRANSPARENT);

        let font_title = create_font(scale, 16, FW_SEMIBOLD.0 as i32);
        let font_tab = create_font(scale, 13, FW_NORMAL.0 as i32);
        let font_tab_active = create_font(scale, 13, FW_MEDIUM.0 as i32);
        let font_dock = create_font(scale, 12, FW_MEDIUM.0 as i32);
        let font_small = create_font(scale, 11, FW_MEDIUM.0 as i32);
        let old_font = SelectObject(mem_dc, font_title);

        // Brand header: [K] Kurowser
        let logo_sz = scale_i32(22, scale);
        let logo_x = scale_i32(14, scale);
        let logo_y = scale_i32(14, scale);
        favicons.draw_logo(mem_dc, logo_x, logo_y, logo_sz);

        SetTextColor(mem_dc, rgb(palette.text_primary));
        let mut brand = wide("Kurowser");
        let mut brand_rect = RECT {
            left: logo_x + logo_sz + scale_i32(10, scale),
            top: scale_i32(10, scale),
            right: width - scale_i32(12, scale),
            bottom: scale_i32(42, scale),
        };
        DrawTextW(
            mem_dc,
            &mut brand,
            &mut brand_rect,
            DT_LEFT | DT_SINGLELINE | DT_VCENTER,
        );

        let hdiv_brush = CreateSolidBrush(rgb(palette.border));
        let hdiv = RECT {
            left: scale_i32(12, scale),
            top: scale_i32(50, scale),
            right: width - scale_i32(12, scale),
            bottom: scale_i32(51, scale),
        };
        FillRect(mem_dc, &hdiv, hdiv_brush);
        let _ = DeleteObject(hdiv_brush);

        let g = sidebar_geometry(&local, scale);

        // Split CTA Row: "+ New Tab" and "+ Folder"
        let new_hover = matches!(hover, SidebarHit::NewTab);
        let (nb_fill, nb_border, nb_txt) = if new_hover {
            (palette.accent_soft, palette.accent, palette.text_primary)
        } else {
            (palette.bg_surface, palette.border, palette.accent)
        };
        draw_rounded_fill(
            mem_dc,
            &g.new_btn,
            scale_i32(10, scale),
            rgb(nb_fill),
            rgb(nb_border),
        );
        draw_plus_icon(
            mem_dc,
            g.new_btn.left + scale_i32(16, scale),
            (g.new_btn.top + g.new_btn.bottom) / 2,
            scale_i32(5, scale),
            rgb(nb_txt),
            scale,
        );
        SelectObject(mem_dc, font_tab_active);
        SetTextColor(mem_dc, rgb(nb_txt));
        let mut new_lbl = wide("New Tab");
        let mut new_lbl_rc = RECT {
            left: g.new_btn.left + scale_i32(28, scale),
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
            (palette.accent_soft, palette.accent, palette.text_primary)
        } else {
            (palette.bg_surface, palette.border, palette.text_muted)
        };
        draw_rounded_fill(
            mem_dc,
            &g.new_folder_btn,
            scale_i32(10, scale),
            rgb(fb_fill),
            rgb(fb_border),
        );
        draw_folder_glyph(
            mem_dc,
            g.new_folder_btn.left + scale_i32(11, scale),
            (g.new_folder_btn.top + g.new_folder_btn.bottom) / 2 - scale_i32(6, scale),
            scale_i32(13, scale),
            rgb(if folder_hover { palette.accent } else { fb_txt }),
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

        // Clipped scrollable tab & folder list
        let max_scroll = max_sidebar_scroll(&local, scale, entries.len());
        let scroll = scroll_offset.clamp(0, max_scroll);
        let saved = SaveDC(mem_dc);
        IntersectClipRect(mem_dc, 0, g.list_top, width - 1, g.list_bottom);

        let mut last_item_bottom = g.list_top;

        for (i, entry) in entries.iter().enumerate() {
            let y = g.list_top + (i as i32) * (g.item_h + g.gap) - scroll;
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
                        right: width - scale_i32(11, scale),
                        bottom: y + g.item_h,
                    };
                    let is_hovered = matches!(
                        hover,
                        SidebarHit::FolderToggle(idx, _)
                            | SidebarHit::FolderAddTab(idx, _)
                            | SidebarHit::FolderMenu(idx, _)
                            if idx == i
                    );
                    let is_drop = drag_target == Some(DragDropTarget::Folder(*folder_id));
                    let is_renaming = renaming_folder == Some(*folder_id);

                    if is_drop {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(10, scale),
                            rgb(color.mix(palette.bg_surface, 0.72)),
                            rgb(palette.accent),
                        );
                    } else if is_hovered {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(10, scale),
                            rgb(palette.bg_hover),
                            rgb(color.mix(palette.border, 0.6)),
                        );
                    } else if *collapsed && *has_active_child {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(10, scale),
                            rgb(color.mix(palette.bg_chrome, 0.88)),
                            rgb(color.mix(palette.border, 0.45)),
                        );
                    }

                    // Chevron (> when collapsed, v when expanded)
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

                    // Vector Folder glyph in folder color
                    let fld_sz = scale_i32(15, scale);
                    let fld_x = item_rect.left + scale_i32(18, scale);
                    let fld_y = item_rect.top + (g.item_h - fld_sz) / 2;
                    draw_folder_glyph(mem_dc, fld_x, fld_y, fld_sz, rgb(*color), !*collapsed);

                    if !is_renaming {
                        SelectObject(mem_dc, font_tab_active);
                        let title_col = if is_hovered || !*collapsed {
                            palette.text_primary
                        } else {
                            palette.text_muted.mix(palette.text_primary, 0.55)
                        };
                        SetTextColor(mem_dc, rgb(title_col));

                        let right_reserve = if is_hovered {
                            scale_i32(52, scale)
                        } else {
                            scale_i32(34, scale)
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
                            top: item_rect.top + scale_i32(7, scale),
                            right: item_rect.right - scale_i32(26, scale),
                            bottom: item_rect.bottom - scale_i32(7, scale),
                        };
                        let menu_box = RECT {
                            left: item_rect.right - scale_i32(24, scale),
                            top: item_rect.top + scale_i32(7, scale),
                            right: item_rect.right - scale_i32(4, scale),
                            bottom: item_rect.bottom - scale_i32(7, scale),
                        };
                        let add_hov = matches!(hover, SidebarHit::FolderAddTab(idx, _) if idx == i);
                        let menu_hov = matches!(hover, SidebarHit::FolderMenu(idx, _) if idx == i);
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
                                palette.accent
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
                    } else if !is_renaming {
                        let badge_rc = RECT {
                            left: item_rect.right - scale_i32(28, scale),
                            top: item_rect.top + scale_i32(9, scale),
                            right: item_rect.right - scale_i32(6, scale),
                            bottom: item_rect.bottom - scale_i32(9, scale),
                        };
                        draw_rounded_fill(
                            mem_dc,
                            &badge_rc,
                            scale_i32(8, scale),
                            rgb(if *collapsed && *has_active_child {
                                color.mix(palette.bg_chrome, 0.55)
                            } else {
                                palette.bg_surface
                            }),
                            rgb(palette.border),
                        );
                        SelectObject(mem_dc, font_small);
                        SetTextColor(
                            mem_dc,
                            rgb(if *collapsed && *has_active_child {
                                palette.text_primary
                            } else {
                                palette.text_muted
                            }),
                        );
                        let mut cnt_w = wide(&tab_count.to_string());
                        let mut cnt_rc = badge_rc;
                        DrawTextW(mem_dc, &mut cnt_w, &mut cnt_rc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
                    }
                }

                SidebarEntry::Tab(tab) => {
                    let in_folder = tab.folder_id.is_some();
                    if in_folder {
                        if let Some(fcol) = tab.folder_color {
                            let guide_col = fcol.mix(
                                palette.bg_chrome,
                                if tab.is_active { 0.15 } else { 0.58 },
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
                        right: width - scale_i32(11, scale),
                        bottom: y + g.item_h,
                    };

                    let is_hovered = matches!(
                        hover,
                        SidebarHit::Tab(idx, _) | SidebarHit::CloseTab(idx, _) if idx == i
                    );
                    let close_hovered = matches!(hover, SidebarHit::CloseTab(idx, _) if idx == i);
                    let folder_drop_hl = tab
                        .folder_id
                        .is_some_and(|fid| drag_target == Some(DragDropTarget::Folder(fid)));

                    if tab.is_active {
                        draw_rounded_fill(
                            mem_dc,
                            &item_rect,
                            scale_i32(10, scale),
                            rgb(palette.bg_active),
                            rgb(tab
                                .folder_color
                                .unwrap_or(palette.accent)
                                .mix(palette.border, 0.45)),
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
                            scale_i32(10, scale),
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
                            rgb(Rgb(0x60, 0xa5, 0xfa))
                        } else if tab.is_loading {
                            rgb(palette.accent)
                        } else {
                            rgb(Rgb(0xfb, 0xbf, 0x24))
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
                    let title_col = if tab.is_active {
                        palette.text_primary
                    } else if is_hovered {
                        palette.text_muted.mix(palette.text_primary, 0.7)
                    } else if !tab.is_live {
                        palette.text_muted.mix(palette.bg_chrome, 0.25)
                    } else {
                        palette.text_muted.mix(palette.text_primary, 0.35)
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
                        "New Tab"
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
                            top: item_rect.top + scale_i32(7, scale),
                            right: item_rect.right - scale_i32(5, scale),
                            bottom: item_rect.bottom - scale_i32(7, scale),
                        };
                        if close_hovered {
                            draw_rounded_fill(
                                mem_dc,
                                &close_box,
                                scale_i32(6, scale),
                                rgb(Rgb(0x4c, 0x1d, 0x2d)),
                                rgb(Rgb(0x7f, 0x1d, 0x3a)),
                            );
                        }
                        let x_col = if close_hovered {
                            rgb(Rgb(0xf8, 0x71, 0x71))
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
                            rgb(palette.accent),
                        );
                    } else if tab.is_live && !tab.is_suspended {
                        let r = scale_i32(3, scale).max(2);
                        draw_dot(
                            mem_dc,
                            item_rect.right - scale_i32(14, scale),
                            (item_rect.top + item_rect.bottom) / 2 - r,
                            r * 2,
                            rgb(Rgb(0x4a, 0xde, 0x80)),
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

        // Scrollbar thumb when scrollable
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
                rgb(palette.border.mix(palette.accent, 0.35)),
                rgb(palette.border),
            );
        }

        let _ = RestoreDC(mem_dc, saved);

        // Quick Dock divider & 2x2 tiles
        let dock_div = RECT {
            left: scale_i32(12, scale),
            top: g.list_bottom + scale_i32(4, scale),
            right: width - scale_i32(12, scale),
            bottom: g.list_bottom + scale_i32(5, scale),
        };
        let ddiv_brush = CreateSolidBrush(rgb(palette.border));
        FillRect(mem_dc, &dock_div, ddiv_brush);
        let _ = DeleteObject(ddiv_brush);

        SelectObject(mem_dc, font_dock);
        for (hit, label, rc) in g.dock_rects {
            let hovered = hover == hit;
            let is_current = match hit {
                SidebarHit::OpenHistory => active_url == "kurowser://history",
                SidebarHit::OpenBookmarks => active_url == "kurowser://bookmarks",
                SidebarHit::OpenDownloads => active_url == "kurowser://downloads",
                SidebarHit::OpenSettings => active_url == "kurowser://settings",
                _ => false,
            };
            let (fill, border, fg) = if is_current {
                (palette.accent_soft, palette.accent, palette.accent)
            } else if hovered {
                (
                    palette.bg_hover,
                    palette.accent.mix(palette.border, 0.5),
                    palette.text_primary,
                )
            } else {
                (palette.bg_surface, palette.border, palette.text_muted)
            };
            draw_rounded_fill(mem_dc, &rc, scale_i32(8, scale), rgb(fill), rgb(border));

            let icon_cx = rc.left + scale_i32(15, scale);
            let icon_cy = (rc.top + rc.bottom) / 2;
            draw_dock_icon(mem_dc, hit, icon_cx, icon_cy, scale, rgb(fg));

            SetTextColor(mem_dc, rgb(fg));
            let mut lbl = wide(label);
            let mut lbl_rc = RECT {
                left: rc.left + scale_i32(27, scale),
                top: rc.top,
                right: rc.right - scale_i32(6, scale),
                bottom: rc.bottom,
            };
            DrawTextW(
                mem_dc,
                &mut lbl,
                &mut lbl_rc,
                DT_LEFT | DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS,
            );
        }

        // Footer separator, live counter & "Hemat" trim button
        let fdiv = RECT {
            left: scale_i32(10, scale),
            top: g.footer_top,
            right: width - scale_i32(10, scale),
            bottom: g.footer_top + 1,
        };
        let fdiv_brush = CreateSolidBrush(rgb(palette.border));
        FillRect(mem_dc, &fdiv, fdiv_brush);
        let _ = DeleteObject(fdiv_brush);

        SelectObject(mem_dc, font_small);
        SetTextColor(mem_dc, rgb(palette.text_muted));
        let footer = format!("Live {}/{} · {} tab", live_count, max_live, total_tab_count);
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
            (palette.accent, palette.accent, palette.bg_surface)
        } else {
            (
                palette.accent_soft,
                palette.accent.mix(palette.border, 0.5),
                palette.accent,
            )
        };
        draw_rounded_fill(
            mem_dc,
            &g.trim_btn,
            scale_i32(7, scale),
            rgb(tr_fill),
            rgb(tr_border),
        );
        SetTextColor(mem_dc, rgb(tr_fg));
        let mut trim_lbl = wide("Hemat");
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
        let _ = DeleteObject(mem_bmp);
        let _ = DeleteDC(mem_dc);
    }
}

// ── Double-Buffered Topbar Render ──

pub fn paint_topbar(
    hdc_screen: HDC,
    rect: &RECT,
    scale: f64,
    palette: &ThemePalette,
    sidebar_expanded: bool,
    can_back: bool,
    can_forward: bool,
    is_loading: bool,
    is_bookmarked: bool,
    sec_icon: SecurityIcon,
    shields_active: bool,
    blocked_count: u64,
    hover: Option<TopbarButton>,
    addr_focused: bool,
) {
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

        let div_brush = CreateSolidBrush(rgb(palette.border));
        let div_rect = RECT {
            left: 0,
            top: height - 1,
            right: width,
            bottom: height,
        };
        FillRect(mem_dc, &div_rect, div_brush);
        let _ = DeleteObject(div_brush);

        let local_btns = topbar_button_rects(&local, scale);

        draw_nav_button(
            mem_dc,
            &local_btns.toggle,
            scale,
            palette,
            true,
            hover == Some(TopbarButton::ToggleSidebar),
            |dc, cx, cy| draw_sidebar_toggle_icon(dc, cx, cy, scale, sidebar_expanded, rgb(palette.accent)),
        );
        draw_nav_button(
            mem_dc,
            &local_btns.back,
            scale,
            palette,
            can_back,
            hover == Some(TopbarButton::Back),
            |dc, cx, cy| {
                let col = if can_back {
                    palette.text_primary
                } else {
                    palette.text_muted.mix(palette.bg_chrome, 0.5)
                };
                draw_arrow_icon(dc, cx, cy, scale, true, rgb(col));
            },
        );
        draw_nav_button(
            mem_dc,
            &local_btns.forward,
            scale,
            palette,
            can_forward,
            hover == Some(TopbarButton::Forward),
            |dc, cx, cy| {
                let col = if can_forward {
                    palette.text_primary
                } else {
                    palette.text_muted.mix(palette.bg_chrome, 0.5)
                };
                draw_arrow_icon(dc, cx, cy, scale, false, rgb(col));
            },
        );
        draw_nav_button(
            mem_dc,
            &local_btns.reload,
            scale,
            palette,
            true,
            hover == Some(TopbarButton::ReloadOrStop),
            |dc, cx, cy| {
                if is_loading {
                    draw_close_x(
                        dc,
                        cx,
                        cy,
                        scale_i32(5, scale),
                        rgb(Rgb(0xf8, 0x71, 0x71)),
                        scale,
                    );
                } else {
                    draw_reload_icon(dc, cx, cy, scale, rgb(palette.text_primary));
                }
            },
        );
        draw_nav_button(
            mem_dc,
            &local_btns.new_tab,
            scale,
            palette,
            true,
            hover == Some(TopbarButton::NewTab),
            |dc, cx, cy| draw_plus_icon(dc, cx, cy, scale_i32(6, scale), rgb(palette.accent), scale),
        );

        // Rounded Omnibox pill frame
        let addr_frame = address_bar_frame_rect(&local, scale);
        if addr_frame.right > addr_frame.left {
            let border = if addr_focused {
                palette.accent
            } else {
                palette.border
            };
            draw_rounded_fill(
                mem_dc,
                &addr_frame,
                scale_i32(12, scale),
                rgb(palette.bg_surface),
                rgb(border),
            );

            // Left security / protocol vector icon
            let sec_cx = addr_frame.left + scale_i32(16, scale);
            let sec_cy = (addr_frame.top + addr_frame.bottom) / 2;
            draw_security_icon(mem_dc, sec_cx, sec_cy, scale, sec_icon, palette);

            // Right inside pill: Brave-style Kurowser Shields toggle + badge
            let shield_rc = local_btns.shield;
            if hover == Some(TopbarButton::Shield) {
                draw_rounded_fill(
                    mem_dc,
                    &shield_rc,
                    scale_i32(8, scale),
                    rgb(palette.bg_hover),
                    rgb(palette.border),
                );
            }
            let font_badge = create_font(scale, 9, FW_SEMIBOLD.0 as i32);
            draw_shield_button_glyph(
                mem_dc,
                &shield_rc,
                scale,
                palette,
                shields_active,
                blocked_count,
                sec_icon == SecurityIcon::Internal,
                font_badge,
            );
            let _ = DeleteObject(font_badge);

            // Right inside pill: Bookmark star button
            let star_rc = local_btns.star;
            if hover == Some(TopbarButton::BookmarkStar) {
                draw_rounded_fill(
                    mem_dc,
                    &star_rc,
                    scale_i32(8, scale),
                    rgb(palette.bg_hover),
                    rgb(palette.bg_hover),
                );
            }
            let star_cx = (star_rc.left + star_rc.right) / 2;
            let star_cy = (star_rc.top + star_rc.bottom) / 2;
            let star_col = if is_bookmarked {
                rgb(Rgb(0xfb, 0xbf, 0x24))
            } else if hover == Some(TopbarButton::BookmarkStar) {
                rgb(palette.text_primary)
            } else {
                rgb(palette.text_muted)
            };
            draw_star_icon(
                mem_dc,
                star_cx,
                star_cy,
                scale_i32(7, scale).max(5) as f64,
                is_bookmarked,
                star_col,
                scale,
            );
        }

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
            scale_i32(8, scale),
            rgb(palette.bg_hover),
            rgb(palette.border),
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
                rgb(palette.accent),
                rgb(palette.accent),
            );
        }
        SecurityIcon::Secure => {
            let col = rgb(Rgb(0x4a, 0xde, 0x80));
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
) {
    let has_badge = blocked_count > 0 && shields_active && !is_internal;
    let cy = (rc.top + rc.bottom) / 2;
    let cx = if has_badge {
        rc.left + scale_i32(13, scale)
    } else {
        (rc.left + rc.right) / 2
    };

    let hw = scale_i32(6, scale).max(4);
    let hh = scale_i32(7, scale).max(5);
    let pts = [
        POINT { x: cx, y: cy - hh },
        POINT { x: cx + hw, y: cy - hh + scale_i32(2, scale) },
        POINT { x: cx + hw, y: cy + scale_i32(1, scale) },
        POINT { x: cx, y: cy + hh },
        POINT { x: cx - hw, y: cy + scale_i32(1, scale) },
        POINT { x: cx - hw, y: cy - hh + scale_i32(2, scale) },
    ];

    let outline = if shields_active {
        palette.accent
    } else {
        palette.text_muted
    };
    let fill = if shields_active && !is_internal {
        palette.accent_soft
    } else {
        palette.bg_surface
    };

    let pen_w = (1.4 * scale).round() as i32;
    let pen = CreatePen(PS_SOLID, pen_w.clamp(1, 2), rgb(outline));
    let brush = CreateSolidBrush(rgb(fill));
    let old_p = SelectObject(hdc, pen);
    let old_b = SelectObject(hdc, brush);
    let _ = Polygon(hdc, &pts);
    SelectObject(hdc, old_b);
    SelectObject(hdc, old_p);
    let _ = DeleteObject(brush);
    let _ = DeleteObject(pen);

    if shields_active {
        let check_pen = CreatePen(PS_SOLID, pen_w.clamp(1, 2), rgb(outline));
        let old_cp = SelectObject(hdc, check_pen);
        let _ = MoveToEx(hdc, cx - scale_i32(2, scale), cy, None);
        let _ = LineTo(hdc, cx, cy + scale_i32(2, scale));
        let _ = LineTo(hdc, cx + scale_i32(4, scale), cy - scale_i32(2, scale));
        SelectObject(hdc, old_cp);
        let _ = DeleteObject(check_pen);
    } else {
        let slash_pen = CreatePen(PS_SOLID, pen_w.clamp(1, 2), rgb(Rgb(0xf8, 0x71, 0x71)));
        let old_sp = SelectObject(hdc, slash_pen);
        let _ = MoveToEx(hdc, cx - hw + 1, cy - hh + 2, None);
        let _ = LineTo(hdc, cx + hw - 1, cy + hh - 1);
        SelectObject(hdc, old_sp);
        let _ = DeleteObject(slash_pen);
    }

    if has_badge {
        let label = if blocked_count > 99 {
            "99+".to_string()
        } else {
            blocked_count.to_string()
        };
        let pill = RECT {
            left: rc.right - scale_i32(21, scale),
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
        SetTextColor(hdc, rgb(Rgb(0x0f, 0x0f, 0x17)));
        let mut txt = wide(&label);
        let mut trc = pill;
        DrawTextW(hdc, &mut txt, &mut trc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
        SelectObject(hdc, prev);
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
            rgb(palette.accent.mix(palette.border, 0.5)),
        );
        draw_search_magnifier(hdc, x + size / 2, y + size / 2, scale, rgb(palette.accent));
        return;
    }

    if domain.starts_with("kurowser.") || domain == "kurowser" {
        favicons.draw_logo(hdc, x, y, size);
        return;
    }

    if favicons.draw_site(hdc, domain, x, y, size) {
        return;
    }

    // Crisp fallback monogram tile derived from domain hash
    const MONO_COLORS: [Rgb; 6] = [
        Rgb(0x5b, 0x4b, 0xc4),
        Rgb(0x0e, 0x74, 0x90),
        Rgb(0x04, 0x78, 0x57),
        Rgb(0xb4, 0x53, 0x09),
        Rgb(0xbe, 0x18, 0x5d),
        Rgb(0x1d, 0x4e, 0xd8),
    ];
    let hash: usize = domain.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
    let badge_col = if domain.is_empty() {
        palette.accent_soft
    } else {
        MONO_COLORS[hash % MONO_COLORS.len()]
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
    SetTextColor(hdc, rgb(Rgb(0xf8, 0xfa, 0xfc)));
    let mut rc = bg;
    DrawTextW(hdc, &mut s, &mut rc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
    SelectObject(hdc, prev);
}

// ── Topbar Geometry & Hit-Testing ──

pub struct TopbarButtons {
    pub toggle: RECT,
    pub back: RECT,
    pub forward: RECT,
    pub reload: RECT,
    pub shield: RECT,
    pub star: RECT,
    pub new_tab: RECT,
}

pub fn topbar_button_rects(rect: &RECT, scale: f64) -> TopbarButtons {
    let btn_size = scale_i32(BUTTON_SIZE, scale);
    let gap = scale_i32(3, scale);
    let btn_y = (rect.bottom - rect.top - btn_size) / 2 + rect.top;
    let mut x = rect.left + scale_i32(10, scale);

    let toggle = RECT {
        left: x,
        top: btn_y,
        right: x + btn_size,
        bottom: btn_y + btn_size,
    };
    x += btn_size + gap + scale_i32(4, scale);

    let back = RECT {
        left: x,
        top: btn_y,
        right: x + btn_size,
        bottom: btn_y + btn_size,
    };
    x += btn_size + gap;

    let forward = RECT {
        left: x,
        top: btn_y,
        right: x + btn_size,
        bottom: btn_y + btn_size,
    };
    x += btn_size + gap;

    let reload = RECT {
        left: x,
        top: btn_y,
        right: x + btn_size,
        bottom: btn_y + btn_size,
    };

    let plus_x = rect.right - btn_size - scale_i32(12, scale);
    let new_tab = RECT {
        left: plus_x,
        top: btn_y,
        right: plus_x + btn_size,
        bottom: btn_y + btn_size,
    };

    let frame_right = new_tab.left - scale_i32(10, scale);
    let star_w = scale_i32(28, scale);
    let star_h = scale_i32(28, scale);
    let star_y = (rect.bottom - rect.top - star_h) / 2 + rect.top;
    let star = RECT {
        left: frame_right - star_w - scale_i32(4, scale),
        top: star_y,
        right: frame_right - scale_i32(4, scale),
        bottom: star_y + star_h,
    };

    let shield_w = scale_i32(40, scale);
    let shield = RECT {
        left: star.left - shield_w - scale_i32(3, scale),
        top: star_y,
        right: star.left - scale_i32(3, scale),
        bottom: star_y + star_h,
    };

    TopbarButtons {
        toggle,
        back,
        forward,
        reload,
        shield,
        star,
        new_tab,
    }
}

pub fn address_bar_frame_rect(topbar: &RECT, scale: f64) -> RECT {
    let btns = topbar_button_rects(topbar, scale);
    let height = scale_i32(34, scale);
    let top = (topbar.bottom - topbar.top - height) / 2 + topbar.top;
    RECT {
        left: btns.reload.right + scale_i32(10, scale),
        top,
        right: btns.new_tab.left - scale_i32(10, scale),
        bottom: top + height,
    }
}

pub fn hit_test_topbar(topbar: &RECT, scale: f64, x: i32, y: i32) -> Option<TopbarButton> {
    if !point_in_rect(x, y, topbar) {
        return None;
    }
    let btns = topbar_button_rects(topbar, scale);
    if point_in_rect(x, y, &btns.toggle) {
        Some(TopbarButton::ToggleSidebar)
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
    } else {
        None
    }
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

    let g = sidebar_geometry(rect, scale);

    if point_in_rect(x, y, &g.new_btn) {
        return SidebarHit::NewTab;
    }
    if point_in_rect(x, y, &g.new_folder_btn) {
        return SidebarHit::NewFolder;
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
        let max_scroll = max_sidebar_scroll(rect, scale, entries.len());
        let scroll = scroll_offset.clamp(0, max_scroll);
        for (i, entry) in entries.iter().enumerate() {
            let item_top = g.list_top + (i as i32) * (g.item_h + g.gap) - scroll;
            let in_folder = matches!(entry, SidebarEntry::Tab(t) if t.folder_id.is_some());
            let item_rect = RECT {
                left: rect.left
                    + if in_folder {
                        scale_i32(25, scale)
                    } else {
                        scale_i32(10, scale)
                    },
                top: item_top,
                right: rect.right - scale_i32(11, scale),
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
                        return SidebarHit::FolderAddTab(i, *folder_id);
                    }
                    if point_in_rect(x, y, &menu_rect) {
                        return SidebarHit::FolderMenu(i, *folder_id);
                    }
                    if point_in_rect(x, y, &item_rect) {
                        return SidebarHit::FolderToggle(i, *folder_id);
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
                        return SidebarHit::CloseTab(i, tab.id);
                    }
                    if point_in_rect(x, y, &item_rect) {
                        return SidebarHit::Tab(i, tab.id);
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
    let g = sidebar_geometry(rect, scale);
    if y < g.list_top || y >= g.list_bottom {
        return None;
    }
    let max_scroll = max_sidebar_scroll(rect, scale, entries.len());
    let scroll = scroll_offset.clamp(0, max_scroll);
    for (i, entry) in entries.iter().enumerate() {
        let item_top = g.list_top + (i as i32) * (g.item_h + g.gap) - scroll;
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

pub fn position_address_bar(addr_hwnd: HWND, topbar: &RECT, scale: f64) {
    let frame = address_bar_frame_rect(topbar, scale);
    let left_pad = scale_i32(30, scale);
    let right_pad = scale_i32(79, scale);
    let width = (frame.right - frame.left - left_pad - right_pad).max(20);
    // Poppins line height fits nicely inside 22px tall EDIT centered in 34px frame
    let edit_h = scale_i32(22, scale).max(16);
    let y = frame.top + ((frame.bottom - frame.top - edit_h) / 2);

    unsafe {
        let _ = SetWindowPos(
            addr_hwnd,
            None,
            frame.left + left_pad,
            y,
            width,
            edit_h,
            SWP_NOZORDER | SWP_NOACTIVATE,
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
        topbar: &RECT,
        scale: f64,
        suggestions: Vec<Suggestion>,
    ) {
        let is_empty = suggestions.is_empty();
        {
            let mut st = self.state.borrow_mut();
            st.suggestions = suggestions;
            st.selected_idx = None;
            st.hover_idx = None;
            st.dpi_scale = scale;
            st.visible = !is_empty;
        }
        self.sync_geometry(parent, topbar, scale);
    }

    pub fn sync_geometry(&self, parent: HWND, topbar: &RECT, scale: f64) {
        if self.hwnd.0.is_null() {
            return;
        }
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

        let frame = address_bar_frame_rect(topbar, scale);
        let mut pt = POINT {
            x: frame.left,
            y: frame.bottom + scale_i32(4, scale),
        };
        unsafe {
            let _ = ClientToScreen(parent, &mut pt);
        }
        let width = (frame.right - frame.left).max(scale_i32(340, scale));
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
    let frame_br = CreateSolidBrush(rgb(p.accent.mix(p.border, 0.55)));
    FrameRect(mem_dc, &rc, frame_br);
    let _ = DeleteObject(frame_br);

    SetBkMode(mem_dc, TRANSPARENT);
    let font_title = create_font(scale, 13, FW_MEDIUM.0 as i32);
    let font_sub = create_font(scale, 11, FW_NORMAL.0 as i32);
    let font_badge = create_font(scale, 10, FW_SEMIBOLD.0 as i32);
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
                scale_i32(9, scale),
                rgb(p.bg_active),
                rgb(p.accent.mix(p.border, 0.4)),
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
                scale_i32(9, scale),
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

        // Kind badge pill on right
        let badge_text = match item.kind {
            SuggestionKind::Bookmark => "Bookmark",
            SuggestionKind::History => "History",
            SuggestionKind::TopSite => "Situs",
            SuggestionKind::Internal => "Kurowser",
            SuggestionKind::DirectUrl => "Buka URL",
            SuggestionKind::Search => "Search",
        };
        let badge_w = scale_i32(68, scale);
        let badge_h = scale_i32(20, scale);
        let badge_rc = RECT {
            left: row_rc.right - badge_w - scale_i32(10, scale),
            top: row_rc.top + (row_rc.bottom - row_rc.top - badge_h) / 2,
            right: row_rc.right - scale_i32(10, scale),
            bottom: row_rc.top + (row_rc.bottom - row_rc.top - badge_h) / 2 + badge_h,
        };
        let (bfill, bfg) = if item.kind == SuggestionKind::Bookmark {
            (Rgb(0x3a, 0x2d, 0x12), Rgb(0xfb, 0xbf, 0x24))
        } else {
            (p.accent_soft, p.accent)
        };
        draw_rounded_fill(
            mem_dc,
            &badge_rc,
            scale_i32(10, scale),
            rgb(bfill),
            rgb(bfill),
        );
        SelectObject(mem_dc, font_badge);
        SetTextColor(mem_dc, rgb(bfg));
        let mut b_w = wide(badge_text);
        let mut b_rc = badge_rc;
        DrawTextW(mem_dc, &mut b_w, &mut b_rc, DT_CENTER | DT_SINGLELINE | DT_VCENTER);

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
                p.accent
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

pub fn apply_dark_titlebar(hwnd: HWND) {
    unsafe {
        let dark: BOOL = TRUE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark as *const BOOL as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
}
