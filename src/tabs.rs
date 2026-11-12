// Tab and Arc/Zen Folder state management with LRU eviction, idle discard & Shields counter
use std::time::{Duration, Instant};

use crate::engine::KurowserWebView;
use crate::storage::{domain_from_url, Rgb, SavedFolder, SavedFolderTab, FOLDER_PALETTE};

pub type TabId = u64;
pub type FolderId = u64;

#[derive(Clone, Debug)]
pub struct TabFolder {
    pub id: FolderId,
    pub name: String,
    pub color: Rgb,
    pub collapsed: bool,
}

pub struct Tab {
    pub id: TabId,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub folder_id: Option<FolderId>,
    pub blocked_count: u64,
    pub webview: Option<KurowserWebView>,
    pub last_active: u64,
    pub last_active_at: Instant,
    pub pinned: bool,
    pub is_loading: bool,
    pub is_suspended: bool,
    pub is_audio: bool,
    pub active_downloads: u32,
    pub has_active_upload: bool,
    pub discard_in_flight: bool,
    pub scroll_x: i32,
    pub scroll_y: i32,
    pub restore_scroll_on_load: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

pub struct SidebarTabInfo<'a> {
    pub id: TabId,
    pub title: &'a str,
    pub domain: &'a str,
    pub folder_id: Option<FolderId>,
    pub folder_color: Option<Rgb>,
    pub is_live: bool,
    pub is_active: bool,
    pub pinned: bool,
    pub is_loading: bool,
    pub is_suspended: bool,
    pub is_audio: bool,
}

pub enum SidebarEntry<'a> {
    FolderHeader {
        folder_id: FolderId,
        name: &'a str,
        color: Rgb,
        collapsed: bool,
        tab_count: usize,
        has_active_child: bool,
    },
    Tab(SidebarTabInfo<'a>),
}

pub struct TabStore {
    pub tabs: Vec<Tab>,
    pub folders: Vec<TabFolder>,
    pub active_id: Option<TabId>,
    next_id: u64,
    next_folder_id: u64,
    sequence: u64,
}

