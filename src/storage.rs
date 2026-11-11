// ─── storage.rs ─── Hardened settings, folders, quick links, shields, history & CSPRNG tokens ───
use crate::adblock::{normalize_host, ShieldPolicy};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};

pub const FOLDER_PALETTE: &[(&str, &str)] = &[
    ("Indigo", "#818cf8"),
    ("Sky", "#38bdf8"),
    ("Emerald", "#34d399"),
    ("Amber", "#fbbf24"),
    ("Sakura", "#f472b6"),
    ("Rose", "#fb7185"),
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
    Midnight,
    Obsidian,
    Cyber,
    Sakura,
    Emerald,
    Crimson,
}

impl Default for ThemePreset {
    fn default() -> Self {
        Self::Midnight
    }
}

impl ThemePreset {
    pub fn id(&self) -> &'static str {
        match self {
            Self::Midnight => "midnight",
            Self::Obsidian => "obsidian",
            Self::Cyber => "cyber",
            Self::Sakura => "sakura",
            Self::Emerald => "emerald",
            Self::Crimson => "crimson",
        }
    }

    pub fn from_id(s: &str) -> Self {
        match s {
            "obsidian" => Self::Obsidian,
            "cyber" => Self::Cyber,
            "sakura" => Self::Sakura,
            "emerald" => Self::Emerald,
            "crimson" => Self::Crimson,
            _ => Self::Midnight,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Midnight => "Midnight Indigo",
            Self::Obsidian => "Obsidian Pro",
            Self::Cyber => "Cyber Cyan",
            Self::Sakura => "Sakura Night",
            Self::Emerald => "Emerald Slate",
            Self::Crimson => "Crimson Dark",
        }
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
}

#[derive(Clone, Debug)]
pub struct ThemePalette {
    pub bg_app: Rgb,
    pub bg_chrome: Rgb,
    pub bg_surface: Rgb,
    pub bg_hover: Rgb,
    pub bg_active: Rgb,
    pub border: Rgb,
    pub text_primary: Rgb,
    pub text_muted: Rgb,
    pub accent: Rgb,
    pub accent_soft: Rgb,
}

