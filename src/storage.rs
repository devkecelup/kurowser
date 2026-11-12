// ─── storage.rs ─── Hardened settings, folders, quick links, shields, history & CSPRNG tokens ───
use crate::adblock::{normalize_host, ShieldPolicy};
use crate::updater::UpdateRuntimeState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};

pub const FOLDER_PALETTE: &[(&str, &str)] = &[
    ("Teal", "#14b8a6"),
    ("Sky", "#38bdf8"),
    ("Emerald", "#10b981"),
    ("Amber", "#f59e0b"),
    ("Slate", "#64748b"),
    ("Rose", "#f43f5e"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SearchEngine {
    Google,
    DuckDuckGo,
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self::Google
    }
}

impl SearchEngine {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Google => "Google",
            Self::DuckDuckGo => "DuckDuckGo",
        }
    }

    pub fn search_url(&self, encoded_query: &str) -> String {
        match self {
            Self::Google => format!("https://www.google.com/search?q={}", encoded_query),
            Self::DuckDuckGo => format!("https://duckduckgo.com/?q={}", encoded_query),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemePreset {
    #[serde(alias = "Midnight", alias = "Obsidian")]
    Graphite,
    Paper,
    #[serde(alias = "Emerald", alias = "Cyber")]
    NordicPine,
    Mist,
    #[serde(alias = "Crimson", alias = "Sakura")]
    Espresso,
    Sand,
}

impl Default for ThemePreset {
    fn default() -> Self {
        Self::Graphite
    }
}

impl ThemePreset {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Graphite => "graphite",
            Self::Paper => "paper",
            Self::NordicPine => "nordic_pine",
            Self::Mist => "mist",
            Self::Espresso => "espresso",
            Self::Sand => "sand",
        }
    }

    pub fn from_id(s: &str) -> Self {
        match s {
            "paper" => Self::Paper,
            "nordic_pine" | "emerald" | "cyber" => Self::NordicPine,
            "mist" => Self::Mist,
            "espresso" | "crimson" | "sakura" => Self::Espresso,
            "sand" => Self::Sand,
            _ => Self::Graphite,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Graphite => "Graphite",
            Self::Paper => "Paper",
            Self::NordicPine => "Nordic Pine",
            Self::Mist => "Mist",
            Self::Espresso => "Espresso",
            Self::Sand => "Sand",
        }
    }

    #[allow(dead_code)]
    pub fn is_light(&self) -> bool {
        matches!(self, Self::Paper | Self::Mist | Self::Sand)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    pub fn parse_hex(s: &str) -> Option<Self> {
        let hex = s.trim().trim_start_matches('#');
        if hex.len() != 6 || !hex.is_ascii() {
            return None;
        }
        let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
        let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
        let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
        Some(Self(r, g, b))
    }

    pub fn mix(self, other: Self, weight_other: f32) -> Self {
        let w = weight_other.clamp(0.0, 1.0);
        let r = (self.0 as f32 * (1.0 - w) + other.0 as f32 * w).round() as u8;
        let g = (self.1 as f32 * (1.0 - w) + other.1 as f32 * w).round() as u8;
        let b = (self.2 as f32 * (1.0 - w) + other.2 as f32 * w).round() as u8;
        Self(r, g, b)
    }

    pub fn relative_luminance(self) -> f64 {
        fn lin(c: u8) -> f64 {
            let v = (c as f64) / 255.0;
            if v <= 0.04045 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * lin(self.0) + 0.7152 * lin(self.1) + 0.0722 * lin(self.2)
    }

    pub fn contrast_ratio(self, other: Self) -> f64 {
        let l1 = self.relative_luminance();
        let l2 = other.relative_luminance();
        let (hi, lo) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
        (hi + 0.05) / (lo + 0.05)
    }
}

fn pick_on_accent(bg: Rgb) -> Rgb {
    let white = Rgb(0xff, 0xff, 0xff);
    let dark_ink = Rgb(0x0c, 0x11, 0x16);
    if white.contrast_ratio(bg) >= dark_ink.contrast_ratio(bg) {
        white
    } else {
        dark_ink
    }
}

fn ensure_contrast_against(color: Rgb, bgs: &[Rgb], min_ratio: f64, is_light_bg: bool) -> Rgb {
    if bgs.iter().all(|&b| color.contrast_ratio(b) >= min_ratio) {
        return color;
    }
    let target = if is_light_bg {
        Rgb(0x0b, 0x12, 0x15)
    } else {
        Rgb(0xf8, 0xfa, 0xfc)
    };
    for step in 1..=32 {
        let candidate = color.mix(target, (step as f32) / 32.0);
        if bgs.iter().all(|&b| candidate.contrast_ratio(b) >= min_ratio) {
            return candidate;
        }
    }
    target
}

#[derive(Clone, Debug)]
pub struct ThemePalette {
    pub is_light: bool,
    pub bg_app: Rgb,
    pub bg_chrome: Rgb,
    pub bg_surface: Rgb,
    pub bg_hover: Rgb,
    pub bg_active: Rgb,
    pub border: Rgb,
    pub border_subtle: Rgb,
    pub text_primary: Rgb,
    pub text_muted: Rgb,
    pub text_inactive_tab: Rgb,
    pub text_disabled: Rgb,
    pub accent: Rgb,
    pub accent_text: Rgb,
    pub accent_soft: Rgb,
    pub on_accent: Rgb,
    pub focus_ring: Rgb,
    pub danger_fg: Rgb,
    pub danger_bg: Rgb,
    pub danger_border: Rgb,
    pub success_fg: Rgb,
    pub warning_fg: Rgb,
}

impl ThemePalette {
    pub fn from_settings(settings: &Settings) -> Self {
        let mut p = match settings.theme_preset {
            ThemePreset::Graphite => Self {
                is_light: false,
                bg_app: Rgb(0x0f, 0x12, 0x17),
                bg_chrome: Rgb(0x16, 0x1a, 0x21),
                bg_surface: Rgb(0x1e, 0x24, 0x2d),
                bg_hover: Rgb(0x26, 0x2d, 0x38),
                bg_active: Rgb(0x16, 0x31, 0x30),
                border: Rgb(0x75, 0x7e, 0x8c),
                border_subtle: Rgb(0x2f, 0x37, 0x44),
                text_primary: Rgb(0xf2, 0xf5, 0xf9),
                text_muted: Rgb(0xa3, 0xad, 0xb9),
                text_inactive_tab: Rgb(0xc1, 0xc9, 0xd4),
                text_disabled: Rgb(0x73, 0x7d, 0x8b),
                accent: Rgb(0x0f, 0x76, 0x6e),
                accent_text: Rgb(0x5e, 0xea, 0xd4),
                accent_soft: Rgb(0x18, 0x38, 0x36),
                on_accent: Rgb(0xff, 0xff, 0xff),
                focus_ring: Rgb(0x2d, 0xd4, 0xbf),
                danger_fg: Rgb(0xfd, 0xa4, 0xaf),
                danger_bg: Rgb(0x3f, 0x19, 0x22),
                danger_border: Rgb(0xfb, 0x71, 0x85),
                success_fg: Rgb(0x6e, 0xe7, 0xb7),
                warning_fg: Rgb(0xfc, 0xd3, 0x4d),
            },
            ThemePreset::Paper => Self {
                is_light: true,
                bg_app: Rgb(0xff, 0xff, 0xff),
                bg_chrome: Rgb(0xf4, 0xf5, 0xf7),
                bg_surface: Rgb(0xff, 0xff, 0xff),
                bg_hover: Rgb(0xe7, 0xea, 0xef),
                bg_active: Rgb(0xdf, 0xf2, 0xef),
                border: Rgb(0x75, 0x7d, 0x8a),
                border_subtle: Rgb(0xd9, 0xde, 0xe6),
                text_primary: Rgb(0x11, 0x18, 0x27),
                text_muted: Rgb(0x4b, 0x55, 0x63),
                text_inactive_tab: Rgb(0x37, 0x41, 0x51),
                text_disabled: Rgb(0x6b, 0x72, 0x80),
                accent: Rgb(0x0f, 0x76, 0x6e),
                accent_text: Rgb(0x0d, 0x63, 0x5c),
                accent_soft: Rgb(0xda, 0xf1, 0xee),
                on_accent: Rgb(0xff, 0xff, 0xff),
                focus_ring: Rgb(0x0f, 0x76, 0x6e),
                danger_fg: Rgb(0xb9, 0x1c, 0x1c),
                danger_bg: Rgb(0xfe, 0xe2, 0xe2),
                danger_border: Rgb(0xdc, 0x26, 0x26),
                success_fg: Rgb(0x06, 0x5f, 0x46),
                warning_fg: Rgb(0x92, 0x40, 0x0e),
            },
            ThemePreset::NordicPine => Self {
                is_light: false,
                bg_app: Rgb(0x0d, 0x15, 0x14),
                bg_chrome: Rgb(0x12, 0x1e, 0x1c),
                bg_surface: Rgb(0x19, 0x29, 0x26),
                bg_hover: Rgb(0x21, 0x35, 0x31),
                bg_active: Rgb(0x16, 0x3c, 0x37),
                border: Rgb(0x72, 0x8b, 0x85),
                border_subtle: Rgb(0x2a, 0x3e, 0x3a),
                text_primary: Rgb(0xee, 0xf5, 0xf3),
                text_muted: Rgb(0x9e, 0xb3, 0xad),
                text_inactive_tab: Rgb(0xbd, 0xd0, 0xca),
                text_disabled: Rgb(0x6e, 0x86, 0x7f),
                accent: Rgb(0x0d, 0x7e, 0x73),
                accent_text: Rgb(0x5e, 0xea, 0xd4),
                accent_soft: Rgb(0x16, 0x38, 0x34),
                on_accent: Rgb(0xff, 0xff, 0xff),
                focus_ring: Rgb(0x2d, 0xd4, 0xbf),
                danger_fg: Rgb(0xfd, 0xa4, 0xaf),
                danger_bg: Rgb(0x3e, 0x1a, 0x21),
                danger_border: Rgb(0xfb, 0x71, 0x85),
                success_fg: Rgb(0x6e, 0xe7, 0xb7),
                warning_fg: Rgb(0xfc, 0xd3, 0x4d),
            },
            ThemePreset::Mist => Self {
                is_light: true,
                bg_app: Rgb(0xf8, 0xfa, 0xfc),
                bg_chrome: Rgb(0xee, 0xf2, 0xf6),
                bg_surface: Rgb(0xff, 0xff, 0xff),
                bg_hover: Rgb(0xe1, 0xe7, 0xef),
                bg_active: Rgb(0xdc, 0xec, 0xf9),
                border: Rgb(0x6f, 0x7c, 0x8e),
                border_subtle: Rgb(0xd3, 0xdc, 0xe6),
                text_primary: Rgb(0x0f, 0x17, 0x2a),
                text_muted: Rgb(0x47, 0x55, 0x69),
                text_inactive_tab: Rgb(0x33, 0x41, 0x55),
                text_disabled: Rgb(0x64, 0x74, 0x8b),
                accent: Rgb(0x1d, 0x4e, 0xd8),
                accent_text: Rgb(0x1d, 0x4e, 0xd8),
                accent_soft: Rgb(0xdb, 0xea, 0xfe),
                on_accent: Rgb(0xff, 0xff, 0xff),
                focus_ring: Rgb(0x1d, 0x4e, 0xd8),
                danger_fg: Rgb(0xb9, 0x1c, 0x1c),
                danger_bg: Rgb(0xfe, 0xe2, 0xe2),
                danger_border: Rgb(0xdc, 0x26, 0x26),
                success_fg: Rgb(0x06, 0x5f, 0x46),
                warning_fg: Rgb(0x92, 0x40, 0x0e),
            },
            ThemePreset::Espresso => Self {
                is_light: false,
                bg_app: Rgb(0x15, 0x12, 0x10),
                bg_chrome: Rgb(0x1c, 0x18, 0x16),
                bg_surface: Rgb(0x26, 0x20, 0x1d),
                bg_hover: Rgb(0x31, 0x29, 0x25),
                bg_active: Rgb(0x3d, 0x28, 0x1f),
                border: Rgb(0x86, 0x7a, 0x73),
                border_subtle: Rgb(0x3a, 0x32, 0x2e),
                text_primary: Rgb(0xf6, 0xf2, 0xef),
                text_muted: Rgb(0xb5, 0xa8, 0xa1),
                text_inactive_tab: Rgb(0xd1, 0xc6, 0xc0),
                text_disabled: Rgb(0x83, 0x77, 0x71),
                accent: Rgb(0xb4, 0x53, 0x09),
                accent_text: Rgb(0xfb, 0xbf, 0x24),
                accent_soft: Rgb(0x38, 0x24, 0x18),
                on_accent: Rgb(0xff, 0xff, 0xff),
                focus_ring: Rgb(0xf5, 0x9e, 0x0b),
                danger_fg: Rgb(0xfd, 0xa4, 0xaf),
                danger_bg: Rgb(0x3f, 0x19, 0x20),
                danger_border: Rgb(0xfb, 0x71, 0x85),
                success_fg: Rgb(0x6e, 0xe7, 0xb7),
                warning_fg: Rgb(0xfc, 0xd3, 0x4d),
            },
            ThemePreset::Sand => Self {
                is_light: true,
                bg_app: Rgb(0xfc, 0xfa, 0xf6),
                bg_chrome: Rgb(0xf3, 0xef, 0xe6),
                bg_surface: Rgb(0xff, 0xff, 0xff),
                bg_hover: Rgb(0xe8, 0xe2, 0xd5),
                bg_active: Rgb(0xef, 0xe4, 0xd0),
                border: Rgb(0x7b, 0x73, 0x65),
                border_subtle: Rgb(0xdd, 0xd6, 0xc7),
                text_primary: Rgb(0x1c, 0x19, 0x17),
                text_muted: Rgb(0x57, 0x53, 0x4e),
                text_inactive_tab: Rgb(0x44, 0x40, 0x3c),
                text_disabled: Rgb(0x78, 0x71, 0x6c),
                accent: Rgb(0x9a, 0x34, 0x12),
                accent_text: Rgb(0x9a, 0x34, 0x12),
                accent_soft: Rgb(0xff, 0xed, 0xd5),
                on_accent: Rgb(0xff, 0xff, 0xff),
                focus_ring: Rgb(0x9a, 0x34, 0x12),
                danger_fg: Rgb(0xb9, 0x1c, 0x1c),
                danger_bg: Rgb(0xfe, 0xe2, 0xe2),
                danger_border: Rgb(0xdc, 0x26, 0x26),
                success_fg: Rgb(0x06, 0x5f, 0x46),
                warning_fg: Rgb(0x92, 0x40, 0x0e),
            },
        };

        if let Some(ref custom) = settings.custom_accent_hex {
            if let Some(rgb) = Rgb::parse_hex(custom) {
                p.accent = rgb;
                p.on_accent = pick_on_accent(rgb);
                p.accent_soft = p.bg_chrome.mix(rgb, if p.is_light { 0.14 } else { 0.18 });
                p.bg_active = p.bg_chrome.mix(rgb, if p.is_light { 0.13 } else { 0.14 });
                let all_bgs = [
                    p.bg_app,
                    p.bg_chrome,
                    p.bg_surface,
                    p.bg_hover,
                    p.bg_active,
                    p.accent_soft,
                ];
                p.accent_text = ensure_contrast_against(rgb, &all_bgs, 4.6, p.is_light);
                p.focus_ring = ensure_contrast_against(
                    rgb,
                    &[p.bg_chrome, p.bg_surface, p.bg_app],
                    3.1,
                    p.is_light,
                );
                let std_bgs = [p.bg_app, p.bg_chrome, p.bg_surface, p.bg_hover, p.bg_active];
                p.text_muted = ensure_contrast_against(p.text_muted, &std_bgs, 4.6, p.is_light);
                p.text_inactive_tab =
                    ensure_contrast_against(p.text_inactive_tab, &std_bgs, 4.6, p.is_light);
                p.success_fg = ensure_contrast_against(p.success_fg, &std_bgs, 4.6, p.is_light);
                p.warning_fg = ensure_contrast_against(p.warning_fg, &std_bgs, 4.6, p.is_light);
                p.border = ensure_contrast_against(p.border, &std_bgs, 3.05, p.is_light);
                p.danger_border =
                    ensure_contrast_against(p.danger_border, &std_bgs, 3.05, p.is_light);
            }
        }
        p
    }
}

pub const DEFAULT_IDLE_DISCARD_SECS: u64 = 180;
pub const DEFAULT_JS_HEAP_LIMIT_MB: u32 = 256;
pub const DEFAULT_IDLE_TRIM_SECS: u64 = 180;

fn default_true() -> bool {
    true
}

fn default_idle_discard_secs() -> u64 {
    DEFAULT_IDLE_DISCARD_SECS
}

fn default_js_heap_limit_mb() -> u32 {
    DEFAULT_JS_HEAP_LIMIT_MB
}

fn default_idle_trim_secs() -> u64 {
    DEFAULT_IDLE_TRIM_SECS
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_preset: ThemePreset,
    pub custom_accent_hex: Option<String>,
    pub search_engine: SearchEngine,
    pub max_live_tabs: usize,
    #[serde(default = "default_idle_discard_secs")]
    pub idle_discard_secs: u64,
    #[serde(default = "default_js_heap_limit_mb")]
    pub js_heap_limit_mb: u32,
    #[serde(default = "default_true")]
    pub lean_webview_flags: bool,
    #[serde(default = "default_true")]
    pub network_in_process: bool,
    #[serde(default = "default_true")]
    pub trim_on_minimize: bool,
    #[serde(default = "default_idle_trim_secs")]
    pub idle_trim_secs: u64,
    #[serde(default)]
    pub sidebar_pinned: bool,
    #[serde(default = "default_true")]
    pub sidebar_hover_expand: bool,
    #[serde(default = "default_true")]
    pub adblock_enabled: bool,
    #[serde(default = "default_true")]
    pub adblock_cosmetic: bool,
    #[serde(default)]
    pub adblock_allowlist: Vec<String>,
    #[serde(default)]
    pub total_blocked_ads: u64,
    #[serde(default = "default_true")]
    pub auto_update: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_preset: ThemePreset::Graphite,
            custom_accent_hex: None,
            search_engine: SearchEngine::Google,
            max_live_tabs: 3,
            idle_discard_secs: DEFAULT_IDLE_DISCARD_SECS,
            js_heap_limit_mb: DEFAULT_JS_HEAP_LIMIT_MB,
            lean_webview_flags: true,
            network_in_process: true,
            trim_on_minimize: true,
            idle_trim_secs: DEFAULT_IDLE_TRIM_SECS,
            sidebar_pinned: false,
            sidebar_hover_expand: true,
            adblock_enabled: true,
            adblock_cosmetic: true,
            adblock_allowlist: Vec::new(),
            total_blocked_ads: 0,
            auto_update: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryItem {
    pub url: String,
    pub title: String,
    pub domain: String,
    pub visit_count: u32,
    pub last_visited: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BookmarkItem {
    pub url: String,
    pub title: String,
    pub domain: String,
    pub added_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: u64,
    pub filename: String,
    pub path: String,
    pub url: String,
    pub status: String,
    pub started_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickLinkItem {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFolderTab {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedFolder {
    pub id: u64,
    pub name: String,
    pub color_hex: String,
    pub collapsed: bool,
    #[serde(default)]
    pub tabs: Vec<SavedFolderTab>,
}

pub fn default_quick_links() -> Vec<QuickLinkItem> {
    [
        ("Google", "https://www.google.com"),
        ("YouTube", "https://www.youtube.com"),
        ("GitHub", "https://github.com"),
        ("Spotify", "https://open.spotify.com"),
        ("Gmail", "https://mail.google.com"),
        ("Reddit", "https://www.reddit.com"),
        ("X", "https://x.com"),
        ("WhatsApp", "https://web.whatsapp.com"),
        ("Figma", "https://www.figma.com"),
        ("Notion", "https://www.notion.so"),
    ]
    .into_iter()
    .map(|(title, url)| QuickLinkItem {
        title: title.to_string(),
        url: url.to_string(),
    })
    .collect()
}

pub fn normalize_safe_url(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.len() > 2048 || t.chars().any(char::is_control) {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    for bad in ["javascript:", "data:", "vbscript:", "file:", "blob:", "about:"] {
        if lower.starts_with(bad) {
            return None;
        }
    }
    if lower.starts_with("kurowser://") {
        let slug = t["kurowser://".len()..]
            .trim_start_matches('/')
            .trim_end_matches('/');
        if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
            return Some("kurowser://start".to_string());
        }
        return Some(format!("kurowser://{}", slug.to_ascii_lowercase()));
    }
    if lower.starts_with("http://") || lower.starts_with("https://") {
        let after = t.split_once("://").map(|(_, r)| r).unwrap_or("");
        if after.trim_start_matches('/').is_empty() || t.chars().any(char::is_whitespace) {
            return None;
        }
        return Some(t.to_string());
    }
    if t.chars().any(char::is_whitespace) {
        return None;
    }
    if t.contains('.') {
        return Some(format!("https://{}", t));
    }
    None
}

pub fn normalize_quick_link_url(raw: &str) -> Option<String> {
    normalize_safe_url(raw)
}

pub fn sanitize_favicon_domain(domain: &str) -> Option<String> {
    let clean = domain.trim().to_ascii_lowercase();
    if clean.is_empty() || clean.len() > 120 {
        return None;
    }
    if clean.starts_with('.') || clean.ends_with('.') || clean.contains("..") {
        return None;
    }
    if !clean
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return None;
    }
    Some(clean)
}

pub fn sanitize_download_filename(raw: &str) -> String {
    let clean: String = raw
        .chars()
        .filter(|&c| {
            !c.is_control()
                && !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
        })
        .take(120)
        .collect();
    let trimmed = clean.trim().trim_matches('.');
    if trimmed.is_empty() || trimmed == ".." {
        "download".to_string()
    } else {
        trimmed.to_string()
    }
}

pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn generate_csprng_hex(num_bytes: usize) -> String {
    let n = num_bytes.clamp(8, 64);
    let mut buf = vec![0u8; n];
    let ok = unsafe { BCryptGenRandom(None, &mut buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG).is_ok() };
    if !ok {
        // Fallback mixing multiple timers & stack entropy only if BCrypt fails
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e3779b97f4a7c15);
        for (i, slot) in buf.iter_mut().enumerate() {
            let v = nanos.rotate_left((i as u32 * 7) % 64) ^ ((std::process::id() as u64) << 13);
            *slot = (v & 0xFF) as u8;
        }
    }
    let mut out = String::with_capacity(n * 2);
    for b in buf {
        out.push(hex_nibble(b >> 4));
        out.push(hex_nibble(b & 0x0F));
    }
    out
}

fn hex_nibble(v: u8) -> char {
    match v {
        0..=9 => (b'0' + v) as char,
        _ => (b'a' + (v - 10)) as char,
    }
}

pub struct BrowserData {
    pub settings: Settings,
    pub history: Vec<HistoryItem>,
    pub bookmarks: Vec<BookmarkItem>,
    pub downloads: Vec<DownloadItem>,
    pub quick_links: Vec<QuickLinkItem>,
    pub folders: Vec<SavedFolder>,
    pub api_token: String,
    pub update_state: UpdateRuntimeState,
    next_download_id: u64,
    unsaved_blocked_ticks: u64,
    data_dir: PathBuf,
}

impl Default for BrowserData {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            history: Vec::new(),
            bookmarks: Vec::new(),
            downloads: Vec::new(),
            quick_links: default_quick_links(),
            folders: Vec::new(),
            api_token: "test_token_only".to_string(),
            update_state: UpdateRuntimeState::Idle,
            next_download_id: 1,
            unsaved_blocked_ticks: 0,
            data_dir: std::env::temp_dir().join("KurowserTest"),
        }
    }
}

impl BrowserData {
    pub fn load() -> Self {
        let local_app = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(local_app).join("Kurowser");
        let _ = std::fs::create_dir_all(&data_dir);

        let mut settings: Settings = read_json(data_dir.join("settings.json")).unwrap_or_default();
        settings.max_live_tabs = settings.max_live_tabs.clamp(2, 4);
        if settings.js_heap_limit_mb != 0 {
            settings.js_heap_limit_mb = settings.js_heap_limit_mb.clamp(128, 4096);
        }
        settings.custom_accent_hex = settings
            .custom_accent_hex
            .as_deref()
            .and_then(Rgb::parse_hex)
            .map(|c| c.hex());
        settings.adblock_allowlist = settings
            .adblock_allowlist
            .into_iter()
            .map(|h| normalize_host(&h))
            .filter(|h| !h.is_empty())
            .take(256)
            .collect();

        let history: Vec<HistoryItem> =
            read_json(data_dir.join("history.json")).unwrap_or_default();
        let bookmarks: Vec<BookmarkItem> =
            read_json(data_dir.join("bookmarks.json")).unwrap_or_default();
        let downloads: Vec<DownloadItem> =
            read_json(data_dir.join("downloads.json")).unwrap_or_default();
        let quick_links: Vec<QuickLinkItem> =
            read_json(data_dir.join("quick_links.json")).unwrap_or_else(default_quick_links);
        let raw_folders: Vec<SavedFolder> =
            read_json(data_dir.join("folders.json")).unwrap_or_default();
        let folders = sanitize_folders(raw_folders);
        let next_download_id = downloads.iter().map(|d| d.id).max().unwrap_or(0) + 1;

        Self {
            settings,
            history,
            bookmarks,
            downloads,
            quick_links,
            folders,
            api_token: generate_csprng_hex(32),
            update_state: UpdateRuntimeState::Idle,
            next_download_id,
            unsaved_blocked_ticks: 0,
            data_dir,
        }
    }

    pub fn palette(&self) -> ThemePalette {
        ThemePalette::from_settings(&self.settings)
    }

    pub fn shield_policy(&self) -> ShieldPolicy {
        ShieldPolicy {
            global_enabled: self.settings.adblock_enabled,
            cosmetic_enabled: self.settings.adblock_cosmetic,
            allowlist: self.settings.adblock_allowlist.clone(),
        }
    }

    pub fn toggle_global_adblock(&mut self) -> bool {
        self.settings.adblock_enabled = !self.settings.adblock_enabled;
        self.save_settings();
        self.settings.adblock_enabled
    }

    /// Toggles per-site exception for `host`. Returns `true` if Shields are now ON for this site.
    pub fn toggle_site_allowlist(&mut self, host: &str) -> bool {
        let clean = normalize_host(host);
        if clean.is_empty() || clean.starts_with("kurowser") {
            return self.settings.adblock_enabled;
        }
        if let Some(pos) = self.settings.adblock_allowlist.iter().position(|h| h == &clean) {
            self.settings.adblock_allowlist.remove(pos);
            self.save_settings();
            self.settings.adblock_enabled
        } else {
            if self.settings.adblock_allowlist.len() < 256 {
                self.settings.adblock_allowlist.push(clean);
                self.save_settings();
            }
            false
        }
    }

    pub fn add_allowlist_host(&mut self, raw: &str) {
        let clean = normalize_host(raw);
        if clean.is_empty() || clean.starts_with("kurowser") {
            return;
        }
        if !self.settings.adblock_allowlist.iter().any(|h| h == &clean)
            && self.settings.adblock_allowlist.len() < 256
        {
            self.settings.adblock_allowlist.push(clean);
            self.save_settings();
        }
    }

    pub fn remove_allowlist_host(&mut self, raw: &str) {
        let clean = normalize_host(raw);
        self.settings.adblock_allowlist.retain(|h| h != &clean);
        self.save_settings();
    }

    pub fn increment_blocked_ads(&mut self) {
        self.settings.total_blocked_ads = self.settings.total_blocked_ads.saturating_add(1);
        self.unsaved_blocked_ticks += 1;
        if self.unsaved_blocked_ticks >= 10 {
            self.unsaved_blocked_ticks = 0;
            self.save_settings();
        }
    }

    pub fn flush_counters(&mut self) {
        if self.unsaved_blocked_ticks > 0 {
            self.unsaved_blocked_ticks = 0;
            self.save_settings();
        }
    }

    pub fn save_settings(&self) {
        write_json(self.data_dir.join("settings.json"), &self.settings);
    }

    pub fn save_history(&self) {
        write_json(self.data_dir.join("history.json"), &self.history);
    }

    pub fn save_bookmarks(&self) {
        write_json(self.data_dir.join("bookmarks.json"), &self.bookmarks);
    }

    pub fn save_downloads(&self) {
        write_json(self.data_dir.join("downloads.json"), &self.downloads);
    }

    pub fn save_quick_links(&self) {
        write_json(self.data_dir.join("quick_links.json"), &self.quick_links);
    }

    pub fn save_folders(&mut self, folders: Vec<SavedFolder>) {
        self.folders = sanitize_folders(folders);
        write_json(self.data_dir.join("folders.json"), &self.folders);
    }

    pub fn set_quick_links(&mut self, raw_items: Vec<QuickLinkItem>) -> Vec<QuickLinkItem> {
        let mut cleaned = Vec::new();
        for item in raw_items.into_iter().take(24) {
            let Some(url) = normalize_quick_link_url(&item.url) else {
                continue;
            };
            let title_trim = item.title.trim();
            let title = if title_trim.is_empty() {
                let d = domain_from_url(&url);
                if d.is_empty() {
                    url.clone()
                } else {
                    d
                }
            } else {
                title_trim.chars().take(40).collect()
            };
            cleaned.push(QuickLinkItem { title, url });
        }
        self.quick_links = cleaned;
        self.save_quick_links();
        self.quick_links.clone()
    }

    pub fn reset_quick_links(&mut self) -> Vec<QuickLinkItem> {
        self.quick_links = default_quick_links();
        self.save_quick_links();
        self.quick_links.clone()
    }

    fn favicons_dir(&self) -> PathBuf {
        self.data_dir.join("favicons")
    }

    pub fn save_favicon_png(&self, domain: &str, png_bytes: &[u8]) {
        const PNG_MAGIC: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
        if png_bytes.len() < 16
            || png_bytes.len() > 256 * 1024
            || !png_bytes.starts_with(PNG_MAGIC)
        {
            return;
        }
        let Some(clean) = sanitize_favicon_domain(domain) else {
            return;
        };
        if clean.starts_with("kurowser") {
            return;
        }
        let dir = self.favicons_dir();
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join(format!("{}.png", clean));
        if !target.exists() {
            let count = std::fs::read_dir(&dir).map(|rd| rd.count()).unwrap_or(0);
            if count >= 256 {
                return;
            }
        }
        let _ = std::fs::write(target, png_bytes);
    }

    pub fn load_favicon_png(&self, domain: &str) -> Option<Vec<u8>> {
        let clean = sanitize_favicon_domain(domain)?;
        let path = self.favicons_dir().join(format!("{}.png", clean));
        std::fs::read(path).ok()
    }

    pub fn cached_favicon_domains(&self) -> Vec<String> {
        let Ok(rd) = std::fs::read_dir(self.favicons_dir()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("png") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(clean) = sanitize_favicon_domain(stem) {
                        out.push(clean);
                    }
                }
            }
        }
        out.sort();
        out
    }

    pub fn load_all_cached_favicons(&self, max_count: usize) -> Vec<(String, Vec<u8>)> {
        let Ok(rd) = std::fs::read_dir(self.favicons_dir()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for entry in rd.flatten() {
            if out.len() >= max_count {
                break;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("png") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(clean) = sanitize_favicon_domain(stem) {
                        if let Ok(bytes) = std::fs::read(&path) {
                            out.push((clean, bytes));
                        }
                    }
                }
            }
        }
        out
    }

    pub fn record_visit(&mut self, url: &str, title: &str) {
        if url.is_empty()
            || url.starts_with("kurowser://")
            || url.starts_with("about:")
            || url.starts_with("data:")
        {
            return;
        }

        let now = unix_now();
        let domain = domain_from_url(url);
        let clean_title = if title.trim().is_empty() {
            domain.clone()
        } else {
            title.trim().chars().take(120).collect()
        };

        if let Some(pos) = self.history.iter().position(|h| h.url == url) {
            let mut item = self.history.remove(pos);
            item.visit_count = item.visit_count.saturating_add(1);
            item.last_visited = now;
            if !title.trim().is_empty() {
                item.title = clean_title;
            }
            self.history.insert(0, item);
        } else {
            self.history.insert(
                0,
                HistoryItem {
                    url: url.to_string(),
                    title: clean_title,
                    domain,
                    visit_count: 1,
                    last_visited: now,
                },
            );
            if self.history.len() > 1000 {
                self.history.truncate(1000);
            }
        }
        self.save_history();
    }

    pub fn update_history_title(&mut self, url: &str, title: &str) {
        if title.trim().is_empty() || url.starts_with("kurowser://") {
            return;
        }
        if let Some(item) = self.history.iter_mut().find(|h| h.url == url) {
            let clean: String = title.trim().chars().take(120).collect();
            if item.title != clean {
                item.title = clean;
                self.save_history();
            }
        }
    }

    pub fn delete_history(&mut self, url: &str) {
        self.history.retain(|h| h.url != url);
        self.save_history();
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.save_history();
    }

    pub fn is_bookmarked(&self, url: &str) -> bool {
        self.bookmarks.iter().any(|b| b.url == url)
    }

    /// Returns `true` if now bookmarked, `false` if removed
    pub fn toggle_bookmark(&mut self, url: &str, title: &str) -> bool {
        if url.is_empty() || url.starts_with("about:") || url.starts_with("data:") {
            return false;
        }
        if let Some(pos) = self.bookmarks.iter().position(|b| b.url == url) {
            self.bookmarks.remove(pos);
            self.save_bookmarks();
            false
        } else {
            let domain = domain_from_url(url);
            let clean_title = if title.trim().is_empty() {
                domain.clone()
            } else {
                title.trim().chars().take(120).collect()
            };
            self.bookmarks.insert(
                0,
                BookmarkItem {
                    url: url.to_string(),
                    title: clean_title,
                    domain,
                    added_at: unix_now(),
                },
            );
            self.save_bookmarks();
            true
        }
    }

    pub fn delete_bookmark(&mut self, url: &str) {
        self.bookmarks.retain(|b| b.url != url);
        self.save_bookmarks();
    }

    pub fn record_download_start(&mut self, url: &str, path: &std::path::Path) -> u64 {
        let id = self.next_download_id;
        self.next_download_id += 1;
        let raw_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("download");
        let filename = sanitize_download_filename(raw_name);
        self.downloads.insert(
            0,
            DownloadItem {
                id,
                filename,
                path: path.to_string_lossy().to_string(),
                url: url.to_string(),
                status: "Downloading".to_string(),
                started_at: unix_now(),
            },
        );
        if self.downloads.len() > 200 {
            self.downloads.truncate(200);
        }
        self.save_downloads();
        id
    }

    pub fn update_download_status(&mut self, url: &str, path: Option<&std::path::Path>, ok: bool) {
        let status = if ok { "Completed" } else { "Failed" };
        let path_str = path.map(|p| p.to_string_lossy().to_string());
        if let Some(item) = self.downloads.iter_mut().find(|d| {
            d.status == "Downloading"
                && (d.url == url || path_str.as_deref() == Some(d.path.as_str()))
        }) {
            item.status = status.to_string();
            if let Some(ps) = path_str {
                item.path = ps;
            }
            self.save_downloads();
        }
    }

    pub fn clear_downloads(&mut self) {
        self.downloads.clear();
        self.save_downloads();
    }
}

fn sanitize_folders(raw: Vec<SavedFolder>) -> Vec<SavedFolder> {
    let mut out = Vec::new();
    for f in raw.into_iter().take(32) {
        let name_trim = f.name.trim();
        let name: String = if name_trim.is_empty() {
            "Folder".to_string()
        } else {
            name_trim.chars().take(36).collect()
        };
        let color_hex = Rgb::parse_hex(&f.color_hex)
            .map(|c| c.hex())
            .unwrap_or_else(|| FOLDER_PALETTE[0].1.to_string());
        let mut tabs = Vec::new();
        for t in f.tabs.into_iter().take(60) {
            let Some(url) = normalize_safe_url(&t.url) else {
                continue;
            };
            let title_trim = t.title.trim();
            let title: String = if title_trim.is_empty() {
                domain_from_url(&url)
            } else {
                title_trim.chars().take(64).collect()
            };
            tabs.push(SavedFolderTab { title, url });
        }
        out.push(SavedFolder {
            id: f.id.max(1),
            name,
            color_hex,
            collapsed: f.collapsed,
            tabs,
        });
    }
    out
}

pub fn domain_from_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("kurowser://") {
        let page = rest.split(['/', '?', '#']).next().unwrap_or("start");
        let clean = if page.is_empty() { "start" } else { page };
        return format!("kurowser.{}", clean);
    }
    let without_scheme = url
        .split_once("://")
        .map(|(_, r)| r)
        .unwrap_or(url);
    let host = without_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("");
    host.strip_prefix("www.").unwrap_or(host).to_lowercase()
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_json<T: for<'de> Deserialize<'de>>(path: PathBuf) -> Option<T> {
    let content = std::fs::read_to_string(path).ok()?;
    let clean = content.trim_start_matches('\u{FEFF}');
    serde_json::from_str(clean).ok()
}

fn write_json<T: Serialize>(path: PathBuf, data: &T) {
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_favicon_domain_blocks_traversal() {
        assert_eq!(
            sanitize_favicon_domain("Google.COM"),
            Some("google.com".to_string())
        );
        assert_eq!(sanitize_favicon_domain("../evil"), None);
        assert_eq!(sanitize_favicon_domain("foo/bar.png"), None);
        assert_eq!(sanitize_favicon_domain("a\\b"), None);
        assert_eq!(sanitize_favicon_domain(""), None);
        assert_eq!(sanitize_favicon_domain(".hidden"), None);
    }

    #[test]
    fn normalize_quick_link_url_accepts_safe_and_blocks_xss() {
        assert_eq!(
            normalize_quick_link_url("youtube.com"),
            Some("https://youtube.com".to_string())
        );
        assert_eq!(
            normalize_quick_link_url("kurowser://bookmarks/"),
            Some("kurowser://bookmarks".to_string())
        );
        assert_eq!(normalize_quick_link_url("javascript:alert(1)"), None);
        assert_eq!(normalize_quick_link_url("data:text/html,hi"), None);
        assert_eq!(normalize_quick_link_url("not a url"), None);
    }

    #[test]
    fn csprng_and_constant_time_eq_work() {
        let t1 = generate_csprng_hex(32);
        let t2 = generate_csprng_hex(32);
        assert_eq!(t1.len(), 64);
        assert_ne!(t1, t2);
        assert!(constant_time_eq(&t1, &t1));
        assert!(!constant_time_eq(&t1, &t2));
        assert!(!constant_time_eq("abc", "abcd"));
    }

    #[test]
    fn sanitize_download_filename_strips_traversal() {
        assert_eq!(sanitize_download_filename("../../evil:exe"), "evilexe");
        assert_eq!(sanitize_download_filename("file/..\\name.pdf"), "file..name.pdf");
        assert_eq!(sanitize_download_filename("report_v1.pdf"), "report_v1.pdf");
        assert_eq!(sanitize_download_filename("..."), "download");
    }

    #[test]
    fn all_theme_presets_and_custom_accents_meet_wcag() {
        let presets = [
            ThemePreset::Graphite,
            ThemePreset::Paper,
            ThemePreset::NordicPine,
            ThemePreset::Mist,
            ThemePreset::Espresso,
            ThemePreset::Sand,
        ];
        for preset in presets {
            let mut s = Settings::default();
            s.theme_preset = preset;
            for accent_opt in [None, Some("#0f766e"), Some("#1d4ed8"), Some("#facc15"), Some("#ec4899")] {
                s.custom_accent_hex = accent_opt.map(|x| x.to_string());
                let p = ThemePalette::from_settings(&s);
                assert_eq!(p.is_light, preset.is_light());
                for bg in [p.bg_app, p.bg_chrome, p.bg_surface, p.bg_hover, p.bg_active] {
                    assert!(p.text_primary.contrast_ratio(bg) >= 4.5);
                    assert!(p.text_muted.contrast_ratio(bg) >= 4.5);
                    assert!(p.text_inactive_tab.contrast_ratio(bg) >= 4.5);
                    assert!(p.accent_text.contrast_ratio(bg) >= 4.5);
                    assert!(p.success_fg.contrast_ratio(bg) >= 4.5);
                    assert!(p.warning_fg.contrast_ratio(bg) >= 4.5);
                    assert!(p.border.contrast_ratio(bg) >= 3.0);
                    assert!(p.danger_border.contrast_ratio(bg) >= 3.0);
                }
                assert!(p.on_accent.contrast_ratio(p.accent) >= 4.5);
                assert!(p.accent_text.contrast_ratio(p.accent_soft) >= 4.5);
                assert!(p.danger_fg.contrast_ratio(p.danger_bg) >= 4.5);
                assert!(p.text_disabled.contrast_ratio(p.bg_chrome) >= 3.0);
                assert!(p.text_disabled.contrast_ratio(p.bg_surface) >= 3.0);
                assert!(p.focus_ring.contrast_ratio(p.bg_chrome) >= 3.0);
                assert!(p.focus_ring.contrast_ratio(p.bg_surface) >= 3.0);
            }
        }
    }

    #[test]
    fn legacy_theme_presets_deserialize_safely() {
        let parsed: ThemePreset = serde_json::from_str("\"Midnight\"").unwrap();
        assert_eq!(parsed, ThemePreset::Graphite);
        assert_eq!(ThemePreset::from_id("emerald"), ThemePreset::NordicPine);
        assert_eq!(ThemePreset::from_id("paper"), ThemePreset::Paper);
    }

    #[test]
    fn settings_without_auto_update_default_to_true() {
        let legacy_json = r#"{"theme_preset":"Graphite","max_live_tabs":3,"sidebar_expanded":true}"#;
        let parsed: Settings = serde_json::from_str(legacy_json).unwrap();
        assert!(parsed.auto_update);
        assert!(!parsed.sidebar_pinned);
        assert!(parsed.sidebar_hover_expand);
        assert_eq!(parsed.idle_discard_secs, DEFAULT_IDLE_DISCARD_SECS);
        assert_eq!(parsed.js_heap_limit_mb, DEFAULT_JS_HEAP_LIMIT_MB);
        assert!(parsed.lean_webview_flags);
        assert!(parsed.network_in_process);
        assert!(parsed.trim_on_minimize);
        assert_eq!(parsed.idle_trim_secs, DEFAULT_IDLE_TRIM_SECS);
    }
}
