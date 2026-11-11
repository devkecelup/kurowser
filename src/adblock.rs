// ─── adblock.rs ─── Kurowser Shields: fast O(labels) ad, tracker & miner blocker ───
use std::collections::HashSet;
use std::sync::OnceLock;

struct RuleEngine {
    blocked_domains: HashSet<&'static str>,
    blocked_path_tokens: &'static [&'static str],
}

static ENGINE: OnceLock<RuleEngine> = OnceLock::new();

fn rules() -> &'static RuleEngine {
    ENGINE.get_or_init(|| {
        let mut set = HashSet::with_capacity(280);
        for d in BLOCKED_DOMAINS {
            set.insert(*d);
        }
        RuleEngine {
            blocked_domains: set,
            blocked_path_tokens: BLOCKED_PATH_TOKENS,
        }
    })
}

/// Cosmetic CSS hiding script injected on external HTTP(S) pages when Shields are ON
/// for the active site. Hides empty ad slots without touching main page content.
pub const COSMETIC_FILTER_JS: &str = r#"(function(){
  if (window.__kurowserShieldsCss) return;
  window.__kurowserShieldsCss = true;
  if (location.protocol !== 'http:' && location.protocol !== 'https:') return;
  const css = `
    ins.adsbygoogle,
    .adsbygoogle,
    [id^="google_ads_iframe"],
    [id^="div-gpt-ad"],
    iframe[src*="doubleclick.net"],
    iframe[src*="googlesyndication.com"],
    iframe[src*="adnxs.com"],
    iframe[src*="taboola.com"],
    iframe[src*="outbrain.com"],
    .taboola-container,
    .OUTBRAIN,
    [data-ad-slot],
    [data-google-query-id] {
      display: none !important;
      visibility: hidden !important;
      height: 0 !important;
      min-height: 0 !important;
    }
  `;
  const apply = () => {
    const root = document.head || document.documentElement;
    if (!root || document.getElementById('kurowser-shields-style')) return;
    const st = document.createElement('style');
    st.id = 'kurowser-shields-style';
    st.textContent = css;
    root.appendChild(st);
  };
  apply();
  if (!document.head) {
    document.addEventListener('DOMContentLoaded', apply, { once: true });
  }
})();"#;

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

    // Third-party script/pixel/iframe URL path & token matching
    let third_party = !page_host.is_empty() && !same_site(&page_host, &req_host);
    if third_party {
        let path_part = req_lower
            .split_once("://")
            .and_then(|(_, rest)| rest.split_once('/'))
            .map(|(_, p)| p)
            .unwrap_or("");
        if !path_part.is_empty() {
            let path_l = format!("/{}", path_part.to_ascii_lowercase());
            for token in eng.blocked_path_tokens {
                if path_l.contains(token) {
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

const BLOCKED_PATH_TOKENS: &[&str] = &[
    "/pagead/",
    "/adsbygoogle.js",
    "/gpt/pubads_impl",
    "/gtag/js",
    "/fbevents.js",
    "/gtm.js",
    "/ads/ga-audiences",
    "/prebid.js",
    "/adserver/",
    "/adservice/",
    "/bannerads/",
    "/popunder",
    "/adsense/",
];

const BLOCKED_DOMAINS: &[&str] = &[
    // Google / YouTube Ad & Tracking Infrastructure
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adservice.google.com",
    "adservice.google.co.id",
    "google-analytics.com",
    "ssl.google-analytics.com",
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
}
