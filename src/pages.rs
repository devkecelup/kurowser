// ─── pages.rs ─── Internal kurowser:// pages with Poppins font, inline SVGs & customizable Akses Cepat ───
use crate::storage::{BrowserData, SearchEngine, ThemePalette, ThemePreset};
use serde::Serialize;

const POPPINS_CSS: &str = r#"
@font-face {
  font-family: 'Poppins';
  font-style: normal;
  font-weight: 400;
  src: url('/fonts/Poppins-Regular.ttf') format('truetype');
}
@font-face {
  font-family: 'Poppins';
  font-style: normal;
  font-weight: 500;
  src: url('/fonts/Poppins-Medium.ttf') format('truetype');
}
@font-face {
  font-family: 'Poppins';
  font-style: normal;
  font-weight: 600;
  src: url('/fonts/Poppins-SemiBold.ttf') format('truetype');
}
"#;

const SVG_KUROWSER_SM: &str = r##"<svg width="26" height="26" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <defs>
    <linearGradient id="k_grad_sm" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#6366f1"/>
      <stop offset="50%" stop-color="#7c3aed"/>
      <stop offset="100%" stop-color="#9333ea"/>
    </linearGradient>
  </defs>
  <rect x="2.5" y="2.5" width="59" height="59" rx="15.5" fill="url(#k_grad_sm)"/>
  <rect x="3.5" y="3.5" width="57" height="57" rx="14.5" fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="1.4"/>
  <path d="M20.5 18v28M24 34L42.5 18M29.8 29L43.2 46" stroke="#ffffff" stroke-width="7.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
</svg>"##;

const SVG_KUROWSER_HERO: &str = r##"<svg width="46" height="46" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" style="filter:drop-shadow(0 10px 24px rgba(99,102,241,0.32))" aria-hidden="true">
  <defs>
    <linearGradient id="k_grad_hero" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#6366f1"/>
      <stop offset="50%" stop-color="#7c3aed"/>
      <stop offset="100%" stop-color="#9333ea"/>
    </linearGradient>
  </defs>
  <rect x="2.5" y="2.5" width="59" height="59" rx="15.5" fill="url(#k_grad_hero)"/>
  <rect x="3.5" y="3.5" width="57" height="57" rx="14.5" fill="none" stroke="rgba(255,255,255,0.22)" stroke-width="1.4"/>
  <path d="M20.5 18v28M24 34L42.5 18M29.8 29L43.2 46" stroke="#ffffff" stroke-width="7.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
</svg>"##;

const SVG_CLOCK: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><polyline points="12 7 12 12 15.5 14"/></svg>"#;
const SVG_STAR: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polygon points="12 2.5 15.09 8.76 22 9.77 17 14.64 18.18 21.52 12 18.27 5.82 21.52 7 14.64 2 9.77 8.91 8.76 12 2.5"/></svg>"#;
const SVG_DOWNLOAD: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v12"/><polyline points="7 11 12 16 17 11"/><path d="M5 20h14"/></svg>"#;
const SVG_SLIDERS: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="4" y1="7" x2="20" y2="7"/><circle cx="9" cy="7" r="2.5" fill="var(--bg-chrome)"/><line x1="4" y1="17" x2="20" y2="17"/><circle cx="15" cy="17" r="2.5" fill="var(--bg-chrome)"/></svg>"#;
const SVG_FOLDER: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6.5A2.5 2.5 0 0 1 5.5 4h4l2 2.5H18.5A2.5 2.5 0 0 1 21 9v8.5a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5Z"/></svg>"#;
const SVG_PLUS: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;
const SVG_RESET: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"/></svg>"#;

const SVG_SHIELD: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 2.5 4.5 5.8v5.7c0 4.9 3.2 9.4 7.5 10.9 4.3-1.5 7.5-6 7.5-10.9V5.8L12 2.5Z"/><path d="m9.2 12.2 2 2 3.8-4"/></svg>"#;

