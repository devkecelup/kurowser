// ─── omnibox.rs ─── Address bar autocomplete & ranking engine ───
use crate::storage::{domain_from_url, BookmarkItem, HistoryItem, SearchEngine};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuggestionKind {
    Bookmark,
    History,
    TopSite,
    Internal,
    DirectUrl,
    Search,
}

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    pub title: String,
    pub url: String,
    pub subtitle: String,
    pub domain: String,
}

const POPULAR_SITES: &[(&str, &str, &str)] = &[
    ("Google", "google.com", "https://www.google.com"),
    ("YouTube", "youtube.com", "https://www.youtube.com"),
    ("GitHub", "github.com", "https://github.com"),
    ("Gmail", "gmail.com", "https://mail.google.com"),
    ("Google Drive", "drive.google.com", "https://drive.google.com"),
    ("Google Translate", "translate.google.com", "https://translate.google.com"),
    ("Google Maps", "maps.google.com", "https://maps.google.com"),
    ("Reddit", "reddit.com", "https://www.reddit.com"),
    ("X / Twitter", "x.com", "https://x.com"),
    ("Stack Overflow", "stackoverflow.com", "https://stackoverflow.com"),
    ("Spotify Web", "open.spotify.com", "https://open.spotify.com"),
    ("WhatsApp Web", "web.whatsapp.com", "https://web.whatsapp.com"),
    ("ChatGPT", "chatgpt.com", "https://chatgpt.com"),
    ("Claude", "claude.ai", "https://claude.ai"),
    ("Gemini", "gemini.google.com", "https://gemini.google.com"),
    ("Notion", "notion.so", "https://www.notion.so"),
    ("Figma", "figma.com", "https://www.figma.com"),
    ("Wikipedia", "wikipedia.org", "https://www.wikipedia.org"),
    ("Instagram", "instagram.com", "https://www.instagram.com"),
    ("TikTok", "tiktok.com", "https://www.tiktok.com"),
    ("Discord", "discord.com", "https://discord.com/app"),
    ("Twitch", "twitch.tv", "https://www.twitch.tv"),
    ("Netflix", "netflix.com", "https://www.netflix.com"),
    ("LinkedIn", "linkedin.com", "https://www.linkedin.com"),
    ("Tokopedia", "tokopedia.com", "https://www.tokopedia.com"),
    ("Shopee Indonesia", "shopee.co.id", "https://shopee.co.id"),
    ("Kompas", "kompas.com", "https://www.kompas.com"),
    ("Detik", "detik.com", "https://www.detik.com"),
    ("DeepL", "deepl.com", "https://www.deepl.com"),
    ("Canva", "canva.com", "https://www.canva.com"),
    ("Vercel", "vercel.com", "https://vercel.com"),
    ("Crates.io", "crates.io", "https://crates.io"),
    ("Docs.rs", "docs.rs", "https://docs.rs"),
    ("Steam Store", "store.steampowered.com", "https://store.steampowered.com"),
    ("Bilibili", "bilibili.tv", "https://www.bilibili.tv"),
];

const INTERNAL_PAGES: &[(&str, &str)] = &[
    ("Kurowser Start Page", "kurowser://start"),
    ("History", "kurowser://history"),
    ("Bookmarks", "kurowser://bookmarks"),
    ("Downloads", "kurowser://downloads"),
    ("Tema & Settings", "kurowser://settings"),
];

