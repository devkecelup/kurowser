// ─── adblock.rs ─── Kurowser Shields: network, first-party YouTube & cosmetic blocker ───
use std::collections::HashSet;
use std::sync::OnceLock;
use windows::Win32::Security::Cryptography::{BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG};

struct RuleEngine {
    blocked_domains: HashSet<&'static str>,
    first_party_ad_paths: &'static [&'static str],
    third_party_ad_paths: &'static [&'static str],
}

static ENGINE: OnceLock<RuleEngine> = OnceLock::new();
static PROCESS_SALT: OnceLock<u64> = OnceLock::new();

pub fn process_salt() -> u64 {
    *PROCESS_SALT.get_or_init(|| {
        let mut buf = [0u8; 8];
        let ok = unsafe {
            BCryptGenRandom(None, &mut buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG).is_ok()
        };
        if ok {
            u64::from_le_bytes(buf)
        } else {
            0x517c_c1b7_2722_0a95
        }
    })
}

fn rules() -> &'static RuleEngine {
    ENGINE.get_or_init(|| {
        let mut set = HashSet::with_capacity(340);
        for d in BLOCKED_DOMAINS {
            set.insert(*d);
        }
        RuleEngine {
            blocked_domains: set,
            first_party_ad_paths: FIRST_PARTY_AD_PATHS,
            third_party_ad_paths: THIRD_PARTY_AD_PATHS,
        }
    })
}

fn salted_host_hash_pair(salt: u64, host: &str) -> (u32, u32) {
    let s1 = (salt & 0xffff_ffff) as u32;
    let s2 = (salt >> 32) as u32;
    let mut h1: u32 = 0x811c_9dc5 ^ s1;
    let mut h2: u32 = 0x9e37_79b9 ^ s2;
    for &b in host.as_bytes() {
        h1 ^= b as u32;
        h1 = h1.wrapping_mul(0x0100_0193);
        h2 ^= b as u32;
        h2 = h2.wrapping_mul(0x85eb_ca6b);
    }
    (h1, h2)
}

const COSMETIC_CSS: &str = r#"
ins.adsbygoogle,
.adsbygoogle,
[id^="google_ads_iframe"],
[id^="div-gpt-ad"],
iframe[src*="doubleclick.net"],
iframe[src*="googlesyndication.com"],
iframe[src*="adnxs.com"],
iframe[src*="taboola.com"],
iframe[src*="outbrain.com"],
iframe[src*="mgid.com"],
.taboola-container,
.OUTBRAIN,
[id^="rc-widget-"],
[id*="mgid"],
.mgbox,
[data-ad-slot],
[data-google-query-id],
[data-ad-region],
ytd-ad-slot-renderer,
ytd-in-feed-ad-layout-renderer,
ytd-banner-promo-renderer,
ytd-banner-promo-renderer-background,
ytd-statement-banner-renderer,
ytd-promoted-sparkles-web-renderer,
ytd-promoted-sparkles-text-search-renderer,
ytd-promoted-video-renderer,
ytd-display-ad-renderer,
ytd-video-masthead-ad-v3-renderer,
ytd-primetime-promo-renderer,
ytd-companion-slot-renderer,
ytd-action-companion-ad-renderer,
ytd-merch-shelf-renderer,
ytd-mealbar-promo-renderer,
ytm-promoted-sparkles-web-renderer,
ytm-companion-slot-renderer,
#masthead-ad,
#player-ads,
#panels > ytd-engagement-panel-section-list-renderer[target-id="engagement-panel-ads"],
ytd-rich-item-renderer:has(> #content > ytd-ad-slot-renderer),
ytd-rich-section-renderer:has(ytd-statement-banner-renderer),
.ytd-ad-slot-renderer,
.ytp-ad-module,
.ytp-ad-overlay-container,
.ytp-ad-text-overlay,
.ytp-ad-image-overlay,
.ytp-ad-progress,
.ytp-ad-progress-list,
.ytp-suggested-action,
.ytp-paid-content-overlay,
#offer-module {
  display: none !important;
  visibility: hidden !important;
  height: 0 !important;
  min-height: 0 !important;
}
"#;