const SITE_ICON_HELPER_JS: &str = r##"
function extractDomain(rawUrl) {
  if (!rawUrl) return '';
  if (rawUrl.startsWith('kurowser://')) return 'kurowser.' + (rawUrl.slice(11).split(/[/?#]/)[0] || 'start');
  try {
    const u = new URL(/^https?:\/\//i.test(rawUrl) ? rawUrl : 'https://' + rawUrl);
    return u.hostname.replace(/^www\./i, '').toLowerCase();
  } catch {
    return rawUrl.replace(/^https?:\/\//i, '').replace(/^www\./i, '').split(/[/?#:]/)[0].toLowerCase();
  }
}

const BRAND_SVGS = {
  'kurowser': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#7c3aed"/><path d="M8 6.5v11M9.5 12.8L16.2 6.5M11.6 10.9L16.5 17.5" stroke="#fff" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>`,
  'google.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#fff"/><path d="M18.6 12.2c0-.5-.05-.95-.13-1.4H12v2.75h3.7c-.16.86-.65 1.6-1.38 2.08v1.72h2.23c1.3-1.2 2.05-2.97 2.05-5.15z" fill="#4285F4"/><path d="M12 19c1.86 0 3.42-.62 4.55-1.66l-2.23-1.72c-.62.42-1.4.66-2.32.66-1.79 0-3.3-1.2-3.84-2.82H5.86v1.78A6.89 6.89 0 0 0 12 19z" fill="#34A853"/><path d="M8.16 13.46A4.1 4.1 0 0 1 7.94 12c0-.5.08-1 .22-1.46V8.76H5.86A6.9 6.9 0 0 0 5.1 12c0 1.12.27 2.18.76 3.24l2.3-1.78z" fill="#FBBC05"/><path d="M12 7.72c1.01 0 1.92.35 2.63 1.03l1.97-1.97C15.42 5.68 13.86 5 12 5a6.89 6.89 0 0 0-6.14 3.76l2.3 1.78C8.7 8.92 10.21 7.72 12 7.72z" fill="#EA4335"/></svg>`,
  'youtube.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#FF0033"/><path d="M19.2 8.7c-.2-.7-.7-1.2-1.4-1.4C16.6 7 12 7 12 7s-4.6 0-5.8.3c-.7.2-1.2.7-1.4 1.4-.3 1.2-.3 3.3-.3 3.3s0 2.1.3 3.3c.2.7.7 1.2 1.4 1.4 1.2.3 5.8.3 5.8.3s4.6 0 5.8-.3c.7-.2 1.2-.7 1.4-1.4.3-1.2.3-3.3.3-3.3s0-2.1-.3-3.3z" fill="#fff" fill-opacity=".18"/><polygon points="10.2,9.2 15.4,12 10.2,14.8" fill="#fff"/></svg>`,
  'github.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#24292F"/><path fill="#fff" d="M12 4.8a7.4 7.4 0 0 0-2.34 14.42c.37.07.5-.16.5-.36v-1.26c-2.05.44-2.48-.98-2.48-.98-.34-.85-.82-1.08-.82-1.08-.67-.46.05-.45.05-.45.74.05 1.13.76 1.13.76.66 1.13 1.73.8 2.15.61.07-.48.26-.8.47-.99-1.64-.18-3.36-.82-3.36-3.64 0-.8.29-1.46.76-1.97-.08-.19-.33-.94.07-1.95 0 0 .62-.2 2.03.75a7 7 0 0 1 3.7 0c1.41-.95 2.03-.75 2.03-.75.4 1.01.15 1.76.07 1.95.47.51.76 1.17.76 1.97 0 2.83-1.72 3.46-3.37 3.64.27.23.5.68.5 1.37v2.03c0 .2.14.43.51.36A7.4 7.4 0 0 0 12 4.8z"/></svg>`,
  'spotify.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#1DB954"/><path d="M6.8 9.7c3.3-1 7.6-.8 10.6.8M7.6 12.6c2.8-.8 6.3-.6 8.8.7M8.5 15.3c2.2-.6 4.9-.5 6.9.6" stroke="#111" stroke-width="1.8" stroke-linecap="round" fill="none"/></svg>`,
  'mail.google.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#fff"/><path d="M5 8.5v7.8c0 .5.4.9.9.9h2.2V11.3L12 14.2l3.9-2.9v5.9h2.2c.5 0 .9-.4.9-.9V8.5l-7 5.2-7-5.2z" fill="#EA4335"/><path d="M5 7.7c0-.7.8-1.1 1.4-.7L12 11.2 17.6 7c.6-.4 1.4 0 1.4.7v.8l-7 5.2-7-5.2v-.8z" fill="#C5221F"/><path d="M5 8.5v7.8c0 .5.4.9.9.9h2.2v-5.9L5 8.5z" fill="#4285F4"/><path d="M19 8.5v7.8c0 .5-.4.9-.9.9h-2.2v-5.9L19 8.5z" fill="#34A853"/></svg>`,
  'reddit.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#FF4500"/><circle cx="12" cy="13.2" r="5" fill="#fff"/><circle cx="10.2" cy="13" r="0.9" fill="#FF4500"/><circle cx="13.8" cy="13" r="0.9" fill="#FF4500"/><path d="M10 15.2c1.2.8 2.8.8 4 0" stroke="#FF4500" stroke-width="1.1" stroke-linecap="round" fill="none"/><circle cx="16.5" cy="7.2" r="1.2" fill="#fff"/><path d="M12 8.2l1-2.5 2.5.8" stroke="#fff" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>`,
  'x.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#0F1419"/><path fill="#fff" d="M13.5 10.9 18 5.6h-1.1l-3.9 4.5-3.1-4.5H6.2l4.8 6.9-4.8 5.6h1.1l4.2-4.8 3.3 4.8h3.7l-5-7.2zm-1.5 1.7-.5-.7L7.7 6.5h1.6l3.1 4.4.5.7 4 5.7h-1.6l-3.3-4.7z"/></svg>`,
  'whatsapp.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#25D366"/><path fill="#fff" d="M12 5.4a6.5 6.5 0 0 0-5.6 9.8l-.8 3.1 3.2-.8A6.5 6.5 0 1 0 12 5.4zm3.4 9.1c-.2.4-.9.8-1.3.8-.4.1-.8 0-1.4-.2-1-.3-2.3-1-3.2-2-.9-.9-1.4-2-1.5-2.6-.1-.5.1-.9.4-1.2.2-.2.5-.3.7-.3h.5c.2 0 .3.1.4.4l.6 1.4c.1.2 0 .4-.1.5l-.4.4c-.1.1-.1.3 0 .4.3.6.9 1.2 1.5 1.5.2.1.3.1.5 0l.5-.5c.1-.1.3-.2.5-.1l1.3.6c.2.1.3.2.3.4v.5z"/></svg>`,
  'figma.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#1E1E2E"/><path d="M9.5 11.8a2.3 2.3 0 1 1 0-4.6H12v4.6H9.5z" fill="#F24E1E"/><path d="M12 7.2h2.5a2.3 2.3 0 1 1 0 4.6H12V7.2z" fill="#FF7262"/><path d="M9.5 16.4a2.3 2.3 0 1 1 0-4.6H12v4.6H9.5z" fill="#A259FF"/><path d="M9.5 21a2.3 2.3 0 0 1 0-4.6H12v2.3A2.3 2.3 0 0 1 9.5 21z" fill="#0ACF83" transform="translate(0 -1.4) scale(.92 1)"/><circle cx="14.3" cy="14.1" r="2.2" fill="#1ABCFE"/></svg>`,
  'notion.so': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#F7F6F3"/><path fill="#191919" d="M6.4 7.4 14.5 6.8c.7-.1.9 0 1.3.3l1.7 1.2c.2.2.3.3.3.6v8.7c0 .5-.2.8-.7.9l-8.5.5c-.5 0-.8-.1-1-.5L6 16.4c-.2-.3-.3-.6-.3-1V8.3c0-.5.2-.8.7-.9zm1.7 2.2v6.6l1.7-.1v-4.2l3.1 4.2 1.9-.1V9.2l-1.7.1v4l-3-4-2 .2z"/></svg>`,
  'chatgpt.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#10A37F"/><circle cx="12" cy="12" r="5" fill="none" stroke="#fff" stroke-width="1.7" stroke-dasharray="4 2"/><circle cx="12" cy="12" r="2.1" fill="#fff"/></svg>`,
  'claude.ai': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#D97757"/><g stroke="#fff" stroke-width="1.8" stroke-linecap="round"><line x1="12" y1="5.8" x2="12" y2="18.2"/><line x1="5.8" y1="12" x2="18.2" y2="12"/><line x1="7.6" y1="7.6" x2="16.4" y2="16.4"/><line x1="16.4" y1="7.6" x2="7.6" y2="16.4"/></g></svg>`,
  'gemini.google.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#1A1F36"/><path fill="#60A5FA" d="M12 4.5c.6 4.2 3.3 6.9 7.5 7.5-4.2.6-6.9 3.3-7.5 7.5-.6-4.2-3.3-6.9-7.5-7.5 4.2-.6 6.9-3.3 7.5-7.5z"/></svg>`,
  'discord.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#5865F2"/><path fill="#fff" d="M16.9 8.1a10.4 10.4 0 0 0-2.6-.8l-.3.6a9.6 9.6 0 0 0-4 0l-.3-.6a10.4 10.4 0 0 0-2.6.8C5.5 10.6 5 13 5.2 15.3c1.1.8 2.2 1.3 3.3 1.6l.6-.9c-.4-.2-.8-.4-1.1-.6l.3-.2c2.2 1 4.6 1 6.8 0l.3.2c-.3.2-.7.4-1.1.6l.6.9c1.1-.3 2.2-.8 3.3-1.6.3-2.6-.4-5-1.3-7.2zM9.8 14c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.2 1.3s-.5 1.3-1.2 1.3zm4.4 0c-.6 0-1.1-.6-1.1-1.3 0-.7.5-1.3 1.1-1.3.7 0 1.2.6 1.2 1.3s-.5 1.3-1.2 1.3z"/></svg>`,
  'instagram.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><defs><linearGradient id="ig_g" x1="0%" y1="100%" x2="100%" y2="0%"><stop offset="0%" stop-color="#F58529"/><stop offset="50%" stop-color="#DD2A7B"/><stop offset="100%" stop-color="#515BD4"/></linearGradient></defs><rect width="24" height="24" rx="6" fill="url(#ig_g)"/><rect x="6" y="6" width="12" height="12" rx="3.4" fill="none" stroke="#fff" stroke-width="1.6"/><circle cx="12" cy="12" r="2.8" fill="none" stroke="#fff" stroke-width="1.6"/><circle cx="15.7" cy="8.3" r="0.9" fill="#fff"/></svg>`,
  'tiktok.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#0E0E12"/><path d="M13.8 6.2v7.9a2.7 2.7 0 1 1-1.9-2.6v-2a4.6 4.6 0 1 0 3.7 4.5V9.5c.8.6 1.8.9 2.9.9V8.5c-1.4-.1-2.5-.9-2.9-2.3h-1.8z" fill="#25F4EE" transform="translate(-.5 -.4)"/><path d="M13.8 6.2v7.9a2.7 2.7 0 1 1-1.9-2.6v-2a4.6 4.6 0 1 0 3.7 4.5V9.5c.8.6 1.8.9 2.9.9V8.5c-1.4-.1-2.5-.9-2.9-2.3h-1.8z" fill="#FE2C55" transform="translate(.5 .4)"/><path d="M13.8 6.2v7.9a2.7 2.7 0 1 1-1.9-2.6v-2a4.6 4.6 0 1 0 3.7 4.5V9.5c.8.6 1.8.9 2.9.9V8.5c-1.4-.1-2.5-.9-2.9-2.3h-1.8z" fill="#fff"/></svg>`,
  'netflix.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#141414"/><path fill="#E50914" d="M7.8 5.5h2.4l3.6 10V5.5h2.4v13h-2.4l-3.6-10v10H7.8v-13z"/></svg>`,
  'twitch.tv': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#9146FF"/><path fill="#fff" d="M7.2 5.6 6 8.3v9.2h3.1v1.8h1.8l1.8-1.8h2.6l3.3-3.3V5.6H7.2zm10 7.9-1.8 1.8h-2.9l-1.7 1.7v-1.7H8V7.2h9.2v6.3zm-2.3-4.1v3.2h-1.4V9.4h1.4zm-3.2 0v3.2h-1.4V9.4h1.4z"/></svg>`,
  'canva.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#00C4CC"/><circle cx="12" cy="12" r="5.5" fill="none" stroke="#fff" stroke-width="2.1" stroke-dasharray="25 10" stroke-linecap="round"/></svg>`,
  'wikipedia.org': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#F4F4F5"/><path d="M5.5 7.5L8.7 16.5L11.2 10L13.7 16.5L16.9 7.5M8 7.5h2.1M13.9 7.5H16" stroke="#18181B" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>`,
  'stackoverflow.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#232629"/><path d="M7.2 14v3.8h9.6V14" stroke="#BCBBBB" stroke-width="1.6" fill="none"/><path d="M8.8 15.6h6.4M9 13l6.2 1.2M9.8 10.2l5.7 2.4M11.3 7.6l4.6 3.7" stroke="#F48024" stroke-width="1.6" stroke-linecap="round"/></svg>`,
  'linkedin.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#0A66C2"/><rect x="6.8" y="10.2" width="2.2" height="7" fill="#fff"/><circle cx="7.9" cy="7.6" r="1.25" fill="#fff"/><path d="M11 10.2h2.1v1c.5-.8 1.3-1.2 2.3-1.2 1.7 0 2.6 1.1 2.6 3.1v4.1h-2.2v-3.7c0-1-.4-1.6-1.2-1.6-.9 0-1.4.6-1.4 1.6v3.7H11v-7z" fill="#fff"/></svg>`,
  'tokopedia.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#03AC0E"/><rect x="6.5" y="7.5" width="11" height="10.5" rx="3" fill="#fff"/><circle cx="9.8" cy="12" r="1.5" fill="#03AC0E"/><circle cx="14.2" cy="12" r="1.5" fill="#03AC0E"/><path d="M10 7.5a2 2 0 0 1 4 0" stroke="#fff" stroke-width="1.6" fill="none"/></svg>`,
  'shopee.co.id': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#EE4D2D"/><path d="M7 9h10l-.8 9H7.8L7 9z" fill="#fff"/><path d="M9.5 9a2.5 2.5 0 0 1 5 0" stroke="#fff" stroke-width="1.5" fill="none"/><path d="M13.5 12c-.4-.3-1.1-.4-1.7-.2-.7.2-.9.8-.3 1.2.6.4 1.6.5 1.8 1.2.2.7-.4 1.3-1.3 1.3-.7 0-1.3-.2-1.7-.5" stroke="#EE4D2D" stroke-width="1.3" stroke-linecap="round" fill="none"/></svg>`,
  'store.steampowered.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#171A21"/><circle cx="14.2" cy="9.4" r="3.1" fill="none" stroke="#66C0F4" stroke-width="1.6"/><circle cx="14.2" cy="9.4" r="1.3" fill="#fff"/><circle cx="8.6" cy="15.2" r="2.4" fill="none" stroke="#fff" stroke-width="1.6"/><line x1="9.9" y1="13.9" x2="12.5" y2="11.3" stroke="#66C0F4" stroke-width="2" stroke-linecap="round"/></svg>`,
  'roblox.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#1B1D24"/><rect x="7" y="7" width="10" height="10" rx="1.6" transform="rotate(14 12 12)" fill="#fff"/><rect x="10.4" y="10.4" width="3.2" height="3.2" rx="0.5" transform="rotate(14 12 12)" fill="#1B1D24"/></svg>`,
  'vercel.com': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#111"/><polygon points="12,6.5 18,17 6,17" fill="#fff"/></svg>`
};

function matchBrandKey(domain) {
  if (!domain) return '';
  if (domain.startsWith('kurowser.')) return 'kurowser';
  if (BRAND_SVGS[domain]) return domain;
  const aliases = [
    ['open.spotify.com', 'spotify.com'],
    ['twitter.com', 'x.com'],
    ['web.whatsapp.com', 'whatsapp.com'],
    ['chat.openai.com', 'chatgpt.com'],
    ['openai.com', 'chatgpt.com'],
    ['drive.google.com', 'google.com'],
    ['translate.google.com', 'google.com'],
    ['maps.google.com', 'google.com'],
    ['youtu.be', 'youtube.com'],
    ['id.wikipedia.org', 'wikipedia.org'],
    ['en.wikipedia.org', 'wikipedia.org'],
    ['shopee.com', 'shopee.co.id'],
  ];
  for (const [prefix, target] of aliases) {
    if (domain === prefix || domain.endsWith('.' + prefix)) return target;
  }
  for (const k of Object.keys(BRAND_SVGS)) {
    if (domain === k || domain.endsWith('.' + k)) return k;
  }
  return '';
}

function fallbackGlobeSvg(domain) {
  const palette = ['#6366f1','#0891b2','#059669','#d97706','#db2777','#2563eb','#7c3aed'];
  let h = 0;
  for (let i = 0; i < domain.length; i++) h = (h * 31 + domain.charCodeAt(i)) >>> 0;
  const col = palette[h % palette.length];
  return `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="${col}"/><circle cx="12" cy="12" r="6.2" fill="none" stroke="#fff" stroke-width="1.5"/><ellipse cx="12" cy="12" rx="3" ry="6.2" fill="none" stroke="#fff" stroke-width="1.3"/><line x1="5.8" y1="12" x2="18.2" y2="12" stroke="#fff" stroke-width="1.3"/></svg>`;
}

function createSiteIconEl(rawUrl, cachedSet) {
  const wrap = document.createElement('div');
  wrap.className = 'site-ico';
  const domain = extractDomain(rawUrl);
  const brand = matchBrandKey(domain);
  if (brand && BRAND_SVGS[brand]) {
    wrap.innerHTML = BRAND_SVGS[brand];
    return wrap;
  }
  if (cachedSet && domain && cachedSet.has(domain)) {
    const img = document.createElement('img');
    img.src = '/favicon/' + encodeURIComponent(domain);
    img.alt = '';
    img.onerror = () => { wrap.innerHTML = fallbackGlobeSvg(domain || 'web'); };
    wrap.appendChild(img);
    return wrap;
  }
  wrap.innerHTML = fallbackGlobeSvg(domain || 'web');
  return wrap;
}
"##;

fn safe_json_for_script<T: Serialize>(value: &T) -> String {
    let raw = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
    raw.replace("</", "<\\/")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn shared_css(p: &ThemePalette) -> String {
    format!(
        r#"{POPPINS_CSS}
*{{margin:0;padding:0;box-sizing:border-box}}
:root{{
  --bg-app:{bg_app};
  --bg-chrome:{bg_chrome};
  --bg-surface:{bg_surface};
  --bg-hover:{bg_hover};
  --bg-active:{bg_active};
  --border:{border};
  --text:{text};
  --muted:{muted};
  --accent:{accent};
  --accent-soft:{accent_soft};
}}
body{{
  background:var(--bg-app);
  color:var(--text);
  font-family:'Poppins','Segoe UI',sans-serif;
  min-height:100vh;
  -webkit-font-smoothing:antialiased;
}}
a{{color:inherit;text-decoration:none}}
button,input,select{{font-family:inherit}}
.topbar-nav{{
  display:flex;align-items:center;justify-content:space-between;
  padding:16px 32px;border-bottom:1px solid var(--border);background:var(--bg-chrome);
}}
.brand-sm{{display:flex;align-items:center;gap:10px;font-weight:600;font-size:16px}}
.brand-sm svg{{flex-shrink:0}}
.nav-pills{{display:flex;gap:8px;flex-wrap:wrap}}
.nav-pill{{
  padding:6px 14px;border-radius:999px;font-size:13px;font-weight:500;
  color:var(--muted);border:1px solid transparent;transition:all .15s;
}}
.nav-pill:hover{{background:var(--bg-hover);color:var(--text)}}
.nav-pill.active{{background:var(--accent-soft);color:var(--accent);border-color:var(--accent)}}
.wrap{{max-width:860px;margin:0 auto;padding:30px 24px 60px}}
.page-head{{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap;margin-bottom:20px}}
.page-head h1{{font-size:24px;font-weight:600}}
.btn{{
  display:inline-flex;align-items:center;gap:7px;
  padding:8px 15px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-chrome);color:var(--text);font-size:13px;font-weight:500;
  cursor:pointer;transition:all .15s;
}}
.btn:hover{{background:var(--bg-hover);border-color:var(--accent)}}
.btn-sm{{padding:5px 11px;font-size:12px;border-radius:8px;gap:5px}}
.btn-danger{{border-color:#7f1d1d;color:#fca5a5;background:#2a1216}}
.btn-danger:hover{{background:#3f181f;border-color:#f87171}}
.btn-accent{{background:var(--accent);color:#0f0f17;border-color:var(--accent);font-weight:600}}
.btn-accent:hover{{filter:brightness(1.08)}}
.search-input{{
  width:100%;padding:11px 16px;border-radius:12px;border:1px solid var(--border);
  background:var(--bg-surface);color:var(--text);font-size:14px;outline:none;margin-bottom:16px;
}}
.search-input:focus{{border-color:var(--accent)}}
.card{{
  background:var(--bg-chrome);border:1px solid var(--border);border-radius:14px;overflow:hidden;
}}
.row{{
  display:flex;align-items:center;justify-content:space-between;gap:14px;
  padding:12px 16px;border-bottom:1px solid var(--border);transition:background .12s;
}}
.row:last-child{{border-bottom:none}}
.row:hover{{background:var(--bg-hover)}}
.row-main{{display:flex;align-items:center;gap:12px;min-width:0;flex:1}}
.row-text{{min-width:0;flex:1}}
.row-title{{font-size:14px;font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.row-sub{{font-size:12px;color:var(--muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.badge{{
  display:inline-flex;align-items:center;gap:6px;
  font-size:11px;font-weight:500;padding:4px 10px;border-radius:999px;
  background:var(--accent-soft);color:var(--accent);white-space:nowrap;
}}
.icon-del{{
  display:inline-flex;align-items:center;justify-content:center;
  background:transparent;border:none;color:var(--muted);cursor:pointer;
  width:30px;height:30px;border-radius:8px;transition:all .12s;
}}
.icon-del:hover{{background:#451a24;color:#f87171}}
.empty{{padding:38px 20px;text-align:center;color:var(--muted);font-size:14px}}
.site-ico{{
  width:26px;height:26px;border-radius:7px;overflow:hidden;flex-shrink:0;
  display:inline-flex;align-items:center;justify-content:center;
}}
.site-ico img{{width:100%;height:100%;object-fit:cover;display:block;border-radius:7px}}
"#,
        bg_app = p.bg_app.hex(),
        bg_chrome = p.bg_chrome.hex(),
        bg_surface = p.bg_surface.hex(),
        bg_hover = p.bg_hover.hex(),
        bg_active = p.bg_active.hex(),
        border = p.border.hex(),
        text = p.text_primary.hex(),
        muted = p.text_muted.hex(),
        accent = p.accent.hex(),
        accent_soft = p.accent_soft.hex(),
    )
}

fn nav_header(active_slug: &str) -> String {
    let links = [
        ("start", "Home", "kurowser://start"),
        ("history", "History", "kurowser://history"),
        ("bookmarks", "Bookmarks", "kurowser://bookmarks"),
        ("downloads", "Downloads", "kurowser://downloads"),
        ("settings", "Tema & Settings", "kurowser://settings"),
    ];
    let mut pills = String::new();
    for (slug, label, href) in links {
        let cls = if slug == active_slug {
            "nav-pill active"
        } else {
            "nav-pill"
        };
        pills.push_str(&format!(r#"<a class="{cls}" href="{href}">{label}</a>"#));
    }
    format!(
        r#"<header class="topbar-nav">
  <a class="brand-sm" href="kurowser://start">
    {SVG_KUROWSER_SM}<span>Kurowser</span>
  </a>
  <nav class="nav-pills">{pills}</nav>
</header>"#
    )
}

pub fn render_start_page(data: &BrowserData, csp_nonce: &str) -> String {
    let p = data.palette();
    let css = shared_css(&p);
    let search_name = data.settings.search_engine.name();
    let is_google = data.settings.search_engine == SearchEngine::Google;

    #[derive(Serialize)]
    struct StartPayload<'a> {
        api_token: &'a str,
        search_name: &'a str,
        is_google: bool,
        quick_links: &'a [crate::storage::QuickLinkItem],
        cached_domains: Vec<String>,
        bookmarks: &'a [crate::storage::BookmarkItem],
        recent_history: Vec<&'a crate::storage::HistoryItem>,
    }

    let payload = StartPayload {
        api_token: &data.api_token,
        search_name,
        is_google,
        quick_links: &data.quick_links,
        cached_domains: data.cached_favicon_domains(),
        bookmarks: &data.bookmarks,
        recent_history: data.history.iter().take(6).collect(),
    };
    let json = safe_json_for_script(&payload);

    let tpl = r#"<!DOCTYPE html>
<html lang="id"><head><meta charset="utf-8"><title>Kurowser</title>
<style>
__CSS__
.hero{
  min-height:100vh;display:flex;flex-direction:column;align-items:center;
  justify-content:center;padding:34px 20px;
  background:radial-gradient(circle at 50% 16%, color-mix(in srgb, var(--accent) 14%, transparent), transparent 55%);
}
.logo-row{display:flex;align-items:center;gap:14px;margin-bottom:26px}
.logo-row h1{font-size:34px;font-weight:600;letter-spacing:-0.4px}
.omni-wrap{position:relative;width:min(600px,92vw);margin-bottom:28px}
.omni{
  width:100%;padding:14px 20px 14px 46px;border-radius:16px;
  border:1px solid var(--border);background:var(--bg-chrome);color:var(--text);
  font-size:15px;outline:none;box-shadow:0 10px 30px rgba(0,0,0,.28);
  transition:border-color .15s,box-shadow .15s;
}
.omni:focus{border-color:var(--accent);box-shadow:0 0 0 3px var(--accent-soft),0 14px 34px rgba(0,0,0,.35)}
.omni-icon{
  position:absolute;left:16px;top:50%;transform:translateY(-50%);
  width:18px;height:18px;stroke:var(--muted);fill:none;stroke-width:2;pointer-events:none;
}
.suggest{
  position:absolute;left:0;right:0;top:calc(100% + 6px);z-index:20;
  background:var(--bg-chrome);border:1px solid var(--border);border-radius:14px;
  overflow:hidden;box-shadow:0 18px 40px rgba(0,0,0,.5);display:none;
}
.suggest.open{display:block}
.s-item{
  display:flex;align-items:center;justify-content:space-between;gap:12px;
  padding:10px 15px;cursor:pointer;font-size:13px;border-bottom:1px solid var(--border);
}
.s-item:last-child{border-bottom:none}
.s-item:hover,.s-item.sel{background:var(--bg-hover)}
.s-left{display:flex;align-items:center;gap:10px;min-width:0;flex:1}
.s-left .site-ico{width:22px;height:22px;border-radius:6px}
.s-title{font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
.s-url{font-size:12px;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;max-width:230px}
.sec-head{
  display:flex;align-items:center;justify-content:space-between;gap:12px;
  width:min(736px,94vw);margin:8px 0 11px;
}
.section-lbl{
  font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:0.8px;
  color:var(--muted);
}
.sec-actions{display:flex;align-items:center;gap:6px}
.grid{
  display:grid;grid-template-columns:repeat(auto-fill,minmax(136px,1fr));
  gap:10px;width:min(736px,94vw);margin-bottom:24px;
}
.tile-card{position:relative}
.tile{
  display:flex;align-items:center;gap:10px;padding:11px 12px;border-radius:12px;
  background:var(--bg-chrome);border:1px solid var(--border);font-size:13px;font-weight:500;
  transition:transform .12s,background .12s,border-color .12s;min-height:48px;
}
.tile:hover{background:var(--bg-hover);border-color:var(--accent);transform:translateY(-1px)}
.tile-text{min-width:0;flex:1;display:flex;flex-direction:column}
.tile-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:1.25}
.tile-host{font-size:10.5px;color:var(--muted);font-weight:400;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.tile-actions{
  position:absolute;top:4px;right:4px;display:flex;gap:2px;
  opacity:0;pointer-events:none;transition:opacity .12s;
  background:var(--bg-chrome);border:1px solid var(--border);border-radius:8px;padding:2px;
}
.tile-card:hover .tile-actions,.tile-card:focus-within .tile-actions{opacity:1;pointer-events:auto}
.t-act{
  display:inline-flex;align-items:center;justify-content:center;
  width:21px;height:21px;border-radius:6px;border:none;background:transparent;
  color:var(--muted);cursor:pointer;transition:all .12s;
}
.t-act:hover{background:var(--accent-soft);color:var(--accent)}
.t-act.del:hover{background:#451a24;color:#f87171}
.tile-add{
  border-style:dashed;color:var(--muted);cursor:pointer;justify-content:center;gap:7px;width:100%;
}
.tile-add:hover{color:var(--accent)}
.footer-bar{display:flex;flex-wrap:wrap;justify-content:center;gap:10px;margin-top:6px}
.f-btn{
  display:inline-flex;align-items:center;gap:7px;
  padding:7px 14px;border-radius:999px;font-size:12px;font-weight:500;
  background:var(--bg-chrome);border:1px solid var(--border);color:var(--muted);transition:all .15s;
}
.f-btn:hover{border-color:var(--accent);color:var(--text);background:var(--bg-hover)}
.modal-backdrop{
  position:fixed;inset:0;background:rgba(6,6,14,.68);backdrop-filter:blur(4px);
  display:none;align-items:center;justify-content:center;padding:18px;z-index:50;
}
.modal-backdrop.open{display:flex}
.modal{
  width:min(420px,95vw);background:var(--bg-chrome);border:1px solid var(--border);
  border-radius:16px;padding:22px;box-shadow:0 24px 60px rgba(0,0,0,.6);
}
.modal-head{display:flex;align-items:center;justify-content:space-between;margin-bottom:16px}
.modal-head h2{font-size:17px;font-weight:600}
.field-lbl{display:block;font-size:12px;font-weight:500;color:var(--muted);margin-bottom:6px}
.url-row{display:flex;align-items:center;gap:10px;margin-bottom:14px}
.url-row .site-ico{width:34px;height:34px;border-radius:9px}
.m-input{
  width:100%;padding:10px 13px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-surface);color:var(--text);font-size:13.5px;outline:none;
}
.m-input:focus{border-color:var(--accent)}
.m-err{font-size:12px;color:#f87171;min-height:18px;margin:6px 0 10px}
.modal-foot{display:flex;align-items:center;justify-content:space-between;gap:10px;margin-top:10px}
.modal-foot-right{display:flex;gap:8px;margin-left:auto}
</style></head>
<body>
<main class="hero">
  <div class="logo-row">
    __SVG_HERO__
    <h1>Kurowser</h1>
  </div>

  <div class="omni-wrap">
    <svg class="omni-icon" viewBox="0 0 24 24"><circle cx="11" cy="11" r="7"/><line x1="16.5" y1="16.5" x2="21" y2="21"/></svg>
    <input id="q" class="omni" autocomplete="off" placeholder="Cari di __SEARCH_NAME__ atau ketik URL (mis. goo)..." autofocus>
    <div id="suggest" class="suggest"></div>
  </div>

  <div id="bm-section" style="display:none;width:100%;flex-direction:column;align-items:center">
    <div class="sec-head"><div class="section-lbl">Bookmarks Tersimpan</div></div>
    <div id="bm-grid" class="grid"></div>
  </div>

  <div class="sec-head">
    <div class="section-lbl">Akses Cepat</div>
    <div class="sec-actions">
      <button id="qa-add-btn" class="btn btn-sm" type="button">__SVG_PLUS__ Tambah</button>
      <button id="qa-reset-btn" class="btn btn-sm" type="button" title="Kembalikan Akses Cepat bawaan">__SVG_RESET__ Reset</button>
    </div>
  </div>
  <div id="qa-grid" class="grid"></div>

  <div class="footer-bar">
    <a class="f-btn" href="kurowser://history">__SVG_CLOCK__ History</a>
    <a class="f-btn" href="kurowser://bookmarks">__SVG_STAR__ Bookmarks</a>
    <a class="f-btn" href="kurowser://downloads">__SVG_DOWNLOAD__ Downloads</a>
    <a class="f-btn" href="kurowser://settings">__SVG_SLIDERS__ Tema & Settings</a>
  </div>
</main>

<div id="qa-modal" class="modal-backdrop" role="dialog" aria-modal="true">
  <div class="modal">
    <div class="modal-head">
      <h2 id="m-title">Tambah Akses Cepat</h2>
      <button id="m-close" class="icon-del" type="button" aria-label="Tutup">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
    <label class="field-lbl" for="m-url">Alamat Website (URL)</label>
    <div class="url-row">
      <div id="m-preview"></div>
      <input id="m-url" class="m-input" placeholder="youtube.com atau https://...">
    </div>
    <label class="field-lbl" for="m-name">Nama Tampilan</label>
    <input id="m-name" class="m-input" placeholder="Otomatis dari nama situs" maxlength="40">
    <div id="m-err" class="m-err"></div>
    <div class="modal-foot">
      <div id="m-order-btns" style="display:none;gap:6px">
        <button id="m-left" class="btn btn-sm" type="button" title="Geser kiri">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <button id="m-right" class="btn btn-sm" type="button" title="Geser kanan">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><polyline points="9 18 15 12 9 6"/></svg>
        </button>
      </div>
      <div class="modal-foot-right">
        <button id="m-cancel" class="btn" type="button">Batal</button>
        <button id="m-save" class="btn btn-accent" type="button">Simpan</button>
      </div>
    </div>
  </div>
</div>

<script nonce="__CSP_NONCE__">
const DATA = __JSON__;
const CACHED_SET = new Set(DATA.cached_domains || []);
__SITE_ICON_JS__

const SVG_EDIT = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.1" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>`;
const SVG_X = `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
const SVG_SEARCH_BADGE = `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="var(--accent-soft)"/><circle cx="11" cy="11" r="4.5" fill="none" stroke="var(--accent)" stroke-width="1.8"/><line x1="14.5" y1="14.5" x2="17.5" y2="17.5" stroke="var(--accent)" stroke-width="1.8" stroke-linecap="round"/></svg>`;

const SITES = [
  ["Google","google.com","https://www.google.com"],
  ["YouTube","youtube.com","https://www.youtube.com"],
  ["GitHub","github.com","https://github.com"],
  ["Gmail","gmail.com","https://mail.google.com"],
  ["Google Drive","drive.google.com","https://drive.google.com"],
  ["Reddit","reddit.com","https://www.reddit.com"],
  ["X","x.com","https://x.com"],
  ["Stack Overflow","stackoverflow.com","https://stackoverflow.com"],
  ["Spotify","open.spotify.com","https://open.spotify.com"],
  ["WhatsApp Web","web.whatsapp.com","https://web.whatsapp.com"],
  ["ChatGPT","chatgpt.com","https://chatgpt.com"],
  ["Claude","claude.ai","https://claude.ai"],
  ["Notion","notion.so","https://www.notion.so"],
  ["Figma","figma.com","https://www.figma.com"],
  ["Tokopedia","tokopedia.com","https://www.tokopedia.com"],
  ["Shopee","shopee.co.id","https://shopee.co.id"]
];

if (DATA.bookmarks && DATA.bookmarks.length > 0) {
  document.getElementById('bm-section').style.display = 'flex';
  const bmGrid = document.getElementById('bm-grid');
  DATA.bookmarks.slice(0, 10).forEach(b => {
    const card = document.createElement('div'); card.className = 'tile-card';
    const a = document.createElement('a');
    a.className = 'tile';
    a.href = b.url;
    const ico = createSiteIconEl(b.url, CACHED_SET);
    const txt = document.createElement('div'); txt.className = 'tile-text';
    const name = document.createElement('span'); name.className = 'tile-name'; name.textContent = b.title || b.domain;
    const host = document.createElement('span'); host.className = 'tile-host'; host.textContent = b.domain || extractDomain(b.url);
    txt.appendChild(name); txt.appendChild(host);
    a.appendChild(ico); a.appendChild(txt);
    card.appendChild(a);
    bmGrid.appendChild(card);
  });
}

let quickLinks = Array.isArray(DATA.quick_links) ? DATA.quick_links : [];
const qaGrid = document.getElementById('qa-grid');

async function persistQuickLinks(items) {
  const res = await fetch('/api/quick-links/save', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': DATA.api_token },
    body: JSON.stringify({ items })
  });
  if (res.ok) {
    const out = await res.json();
    if (Array.isArray(out.items)) quickLinks = out.items;
  }
  renderQuickLinks();
}

function renderQuickLinks() {
  qaGrid.innerHTML = '';
  quickLinks.forEach((item, idx) => {
    const card = document.createElement('div'); card.className = 'tile-card';
    const a = document.createElement('a'); a.className = 'tile'; a.href = item.url;
    const ico = createSiteIconEl(item.url, CACHED_SET);
    const txt = document.createElement('div'); txt.className = 'tile-text';
    const name = document.createElement('span'); name.className = 'tile-name'; name.textContent = item.title || extractDomain(item.url);
    const host = document.createElement('span'); host.className = 'tile-host'; host.textContent = extractDomain(item.url);
    txt.appendChild(name); txt.appendChild(host);
    a.appendChild(ico); a.appendChild(txt);

    const acts = document.createElement('div'); acts.className = 'tile-actions';
    const editBtn = document.createElement('button'); editBtn.className = 't-act'; editBtn.type = 'button'; editBtn.title = 'Edit shortcut'; editBtn.innerHTML = SVG_EDIT;
    editBtn.onclick = (e) => { e.preventDefault(); e.stopPropagation(); openModal(idx); };
    const delBtn = document.createElement('button'); delBtn.className = 't-act del'; delBtn.type = 'button'; delBtn.title = 'Hapus shortcut'; delBtn.innerHTML = SVG_X;
    delBtn.onclick = async (e) => {
      e.preventDefault(); e.stopPropagation();
      const next = quickLinks.filter((_, i) => i !== idx);
      await persistQuickLinks(next);
    };
    acts.appendChild(editBtn); acts.appendChild(delBtn);
    card.appendChild(a); card.appendChild(acts);
    qaGrid.appendChild(card);
  });

  if (quickLinks.length < 24) {
    const addBtn = document.createElement('button');
    addBtn.className = 'tile tile-add';
    addBtn.type = 'button';
    addBtn.innerHTML = `__SVG_PLUS__ <span>Tambah Situs</span>`;
    addBtn.onclick = () => openModal(-1);
    qaGrid.appendChild(addBtn);
  }
}

const modal = document.getElementById('qa-modal');
const mTitle = document.getElementById('m-title');
const mUrl = document.getElementById('m-url');
const mName = document.getElementById('m-name');
const mErr = document.getElementById('m-err');
const mPrev = document.getElementById('m-preview');
const mOrderBtns = document.getElementById('m-order-btns');
let editIndex = -1;
let customTitleTouched = false;

function updateModalPreview() {
  mPrev.innerHTML = '';
  mPrev.appendChild(createSiteIconEl(mUrl.value.trim(), CACHED_SET));
  if (!customTitleTouched) {
    const dom = extractDomain(mUrl.value.trim());
    if (dom) {
      const base = dom.split('.')[0];
      mName.value = base ? (base.charAt(0).toUpperCase() + base.slice(1)) : dom;
    }
  }
}

function openModal(idx) {
  editIndex = idx;
  mErr.textContent = '';
  if (idx >= 0 && quickLinks[idx]) {
    mTitle.textContent = 'Edit Akses Cepat';
    mUrl.value = quickLinks[idx].url;
    mName.value = quickLinks[idx].title;
    customTitleTouched = true;
    mOrderBtns.style.display = 'flex';
  } else {
    mTitle.textContent = 'Tambah Akses Cepat';
    mUrl.value = '';
    mName.value = '';
    customTitleTouched = false;
    mOrderBtns.style.display = 'none';
  }
  updateModalPreview();
  modal.classList.add('open');
  setTimeout(() => mUrl.focus(), 20);
}

function closeModal() {
  modal.classList.remove('open');
}

function normalizeClientUrl(raw) {
  const t = raw.trim();
  if (!t || /\s/.test(t)) return '';
  if (/^(javascript|data|vbscript|file|blob|about):/i.test(t)) return '';
  if (/^(https?:\/\/|kurowser:\/\/)/i.test(t)) return t;
  if (/^localhost(:\d+)?(\/.*)?$/i.test(t) || /^127\.0\.0\.1(:\d+)?(\/.*)?$/.test(t)) return 'http://' + t;
  if (t.includes('.')) return 'https://' + t;
  return '';
}

async function saveModal() {
  const url = normalizeClientUrl(mUrl.value);
  if (!url) {
    mErr.textContent = 'Masukkan URL valid, mis. youtube.com atau https://example.com';
    mUrl.focus();
    return;
  }
  const title = (mName.value.trim() || extractDomain(url) || url).slice(0, 40);
  const next = quickLinks.slice();
  if (editIndex >= 0 && editIndex < next.length) {
    next[editIndex] = { title, url };
  } else {
    next.push({ title, url });
  }
  closeModal();
  await persistQuickLinks(next);
}

document.getElementById('qa-add-btn').onclick = () => openModal(-1);
document.getElementById('qa-reset-btn').onclick = async () => {
  const res = await fetch('/api/quick-links/reset', {
    method: 'POST',
    headers: { 'X-Kurowser-Token': DATA.api_token }
  });
  if (res.ok) {
    const out = await res.json();
    if (Array.isArray(out.items)) quickLinks = out.items;
    renderQuickLinks();
  }
};
mUrl.addEventListener('input', () => { mErr.textContent = ''; updateModalPreview(); });
mName.addEventListener('input', () => { customTitleTouched = mName.value.trim().length > 0; });
document.getElementById('m-save').onclick = saveModal;
document.getElementById('m-cancel').onclick = closeModal;
document.getElementById('m-close').onclick = closeModal;
document.getElementById('m-left').onclick = async () => {
  if (editIndex > 0) {
    const next = quickLinks.slice();
    [next[editIndex - 1], next[editIndex]] = [next[editIndex], next[editIndex - 1]];
    editIndex -= 1;
    await persistQuickLinks(next);
  }
};
document.getElementById('m-right').onclick = async () => {
  if (editIndex >= 0 && editIndex < quickLinks.length - 1) {
    const next = quickLinks.slice();
    [next[editIndex + 1], next[editIndex]] = [next[editIndex], next[editIndex + 1]];
    editIndex += 1;
    await persistQuickLinks(next);
  }
};
modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
modal.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') closeModal();
  else if (e.key === 'Enter') saveModal();
});

renderQuickLinks();

const inp = document.getElementById('q');
const box = document.getElementById('suggest');
let selIdx = -1;
let items = [];

function goTo(input) {
  const v = input.trim();
  if (!v) return;
  if (/^https?:\/\//i.test(v)) { location.href = v; return; }
  if (/^kurowser:\/\//i.test(v)) { location.href = v; return; }
  if (!v.includes(' ') && v.includes('.')) { location.href = 'https://' + v; return; }
  const base = DATA.is_google ? 'https://www.google.com/search?q=' : 'https://duckduckgo.com/?q=';
  location.href = base + encodeURIComponent(v);
}

function renderSuggest(q) {
  const ql = q.trim().toLowerCase();
  if (!ql) { box.classList.remove('open'); items = []; selIdx = -1; return; }
  const list = [];
  const seen = new Set();
  const push = (t, u, isSearch = false) => {
    if (!seen.has(u)) { seen.add(u); list.push({t, u, isSearch}); }
  };
  (DATA.bookmarks || []).forEach(b => {
    if (b.domain.toLowerCase().includes(ql) || b.title.toLowerCase().includes(ql)) push(b.title, b.url);
  });
  (DATA.recent_history || []).forEach(h => {
    if (h.domain.toLowerCase().includes(ql) || h.title.toLowerCase().includes(ql)) push(h.title, h.url);
  });
  SITES.forEach(([n, d, u]) => {
    if (d.startsWith(ql) || n.toLowerCase().startsWith(ql)) push(d + ' — ' + n, u);
  });
  const finalItems = list.slice(0, 4);
  const searchUrl = (DATA.is_google ? 'https://www.google.com/search?q=' : 'https://duckduckgo.com/?q=') + encodeURIComponent(q.trim());
  finalItems.push({ t: 'Cari "' + q.trim() + '" di ' + DATA.search_name, u: searchUrl, isSearch: true });
  items = finalItems;
  selIdx = -1;
  box.innerHTML = '';
  items.forEach((it) => {
    const div = document.createElement('div');
    div.className = 's-item';
    const left = document.createElement('div'); left.className = 's-left';
    let ico;
    if (it.isSearch) {
      ico = document.createElement('div'); ico.className = 'site-ico'; ico.innerHTML = SVG_SEARCH_BADGE;
    } else {
      ico = createSiteIconEl(it.u, CACHED_SET);
    }
    const st = document.createElement('span'); st.className = 's-title'; st.textContent = it.t;
    left.appendChild(ico); left.appendChild(st);
    const su = document.createElement('span'); su.className = 's-url'; su.textContent = it.isSearch ? DATA.search_name : it.u;
    div.appendChild(left); div.appendChild(su);
    div.addEventListener('mousedown', e => { e.preventDefault(); location.href = it.u; });
    box.appendChild(div);
  });
  box.classList.add('open');
}

inp.addEventListener('input', () => renderSuggest(inp.value));
inp.addEventListener('keydown', e => {
  if (e.key === 'ArrowDown' && items.length) {
    e.preventDefault(); selIdx = (selIdx + 1) % items.length;
    [...box.children].forEach((c, i) => c.classList.toggle('sel', i === selIdx));
  } else if (e.key === 'ArrowUp' && items.length) {
    e.preventDefault(); selIdx = selIdx <= 0 ? items.length - 1 : selIdx - 1;
    [...box.children].forEach((c, i) => c.classList.toggle('sel', i === selIdx));
  } else if (e.key === 'Escape') {
    box.classList.remove('open'); selIdx = -1;
  } else if (e.key === 'Enter') {
    if (selIdx >= 0 && items[selIdx]) location.href = items[selIdx].u;
    else goTo(inp.value);
  }
});
document.addEventListener('click', e => { if (!e.target.closest('.omni-wrap')) box.classList.remove('open'); });
</script>
</body></html>"#;

    tpl.replace("__CSS__", &css)
        .replace("__SVG_HERO__", SVG_KUROWSER_HERO)
        .replace("__SEARCH_NAME__", search_name)
        .replace("__SVG_PLUS__", SVG_PLUS)
        .replace("__SVG_RESET__", SVG_RESET)
        .replace("__SVG_CLOCK__", SVG_CLOCK)
        .replace("__SVG_STAR__", SVG_STAR)
        .replace("__SVG_DOWNLOAD__", SVG_DOWNLOAD)
        .replace("__SVG_SLIDERS__", SVG_SLIDERS)
        .replace("__CSP_NONCE__", csp_nonce)
        .replace("__JSON__", &json)
        .replace("__SITE_ICON_JS__", SITE_ICON_HELPER_JS)
}

pub fn render_history_page(data: &BrowserData, csp_nonce: &str) -> String {
    let p = data.palette();
    let css = shared_css(&p);
    let header = nav_header("history");
    let json = safe_json_for_script(&data.history);
    let cached_json = safe_json_for_script(&data.cached_favicon_domains());
    let token_json = safe_json_for_script(&data.api_token);

    let tpl = r#"<!DOCTYPE html>
<html lang="id"><head><meta charset="utf-8"><title>History — Kurowser</title>
<style>__CSS__</style></head>
<body>
__HEADER__
<div class="wrap">
  <div class="page-head">
    <h1>Riwayat Penjelajahan</h1>
    <button id="clear-btn" class="btn btn-danger">Hapus Semua History</button>
  </div>
  <input id="filter" class="search-input" placeholder="Cari judul atau alamat situs...">
  <div id="list" class="card"></div>
</div>
<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
const CACHED_SET = new Set(__CACHED_JSON__ || []);
__SITE_ICON_JS__
const SVG_X = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
let ITEMS = __JSON__;
const listEl = document.getElementById('list');
const filterEl = document.getElementById('filter');

function render() {
  const q = filterEl.value.trim().toLowerCase();
  const filtered = q ? ITEMS.filter(x => x.title.toLowerCase().includes(q) || x.url.toLowerCase().includes(q)) : ITEMS;
  listEl.innerHTML = '';
  if (!filtered.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = q ? 'Tidak ada riwayat yang cocok.' : 'Belum ada riwayat penjelajahan.';
    listEl.appendChild(empty);
    return;
  }
  filtered.slice(0, 200).forEach(item => {
    const row = document.createElement('div'); row.className = 'row';
    const main = document.createElement('a'); main.className = 'row-main'; main.href = item.url;
    main.appendChild(createSiteIconEl(item.url, CACHED_SET));
    const tx = document.createElement('div'); tx.className = 'row-text';
    const t = document.createElement('div'); t.className = 'row-title'; t.textContent = item.title || item.domain;
    const u = document.createElement('div'); u.className = 'row-sub'; u.textContent = item.url;
    tx.appendChild(t); tx.appendChild(u);
    main.appendChild(tx);

    const badge = document.createElement('span'); badge.className = 'badge';
    badge.textContent = item.visit_count + 'x';

    const del = document.createElement('button'); del.className = 'icon-del'; del.innerHTML = SVG_X;
    del.title = 'Hapus item';
    del.onclick = async () => {
      await fetch('/api/history/delete', { method: 'POST', headers: {'Content-Type':'application/json','X-Kurowser-Token':API_TOKEN}, body: JSON.stringify({url: item.url}) });
      ITEMS = ITEMS.filter(x => x.url !== item.url);
      render();
    };
    row.appendChild(main); row.appendChild(badge); row.appendChild(del);
    listEl.appendChild(row);
  });
}

document.getElementById('clear-btn').onclick = async () => {
  await fetch('/api/history/clear', { method: 'POST', headers: {'X-Kurowser-Token':API_TOKEN} });
  ITEMS = [];
  render();
};
filterEl.addEventListener('input', render);
render();
</script></body></html>"#;

    tpl.replace("__CSS__", &css)
        .replace("__HEADER__", &header)
        .replace("__CSP_NONCE__", csp_nonce)
        .replace("__TOKEN_JSON__", &token_json)
        .replace("__CACHED_JSON__", &cached_json)
        .replace("__SITE_ICON_JS__", SITE_ICON_HELPER_JS)
        .replace("__JSON__", &json)
}

pub fn render_bookmarks_page(data: &BrowserData, csp_nonce: &str) -> String {
    let p = data.palette();
    let css = shared_css(&p);
    let header = nav_header("bookmarks");
    let json = safe_json_for_script(&data.bookmarks);
    let cached_json = safe_json_for_script(&data.cached_favicon_domains());
    let token_json = safe_json_for_script(&data.api_token);

    let tpl = r#"<!DOCTYPE html>
<html lang="id"><head><meta charset="utf-8"><title>Bookmarks — Kurowser</title>
<style>__CSS__</style></head>
<body>
__HEADER__
<div class="wrap">
  <div class="page-head">
    <h1>Bookmarks Tersimpan</h1>
    <span class="badge">__SVG_STAR__ Klik bintang di address bar atau Ctrl+D</span>
  </div>
  <input id="filter" class="search-input" placeholder="Cari bookmark...">
  <div id="list" class="card"></div>
</div>
<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
const CACHED_SET = new Set(__CACHED_JSON__ || []);
__SITE_ICON_JS__
const SVG_X = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
let ITEMS = __JSON__;
const listEl = document.getElementById('list');
const filterEl = document.getElementById('filter');

function render() {
  const q = filterEl.value.trim().toLowerCase();
  const filtered = q ? ITEMS.filter(x => x.title.toLowerCase().includes(q) || x.url.toLowerCase().includes(q)) : ITEMS;
  listEl.innerHTML = '';
  if (!filtered.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = 'Belum ada bookmark. Buka website favoritmu lalu klik ikon bintang di kanan address bar.';
    listEl.appendChild(empty);
    return;
  }
  filtered.forEach(item => {
    const row = document.createElement('div'); row.className = 'row';
    const main = document.createElement('a'); main.className = 'row-main'; main.href = item.url;
    main.appendChild(createSiteIconEl(item.url, CACHED_SET));
    const tx = document.createElement('div'); tx.className = 'row-text';
    const t = document.createElement('div'); t.className = 'row-title'; t.textContent = item.title || item.domain;
    const u = document.createElement('div'); u.className = 'row-sub'; u.textContent = item.url;
    tx.appendChild(t); tx.appendChild(u);
    main.appendChild(tx);

    const del = document.createElement('button'); del.className = 'icon-del'; del.innerHTML = SVG_X;
    del.onclick = async () => {
      await fetch('/api/bookmarks/delete', { method: 'POST', headers: {'Content-Type':'application/json','X-Kurowser-Token':API_TOKEN}, body: JSON.stringify({url: item.url}) });
      ITEMS = ITEMS.filter(x => x.url !== item.url);
      render();
    };
    row.appendChild(main); row.appendChild(del);
    listEl.appendChild(row);
  });
}
filterEl.addEventListener('input', render);
render();
</script></body></html>"#;

    tpl.replace("__CSS__", &css)
        .replace("__HEADER__", &header)
        .replace("__SVG_STAR__", SVG_STAR)
        .replace("__CSP_NONCE__", csp_nonce)
        .replace("__TOKEN_JSON__", &token_json)
        .replace("__CACHED_JSON__", &cached_json)
        .replace("__SITE_ICON_JS__", SITE_ICON_HELPER_JS)
        .replace("__JSON__", &json)
}

pub fn render_downloads_page(data: &BrowserData, csp_nonce: &str) -> String {
    let p = data.palette();
    let css = shared_css(&p);
    let header = nav_header("downloads");
    let json = safe_json_for_script(&data.downloads);
    let token_json = safe_json_for_script(&data.api_token);

    let tpl = r#"<!DOCTYPE html>
<html lang="id"><head><meta charset="utf-8"><title>Downloads — Kurowser</title>
<style>__CSS__</style></head>
<body>
__HEADER__
<div class="wrap">
  <div class="page-head">
    <h1>Daftar Unduhan</h1>
    <div style="display:flex;gap:8px">
      <button id="open-folder" class="btn">__SVG_FOLDER__ Buka Folder Downloads</button>
      <button id="clear-btn" class="btn btn-danger">Bersihkan Daftar</button>
    </div>
  </div>
  <div id="list" class="card"></div>
</div>
<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
const SVG_FILE = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent)" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`;
let ITEMS = __JSON__;
const listEl = document.getElementById('list');

function render() {
  listEl.innerHTML = '';
  if (!ITEMS.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    empty.textContent = 'Belum ada file yang diunduh.';
    listEl.appendChild(empty);
    return;
  }
  ITEMS.forEach(item => {
    const row = document.createElement('div'); row.className = 'row';
    const main = document.createElement('div'); main.className = 'row-main';
    const ico = document.createElement('div'); ico.className = 'site-ico'; ico.innerHTML = SVG_FILE;
    const tx = document.createElement('div'); tx.className = 'row-text';
    const t = document.createElement('div'); t.className = 'row-title'; t.textContent = item.filename;
    const u = document.createElement('div'); u.className = 'row-sub'; u.textContent = item.path || item.url;
    tx.appendChild(t); tx.appendChild(u);
    main.appendChild(ico); main.appendChild(tx);

    const badge = document.createElement('span'); badge.className = 'badge';
    badge.textContent = item.status;
    row.appendChild(main); row.appendChild(badge);
    listEl.appendChild(row);
  });
}
document.getElementById('open-folder').onclick = () => fetch('/api/downloads/open-folder', { method: 'POST', headers: {'X-Kurowser-Token':API_TOKEN} });
document.getElementById('clear-btn').onclick = async () => {
  await fetch('/api/downloads/clear', { method: 'POST', headers: {'X-Kurowser-Token':API_TOKEN} });
  ITEMS = [];
  render();
};
render();
</script></body></html>"#;

    tpl.replace("__CSS__", &css)
        .replace("__HEADER__", &header)
        .replace("__SVG_FOLDER__", SVG_FOLDER)
        .replace("__CSP_NONCE__", csp_nonce)
        .replace("__TOKEN_JSON__", &token_json)
        .replace("__JSON__", &json)
}

pub fn render_settings_page(data: &BrowserData, csp_nonce: &str) -> String {
    let p = data.palette();
    let css = shared_css(&p);
    let header = nav_header("settings");
    let current_preset = data.settings.theme_preset.id();
    let current_accent = data
        .settings
        .custom_accent_hex
        .clone()
        .unwrap_or_else(|| p.accent.hex());
    let current_search = match data.settings.search_engine {
        SearchEngine::Google => "google",
        SearchEngine::DuckDuckGo => "duckduckgo",
    };
    let max_tabs = data.settings.max_live_tabs;
    let adblock_enabled = data.settings.adblock_enabled;
    let adblock_cosmetic = data.settings.adblock_cosmetic;
    let total_blocked = data.settings.total_blocked_ads;

    let presets = [
        (ThemePreset::Midnight, "#181826", "#a78bfa"),
        (ThemePreset::Obsidian, "#141418", "#60a5fa"),
        (ThemePreset::Cyber, "#0f172a", "#22d3ee"),
        (ThemePreset::Sakura, "#1d1523", "#f472b6"),
        (ThemePreset::Emerald, "#111d19", "#34d399"),
        (ThemePreset::Crimson, "#1e1318", "#fb7185"),
    ];

    let mut preset_cards = String::new();
    for (preset, bg, acc) in presets {
        let active_cls = if preset.id() == current_preset {
            "theme-card active"
        } else {
            "theme-card"
        };
        preset_cards.push_str(&format!(
            r#"<button class="{active_cls}" data-preset="{id}" type="button">
  <span class="swatch" style="background:{bg};border-color:{acc}"><i style="background:{acc}"></i></span>
  <span>{label}</span>
</button>"#,
            id = preset.id(),
            label = preset.label(),
        ));
    }

    format!(
        r##"<!DOCTYPE html>
<html lang="id"><head><meta charset="utf-8"><title>Tema & Settings — Kurowser</title>
<style>
{css}
.sec-title{{font-size:15px;font-weight:600;margin:22px 0 10px;display:flex;align-items:center;gap:8px}}
.themes-grid{{display:grid;grid-template-columns:repeat(auto-fill,minmax(165px,1fr));gap:10px;margin-bottom:18px}}
.theme-card{{
  display:flex;align-items:center;gap:10px;padding:12px 14px;border-radius:12px;
  background:var(--bg-chrome);border:1px solid var(--border);color:var(--text);
  font-size:13px;font-weight:500;cursor:pointer;transition:all .15s;text-align:left;
}}
.theme-card:hover{{background:var(--bg-hover);border-color:var(--accent)}}
.theme-card.active{{background:var(--bg-active);border-color:var(--accent);box-shadow:0 0 0 2px var(--accent-soft)}}
.swatch{{
  width:24px;height:24px;border-radius:8px;border:2px solid;display:flex;align-items:center;justify-content:center;flex-shrink:0;
}}
.swatch i{{width:10px;height:10px;border-radius:50%;display:block}}
.control-row{{display:flex;align-items:center;justify-content:space-between;gap:16px;padding:16px 18px;border-bottom:1px solid var(--border)}}
.control-row:last-child{{border-bottom:none}}
.chips{{display:flex;align-items:center;gap:8px;flex-wrap:wrap}}
.chip{{width:26px;height:26px;border-radius:50%;border:2px solid rgba(255,255,255,.2);cursor:pointer}}
.sel-box{{padding:8px 12px;border-radius:10px;background:var(--bg-surface);color:var(--text);border:1px solid var(--border);font-size:13px}}
.switch-btn{{
  position:relative;width:46px;height:26px;border-radius:999px;border:1px solid var(--border);
  background:var(--bg-surface);cursor:pointer;transition:all .18s;flex-shrink:0;
}}
.switch-btn::after{{
  content:'';position:absolute;top:3px;left:3px;width:18px;height:18px;border-radius:50%;
  background:var(--muted);transition:transform .18s,background .18s;
}}
.switch-btn.on{{background:var(--accent-soft);border-color:var(--accent)}}
.switch-btn.on::after{{transform:translateX(20px);background:var(--accent)}}
.allow-form{{display:flex;gap:8px;width:min(360px,100%)}}
.allow-input{{
  flex:1;padding:8px 12px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-surface);color:var(--text);font-size:13px;outline:none;
}}
.allow-input:focus{{border-color:var(--accent)}}
.allow-chips{{display:flex;flex-wrap:wrap;gap:8px;padding:0 18px 16px}}
.allow-pill{{
  display:inline-flex;align-items:center;gap:6px;padding:4px 10px;border-radius:999px;
  background:var(--bg-surface);border:1px solid var(--border);font-size:12px;
}}
.allow-pill button{{
  background:none;border:none;color:var(--muted);cursor:pointer;font-size:14px;line-height:1;
}}
.allow-pill button:hover{{color:#f87171}}
.toast{{position:fixed;bottom:20px;right:24px;padding:10px 18px;border-radius:12px;background:var(--accent);color:#0f0f17;font-size:13px;font-weight:600;opacity:0;transition:opacity .2s;pointer-events:none}}
.toast.show{{opacity:1}}
</style></head>
<body>
{header}
<div class="wrap">
  <div class="page-head">
    <h1>Tampilan & Pengaturan Kurowser</h1>
  </div>

  <div class="sec-title">Preset Tema</div>
  <div class="themes-grid" id="presets">{preset_cards}</div>

  <div class="card">
    <div class="control-row">
      <div>
        <div class="row-title">Custom Accent Color</div>
        <div class="row-sub">Ubah warna aksen sidebar, tombol aktif, dan border address bar</div>
      </div>
      <div class="chips">
        <button class="chip" data-accent="#a78bfa" style="background:#a78bfa" type="button" title="Lavender"></button>
        <button class="chip" data-accent="#60a5fa" style="background:#60a5fa" type="button" title="Azure"></button>
        <button class="chip" data-accent="#22d3ee" style="background:#22d3ee" type="button" title="Cyan"></button>
        <button class="chip" data-accent="#34d399" style="background:#34d399" type="button" title="Emerald"></button>
        <button class="chip" data-accent="#f472b6" style="background:#f472b6" type="button" title="Sakura"></button>
        <button class="chip" data-accent="#fbbf24" style="background:#fbbf24" type="button" title="Amber"></button>
        <input id="accent-picker" type="color" value="{current_accent}" style="width:36px;height:30px;border:none;background:none;cursor:pointer">
        <button id="accent-reset" class="btn" type="button">Reset Preset</button>
      </div>
    </div>

    <div class="control-row">
      <div>
        <div class="row-title">Search Engine Default</div>
        <div class="row-sub">Dipakai saat mengetik kata pencarian di address bar</div>
      </div>
      <select id="search-eng" class="sel-box">
        <option value="google">Google</option>
        <option value="duckduckgo">DuckDuckGo (Hemat Tracker)</option>
      </select>
    </div>

    <div class="control-row">
      <div>
        <div class="row-title">Batas Tab Hidup (RAM Cap)</div>
        <div class="row-sub">Jumlah maksimal renderer tab aktif di memori; sisanya otomatis tidur / reload saat diklik</div>
      </div>
      <select id="max-tabs" class="sel-box">
        <option value="2">2 Tab (Hemat · Gaming)</option>
        <option value="3">3 Tab (Seimbang)</option>
        <option value="4">4 Tab (Multitasking)</option>
      </select>
    </div>
  </div>

  <div class="sec-title">{SVG_SHIELD} Kurowser Shields (Ad & Tracker Blocker) <span id="shields-status-badge" class="badge"></span></div>
  <div class="card">
    <div class="control-row">
      <div>
        <div class="row-title">Blokir Iklan & Tracker Otomatis</div>
        <div class="row-sub">Memblokir script iklan, analytics, telemetry & tracker pihak ketiga ({total_blocked} diblokir total)</div>
      </div>
      <button id="toggle-adblock" class="switch-btn" type="button" aria-label="Toggle Kurowser Shields"></button>
    </div>

    <div class="control-row">
      <div>
        <div class="row-title">Cosmetic Element Hiding</div>
        <div class="row-sub">Menyembunyikan container / banner iklan kosong agar halaman bersih</div>
      </div>
      <button id="toggle-cosmetic" class="switch-btn" type="button" aria-label="Toggle Cosmetic Filter"></button>
    </div>

    <div class="control-row">
      <div>
        <div class="row-title">Situs Dikecualikan (Allowlist)</div>
        <div class="row-sub">Shields otomatis nonaktif pada domain di bawah ini (atau klik ikon Perisai di address bar)</div>
      </div>
      <form id="allow-form" class="allow-form">
        <input id="allow-input" class="allow-input" placeholder="contoh.com" autocomplete="off">
        <button class="btn btn-sm btn-accent" type="submit">Tambah</button>
      </form>
    </div>
    <div id="allow-chips" class="allow-chips"></div>
  </div>
</div>
<div id="toast" class="toast">Tema & pengaturan disimpan!</div>

<script nonce="{csp_nonce}">
const API_TOKEN = {token_json};
let state = {{
  theme_preset: "{current_preset}",
  custom_accent_hex: {custom_accent_json},
  search_engine: "{current_search}",
  max_live_tabs: {max_tabs},
  adblock_enabled: {adblock_enabled},
  adblock_cosmetic: {adblock_cosmetic}
}};
let allowlist = {allowlist_json};

document.getElementById('search-eng').value = state.search_engine;
document.getElementById('max-tabs').value = String(state.max_live_tabs);

function showToast(msg) {{
  const t = document.getElementById('toast');
  if (msg) t.textContent = msg;
  t.classList.add('show');
  setTimeout(() => t.classList.remove('show'), 1400);
}}

function renderShieldUi() {{
  const abBtn = document.getElementById('toggle-adblock');
  const cosBtn = document.getElementById('toggle-cosmetic');
  const badge = document.getElementById('shields-status-badge');
  abBtn.classList.toggle('on', !!state.adblock_enabled);
  abBtn.setAttribute('aria-pressed', String(!!state.adblock_enabled));
  cosBtn.classList.toggle('on', !!state.adblock_cosmetic);
  cosBtn.setAttribute('aria-pressed', String(!!state.adblock_cosmetic));
  badge.textContent = state.adblock_enabled ? 'AKTIF' : 'MATI';

  const box = document.getElementById('allow-chips');
  box.innerHTML = '';
  if (!allowlist.length) {{
    const span = document.createElement('span');
    span.className = 'row-sub';
    span.textContent = 'Belum ada situs dalam daftar pengecualian.';
    box.appendChild(span);
    return;
  }}
  allowlist.forEach(host => {{
    const pill = document.createElement('span');
    pill.className = 'allow-pill';
    const lbl = document.createElement('span');
    lbl.textContent = host;
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '×';
    del.title = 'Hapus dari allowlist';
    del.addEventListener('click', async () => {{
      const res = await fetch('/api/shields/allowlist/remove', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN }},
        body: JSON.stringify({{ host }})
      }});
      if (res.ok) {{
        const out = await res.json();
        if (Array.isArray(out.allowlist)) allowlist = out.allowlist;
        renderShieldUi();
        showToast('Pengecualian dihapus!');
      }}
    }});
    pill.appendChild(lbl);
    pill.appendChild(del);
    box.appendChild(pill);
  }});
}}

async function save(reloadAfter = true) {{
  const res = await fetch('/api/settings', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN }},
    body: JSON.stringify(state)
  }});
  if (res.ok) {{
    showToast('Tema & pengaturan disimpan!');
    if (reloadAfter) setTimeout(() => location.reload(), 180);
  }}
}}

document.querySelectorAll('.theme-card').forEach(btn => {{
  btn.addEventListener('click', () => {{
    state.theme_preset = btn.dataset.preset;
    state.custom_accent_hex = null;
    save(true);
  }});
}});

document.querySelectorAll('.chip[data-accent]').forEach(chip => {{
  chip.addEventListener('click', () => {{
    state.custom_accent_hex = chip.dataset.accent;
    save(true);
  }});
}});

document.getElementById('accent-reset').addEventListener('click', () => {{
  state.custom_accent_hex = null;
  save(true);
}});

document.getElementById('accent-picker').addEventListener('change', e => {{
  state.custom_accent_hex = e.target.value;
  save(true);
}});
document.getElementById('search-eng').addEventListener('change', e => {{
  state.search_engine = e.target.value;
  save(true);
}});
document.getElementById('max-tabs').addEventListener('change', e => {{
  state.max_live_tabs = parseInt(e.target.value, 10) || 3;
  save(true);
}});

document.getElementById('toggle-adblock').addEventListener('click', () => {{
  state.adblock_enabled = !state.adblock_enabled;
  renderShieldUi();
  save(false);
}});
document.getElementById('toggle-cosmetic').addEventListener('click', () => {{
  state.adblock_cosmetic = !state.adblock_cosmetic;
  renderShieldUi();
  save(false);
}});

document.getElementById('allow-form').addEventListener('submit', async e => {{
  e.preventDefault();
  const inp = document.getElementById('allow-input');
  const host = inp.value.trim();
  if (!host) return;
  const res = await fetch('/api/shields/allowlist/add', {{
    method: 'POST',
    headers: {{ 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN }},
    body: JSON.stringify({{ host }})
  }});
  if (res.ok) {{
    const out = await res.json();
    if (Array.isArray(out.allowlist)) allowlist = out.allowlist;
    inp.value = '';
    renderShieldUi();
    showToast('Situs ditambahkan ke allowlist!');
  }}
}});

renderShieldUi();
</script></body></html>"##,
        custom_accent_json = safe_json_for_script(&data.settings.custom_accent_hex),
        allowlist_json = safe_json_for_script(&data.settings.adblock_allowlist),
        token_json = safe_json_for_script(&data.api_token),
    )
}