pub fn suggest(
    raw_query: &str,
    history: &[HistoryItem],
    bookmarks: &[BookmarkItem],
    search_engine: SearchEngine,
) -> Vec<Suggestion> {
    let q = raw_query.trim();
    let q_lower = q.to_lowercase();

    // Empty or focused internal URL -> show recent Bookmarks & top History
    if q_lower.is_empty() || q_lower == "kurowser://" {
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        for b in bookmarks.iter().take(3) {
            if seen.insert(canonical_key(&b.url)) {
                results.push(Suggestion {
                    kind: SuggestionKind::Bookmark,
                    title: b.title.clone(),
                    url: b.url.clone(),
                    subtitle: format!("Bookmark · {}", b.url),
                    domain: b.domain.clone(),
                });
            }
        }
        for h in history.iter().take(6) {
            if results.len() >= 6 {
                break;
            }
            if seen.insert(canonical_key(&h.url)) {
                results.push(Suggestion {
                    kind: SuggestionKind::History,
                    title: h.title.clone(),
                    url: h.url.clone(),
                    subtitle: h.url.clone(),
                    domain: h.domain.clone(),
                });
            }
        }
        if results.len() < 5 {
            for &(title, url) in INTERNAL_PAGES {
                if results.len() >= 6 {
                    break;
                }
                if seen.insert(canonical_key(url)) {
                    results.push(Suggestion {
                        kind: SuggestionKind::Internal,
                        title: title.to_string(),
                        url: url.to_string(),
                        subtitle: url.to_string(),
                        domain: domain_from_url(url),
                    });
                }
            }
        }
        return results;
    }

    let mut scored: Vec<(i32, Suggestion)> = Vec::new();

    // 1. Bookmarks
    for b in bookmarks {
        let domain_l = b.domain.to_lowercase();
        let title_l = b.title.to_lowercase();
        let url_l = b.url.to_lowercase();
        if let Some(score) = match_score(&q_lower, &domain_l, &title_l, &url_l, 140) {
            scored.push((
                score,
                Suggestion {
                    kind: SuggestionKind::Bookmark,
                    title: b.title.clone(),
                    url: b.url.clone(),
                    subtitle: format!("Bookmark · {}", b.url),
                    domain: b.domain.clone(),
                },
            ));
        }
    }

    // 2. History
    for h in history {
        let domain_l = h.domain.to_lowercase();
        let title_l = h.title.to_lowercase();
        let url_l = h.url.to_lowercase();
        let freq_boost = (h.visit_count.min(20) as i32) * 3;
        if let Some(score) = match_score(&q_lower, &domain_l, &title_l, &url_l, 110 + freq_boost) {
            scored.push((
                score,
                Suggestion {
                    kind: SuggestionKind::History,
                    title: h.title.clone(),
                    url: h.url.clone(),
                    subtitle: h.url.clone(),
                    domain: h.domain.clone(),
                },
            ));
        }
    }

    // 3. Popular sites dictionary (makes typing `goo` -> `google.com` work immediately!)
    for &(name, host, url) in POPULAR_SITES {
        let host_l = host.to_lowercase();
        let name_l = name.to_lowercase();
        let url_l = url.to_lowercase();
        if let Some(score) = match_score(&q_lower, &host_l, &name_l, &url_l, 85) {
            scored.push((
                score,
                Suggestion {
                    kind: SuggestionKind::TopSite,
                    title: format!("{} ({})", host, name),
                    url: url.to_string(),
                    subtitle: url.to_string(),
                    domain: host.to_string(),
                },
            ));
        }
    }

    // 4. Internal Kurowser pages
    for &(title, url) in INTERNAL_PAGES {
        let title_l = title.to_lowercase();
        let url_l = url.to_lowercase();
        let slug = url.trim_start_matches("kurowser://");
        if let Some(score) = match_score(&q_lower, slug, &title_l, &url_l, 70) {
            scored.push((
                score,
                Suggestion {
                    kind: SuggestionKind::Internal,
                    title: title.to_string(),
                    url: url.to_string(),
                    subtitle: url.to_string(),
                    domain: domain_from_url(url),
                },
            ));
        }
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    let mut out: Vec<Suggestion> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for (_, item) in scored {
        if out.len() >= 5 {
            break;
        }
        if seen.insert(canonical_key(&item.url)) {
            out.push(item);
        }
    }

    // Add Direct URL / Search fallback smartly
    let looks_like_url = q_lower.starts_with("http://")
        || q_lower.starts_with("https://")
        || q_lower.starts_with("kurowser://")
        || q_lower.starts_with("localhost")
        || (q.contains('.') && !q.contains(' '));

    if looks_like_url {
        let normalized = crate::engine::normalize_input(q, search_engine);
        if seen.insert(canonical_key(&normalized)) {
            out.insert(
                0,
                Suggestion {
                    kind: SuggestionKind::DirectUrl,
                    title: format!("Buka {}", q),
                    url: normalized.clone(),
                    subtitle: normalized.clone(),
                    domain: domain_from_url(&normalized),
                },
            );
            if out.len() > 6 {
                out.truncate(6);
            }
        }
    } else {
        let search_url = crate::engine::normalize_input(q, search_engine);
        let search_item = Suggestion {
            kind: SuggestionKind::Search,
            title: format!("Cari \"{}\" di {}", q, search_engine.name()),
            url: search_url,
            subtitle: format!("Pencarian {}", search_engine.name()),
            domain: if search_engine == SearchEngine::Google {
                "google.com".to_string()
            } else {
                "duckduckgo.com".to_string()
            },
        };
        // Put top domain autocomplete #1 if strong prefix match exists, otherwise Search first
        if out.is_empty() {
            out.push(search_item);
        } else {
            out.insert(1.min(out.len()), search_item);
            if out.len() > 6 {
                out.truncate(6);
            }
        }
    }

    out
}

fn match_score(
    q: &str,
    domain: &str,
    title: &str,
    url: &str,
    base_weight: i32,
) -> Option<i32> {
    if domain.starts_with(q) {
        return Some(base_weight + 100 - (domain.len().min(40) as i32));
    }
    if title.starts_with(q) {
        return Some(base_weight + 75);
    }
    if domain.contains(q) {
        return Some(base_weight + 45);
    }
    if title.contains(q) {
        return Some(base_weight + 30);
    }
    if url.contains(q) {
        return Some(base_weight + 15);
    }
    None
}

fn canonical_key(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/').to_lowercase();
    trimmed
        .replace("://www.", "://")
        .replace("http://", "https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_goo_suggests_google_first() {
        let out = suggest("goo", &[], &[], SearchEngine::Google);
        assert!(!out.is_empty());
        assert_eq!(out[0].domain, "google.com");
        assert_eq!(out[0].url, "https://www.google.com");
        assert!(out.iter().any(|s| s.kind == SuggestionKind::Search
            && s.url.starts_with("https://www.google.com/search?q=goo")));
    }

    #[test]
    fn history_and_bookmarks_rank_and_dedup() {
        let bm = vec![BookmarkItem {
            url: "https://github.com/rust-lang".into(),
            title: "Rust Lang GitHub".into(),
            domain: "github.com".into(),
            added_at: 1,
        }];
        let hist = vec![HistoryItem {
            url: "https://github.com".into(),
            title: "GitHub".into(),
            domain: "github.com".into(),
            visit_count: 10,
            last_visited: 2,
        }];
        let out = suggest("git", &hist, &bm, SearchEngine::Google);
        assert_eq!(out[0].kind, SuggestionKind::Bookmark);
        assert_eq!(out[0].url, "https://github.com/rust-lang");
        // github.com from History deduplicates against POPULAR_SITES github.com
        let gh_count = out.iter().filter(|s| s.url == "https://github.com").count();
        assert_eq!(gh_count, 1);
    }

    #[test]
    fn normalize_url_vs_search() {
        assert_eq!(
            crate::engine::normalize_input("apa itu rust", SearchEngine::Google),
            "https://www.google.com/search?q=apa+itu+rust"
        );
        assert_eq!(
            crate::engine::normalize_input("google.com", SearchEngine::Google),
            "https://google.com"
        );
    }
}