/// Builds early document-start script for HTTP/HTTPS pages when Shields are ON.
/// Returns `None` when global Shields are disabled.
/// Allowlist hostnames are never embedded in plaintext; only salted per-process hashes are included.
pub fn build_document_script(policy: &ShieldPolicy, salt: u64) -> Option<String> {
    if !policy.global_enabled {
        return None;
    }

    let s1 = (salt & 0xffff_ffff) as u32;
    let s2 = (salt >> 32) as u32;
    let allow_hashes: Vec<String> = policy
        .allowlist
        .iter()
        .map(|h| normalize_host(h))
        .filter(|h| !h.is_empty())
        .map(|h| {
            let (h1, h2) = salted_host_hash_pair(salt, &h);
            format!("\"{:08x}:{:08x}\"", h1, h2)
        })
        .collect();
    let allow_json = format!("[{}]", allow_hashes.join(","));
    let cosmetic_flag = if policy.cosmetic_enabled { "true" } else { "false" };
    let css_literal = serde_json::to_string(COSMETIC_CSS).unwrap_or_else(|_| "\"\"".to_string());

    Some(format!(
        r#"(function(){{
  if (window.__kurowserShieldsBoot) return;
  window.__kurowserShieldsBoot = true;
  if (location.protocol !== 'http:' && location.protocol !== 'https:') return;

  const ALLOW_HASHES = new Set({allow_json});
  function hostHash(h) {{
    let h1 = (0x811c9dc5 ^ {s1}) >>> 0;
    let h2 = (0x9e3779b9 ^ {s2}) >>> 0;
    for (let i = 0; i < h.length; i++) {{
      const c = h.charCodeAt(i);
      if (c > 127) return '';
      h1 = Math.imul(h1 ^ c, 0x01000193) >>> 0;
      h2 = Math.imul(h2 ^ c, 0x85ebca6b) >>> 0;
    }}
    return h1.toString(16).padStart(8, '0') + ':' + h2.toString(16).padStart(8, '0');
  }}

  if (ALLOW_HASHES.size > 0) {{
    let cur = (location.hostname || '').replace(/^www\./i, '').toLowerCase();
    while (cur) {{
      if (ALLOW_HASHES.has(hostHash(cur))) return;
      const dot = cur.indexOf('.');
      if (dot < 0 || dot + 1 >= cur.length) break;
      cur = cur.slice(dot + 1);
    }}
  }}

  const COSMETIC = {cosmetic_flag};
  const CSS_TEXT = {css_literal};

  function ensureStyle() {{
    if (!COSMETIC) return;
    const root = document.head || document.documentElement;
    if (!root || document.getElementById('kurowser-shields-style')) return;
    const st = document.createElement('style');
    st.id = 'kurowser-shields-style';
    st.textContent = CSS_TEXT;
    root.appendChild(st);
  }}
  if (COSMETIC) {{
    ensureStyle();
    document.addEventListener('DOMContentLoaded', ensureStyle, {{ once: true }});
  }}

  const host = (location.hostname || '').toLowerCase();
  const isYt = host === 'youtube.com' || host.endsWith('.youtube.com') || host === 'youtu.be' || host === 'youtube-nocookie.com' || host.endsWith('.youtube-nocookie.com');
  if (!isYt) return;

  const AD_KEYS = ['adPlacements', 'playerAds', 'adSlots', 'adBreakHeartbeatParams'];
  function stripYtAds(obj, depth) {{
    if (!obj || typeof obj !== 'object' || depth > 8) return obj;
    if (Array.isArray(obj)) {{
      for (let i = obj.length - 1; i >= 0; i--) {{
        const item = obj[i];
        if (item && typeof item === 'object' && (item.adSlotRenderer || item.promotedSparklesWebRenderer || item.displayAdRenderer || item.statementBannerRenderer || item.bannerPromoRenderer || (item.richItemRenderer && item.richItemRenderer.content && item.richItemRenderer.content.adSlotRenderer))) {{
          try {{ obj.splice(i, 1); }} catch {{}}
        }} else {{
          stripYtAds(item, depth + 1);
        }}
      }}
      return obj;
    }}
    for (const k of AD_KEYS) {{
      if (k in obj) {{
        try {{
          if (Array.isArray(obj[k])) obj[k] = [];
          else delete obj[k];
        }} catch {{}}
      }}
    }}
    for (const k of Object.keys(obj)) {{
      if (k === 'adPlacements' || k === 'playerAds' || k === 'adSlots' || k === 'adBreakHeartbeatParams') continue;
      const v = obj[k];
      if (v && typeof v === 'object') stripYtAds(v, depth + 1);
    }}
    return obj;
  }}

  for (const prop of ['ytInitialPlayerResponse', 'ytInitialData']) {{
    try {{
      let stored = window[prop];
      if (stored && typeof stored === 'object') stripYtAds(stored, 0);
      const desc = Object.getOwnPropertyDescriptor(window, prop);
      if (!desc || desc.configurable) {{
        Object.defineProperty(window, prop, {{
          configurable: true,
          enumerable: true,
          get() {{ return stored; }},
          set(val) {{ stored = (val && typeof val === 'object') ? stripYtAds(val, 0) : val; }}
        }});
      }}
    }} catch {{}}
  }}

  if (typeof window.fetch === 'function') {{
    const origFetch = window.fetch;
    window.fetch = async function(input, init) {{
      const url = typeof input === 'string' ? input : (input && input.url) || '';
      const res = await origFetch.apply(this, arguments);
      if (url && url.includes('/youtubei/v1/') && res && res.ok) {{
        try {{
          const clone = res.clone();
          const txt = await clone.text();
          if (txt && (txt.includes('adPlacements') || txt.includes('playerAds') || txt.includes('adSlots') || txt.includes('adSlotRenderer') || txt.includes('adBreakHeartbeatParams'))) {{
            const hdrs = new Headers(res.headers);
            hdrs.delete('content-length');
            hdrs.delete('content-encoding');
            const parsed = stripYtAds(JSON.parse(txt), 0);
            return new Response(JSON.stringify(parsed), {{
              status: res.status,
              statusText: res.statusText,
              headers: hdrs
            }});
          }}
        }} catch {{}}
      }}
      return res;
    }};
  }}

  let savedMuted = null;
  function checkSkipYtAd() {{
    if (COSMETIC) ensureStyle();
    const player = document.querySelector('.html5-video-player');
    if (!player) return;
    const isAd = player.classList.contains('ad-showing') || player.classList.contains('ad-interrupting');
    const video = player.querySelector('video');
    if (isAd) {{
      const skips = player.querySelectorAll('.ytp-skip-ad-button, .ytp-ad-skip-button, .ytp-ad-skip-button-modern, .ytp-ad-skip-button-slot button');
      for (const b of skips) {{ try {{ b.click(); }} catch {{}} }}
      if (video) {{
        if (savedMuted === null) savedMuted = video.muted;
        video.muted = true;
        if (Number.isFinite(video.duration) && video.duration > 0 && video.duration < 1800) {{
          const target = Math.max(0, video.duration - 0.05);
          if (video.currentTime < target) video.currentTime = target;
        }}
      }}
    }} else if (savedMuted !== null && video) {{
      video.muted = savedMuted;
      savedMuted = null;
    }}
  }}

  const startObserver = () => {{
    ensureStyle();
    checkSkipYtAd();
    const root = document.documentElement;
    if (!root) return;
    const obs = new MutationObserver(() => checkSkipYtAd());
    obs.observe(root, {{ childList: true, subtree: true, attributes: true, attributeFilter: ['class'] }});
    setInterval(checkSkipYtAd, 650);
  }};
  if (document.documentElement) startObserver();
  else window.addEventListener('DOMContentLoaded', startObserver, {{ once: true }});
}})();"#,
        allow_json = allow_json,
        s1 = s1,
        s2 = s2,
        cosmetic_flag = cosmetic_flag,
        css_literal = css_literal,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShieldPolicy {
    pub global_enabled: bool,
    pub cosmetic_enabled: bool,
    pub allowlist: Vec<String>,
}