impl TabStore {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            folders: Vec::new(),
            active_id: None,
            next_id: 1,
            next_folder_id: 1,
            sequence: 0,
        }
    }

    pub fn load_saved_folders(&mut self, saved: &[SavedFolder]) {
        self.folders.clear();
        let mut max_fid = 0u64;
        for sf in saved {
            let fid = if sf.id == 0 || self.folders.iter().any(|f| f.id == sf.id) {
                max_fid + 1
            } else {
                sf.id
            };
            max_fid = max_fid.max(fid);
            let color = Rgb::parse_hex(&sf.color_hex)
                .unwrap_or_else(|| Rgb::parse_hex(FOLDER_PALETTE[0].1).unwrap_or(Rgb(0x81, 0x8c, 0xf8)));
            self.folders.push(TabFolder {
                id: fid,
                name: truncate_chars(sf.name.trim(), 36),
                color,
                collapsed: sf.collapsed,
            });
            for st in &sf.tabs {
                let id = self.next_id;
                self.next_id += 1;
                self.tabs.push(Tab {
                    id,
                    title: if st.title.trim().is_empty() {
                        domain_from_url(&st.url)
                    } else {
                        truncate_chars(st.title.trim(), 48)
                    },
                    url: st.url.clone(),
                    domain: domain_from_url(&st.url),
                    folder_id: Some(fid),
                    blocked_count: 0,
                    webview: None,
                    last_active: 0,
                    last_active_at: Instant::now(),
                    pinned: false,
                    is_loading: false,
                    is_suspended: false,
                    is_audio: false,
                    active_downloads: 0,
                    has_active_upload: false,
                    discard_in_flight: false,
                    scroll_x: 0,
                    scroll_y: 0,
                    restore_scroll_on_load: false,
                    can_go_back: false,
                    can_go_forward: false,
                });
            }
        }
        self.next_folder_id = max_fid + 1;
    }

    pub fn export_saved_folders(&self) -> Vec<SavedFolder> {
        self.folders
            .iter()
            .map(|f| {
                let tabs = self
                    .tabs
                    .iter()
                    .filter(|t| t.folder_id == Some(f.id))
                    .map(|t| SavedFolderTab {
                        title: t.title.clone(),
                        url: t.url.clone(),
                    })
                    .collect();
                SavedFolder {
                    id: f.id,
                    name: f.name.clone(),
                    color_hex: f.color.hex(),
                    collapsed: f.collapsed,
                    tabs,
                }
            })
            .collect()
    }

    pub fn add_tab(&mut self, title: &str, url: &str) -> TabId {
        self.add_tab_in_folder(title, url, None)
    }

    pub fn add_tab_in_folder(
        &mut self,
        title: &str,
        url: &str,
        folder_id: Option<FolderId>,
    ) -> TabId {
        let valid_folder = folder_id.filter(|fid| self.folders.iter().any(|f| f.id == *fid));
        if let Some(fid) = valid_folder {
            if let Some(f) = self.folders.iter_mut().find(|f| f.id == fid) {
                f.collapsed = false;
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.sequence += 1;
        self.tabs.push(Tab {
            id,
            title: title.to_string(),
            url: url.to_string(),
            domain: domain_from_url(url),
            folder_id: valid_folder,
            blocked_count: 0,
            webview: None,
            last_active: self.sequence,
            last_active_at: Instant::now(),
            pinned: false,
            is_loading: false,
            is_suspended: false,
            is_audio: false,
            active_downloads: 0,
            has_active_upload: false,
            discard_in_flight: false,
            scroll_x: 0,
            scroll_y: 0,
            restore_scroll_on_load: false,
            can_go_back: false,
            can_go_forward: false,
        });
        id
    }

    pub fn create_folder(&mut self, name: &str, color: Option<Rgb>) -> FolderId {
        let fid = self.next_folder_id;
        self.next_folder_id += 1;
        let palette_idx = ((fid as usize).saturating_sub(1)) % FOLDER_PALETTE.len();
        let chosen_color = color.unwrap_or_else(|| {
            Rgb::parse_hex(FOLDER_PALETTE[palette_idx].1).unwrap_or(Rgb(0x81, 0x8c, 0xf8))
        });
        let clean_name = if name.trim().is_empty() {
            format!("Folder {}", self.folders.len() + 1)
        } else {
            truncate_chars(name.trim(), 36)
        };
        self.folders.push(TabFolder {
            id: fid,
            name: clean_name,
            color: chosen_color,
            collapsed: false,
        });
        fid
    }

    pub fn create_folder_from_tab(&mut self, tab_id: TabId, name: &str) -> Option<FolderId> {
        if self.get_tab(tab_id).is_none() {
            return None;
        }
        let fid = self.create_folder(name, None);
        self.move_tab_to_folder(tab_id, Some(fid));
        Some(fid)
    }

    pub fn get_folder(&self, fid: FolderId) -> Option<&TabFolder> {
        self.folders.iter().find(|f| f.id == fid)
    }

    pub fn rename_folder(&mut self, fid: FolderId, name: &str) -> bool {
        let clean = name.trim();
        if clean.is_empty() {
            return false;
        }
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == fid) {
            f.name = truncate_chars(clean, 36);
            return true;
        }
        false
    }

    pub fn set_folder_color(&mut self, fid: FolderId, color: Rgb) {
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == fid) {
            f.color = color;
        }
    }

    pub fn toggle_folder_collapsed(&mut self, fid: FolderId) {
        if let Some(f) = self.folders.iter_mut().find(|f| f.id == fid) {
            f.collapsed = !f.collapsed;
        }
    }

    pub fn move_tab_to_folder(&mut self, tab_id: TabId, folder_id: Option<FolderId>) {
        if let Some(fid) = folder_id {
            if !self.folders.iter().any(|f| f.id == fid) {
                return;
            }
            if let Some(f) = self.folders.iter_mut().find(|f| f.id == fid) {
                f.collapsed = false;
            }
        }
        if let Some(tab) = self.get_tab_mut(tab_id) {
            tab.folder_id = folder_id;
            if folder_id.is_some() {
                tab.pinned = false;
            }
        }
    }

    pub fn delete_folder_keep_tabs(&mut self, fid: FolderId) {
        for tab in &mut self.tabs {
            if tab.folder_id == Some(fid) {
                tab.folder_id = None;
            }
        }
        self.folders.retain(|f| f.id != fid);
    }

    pub fn delete_folder_with_tabs(&mut self, fid: FolderId) -> Vec<KurowserWebView> {
        let mut closed_wvs = Vec::new();
        let active_was_in_folder = self
            .active_tab()
            .and_then(|t| t.folder_id)
            == Some(fid);

        let mut remaining = Vec::with_capacity(self.tabs.len());
        for mut t in self.tabs.drain(..) {
            if t.folder_id == Some(fid) {
                if let Some(wv) = t.webview.take() {
                    closed_wvs.push(wv);
                }
            } else {
                remaining.push(t);
            }
        }
        self.tabs = remaining;
        self.folders.retain(|f| f.id != fid);

        if active_was_in_folder {
            self.active_id = self.tabs.first().map(|t| t.id);
        }
        closed_wvs
    }

    pub fn get_tab(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn get_tab_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active_id.and_then(|id| self.get_tab(id))
    }

    pub fn set_active(&mut self, id: TabId) {
        self.sequence += 1;
        let seq = self.sequence;
        let mut parent_fid = None;
        if let Some(tab) = self.get_tab_mut(id) {
            tab.last_active = seq;
            tab.last_active_at = Instant::now();
            tab.is_suspended = false;
            tab.discard_in_flight = false;
            parent_fid = tab.folder_id;
        }
        if let Some(fid) = parent_fid {
            if let Some(f) = self.folders.iter_mut().find(|f| f.id == fid) {
                f.collapsed = false;
            }
        }
        self.active_id = Some(id);
    }

    pub fn close_tab(&mut self, id: TabId) -> Option<KurowserWebView> {
        let idx = self.tabs.iter().position(|t| t.id == id)?;
        let tab = self.tabs.remove(idx);

        if self.active_id == Some(id) {
            if self.tabs.is_empty() {
                self.active_id = None;
            } else {
                let new_idx = idx.min(self.tabs.len() - 1);
                self.active_id = Some(self.tabs[new_idx].id);
            }
        }
        tab.webview
    }

    pub fn toggle_pin(&mut self, id: TabId) {
        let Some(pos) = self.tabs.iter().position(|t| t.id == id) else {
            return;
        };
        let mut tab = self.tabs.remove(pos);
        tab.pinned = !tab.pinned;
        if tab.pinned {
            tab.folder_id = None;
        }
        let insert_at = self.tabs.iter().take_while(|t| t.pinned).count();
        self.tabs.insert(insert_at, tab);
    }

    pub fn live_count(&self) -> usize {
        self.tabs.iter().filter(|t| t.webview.is_some()).count()
    }

    pub fn find_lru_live(&self) -> Option<TabId> {
        let normal_candidate = self
            .tabs
            .iter()
            .filter(|t| {
                t.webview.is_some()
                    && Some(t.id) != self.active_id
                    && !t.pinned
                    && !t.is_audio
                    && t.active_downloads == 0
                    && !t.has_active_upload
            })
            .min_by_key(|t| t.last_active)
            .map(|t| t.id);

        if normal_candidate.is_some() {
            return normal_candidate;
        }

        self.tabs
            .iter()
            .filter(|t| t.webview.is_some() && Some(t.id) != self.active_id)
            .min_by_key(|t| t.last_active)
            .map(|t| t.id)
    }

    pub fn evict_lru(&mut self) -> Option<KurowserWebView> {
        let lru_id = self.find_lru_live()?;
        self.discard_tab(lru_id)
    }

    pub fn is_idle_discard_eligible(
        tab: &Tab,
        active_id: Option<TabId>,
        timeout: Duration,
        now: Instant,
    ) -> bool {
        if timeout.is_zero() {
            return false;
        }
        Some(tab.id) != active_id
            && !tab.pinned
            && !tab.is_loading
            && !tab.is_audio
            && tab.active_downloads == 0
            && !tab.has_active_upload
            && now.saturating_duration_since(tab.last_active_at) >= timeout
    }

    pub fn discard_tab(&mut self, id: TabId) -> Option<KurowserWebView> {
        let tab = self.get_tab_mut(id)?;
        tab.is_suspended = false;
        tab.is_loading = false;
        tab.is_audio = false;
        tab.discard_in_flight = false;
        tab.can_go_back = false;
        tab.can_go_forward = false;
        tab.restore_scroll_on_load = tab.scroll_x != 0 || tab.scroll_y != 0;
        tab.webview.take()
    }

    pub fn find_by_url(&self, url: &str) -> Option<TabId> {
        self.tabs.iter().find(|t| t.url == url).map(|t| t.id)
    }

    fn make_tab_info<'a>(&'a self, t: &'a Tab, folder_color: Option<Rgb>) -> SidebarTabInfo<'a> {
        SidebarTabInfo {
            id: t.id,
            title: t.title.as_str(),
            domain: t.domain.as_str(),
            folder_id: t.folder_id,
            folder_color,
            is_live: t.webview.is_some(),
            is_active: Some(t.id) == self.active_id,
            pinned: t.pinned,
            is_loading: t.is_loading,
            is_suspended: t.is_suspended,
            is_audio: t.is_audio,
        }
    }

    pub fn sidebar_entries(&self) -> Vec<SidebarEntry<'_>> {
        let mut out = Vec::with_capacity(self.folders.len() + self.tabs.len());

        // 1. Pinned tabs at very top
        for t in self.tabs.iter().filter(|t| t.pinned && t.folder_id.is_none()) {
            out.push(SidebarEntry::Tab(self.make_tab_info(t, None)));
        }

        // 2. Arc/Zen Tab Folders + their child tabs when expanded
        for f in &self.folders {
            let children: Vec<&Tab> = self
                .tabs
                .iter()
                .filter(|t| t.folder_id == Some(f.id))
                .collect();
            let has_active_child = children
                .iter()
                .any(|t| Some(t.id) == self.active_id);
            out.push(SidebarEntry::FolderHeader {
                folder_id: f.id,
                name: f.name.as_str(),
                color: f.color,
                collapsed: f.collapsed,
                tab_count: children.len(),
                has_active_child,
            });
            if !f.collapsed {
                for t in children {
                    out.push(SidebarEntry::Tab(self.make_tab_info(t, Some(f.color))));
                }
            }
        }

        // 3. Regular ungrouped tabs
        for t in self.tabs.iter().filter(|t| !t.pinned && t.folder_id.is_none()) {
            out.push(SidebarEntry::Tab(self.make_tab_info(t, None)));
        }

        out
    }

    pub fn ordered_tab_ids(&self) -> Vec<TabId> {
        self.sidebar_entries()
            .into_iter()
            .filter_map(|e| match e {
                SidebarEntry::Tab(t) => Some(t.id),
                SidebarEntry::FolderHeader { .. } => None,
            })
            .collect()
    }

    pub fn next_tab_id(&self) -> Option<TabId> {
        let ids = self.ordered_tab_ids();
        if ids.len() <= 1 {
            return None;
        }
        let active = self.active_id?;
        let idx = ids.iter().position(|&id| id == active)?;
        Some(ids[(idx + 1) % ids.len()])
    }

    pub fn prev_tab_id(&self) -> Option<TabId> {
        let ids = self.ordered_tab_ids();
        if ids.len() <= 1 {
            return None;
        }
        let active = self.active_id?;
        let idx = ids.iter().position(|&id| id == active)?;
        let prev = if idx == 0 { ids.len() - 1 } else { idx - 1 };
        Some(ids[prev])
    }

    pub fn nth_tab_id(&self, index: usize) -> Option<TabId> {
        let ids = self.ordered_tab_ids();
        if ids.is_empty() {
            return None;
        }
        if index >= 8 {
            return ids.last().copied();
        }
        ids.get(index).copied()
    }

    pub fn update_title(&mut self, id: TabId, title: &str) {
        if let Some(tab) = self.get_tab_mut(id) {
            if !title.is_empty() {
                tab.title = truncate_chars(title, 48);
            }
        }
    }

    pub fn update_url(&mut self, id: TabId, display_url: &str) {
        if let Some(tab) = self.get_tab_mut(id) {
            if !display_url.is_empty()
                && !display_url.starts_with("about:blank")
                && !display_url.starts_with("data:")
            {
                tab.url = display_url.to_string();
                tab.domain = domain_from_url(display_url);
            }
        }
    }

    pub fn update_page_state(&mut self, id: TabId, display_url: &str, finished: bool) {
        if let Some(tab) = self.get_tab_mut(id) {
            tab.is_loading = !finished;
            if !display_url.is_empty()
                && !display_url.starts_with("about:blank")
                && !display_url.starts_with("data:")
            {
                tab.url = display_url.to_string();
                tab.domain = domain_from_url(display_url);
            }
        }
    }

    pub fn increment_blocked(&mut self, id: TabId) {
        if let Some(tab) = self.get_tab_mut(id) {
            tab.blocked_count = tab.blocked_count.saturating_add(1);
        }
    }

    pub fn reset_blocked(&mut self, id: TabId) {
        if let Some(tab) = self.get_tab_mut(id) {
            tab.blocked_count = 0;
        }
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let collected: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", collected)
    } else {
        collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_crud_move_and_roundtrip() {
        let mut store = TabStore::new();
        let t1 = store.add_tab("GitHub", "https://github.com");
        let t2 = store.add_tab("Docs", "https://docs.rs");
        let fid = store.create_folder("Work", None);

        store.move_tab_to_folder(t1, Some(fid));
        store.move_tab_to_folder(t2, Some(fid));
        assert_eq!(store.sidebar_entries().len(), 3); // header + 2 tabs

        store.toggle_folder_collapsed(fid);
        assert_eq!(store.sidebar_entries().len(), 1); // header only when collapsed

        // Activating a child expands its folder
        store.set_active(t2);
        assert!(!store.get_folder(fid).unwrap().collapsed);
        assert_eq!(store.sidebar_entries().len(), 3);

        // Save/load roundtrip preserves folders and leaves webviews lazy
        let exported = store.export_saved_folders();
        let mut restored = TabStore::new();
        restored.load_saved_folders(&exported);
        assert_eq!(restored.folders.len(), 1);
        assert_eq!(restored.tabs.len(), 2);
        assert_eq!(restored.live_count(), 0);

        // Delete keeping tabs
        restored.delete_folder_keep_tabs(restored.folders[0].id);
        assert!(restored.folders.is_empty());
        assert_eq!(restored.tabs.len(), 2);
        assert!(restored.tabs.iter().all(|t| t.folder_id.is_none()));
    }

    #[test]
    fn idle_discard_guards_protect_pinned_audio_downloads_uploads() {
        let mut store = TabStore::new();
        let active = store.add_tab("Active", "https://example.com");
        let bg = store.add_tab("Bg", "https://github.com");
        store.set_active(active);

        let timeout = Duration::from_secs(180);
        let now = Instant::now();
        let past = now.checked_sub(Duration::from_secs(200)).unwrap();

        {
            let tab = store.get_tab_mut(active).unwrap();
            tab.last_active_at = past;
        }
        assert!(!TabStore::is_idle_discard_eligible(
            store.get_tab(active).unwrap(),
            Some(active),
            timeout,
            now
        ));

        {
            let tab = store.get_tab_mut(bg).unwrap();
            tab.last_active_at = past;
        }
        assert!(TabStore::is_idle_discard_eligible(
            store.get_tab(bg).unwrap(),
            Some(active),
            timeout,
            now
        ));

        store.get_tab_mut(bg).unwrap().pinned = true;
        assert!(!TabStore::is_idle_discard_eligible(
            store.get_tab(bg).unwrap(),
            Some(active),
            timeout,
            now
        ));
        store.get_tab_mut(bg).unwrap().pinned = false;

        store.get_tab_mut(bg).unwrap().is_audio = true;
        assert!(!TabStore::is_idle_discard_eligible(
            store.get_tab(bg).unwrap(),
            Some(active),
            timeout,
            now
        ));
        store.get_tab_mut(bg).unwrap().is_audio = false;

        store.get_tab_mut(bg).unwrap().active_downloads = 1;
        assert!(!TabStore::is_idle_discard_eligible(
            store.get_tab(bg).unwrap(),
            Some(active),
            timeout,
            now
        ));
        store.get_tab_mut(bg).unwrap().active_downloads = 0;

        store.get_tab_mut(bg).unwrap().has_active_upload = true;
        assert!(!TabStore::is_idle_discard_eligible(
            store.get_tab(bg).unwrap(),
            Some(active),
            timeout,
            now
        ));
        store.get_tab_mut(bg).unwrap().has_active_upload = false;

        assert!(!TabStore::is_idle_discard_eligible(
            store.get_tab(bg).unwrap(),
            Some(active),
            Duration::ZERO,
            now
        ));
    }
}