impl ThemePalette {
    pub fn from_settings(settings: &Settings) -> Self {
        let mut p = match settings.theme_preset {
            ThemePreset::Midnight => Self {
                bg_app: Rgb(0x13, 0x13, 0x1f),
                bg_chrome: Rgb(0x18, 0x18, 0x26),
                bg_surface: Rgb(0x0f, 0x0f, 0x19),
                bg_hover: Rgb(0x21, 0x21, 0x35),
                bg_active: Rgb(0x27, 0x27, 0x42),
                border: Rgb(0x2c, 0x2c, 0x44),
                text_primary: Rgb(0xed, 0xed, 0xf7),
                text_muted: Rgb(0x88, 0x88, 0xa8),
                accent: Rgb(0xa7, 0x8b, 0xfa),
                accent_soft: Rgb(0x2b, 0x24, 0x4b),
            },
            ThemePreset::Obsidian => Self {
                bg_app: Rgb(0x0d, 0x0d, 0x10),
                bg_chrome: Rgb(0x14, 0x14, 0x18),
                bg_surface: Rgb(0x09, 0x09, 0x0c),
                bg_hover: Rgb(0x1e, 0x1e, 0x24),
                bg_active: Rgb(0x26, 0x26, 0x2f),
                border: Rgb(0x2b, 0x2b, 0x34),
                text_primary: Rgb(0xf2, 0xf2, 0xf5),
                text_muted: Rgb(0x8e, 0x8e, 0x9b),
                accent: Rgb(0x60, 0xa5, 0xfa),
                accent_soft: Rgb(0x1c, 0x2a, 0x3f),
            },
            ThemePreset::Cyber => Self {
                bg_app: Rgb(0x0a, 0x10, 0x1b),
                bg_chrome: Rgb(0x0f, 0x17, 0x2a),
                bg_surface: Rgb(0x08, 0x0d, 0x17),
                bg_hover: Rgb(0x18, 0x23, 0x3b),
                bg_active: Rgb(0x1d, 0x2b, 0x48),
                border: Rgb(0x23, 0x35, 0x54),
                text_primary: Rgb(0xe8, 0xf2, 0xff),
                text_muted: Rgb(0x7c, 0x93, 0xb8),
                accent: Rgb(0x22, 0xd3, 0xee),
                accent_soft: Rgb(0x11, 0x33, 0x44),
            },
            ThemePreset::Sakura => Self {
                bg_app: Rgb(0x15, 0x0f, 0x19),
                bg_chrome: Rgb(0x1d, 0x15, 0x23),
                bg_surface: Rgb(0x10, 0x0b, 0x14),
                bg_hover: Rgb(0x29, 0x1d, 0x32),
                bg_active: Rgb(0x34, 0x22, 0x3d),
                border: Rgb(0x3a, 0x28, 0x45),
                text_primary: Rgb(0xf8, 0xeb, 0xf4),
                text_muted: Rgb(0xa3, 0x84, 0x9b),
                accent: Rgb(0xf4, 0x72, 0xb6),
                accent_soft: Rgb(0x3f, 0x1d, 0x34),
            },
            ThemePreset::Emerald => Self {
                bg_app: Rgb(0x0c, 0x15, 0x12),
                bg_chrome: Rgb(0x11, 0x1d, 0x19),
                bg_surface: Rgb(0x09, 0x10, 0x0e),
                bg_hover: Rgb(0x18, 0x29, 0x23),
                bg_active: Rgb(0x1e, 0x35, 0x2c),
                border: Rgb(0x24, 0x3c, 0x33),
                text_primary: Rgb(0xe8, 0xf7, 0xf1),
                text_muted: Rgb(0x7d, 0xa2, 0x95),
                accent: Rgb(0x34, 0xd3, 0x99),
                accent_soft: Rgb(0x14, 0x38, 0x2b),
            },
            ThemePreset::Crimson => Self {
                bg_app: Rgb(0x16, 0x0d, 0x11),
                bg_chrome: Rgb(0x1e, 0x13, 0x18),
                bg_surface: Rgb(0x11, 0x0a, 0x0d),
                bg_hover: Rgb(0x2c, 0x1a, 0x22),
                bg_active: Rgb(0x38, 0x20, 0x2a),
                border: Rgb(0x41, 0x26, 0x31),
                text_primary: Rgb(0xfa, 0xeb, 0xf0),
                text_muted: Rgb(0xa6, 0x81, 0x8d),
                accent: Rgb(0xfb, 0x71, 0x85),
                accent_soft: Rgb(0x42, 0x1d, 0x28),
            },
        };

        if let Some(ref custom) = settings.custom_accent_hex {
            if let Some(rgb) = Rgb::parse_hex(custom) {
                p.accent = rgb;
                p.accent_soft = p.bg_chrome.mix(rgb, 0.22);
                p.bg_active = p.bg_chrome.mix(rgb, 0.16);
            }
        }
        p
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_preset: ThemePreset,
    pub custom_accent_hex: Option<String>,
    pub search_engine: SearchEngine,
    pub max_live_tabs: usize,
    pub sidebar_expanded: bool,
    #[serde(default = "default_true")]
    pub adblock_enabled: bool,
    #[serde(default = "default_true")]
    pub adblock_cosmetic: bool,
    #[serde(default)]
    pub adblock_allowlist: Vec<String>,
    #[serde(default)]
    pub total_blocked_ads: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_preset: ThemePreset::Midnight,
            custom_accent_hex: None,
            search_engine: SearchEngine::Google,
            max_live_tabs: 3,
            sidebar_expanded: true,
            adblock_enabled: true,
            adblock_cosmetic: true,
            adblock_allowlist: Vec::new(),
            total_blocked_ads: 0,
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
    next_download_id: u64,
    unsaved_blocked_ticks: u64,
    data_dir: PathBuf,
}

impl BrowserData {
    pub fn load() -> Self {
        let local_app = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(local_app).join("Kurowser");
        let _ = std::fs::create_dir_all(&data_dir);

        let mut settings: Settings = read_json(data_dir.join("settings.json")).unwrap_or_default();
        settings.max_live_tabs = settings.max_live_tabs.clamp(2, 4);
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
    serde_json::from_str(&content).ok()
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
}