impl ShieldPolicy {
    pub fn is_active_for_site(&self, page_host: &str) -> bool {
        if !self.global_enabled {
            return false;
        }
        let clean = normalize_host(page_host);
        if clean.is_empty() || clean.starts_with("kurowser") {
            return false;
        }
        !is_host_in_list(&clean, &self.allowlist)
    }
}

pub fn normalize_host(raw: &str) -> String {
    let clean = raw
        .trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(['/', '?', '#', ':'])
        .next()
        .unwrap_or("")
        .trim_start_matches("www.")
        .to_ascii_lowercase();
    if clean.is_empty()
        || clean.len() > 253
        || clean.contains("..")
        || !clean
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
    {
        return String::new();
    }
    clean
}

fn is_host_in_list(host: &str, allowlist: &[String]) -> bool {
    for allowed in allowlist {
        let a = allowed.as_str();
        if host == a || host.ends_with(&format!(".{}", a)) {
            return true;
        }
    }
    false
}

fn is_youtube_host(host: &str) -> bool {
    host == "youtube.com"
        || host.ends_with(".youtube.com")
        || host == "youtube-nocookie.com"
        || host.ends_with(".youtube-nocookie.com")
        || host == "youtu.be"
}

/// Returns `true` if `request_url` should be blocked as an ad, tracker, popup, or cryptominer
/// given the current top-level `page_url` and whether `is_top_document` navigation.
pub fn should_block_request(
    request_url: &str,
    page_url: &str,
    is_top_document: bool,
    policy: &ShieldPolicy,
) -> bool {
    if !policy.global_enabled {
        return false;
    }

    let req_lower = request_url.trim();
    if !(req_lower.starts_with("https://") || req_lower.starts_with("http://")) {
        return false;
    }

    let page_host = extract_host(page_url);
    if !page_host.is_empty() && is_host_in_list(&page_host, &policy.allowlist) {
        return false;
    }

    let req_host = extract_host(req_lower);
    if req_host.is_empty() {
        return false;
    }

    // If top-level navigation target itself is explicitly allowlisted, allow
    if is_top_document && is_host_in_list(&req_host, &policy.allowlist) {
        return false;
    }

    let eng = rules();

    // Check exact host and every parent domain label (e.g. a.b.doubleclick.net -> b.doubleclick.net -> doubleclick.net)
    if matches_domain_or_parent(&req_host, &eng.blocked_domains) {
        return true;
    }

    // Never block a main document page based purely on URL substring tokens (avoid false positives on articles about ads)
    if is_top_document {
        return false;
    }

    let path_part = req_lower
        .split_once("://")
        .and_then(|(_, rest)| rest.split_once('/'))
        .map(|(_, p)| p)
        .unwrap_or("");
    if !path_part.is_empty() {
        let path_no_frag = path_part.split('#').next().unwrap_or(path_part);
        let path_only = format!(
            "/{}",
            path_no_frag
                .split('?')
                .next()
                .unwrap_or(path_no_frag)
                .to_ascii_lowercase()
        );

        // Explicit ad / tracking endpoints that also fire first-party (e.g. youtube.com/pagead/*, /api/stats/ads, /ptracking)
        for token in eng.first_party_ad_paths {
            if path_only.contains(token) {
                return true;
            }
        }
        if is_youtube_host(&req_host) && path_only.starts_with("/youtubei/v1/log_event") {
            return true;
        }

        let third_party = !page_host.is_empty() && !same_site(&page_host, &req_host);
        if third_party {
            for token in eng.third_party_ad_paths {
                if path_only.contains(token) {
                    return true;
                }
            }
        }
    }

    false
}

fn extract_host(url: &str) -> String {
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
    host.strip_prefix("www.").unwrap_or(host).to_ascii_lowercase()
}

fn matches_domain_or_parent(host: &str, set: &HashSet<&'static str>) -> bool {
    let mut current = host;
    loop {
        if set.contains(current) {
            return true;
        }
        match current.find('.') {
            Some(dot) if dot + 1 < current.len() => {
                current = &current[dot + 1..];
            }
            _ => break,
        }
    }
    false
}

fn same_site(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    a.ends_with(&format!(".{}", b)) || b.ends_with(&format!(".{}", a))
}

const FIRST_PARTY_AD_PATHS: &[&str] = &[
    "/pagead/",
    "/api/stats/ads",
    "/ptracking",
    "/get_midroll_info",
    "/pcs/activeview",
    "/ads/ga-audiences",
    "/adsbygoogle.js",
    "/gpt/pubads_impl",
    "/adservice/",
    "/bannerads/",
    "/adsense/",
];

const THIRD_PARTY_AD_PATHS: &[&str] = &[
    "/gtag/js",
    "/fbevents.js",
    "/gtm.js",
    "/prebid.js",
    "/prebid",
    "/adserver/",
    "/popunder",
    "/pagead2/",
    "/doubleclick/",
];

const BLOCKED_DOMAINS: &[&str] = &[
    // Google / YouTube Ad & Tracking Infrastructure (never block googlevideo.com video streams!)
    "doubleclick.net",
    "pubads.g.doubleclick.net",
    "pagead.l.doubleclick.net",
    "googleads.g.doubleclick.net",
    "googleads4.g.doubleclick.net",
    "static.doubleclick.net",
    "mediavisor.doubleclick.net",
    "ad.doubleclick.net",
    "2mdn.net",
    "s0.2mdn.net",
    "s1.2mdn.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adservice.google.com",
    "adservice.google.co.id",
    "pagead-googlehosted.l.google.com",
    "partnerad.l.google.com",
    "ads.youtube.com",
    "ad.youtube.com",
    "google-analytics.com",
    "ssl.google-analytics.com",
    "analytics.google.com",
    "adtrafficquality.google",
    "googletagmanager.com",
    "googletagservices.com",
    "pagead2.googlesyndication.com",
    "tpc.googlesyndication.com",
    "fundingchoicesmessages.google.com",
    "ade.googlesyndication.com",
    "adclick.g.doubleclick.net",
    "stats.g.doubleclick.net",
    "cm.g.doubleclick.net",
    "securepubads.g.doubleclick.net",
    "app-measurement.com",
    "firebaselogging-pa.googleapis.com",
    "crashlyticsreports-pa.googleapis.com",
    // Meta / Facebook Trackers & Ads
    "an.facebook.com",
    "pixel.facebook.com",
    // Amazon Ads
    "amazon-adsystem.com",
    "aax.amazon-adsystem.com",
    "c.amazon-adsystem.com",
    // Microsoft / Bing / LinkedIn Ads
    "bat.bing.com",
    "c.bing.com",
    "ads.linkedin.com",
    "px.ads.linkedin.com",
    "clarity.ms",
    // X / TikTok / Reddit / Pinterest / Snap Ads
    "ads-twitter.com",
    "static.ads-twitter.com",
    "ads-api.twitter.com",
    "analytics.twitter.com",
    "ads.tiktok.com",
    "analytics.tiktok.com",
    "business-api.tiktok.com",
    "events.reddit.com",
    "events.redditmedia.com",
    "ads.pinterest.com",
    "ct.pinterest.com",
    "tr.snapchat.com",
    "sc-static.net",
    // Major Programmatic Ad Networks & SSPs/DSPs
    "adnxs.com",
    "ib.adnxs.com",
    "criteo.com",
    "criteo.net",
    "gum.criteo.com",
    "static.criteo.net",
    "taboola.com",
    "cdn.taboola.com",
    "trc.taboola.com",
    "outbrain.com",
    "widgets.outbrain.com",
    "log.outbrainimg.com",
    "pubmatic.com",
    "ads.pubmatic.com",
    "hbopenbid.pubmatic.com",
    "rubiconproject.com",
    "fastlane.rubiconproject.com",
    "pixel.rubiconproject.com",
    "openx.net",
    "rtb.openx.net",
    "casalemedia.com",
    "htlb.casalemedia.com",
    "adsrvr.org",
    "match.adsrvr.org",
    "js.adsrvr.org",
    "indexww.com",
    "js-sec.indexww.com",
    "quantserve.com",
    "pixel.quantserve.com",
    "scorecardresearch.com",
    "sb.scorecardresearch.com",
    "moatads.com",
    "z.moatads.com",
    "media.net",
    "contextual.media.net",
    "revcontent.com",
    "trends.revcontent.com",
    "mgid.com",
    "jsc.mgid.com",
    "servicer.mgid.com",
    "bidswitch.net",
    "x.bidswitch.net",
    "smartadserver.com",
    "yieldmo.com",
    "triplelift.com",
    "eb2.3lift.com",
    "sharethrough.com",
    "teads.tv",
    "spotxchange.com",
    "innovid.com",
    "adform.net",
    "track.adform.net",
    "adroll.com",
    "d.adroll.com",
    "advertising.com",
    "adtechus.com",
    "adtech.de",
    "serving-sys.com",
    "sizmek.com",
    "flashtalking.com",
    "zemanta.com",
    "nativo.com",
    "kargo.com",
    "33across.com",
    "sonobi.com",
    "sovrn.com",
    "lijit.com",
    "rhythmone.com",
    "undertone.com",
    "connatix.com",
    "aniview.com",
    "primis.tech",
    "gumgum.com",
    "seedtag.com",
    "imrworldwide.com",
    "bluekai.com",
    "krxd.net",
    "demdex.net",
    "dpm.demdex.net",
    "omtrdc.net",
    "everesttech.net",
    "rlcdn.com",
    "crwdcntrl.net",
    "exelator.com",
    "eyeota.net",
    "agkn.com",
    "tapad.com",
    "turn.com",
    "mathtag.com",
    "mediamath.com",
    "rfihub.com",
    "contextweb.com",
    "liveintent.com",
    "id5-sync.com",
    "adswizz.com",
    // Popup / Popunder / Aggressive Ad Networks
    "propellerads.com",
    "propu.sh",
    "popads.net",
    "popcash.net",
    "adsterra.com",
    "highperformanceformat.com",
    "profitablegatecpm.com",
    "exoclick.com",
    "realsrv.com",
    "trafficjunky.net",
    "juicyads.com",
    "hilltopads.net",
    "onclickalgo.com",
    "acscdn.com",
    "adtng.com",
    "admaven.com",
    "bidvertiser.com",
    "infolinks.com",
    "chitika.net",
    "adcash.com",
    "zeroredirect1.com",
    "alexametrics.com",
    // Mobile / Web Attribution & Analytics / Session Replay / Telemetry
    "hotjar.com",
    "static.hotjar.com",
    "script.hotjar.com",
    "fullstory.com",
    "rs.fullstory.com",
    "mixpanel.com",
    "api-js.mixpanel.com",
    "amplitude.com",
    "api.amplitude.com",
    "cdn.amplitude.com",
    "segment.io",
    "api.segment.io",
    "cdn.segment.com",
    "heap.io",
    "heapanalytics.com",
    "newrelic.com",
    "js-agent.newrelic.com",
    "bam.nr-data.net",
    "bugsnag.com",
    "chartbeat.com",
    "static.chartbeat.com",
    "optimizely.com",
    "cdn.optimizely.com",
    "crazyegg.com",
    "script.crazyegg.com",
    "mouseflow.com",
    "luckyorange.com",
    "appsflyer.com",
    "adjust.com",
    "app.adjust.com",
    "branch.io",
    "api2.branch.io",
    "kochava.com",
    "singular.net",
    "onesignal.com",
    "intercom.io",
    "mc.yandex.ru",
    "yandexadexchange.net",
    "ad.mail.ru",
    "top-fwz1.mail.ru",
    "hm.baidu.com",
    "pos.baidu.com",
    // Ad networks common in SEA/Indonesia media sites
    "innity.com",
    "advenueplatform.com",
    "avp.innity.com",
    "grid.co.id",
    "dable.io",
    "api.dable.io",
    "popin.cc",
    "api.popin.cc",
    // Audio / Streaming ad & telemetry beacons
    "adeventtracker.spotify.com",
    "adstudio-assets.scdn.co",
    "analytics.spotify.com",
    "pixel.spotify.com",
    // Cryptominers
    "coinhive.com",
    "coin-hive.com",
    "jsecoin.com",
    "cryptoloot.pro",
    "webminepool.com",
    "minero.cc",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> ShieldPolicy {
        ShieldPolicy {
            global_enabled: true,
            cosmetic_enabled: true,
            allowlist: Vec::new(),
        }
    }

    #[test]
    fn blocks_known_ad_domains_and_subdomains() {
        let p = default_policy();
        assert!(should_block_request(
            "https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js",
            "https://example.com/",
            false,
            &p
        ));
        assert!(should_block_request(
            "https://sub.ad.doubleclick.net/pixel",
            "https://news.example.com/",
            false,
            &p
        ));
        assert!(should_block_request(
            "https://www.google-analytics.com/analytics.js",
            "https://github.com/",
            false,
            &p
        ));
        assert!(should_block_request(
            "https://s0.2mdn.net/ads/richmedia/studio/pv2/creative.js",
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
            false,
            &p
        ));
    }

    #[test]
    fn blocks_youtube_first_party_ad_endpoints_without_breaking_video() {
        let p = default_policy();
        let page = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

        assert!(should_block_request(
            "https://www.youtube.com/pagead/viewthroughconversion/123/",
            page,
            false,
            &p
        ));
        assert!(should_block_request(
            "https://www.youtube.com/api/stats/ads?ns=yt&el=detailpage",
            page,
            false,
            &p
        ));
        assert!(should_block_request(
            "https://www.youtube.com/ptracking?html5=1&video_id=abc",
            page,
            false,
            &p
        ));
        assert!(should_block_request(
            "https://www.youtube.com/get_midroll_info?ei=xyz",
            page,
            false,
            &p
        ));
        assert!(should_block_request(
            "https://www.youtube.com/youtubei/v1/log_event?alt=json",
            page,
            false,
            &p
        ));

        // Main watch page, player API, and videoplayback streams MUST stay allowed
        assert!(!should_block_request(page, page, true, &p));
        assert!(!should_block_request(
            "https://www.youtube.com/youtubei/v1/player?prettyPrint=false",
            page,
            false,
            &p
        ));
        assert!(!should_block_request(
            "https://rr3---sn-npoeenez.googlevideo.com/videoplayback?expire=1700000000&mime=video%2Fwebm",
            page,
            false,
            &p
        ));
        assert!(!should_block_request(
            "https://example.com/article?redirect=/pagead/info",
            "https://example.com/",
            false,
            &p
        ));
    }

    #[test]
    fn respects_global_off_and_per_site_allowlist() {
        let off = ShieldPolicy {
            global_enabled: false,
            cosmetic_enabled: false,
            allowlist: Vec::new(),
        };
        assert!(!should_block_request(
            "https://doubleclick.net/ad.js",
            "https://example.com/",
            false,
            &off
        ));

        let allowlisted = ShieldPolicy {
            global_enabled: true,
            cosmetic_enabled: true,
            allowlist: vec!["example.com".to_string()],
        };
        assert!(!should_block_request(
            "https://doubleclick.net/ad.js",
            "https://sub.example.com/article",
            false,
            &allowlisted
        ));
    }

    #[test]
    fn does_not_block_normal_first_party_or_internal_pages() {
        let p = default_policy();
        assert!(!should_block_request(
            "https://github.com/assets/app.js",
            "https://github.com/rust-lang/rust",
            false,
            &p
        ));
        assert!(!should_block_request(
            "kurowser://start/fonts/Poppins-Regular.ttf",
            "kurowser://start",
            false,
            &p
        ));
        assert!(!should_block_request(
            "https://example.com/blog/how-adblock-works",
            "https://example.com/",
            true,
            &p
        ));
    }

    #[test]
    fn document_script_respects_policy_and_hides_allowlist_plaintext() {
        let off = ShieldPolicy {
            global_enabled: false,
            cosmetic_enabled: true,
            allowlist: vec!["secret-partner.internal".to_string()],
        };
        assert!(build_document_script(&off, 42).is_none());

        let on = ShieldPolicy {
            global_enabled: true,
            cosmetic_enabled: true,
            allowlist: vec!["secret-partner.internal".to_string()],
        };
        let js = build_document_script(&on, 0x1234_5678_9abc_def0).expect("script when global on");
        assert!(js.contains("ytInitialPlayerResponse"));
        assert!(js.contains("/youtubei/v1/"));
        assert!(js.contains("ytd-ad-slot-renderer"));
        assert!(!js.contains("secret-partner.internal"));
    }
}
