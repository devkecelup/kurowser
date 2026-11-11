// ─── pages.rs ─── Internal kurowser:// pages with Poppins font, inline SVGs & WCAG-verified themes ───
use crate::storage::{BrowserData, SearchEngine, Settings, ThemePalette, ThemePreset};
use serde::Serialize;

const POPPINS_CSS: &str = r#"
@font-face {
  font-family: 'Poppins';
  font-style: normal;
  font-weight: 400;
  font-display: swap;
  src: url('/fonts/Poppins-Regular.ttf') format('truetype');
}
@font-face {
  font-family: 'Poppins';
  font-style: normal;
  font-weight: 500;
  font-display: swap;
  src: url('/fonts/Poppins-Medium.ttf') format('truetype');
}
@font-face {
  font-family: 'Poppins';
  font-style: normal;
  font-weight: 600;
  font-display: swap;
  src: url('/fonts/Poppins-SemiBold.ttf') format('truetype');
}
"#;

const SVG_KUROWSER_SM: &str = r##"<svg width="26" height="26" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <defs>
    <linearGradient id="k_grad_sm" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#14857c"/>
      <stop offset="55%" stop-color="#0f766e"/>
      <stop offset="100%" stop-color="#0b5d56"/>
    </linearGradient>
  </defs>
  <rect x="2.5" y="2.5" width="59" height="59" rx="15.5" fill="url(#k_grad_sm)"/>
  <rect x="3.5" y="3.5" width="57" height="57" rx="14.5" fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="1.4"/>
  <path d="M20.5 18v28M24 34L42.5 18M29.8 29L43.2 46" stroke="#ffffff" stroke-width="7.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
</svg>"##;

const SVG_KUROWSER_HERO: &str = r##"<svg width="44" height="44" viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
  <defs>
    <linearGradient id="k_grad_hero" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#14857c"/>
      <stop offset="55%" stop-color="#0f766e"/>
      <stop offset="100%" stop-color="#0b5d56"/>
    </linearGradient>
  </defs>
  <rect x="2.5" y="2.5" width="59" height="59" rx="15.5" fill="url(#k_grad_hero)"/>
  <rect x="3.5" y="3.5" width="57" height="57" rx="14.5" fill="none" stroke="rgba(255,255,255,0.20)" stroke-width="1.4"/>
  <path d="M20.5 18v28M24 34L42.5 18M29.8 29L43.2 46" stroke="#ffffff" stroke-width="7.4" stroke-linecap="round" stroke-linejoin="round" fill="none"/>
</svg>"##;

const SVG_CLOCK: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><polyline points="12 7 12 12 15.5 14"/></svg>"#;
const SVG_STAR: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polygon points="12 2.5 15.09 8.76 22 9.77 17 14.64 18.18 21.52 12 18.27 5.82 21.52 7 14.64 2 9.77 8.91 8.76 12 2.5"/></svg>"#;
const SVG_DOWNLOAD: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3v12"/><polyline points="7 11 12 16 17 11"/><path d="M5 20h14"/></svg>"#;
const SVG_SLIDERS: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="4" y1="7" x2="20" y2="7"/><circle cx="9" cy="7" r="2.5" fill="var(--bg-app)"/><line x1="4" y1="17" x2="20" y2="17"/><circle cx="15" cy="17" r="2.5" fill="var(--bg-app)"/></svg>"#;
const SVG_FOLDER: &str = r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6.5A2.5 2.5 0 0 1 5.5 4h4l2 2.5H18.5A2.5 2.5 0 0 1 21 9v8.5a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5Z"/></svg>"#;
const SVG_PLUS: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>"#;
const SVG_RESET: &str = r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="1 4 1 10 7 10"/><path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10"/></svg>"#;
const SVG_SHIELD: &str = r#"<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 2.5C15.1 2.5 17.8 3.6 19.5 4.8V10.6C19.5 15.7 16.3 19.6 12 21.3C7.7 19.6 4.5 15.7 4.5 10.6V4.8C6.2 3.6 8.9 2.5 12 2.5Z"/><path d="m8.9 12.1 2.1 2.1 4.2-4.3"/></svg>"#;

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
  'kurowser': `<svg viewBox="0 0 24 24" width="100%" height="100%"><rect width="24" height="24" rx="6" fill="#0f766e"/><path d="M8 6.5v11M9.5 12.8L16.2 6.5M11.6 10.9L16.5 17.5" stroke="#fff" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" fill="none"/></svg>`,
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
  const palette = ['#0f766e','#0369a1','#047857','#9a3412','#334155','#475569'];
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
    let color_scheme = if p.is_light { "light" } else { "dark" };
    format!(
        r#"{POPPINS_CSS}
*{{margin:0;padding:0;box-sizing:border-box}}
:root{{
  color-scheme:{color_scheme};
  --bg-app:{bg_app};
  --bg-chrome:{bg_chrome};
  --bg-surface:{bg_surface};
  --bg-hover:{bg_hover};
  --bg-active:{bg_active};
  --border:{border};
  --border-subtle:{border_subtle};
  --text:{text};
  --muted:{muted};
  --text-inactive:{text_inactive};
  --text-disabled:{text_disabled};
  --accent:{accent};
  --accent-text:{accent_text};
  --accent-soft:{accent_soft};
  --on-accent:{on_accent};
  --focus-ring:{focus_ring};
  --danger-fg:{danger_fg};
  --danger-bg:{danger_bg};
  --danger-border:{danger_border};
  --success-fg:{success_fg};
  --warning-fg:{warning_fg};
}}
body{{
  background:var(--bg-app);
  color:var(--text);
  font-family:'Poppins','Segoe UI',sans-serif;
  font-size:14px;
  line-height:1.5;
  min-height:100vh;
  -webkit-font-smoothing:antialiased;
}}
a{{color:inherit;text-decoration:none}}
button,input,select{{font-family:inherit}}
button,[role="button"],select,summary{{cursor:pointer}}
button:disabled{{opacity:0.55;cursor:not-allowed}}
a:focus-visible,button:focus-visible,input:focus-visible,select:focus-visible{{
  outline:2px solid var(--focus-ring);
  outline-offset:2px;
}}
.sr-only{{
  position:absolute;width:1px;height:1px;padding:0;margin:-1px;
  overflow:hidden;clip:rect(0,0,0,0);white-space:nowrap;border:0;
}}
.topbar-nav{{
  display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap;
  padding:10px 28px 0;border-bottom:1px solid var(--border-subtle);background:var(--bg-chrome);
}}
.brand-sm{{
  display:inline-flex;align-items:center;gap:10px;font-weight:600;font-size:15px;
  min-height:44px;padding:4px 0;margin-bottom:4px;border-radius:8px;
}}
.brand-sm svg{{flex-shrink:0}}
.nav-tabs{{display:flex;gap:4px;flex-wrap:wrap;align-items:center}}
.nav-tab{{
  display:inline-flex;align-items:center;min-height:44px;
  padding:8px 14px;font-size:13.5px;font-weight:500;
  color:var(--muted);border-bottom:2px solid transparent;border-radius:6px 6px 0 0;
  transition:color .14s,background-color .14s,border-color .14s;
}}
.nav-tab:hover{{background:var(--bg-hover);color:var(--text)}}
.nav-tab.active{{color:var(--accent-text);border-bottom-color:var(--accent-text);font-weight:600}}
.wrap{{max-width:860px;margin:0 auto;padding:28px 24px 56px}}
.page-head{{display:flex;align-items:center;justify-content:space-between;gap:16px;flex-wrap:wrap;margin-bottom:18px}}
.page-head h1{{font-size:22px;font-weight:600;letter-spacing:-0.2px}}
.page-sub{{font-size:13px;color:var(--muted);margin-top:2px}}
.btn{{
  display:inline-flex;align-items:center;justify-content:center;gap:7px;
  min-height:44px;padding:9px 16px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-chrome);color:var(--text);font-size:13.5px;font-weight:500;
  transition:background-color .14s,border-color .14s,color .14s;
}}
.btn:hover:not(:disabled){{background:var(--bg-hover);border-color:var(--accent-text)}}
.btn-sm{{min-height:44px;padding:8px 13px;font-size:13px;border-radius:10px;gap:6px}}
.btn-danger{{border-color:var(--danger-border);color:var(--danger-fg);background:var(--danger-bg)}}
.btn-danger:hover:not(:disabled){{filter:brightness(1.06)}}
.btn-accent{{background:var(--accent);color:var(--on-accent);border-color:var(--accent);font-weight:600}}
.btn-accent:hover:not(:disabled){{filter:brightness(1.06)}}
.search-input{{
  width:100%;min-height:44px;padding:10px 15px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-surface);color:var(--text);font-size:14px;margin-bottom:16px;
}}
.search-input:focus-visible{{border-color:var(--focus-ring);outline:2px solid var(--focus-ring);outline-offset:0}}
.err-banner{{
  display:none;align-items:center;justify-content:space-between;gap:12px;
  padding:11px 14px;border-radius:10px;margin-bottom:16px;
  background:var(--danger-bg);border:1px solid var(--danger-border);color:var(--danger-fg);
  font-size:13px;font-weight:500;
}}
.err-banner.open{{display:flex}}
.card{{
  background:var(--bg-chrome);border:1px solid var(--border-subtle);border-radius:12px;overflow:hidden;
}}
.row{{
  display:flex;align-items:center;justify-content:space-between;gap:14px;
  padding:10px 14px;border-bottom:1px solid var(--border-subtle);transition:background-color .12s;
}}
.row:last-child{{border-bottom:none}}
.row:hover{{background:var(--bg-hover)}}
.row-main{{display:flex;align-items:center;gap:12px;min-width:0;flex:1;min-height:44px}}
.row-text{{min-width:0;flex:1}}
.row-title{{font-size:14px;font-weight:500;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.row-sub{{font-size:12.5px;color:var(--muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.visit-meta{{font-size:12px;font-weight:500;color:var(--muted);font-variant-numeric:tabular-nums;white-space:nowrap}}
.status-tag{{
  display:inline-flex;align-items:center;padding:4px 9px;border-radius:6px;
  font-size:12px;font-weight:600;background:var(--bg-surface);border:1px solid var(--border-subtle);
  color:var(--accent-text);white-space:nowrap;
}}
.status-tag.done{{color:var(--success-fg)}}
.status-tag.err{{color:var(--danger-fg);background:var(--danger-bg);border-color:var(--danger-border)}}
.icon-del{{
  display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;
  background:transparent;border:1px solid transparent;color:var(--muted);
  width:44px;height:44px;min-width:44px;min-height:44px;border-radius:10px;transition:all .12s;
}}
.icon-del:hover{{background:var(--danger-bg);color:var(--danger-fg);border-color:var(--danger-border)}}
.empty{{
  padding:36px 20px;text-align:center;color:var(--muted);font-size:14px;
  display:flex;flex-direction:column;align-items:center;gap:12px;
}}
.site-ico{{
  width:26px;height:26px;border-radius:6px;overflow:hidden;flex-shrink:0;
  display:inline-flex;align-items:center;justify-content:center;
}}
.site-ico img{{width:100%;height:100%;object-fit:cover;display:block;border-radius:6px}}
@media (max-width:640px){{
  .topbar-nav{{padding:10px 16px 0}}
  .wrap{{padding:20px 16px 44px}}
  .row{{padding:10px 12px;gap:10px}}
}}
@media (prefers-reduced-motion:reduce){{
  *,*::before,*::after{{
    transition-duration:0.001ms!important;
    animation-duration:0.001ms!important;
    scroll-behavior:auto!important;
  }}
}}
"#,
        color_scheme = color_scheme,
        bg_app = p.bg_app.hex(),
        bg_chrome = p.bg_chrome.hex(),
        bg_surface = p.bg_surface.hex(),
        bg_hover = p.bg_hover.hex(),
        bg_active = p.bg_active.hex(),
        border = p.border.hex(),
        border_subtle = p.border_subtle.hex(),
        text = p.text_primary.hex(),
        muted = p.text_muted.hex(),
        text_inactive = p.text_inactive_tab.hex(),
        text_disabled = p.text_disabled.hex(),
        accent = p.accent.hex(),
        accent_text = p.accent_text.hex(),
        accent_soft = p.accent_soft.hex(),
        on_accent = p.on_accent.hex(),
        focus_ring = p.focus_ring.hex(),
        danger_fg = p.danger_fg.hex(),
        danger_bg = p.danger_bg.hex(),
        danger_border = p.danger_border.hex(),
        success_fg = p.success_fg.hex(),
        warning_fg = p.warning_fg.hex(),
    )
}

fn nav_header(active_slug: &str) -> String {
    let links = [
        ("start", "Beranda", "kurowser://start"),
        ("history", "History", "kurowser://history"),
        ("bookmarks", "Bookmark", "kurowser://bookmarks"),
        ("downloads", "Unduhan", "kurowser://downloads"),
        ("settings", "Pengaturan", "kurowser://settings"),
    ];
    let mut tabs = String::new();
    for (slug, label, href) in links {
        let is_active = slug == active_slug;
        let cls = if is_active {
            "nav-tab active"
        } else {
            "nav-tab"
        };
        let current = if is_active {
            r#" aria-current="page""#
        } else {
            ""
        };
        tabs.push_str(&format!(
            r#"<a class="{cls}" href="{href}"{current}>{label}</a>"#
        ));
    }
    format!(
        r#"<header class="topbar-nav">
  <a class="brand-sm" href="kurowser://start">
    {SVG_KUROWSER_SM}<span>Kurowser</span>
  </a>
  <nav class="nav-tabs" aria-label="Navigasi Kurowser">{tabs}</nav>
</header>"#
    )
}

pub fn render_start_page(data: &BrowserData, csp_nonce: &str) -> String {
    let p = data.palette();
    let css = shared_css(&p);
    let _ = SearchEngine::Google;

    #[derive(Serialize)]
    struct StartPayload<'a> {
        api_token: &'a str,
        quick_links: &'a [crate::storage::QuickLinkItem],
        cached_domains: Vec<String>,
        bookmarks: &'a [crate::storage::BookmarkItem],
    }

    let payload = StartPayload {
        api_token: &data.api_token,
        quick_links: &data.quick_links,
        cached_domains: data.cached_favicon_domains(),
        bookmarks: &data.bookmarks,
    };
    let json = safe_json_for_script(&payload);

    let tpl = r#"<!DOCTYPE html>
<html lang="id"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Kurowser</title>
<style>
__CSS__
.hero{
  min-height:100vh;display:flex;flex-direction:column;align-items:center;
  justify-content:center;padding:34px 20px;background:var(--bg-app);
}
.hero-head{text-align:center;margin-bottom:26px}
.logo-row{display:inline-flex;align-items:center;gap:14px;margin-bottom:6px}
.logo-row h1{font-size:32px;font-weight:600;letter-spacing:-0.4px}
.hero-hint{font-size:13px;color:var(--muted)}
.hero-kbd{
  display:inline-flex;align-items:center;padding:2px 7px;margin:0 2px;
  border-radius:6px;background:var(--bg-chrome);border:1px solid var(--border);
  font-size:11.5px;font-weight:600;color:var(--text);
}
.sec-wrap{width:min(760px,100%);margin-bottom:22px}
.sec-head{
  display:flex;align-items:center;justify-content:space-between;gap:12px;flex-wrap:wrap;
  margin:4px 0 11px;
}
.section-lbl{font-size:14px;font-weight:600;color:var(--muted)}
.sec-actions{display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.grid{
  display:grid;grid-template-columns:repeat(auto-fill,minmax(168px,1fr));
  gap:10px;width:100%;
}
.tile-card{
  display:flex;align-items:stretch;border-radius:12px;
  background:var(--bg-chrome);border:1px solid var(--border);
  overflow:hidden;transition:background-color .12s,border-color .12s;
}
.tile-card:hover{background:var(--bg-hover);border-color:var(--accent-text)}
.tile-link{
  display:flex;align-items:center;gap:10px;padding:10px 6px 10px 12px;
  min-height:52px;min-width:0;flex:1;font-size:13.5px;font-weight:500;
}
.tile-text{min-width:0;flex:1;display:flex;flex-direction:column}
.tile-name{overflow:hidden;text-overflow:ellipsis;white-space:nowrap;line-height:1.25}
.tile-host{font-size:11px;color:var(--muted);font-weight:400;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.tile-opt{
  display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;
  width:44px;min-width:44px;min-height:44px;border:none;background:transparent;
  color:var(--muted);transition:color .12s,background-color .12s;
}
.tile-opt:hover,.tile-opt:focus-visible{color:var(--accent-text);background:var(--bg-active)}
.tile-add{
  display:inline-flex;align-items:center;justify-content:center;gap:8px;width:100%;
  min-height:52px;padding:10px 12px;border-radius:12px;
  background:var(--bg-chrome);border:1px dashed var(--border);
  color:var(--muted);font-size:13.5px;font-weight:500;
  transition:color .12s,border-color .12s,background-color .12s;
}
.tile-add:hover{color:var(--accent-text);border-color:var(--accent-text);background:var(--bg-hover)}
.footer-bar{display:flex;flex-wrap:wrap;justify-content:center;gap:6px;margin-top:8px}
.f-btn{
  display:inline-flex;align-items:center;gap:7px;min-height:44px;
  padding:8px 13px;border-radius:10px;font-size:13px;font-weight:500;
  color:var(--muted);border:1px solid transparent;transition:all .14s;
}
.f-btn:hover{color:var(--text);background:var(--bg-hover);border-color:var(--border-subtle)}
.modal-backdrop{
  position:fixed;inset:0;background:rgba(8,12,16,.72);
  display:none;align-items:center;justify-content:center;padding:16px;z-index:50;
}
.modal-backdrop.open{display:flex}
.modal{
  width:min(440px,100%);background:var(--bg-chrome);border:1px solid var(--border);
  border-radius:14px;padding:22px;box-shadow:0 12px 28px rgba(0,0,0,.32);
}
.modal-head{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-bottom:14px}
.modal-head h2{font-size:17px;font-weight:600}
.field-lbl{display:block;font-size:12.5px;font-weight:500;color:var(--muted);margin-bottom:6px}
.url-row{display:flex;align-items:center;gap:10px;margin-bottom:14px}
.url-row .site-ico{width:34px;height:34px;border-radius:8px}
.m-input{
  width:100%;min-height:44px;padding:10px 13px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-surface);color:var(--text);font-size:14px;
}
.m-input:focus-visible{border-color:var(--focus-ring);outline:2px solid var(--focus-ring);outline-offset:0}
.m-err{font-size:12.5px;color:var(--danger-fg);min-height:20px;margin:8px 0 10px}
.modal-foot{display:flex;align-items:center;justify-content:space-between;gap:10px;flex-wrap:wrap;margin-top:8px}
.modal-foot-left{display:flex;gap:6px;align-items:center;flex-wrap:wrap}
.modal-foot-right{display:flex;gap:8px;margin-left:auto}
@media (max-width:640px){
  .hero{padding:24px 16px}
  .logo-row{gap:10px}
  .logo-row h1{font-size:26px}
  .modal{padding:18px}
}
</style></head>
<body>
<main class="hero">
  <header class="hero-head">
    <div class="logo-row">
      __SVG_HERO__
      <h1>Kurowser</h1>
    </div>
    <p class="hero-hint">Gunakan kolom pencarian di sidebar atau tekan <span class="hero-kbd">Ctrl+L</span></p>
  </header>

  <section id="bm-section" class="sec-wrap" aria-labelledby="bm-heading" style="display:none">
    <div class="sec-head"><h2 id="bm-heading" class="section-lbl">Bookmark Tersimpan</h2></div>
    <div id="bm-grid" class="grid"></div>
  </section>

  <section class="sec-wrap" aria-labelledby="qa-heading">
    <div class="sec-head">
      <h2 id="qa-heading" class="section-lbl">Akses Cepat</h2>
      <div class="sec-actions">
        <button id="qa-add-btn" class="btn btn-sm" type="button">__SVG_PLUS__ Tambah</button>
        <button id="qa-reset-btn" class="btn btn-sm" type="button" title="Kembalikan Akses Cepat bawaan">__SVG_RESET__ Reset</button>
      </div>
    </div>
    <div id="qa-err" class="err-banner" role="alert"></div>
    <div id="qa-grid" class="grid"></div>
  </section>

  <nav class="footer-bar" aria-label="Halaman Kurowser">
    <a class="f-btn" href="kurowser://history">__SVG_CLOCK__ History</a>
    <a class="f-btn" href="kurowser://bookmarks">__SVG_STAR__ Bookmark</a>
    <a class="f-btn" href="kurowser://downloads">__SVG_DOWNLOAD__ Unduhan</a>
    <a class="f-btn" href="kurowser://settings">__SVG_SLIDERS__ Pengaturan</a>
  </nav>
</main>

<div id="qa-modal" class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="m-title">
  <div class="modal">
    <div class="modal-head">
      <h2 id="m-title">Tambah Akses Cepat</h2>
      <button id="m-close" class="icon-del" type="button" aria-label="Tutup dialog">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
      </button>
    </div>
    <label class="field-lbl" for="m-url">Alamat Website (URL)</label>
    <div class="url-row">
      <div id="m-preview"></div>
      <input id="m-url" class="m-input" placeholder="youtube.com atau https://example.com">
    </div>
    <label class="field-lbl" for="m-name">Nama Tampilan</label>
    <input id="m-name" class="m-input" placeholder="Otomatis dari nama situs" maxlength="40">
    <div id="m-err" class="m-err" role="alert"></div>
    <div class="modal-foot">
      <div id="m-order-btns" class="modal-foot-left" style="display:none">
        <button id="m-left" class="btn btn-sm" type="button" aria-label="Geser ke kiri" title="Geser ke kiri">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <button id="m-right" class="btn btn-sm" type="button" aria-label="Geser ke kanan" title="Geser ke kanan">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><polyline points="9 18 15 12 9 6"/></svg>
        </button>
        <button id="m-del" class="btn btn-sm btn-danger" type="button">Hapus</button>
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

const SVG_MORE = `<svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="12" cy="5.5" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="12" cy="18.5" r="1.7"/></svg>`;

if (DATA.bookmarks && DATA.bookmarks.length > 0) {
  document.getElementById('bm-section').style.display = 'block';
  const bmGrid = document.getElementById('bm-grid');
  DATA.bookmarks.slice(0, 8).forEach(b => {
    const card = document.createElement('div'); card.className = 'tile-card';
    const a = document.createElement('a');
    a.className = 'tile-link';
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
const qaErr = document.getElementById('qa-err');

function showPageErr(msg) {
  qaErr.textContent = msg;
  qaErr.classList.add('open');
}
function clearPageErr() {
  qaErr.textContent = '';
  qaErr.classList.remove('open');
}

async function persistQuickLinks(items, opts = {}) {
  clearPageErr();
  try {
    const res = await fetch('/api/quick-links/save', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': DATA.api_token },
      body: JSON.stringify({ items })
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    const out = await res.json();
    if (Array.isArray(out.items)) quickLinks = out.items;
    renderQuickLinks();
    return true;
  } catch (err) {
    const msg = 'Gagal menyimpan Akses Cepat. Periksa status aplikasi dan coba lagi.';
    if (opts.inModal) {
      mErr.textContent = msg;
    } else {
      showPageErr(msg);
    }
    return false;
  }
}

function renderQuickLinks() {
  qaGrid.innerHTML = '';
  quickLinks.forEach((item, idx) => {
    const card = document.createElement('div'); card.className = 'tile-card';
    const a = document.createElement('a'); a.className = 'tile-link'; a.href = item.url;
    const ico = createSiteIconEl(item.url, CACHED_SET);
    const txt = document.createElement('div'); txt.className = 'tile-text';
    const label = item.title || extractDomain(item.url);
    const name = document.createElement('span'); name.className = 'tile-name'; name.textContent = label;
    const host = document.createElement('span'); host.className = 'tile-host'; host.textContent = extractDomain(item.url);
    txt.appendChild(name); txt.appendChild(host);
    a.appendChild(ico); a.appendChild(txt);

    const optBtn = document.createElement('button');
    optBtn.className = 'tile-opt';
    optBtn.type = 'button';
    optBtn.setAttribute('aria-label', 'Edit atau hapus shortcut ' + label);
    optBtn.title = 'Edit atau hapus shortcut';
    optBtn.innerHTML = SVG_MORE;
    optBtn.onclick = () => openModal(idx, optBtn);

    card.appendChild(a);
    card.appendChild(optBtn);
    qaGrid.appendChild(card);
  });

  if (quickLinks.length < 24) {
    const addBtn = document.createElement('button');
    addBtn.className = 'tile-add';
    addBtn.type = 'button';
    addBtn.innerHTML = `__SVG_PLUS__ <span>Tambah Situs</span>`;
    addBtn.onclick = () => openModal(-1, addBtn);
    qaGrid.appendChild(addBtn);
  }
}

const modal = document.getElementById('qa-modal');
const modalBox = modal.querySelector('.modal');
const mTitle = document.getElementById('m-title');
const mUrl = document.getElementById('m-url');
const mName = document.getElementById('m-name');
const mErr = document.getElementById('m-err');
const mPrev = document.getElementById('m-preview');
const mOrderBtns = document.getElementById('m-order-btns');
const mSave = document.getElementById('m-save');
const mDel = document.getElementById('m-del');
const mLeft = document.getElementById('m-left');
const mRight = document.getElementById('m-right');
let editIndex = -1;
let customTitleTouched = false;
let lastTriggerEl = null;

function getModalFocusable() {
  return [...modalBox.querySelectorAll('button:not([disabled]),input:not([disabled]),[tabindex]:not([tabindex="-1"])')]
    .filter(el => el.offsetParent !== null);
}

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

function updateOrderButtons() {
  mLeft.disabled = editIndex <= 0;
  mRight.disabled = editIndex < 0 || editIndex >= quickLinks.length - 1;
}

function openModal(idx, triggerEl) {
  lastTriggerEl = triggerEl || document.activeElement;
  editIndex = idx;
  mErr.textContent = '';
  if (idx >= 0 && quickLinks[idx]) {
    mTitle.textContent = 'Edit Akses Cepat';
    mUrl.value = quickLinks[idx].url;
    mName.value = quickLinks[idx].title;
    customTitleTouched = true;
    mOrderBtns.style.display = 'flex';
    updateOrderButtons();
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
  if (lastTriggerEl && typeof lastTriggerEl.focus === 'function' && document.contains(lastTriggerEl)) {
    lastTriggerEl.focus();
  }
  lastTriggerEl = null;
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
    mErr.textContent = 'Masukkan URL valid, contoh: youtube.com atau https://example.com';
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
  mSave.disabled = true;
  mSave.setAttribute('aria-busy', 'true');
  try {
    const ok = await persistQuickLinks(next, { inModal: true });
    if (ok) closeModal();
  } finally {
    mSave.disabled = false;
    mSave.removeAttribute('aria-busy');
  }
}

document.getElementById('qa-add-btn').onclick = (e) => openModal(-1, e.currentTarget);
document.getElementById('qa-reset-btn').onclick = async (e) => {
  const btn = e.currentTarget;
  clearPageErr();
  btn.disabled = true;
  btn.setAttribute('aria-busy', 'true');
  try {
    const res = await fetch('/api/quick-links/reset', {
      method: 'POST',
      headers: { 'X-Kurowser-Token': DATA.api_token }
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    const out = await res.json();
    if (Array.isArray(out.items)) quickLinks = out.items;
    renderQuickLinks();
  } catch {
    showPageErr('Gagal mengembalikan Akses Cepat bawaan.');
  } finally {
    btn.disabled = false;
    btn.removeAttribute('aria-busy');
  }
};
mUrl.addEventListener('input', () => { mErr.textContent = ''; updateModalPreview(); });
mName.addEventListener('input', () => { customTitleTouched = mName.value.trim().length > 0; });
mSave.onclick = saveModal;
document.getElementById('m-cancel').onclick = closeModal;
document.getElementById('m-close').onclick = closeModal;
mDel.onclick = async () => {
  if (editIndex < 0 || editIndex >= quickLinks.length) return;
  const next = quickLinks.filter((_, i) => i !== editIndex);
  mDel.disabled = true;
  try {
    const ok = await persistQuickLinks(next, { inModal: true });
    if (ok) closeModal();
  } finally {
    mDel.disabled = false;
  }
};
mLeft.onclick = async () => {
  if (editIndex > 0) {
    const next = quickLinks.slice();
    [next[editIndex - 1], next[editIndex]] = [next[editIndex], next[editIndex - 1]];
    const ok = await persistQuickLinks(next, { inModal: true });
    if (ok) {
      editIndex -= 1;
      updateOrderButtons();
    }
  }
};
mRight.onclick = async () => {
  if (editIndex >= 0 && editIndex < quickLinks.length - 1) {
    const next = quickLinks.slice();
    [next[editIndex + 1], next[editIndex]] = [next[editIndex], next[editIndex + 1]];
    const ok = await persistQuickLinks(next, { inModal: true });
    if (ok) {
      editIndex += 1;
      updateOrderButtons();
    }
  }
};
modal.addEventListener('click', (e) => { if (e.target === modal) closeModal(); });
modal.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') {
    e.preventDefault();
    closeModal();
  } else if (e.key === 'Tab') {
    const focusables = getModalFocusable();
    if (!focusables.length) return;
    const first = focusables[0];
    const last = focusables[focusables.length - 1];
    if (e.shiftKey && document.activeElement === first) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && document.activeElement === last) {
      e.preventDefault();
      first.focus();
    }
  } else if (e.key === 'Enter' && (document.activeElement === mUrl || document.activeElement === mName)) {
    e.preventDefault();
    saveModal();
  }
});

renderQuickLinks();
</script>
</body></html>"#;

    tpl.replace("__CSS__", &css)
        .replace("__SVG_HERO__", SVG_KUROWSER_HERO)
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
<html lang="id"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>History - Kurowser</title>
<style>__CSS__</style></head>
<body>
__HEADER__
<main class="wrap">
  <div class="page-head">
    <h1>Riwayat Penjelajahan</h1>
    <button id="clear-btn" class="btn btn-danger" type="button">Hapus Semua</button>
  </div>
  <div id="api-err" class="err-banner" role="alert"></div>
  <label class="sr-only" for="filter">Cari riwayat penjelajahan</label>
  <input id="filter" class="search-input" type="search" placeholder="Cari judul atau alamat situs...">
  <div id="list" class="card"></div>
</main>
<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
const CACHED_SET = new Set(__CACHED_JSON__ || []);
__SITE_ICON_JS__
const SVG_X = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
let ITEMS = __JSON__;
const listEl = document.getElementById('list');
const filterEl = document.getElementById('filter');
const clearBtn = document.getElementById('clear-btn');
const errEl = document.getElementById('api-err');

function showErr(msg) {
  errEl.textContent = msg;
  errEl.classList.add('open');
}
function clearErr() {
  errEl.textContent = '';
  errEl.classList.remove('open');
}

function render() {
  const q = filterEl.value.trim().toLowerCase();
  const filtered = q ? ITEMS.filter(x => x.title.toLowerCase().includes(q) || x.url.toLowerCase().includes(q)) : ITEMS;
  clearBtn.disabled = ITEMS.length === 0;
  listEl.innerHTML = '';
  if (!filtered.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    const msg = document.createElement('p');
    msg.textContent = q ? 'Tidak ada riwayat yang cocok dengan kata kunci tersebut.' : 'Belum ada riwayat penjelajahan.';
    empty.appendChild(msg);
    if (q) {
      const resetFilter = document.createElement('button');
      resetFilter.type = 'button';
      resetFilter.className = 'btn btn-sm';
      resetFilter.textContent = 'Bersihkan Pencarian';
      resetFilter.onclick = () => { filterEl.value = ''; render(); filterEl.focus(); };
      empty.appendChild(resetFilter);
    } else {
      const homeLink = document.createElement('a');
      homeLink.className = 'btn btn-sm';
      homeLink.href = 'kurowser://start';
      homeLink.textContent = 'Buka Beranda';
      empty.appendChild(homeLink);
    }
    listEl.appendChild(empty);
    return;
  }
  filtered.slice(0, 200).forEach(item => {
    const row = document.createElement('div'); row.className = 'row';
    const main = document.createElement('a'); main.className = 'row-main'; main.href = item.url;
    main.appendChild(createSiteIconEl(item.url, CACHED_SET));
    const tx = document.createElement('div'); tx.className = 'row-text';
    const label = item.title || item.domain;
    const t = document.createElement('div'); t.className = 'row-title'; t.textContent = label;
    const u = document.createElement('div'); u.className = 'row-sub'; u.textContent = item.url;
    tx.appendChild(t); tx.appendChild(u);
    main.appendChild(tx);

    const meta = document.createElement('span'); meta.className = 'visit-meta';
    meta.textContent = item.visit_count + 'x';

    const del = document.createElement('button');
    del.className = 'icon-del';
    del.type = 'button';
    del.setAttribute('aria-label', 'Hapus riwayat ' + label);
    del.title = 'Hapus item';
    del.innerHTML = SVG_X;
    del.onclick = async () => {
      clearErr();
      del.disabled = true;
      try {
        const res = await fetch('/api/history/delete', {
          method: 'POST',
          headers: {'Content-Type':'application/json','X-Kurowser-Token':API_TOKEN},
          body: JSON.stringify({url: item.url})
        });
        if (!res.ok) throw new Error('HTTP ' + res.status);
        ITEMS = ITEMS.filter(x => x.url !== item.url);
        render();
      } catch {
        del.disabled = false;
        showErr('Gagal menghapus riwayat. Silakan coba lagi.');
      }
    };
    row.appendChild(main); row.appendChild(meta); row.appendChild(del);
    listEl.appendChild(row);
  });
}

clearBtn.onclick = async () => {
  clearErr();
  clearBtn.disabled = true;
  clearBtn.setAttribute('aria-busy', 'true');
  try {
    const res = await fetch('/api/history/clear', { method: 'POST', headers: {'X-Kurowser-Token':API_TOKEN} });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    ITEMS = [];
    render();
  } catch {
    showErr('Gagal menghapus seluruh riwayat. Silakan coba lagi.');
    render();
  } finally {
    clearBtn.removeAttribute('aria-busy');
  }
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
<html lang="id"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Bookmark - Kurowser</title>
<style>__CSS__</style></head>
<body>
__HEADER__
<main class="wrap">
  <div class="page-head">
    <div>
      <h1>Bookmark Tersimpan</h1>
      <p class="page-sub">Klik ikon bintang di kanan address bar atau tekan Ctrl+D untuk menyimpan halaman.</p>
    </div>
  </div>
  <div id="api-err" class="err-banner" role="alert"></div>
  <label class="sr-only" for="filter">Cari bookmark</label>
  <input id="filter" class="search-input" type="search" placeholder="Cari bookmark...">
  <div id="list" class="card"></div>
</main>
<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
const CACHED_SET = new Set(__CACHED_JSON__ || []);
__SITE_ICON_JS__
const SVG_X = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
let ITEMS = __JSON__;
const listEl = document.getElementById('list');
const filterEl = document.getElementById('filter');
const errEl = document.getElementById('api-err');

function showErr(msg) {
  errEl.textContent = msg;
  errEl.classList.add('open');
}
function clearErr() {
  errEl.textContent = '';
  errEl.classList.remove('open');
}

function render() {
  const q = filterEl.value.trim().toLowerCase();
  const filtered = q ? ITEMS.filter(x => x.title.toLowerCase().includes(q) || x.url.toLowerCase().includes(q)) : ITEMS;
  listEl.innerHTML = '';
  if (!filtered.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    const msg = document.createElement('p');
    msg.textContent = q
      ? 'Tidak ada bookmark yang cocok dengan pencarian tersebut.'
      : 'Belum ada bookmark tersimpan.';
    empty.appendChild(msg);
    if (q) {
      const resetFilter = document.createElement('button');
      resetFilter.type = 'button';
      resetFilter.className = 'btn btn-sm';
      resetFilter.textContent = 'Bersihkan Pencarian';
      resetFilter.onclick = () => { filterEl.value = ''; render(); filterEl.focus(); };
      empty.appendChild(resetFilter);
    } else {
      const homeLink = document.createElement('a');
      homeLink.className = 'btn btn-sm';
      homeLink.href = 'kurowser://start';
      homeLink.textContent = 'Buka Beranda';
      empty.appendChild(homeLink);
    }
    listEl.appendChild(empty);
    return;
  }
  filtered.forEach(item => {
    const row = document.createElement('div'); row.className = 'row';
    const main = document.createElement('a'); main.className = 'row-main'; main.href = item.url;
    main.appendChild(createSiteIconEl(item.url, CACHED_SET));
    const tx = document.createElement('div'); tx.className = 'row-text';
    const label = item.title || item.domain;
    const t = document.createElement('div'); t.className = 'row-title'; t.textContent = label;
    const u = document.createElement('div'); u.className = 'row-sub'; u.textContent = item.url;
    tx.appendChild(t); tx.appendChild(u);
    main.appendChild(tx);

    const del = document.createElement('button');
    del.className = 'icon-del';
    del.type = 'button';
    del.setAttribute('aria-label', 'Hapus bookmark ' + label);
    del.title = 'Hapus bookmark';
    del.innerHTML = SVG_X;
    del.onclick = async () => {
      clearErr();
      del.disabled = true;
      try {
        const res = await fetch('/api/bookmarks/delete', {
          method: 'POST',
          headers: {'Content-Type':'application/json','X-Kurowser-Token':API_TOKEN},
          body: JSON.stringify({url: item.url})
        });
        if (!res.ok) throw new Error('HTTP ' + res.status);
        ITEMS = ITEMS.filter(x => x.url !== item.url);
        render();
      } catch {
        del.disabled = false;
        showErr('Gagal menghapus bookmark. Silakan coba lagi.');
      }
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
<html lang="id"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Unduhan - Kurowser</title>
<style>__CSS__</style></head>
<body>
__HEADER__
<main class="wrap">
  <div class="page-head">
    <h1>Daftar Unduhan</h1>
    <div style="display:flex;gap:8px;flex-wrap:wrap">
      <button id="open-folder" class="btn" type="button">__SVG_FOLDER__ Buka Folder Unduhan</button>
      <button id="clear-btn" class="btn btn-danger" type="button">Bersihkan Daftar</button>
    </div>
  </div>
  <div id="api-err" class="err-banner" role="alert"></div>
  <div id="list" class="card"></div>
</main>
<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
const SVG_FILE = `<svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="var(--accent-text)" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`;
let ITEMS = __JSON__;
const listEl = document.getElementById('list');
const clearBtn = document.getElementById('clear-btn');
const openFolderBtn = document.getElementById('open-folder');
const errEl = document.getElementById('api-err');

function showErr(msg) {
  errEl.textContent = msg;
  errEl.classList.add('open');
}
function clearErr() {
  errEl.textContent = '';
  errEl.classList.remove('open');
}

async function openDownloadsFolder() {
  clearErr();
  openFolderBtn.disabled = true;
  try {
    const res = await fetch('/api/downloads/open-folder', {
      method: 'POST',
      headers: {'X-Kurowser-Token':API_TOKEN}
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
  } catch {
    showErr('Gagal membuka folder Unduhan.');
  } finally {
    openFolderBtn.disabled = false;
  }
}

function render() {
  clearBtn.disabled = ITEMS.length === 0;
  listEl.innerHTML = '';
  if (!ITEMS.length) {
    const empty = document.createElement('div');
    empty.className = 'empty';
    const msg = document.createElement('p');
    msg.textContent = 'Belum ada file yang diunduh.';
    empty.appendChild(msg);
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'btn btn-sm';
    btn.textContent = 'Buka Folder Unduhan';
    btn.onclick = openDownloadsFolder;
    empty.appendChild(btn);
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

    const statusLow = String(item.status || '').toLowerCase();
    const tag = document.createElement('span');
    tag.className = 'status-tag' + (statusLow.includes('selesai') || statusLow.includes('ok') || statusLow.includes('complete') ? ' done' : (statusLow.includes('gagal') || statusLow.includes('fail') ? ' err' : ''));
    tag.textContent = item.status;
    row.appendChild(main); row.appendChild(tag);
    listEl.appendChild(row);
  });
}
openFolderBtn.onclick = openDownloadsFolder;
clearBtn.onclick = async () => {
  clearErr();
  clearBtn.disabled = true;
  clearBtn.setAttribute('aria-busy', 'true');
  try {
    const res = await fetch('/api/downloads/clear', { method: 'POST', headers: {'X-Kurowser-Token':API_TOKEN} });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    ITEMS = [];
    render();
  } catch {
    showErr('Gagal membersihkan daftar unduhan.');
    render();
  } finally {
    clearBtn.removeAttribute('aria-busy');
  }
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

fn render_preset_group(presets: &[ThemePreset], current_preset: &str) -> String {
    let mut html = String::new();
    for &preset in presets {
        let mut s = Settings::default();
        s.theme_preset = preset;
        let pal = ThemePalette::from_settings(&s);
        let active = preset.id() == current_preset;
        let active_cls = if active { "theme-card active" } else { "theme-card" };
        let pressed = if active { "true" } else { "false" };
        html.push_str(&format!(
            r#"<button class="{active_cls}" data-preset="{id}" type="button" aria-pressed="{pressed}">
  <span class="swatch" style="background:{bg};border-color:{border}">
    <span class="swatch-surface" style="background:{app}"></span>
    <span class="swatch-dot" style="background:{acc}"></span>
  </span>
  <span>{label}</span>
</button>"#,
            id = preset.id(),
            label = preset.label(),
            bg = pal.bg_chrome.hex(),
            app = pal.bg_app.hex(),
            acc = pal.accent.hex(),
            border = pal.border.hex(),
        ));
    }
    html
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

    let dark_cards = render_preset_group(
        &[ThemePreset::Graphite, ThemePreset::NordicPine, ThemePreset::Espresso],
        current_preset,
    );
    let light_cards = render_preset_group(
        &[ThemePreset::Paper, ThemePreset::Mist, ThemePreset::Sand],
        current_preset,
    );

    let accent_swatches = [
        ("#0f766e", "Petrol Teal"),
        ("#1d4ed8", "Ocean Slate"),
        ("#b45309", "Warm Amber"),
    ];
    let mut chips_html = String::new();
    for (hex, name) in accent_swatches {
        let selected = current_accent.eq_ignore_ascii_case(hex);
        let active_cls = if selected { "chip active" } else { "chip" };
        let pressed = if selected { "true" } else { "false" };
        chips_html.push_str(&format!(
            r#"<button class="{active_cls}" data-accent="{hex}" type="button" aria-label="Pilih warna aksen {name}" aria-pressed="{pressed}" title="{name}"><span class="chip-dot" style="background:{hex}"></span></button>"#
        ));
    }

    let state_json = safe_json_for_script(&serde_json::json!({
        "theme_preset": current_preset,
        "custom_accent_hex": data.settings.custom_accent_hex,
        "search_engine": current_search,
        "max_live_tabs": max_tabs,
        "sidebar_hover_expand": data.settings.sidebar_hover_expand,
        "adblock_enabled": adblock_enabled,
        "adblock_cosmetic": adblock_cosmetic,
        "auto_update": data.settings.auto_update,
    }));
    let allowlist_json = safe_json_for_script(&data.settings.adblock_allowlist);
    let update_json = safe_json_for_script(&data.update_state.as_json());
    let token_json = safe_json_for_script(&data.api_token);

    let tpl = r#"<!DOCTYPE html>
<html lang="id"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Pengaturan - Kurowser</title>
<style>
__CSS__
.sec-title{font-size:16px;font-weight:600;margin:24px 0 12px;display:flex;align-items:center;gap:8px;flex-wrap:wrap}
.sec-sub{font-size:12.5px;font-weight:600;color:var(--muted);margin:14px 0 8px}
.themes-grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(168px,1fr));gap:10px;margin-bottom:14px}
.theme-card{
  display:flex;align-items:center;gap:10px;min-height:48px;padding:10px 12px;border-radius:10px;
  background:var(--bg-chrome);border:1px solid var(--border);color:var(--text);
  font-size:13.5px;font-weight:500;text-align:left;transition:background-color .14s,border-color .14s;
}
.theme-card:hover{background:var(--bg-hover);border-color:var(--accent-text)}
.theme-card.active{background:var(--bg-active);border-color:var(--accent-text);box-shadow:inset 0 0 0 1px var(--accent-text)}
.swatch{
  position:relative;width:28px;height:28px;border-radius:7px;border:1.5px solid;
  overflow:hidden;flex-shrink:0;display:flex;align-items:center;justify-content:center;
}
.swatch-surface{position:absolute;inset:0 0 0 50%;opacity:0.95}
.swatch-dot{position:relative;z-index:1;width:10px;height:10px;border-radius:50%}
.control-row{
  display:flex;align-items:center;justify-content:space-between;gap:16px;
  padding:14px 18px;border-bottom:1px solid var(--border-subtle);
}
.control-row:last-child{border-bottom:none}
.control-info{min-width:0;flex:1}
.control-row .row-title,.control-row .row-sub{white-space:normal;overflow:visible;text-overflow:clip}
.chips{display:flex;align-items:center;gap:4px;flex-wrap:wrap}
.chip{
  display:inline-flex;align-items:center;justify-content:center;
  width:44px;height:44px;min-width:44px;min-height:44px;
  padding:7px;border-radius:10px;border:1px solid transparent;background:transparent;
}
.chip:hover{background:var(--bg-hover)}
.chip.active{border-color:var(--focus-ring);background:var(--bg-active)}
.chip-dot{width:24px;height:24px;border-radius:50%;border:1.5px solid var(--border);display:block}
.color-picker{
  width:44px;height:44px;padding:4px;border-radius:10px;
  border:1px solid var(--border);background:var(--bg-surface);cursor:pointer;
}
.sel-box{
  min-height:44px;padding:8px 12px;border-radius:10px;background:var(--bg-surface);
  color:var(--text);border:1px solid var(--border);font-size:13.5px;
}
.switch-btn{
  display:inline-flex;align-items:center;justify-content:center;flex-shrink:0;
  width:52px;height:44px;padding:0;border:none;background:transparent;border-radius:10px;
}
.switch-track{
  position:relative;display:block;width:48px;height:26px;border-radius:13px;
  border:1.5px solid var(--border);background:var(--bg-surface);transition:background-color .16s,border-color .16s;
}
.switch-track::after{
  content:'';position:absolute;top:3px;left:3px;width:17px;height:17px;border-radius:50%;
  background:var(--muted);transition:transform .16s,background-color .16s;
}
.switch-btn.on .switch-track{background:var(--accent-soft);border-color:var(--accent-text)}
.switch-btn.on .switch-track::after{transform:translateX(22px);background:var(--accent-text)}
.status-text{font-size:12.5px;font-weight:600;color:var(--success-fg)}
.status-text.off{color:var(--muted)}
.allow-form{display:flex;gap:8px;width:min(360px,100%)}
.allow-input{
  flex:1;min-width:0;min-height:44px;padding:8px 12px;border-radius:10px;border:1px solid var(--border);
  background:var(--bg-surface);color:var(--text);font-size:13.5px;
}
.allow-input:focus-visible{border-color:var(--focus-ring);outline:2px solid var(--focus-ring);outline-offset:0}
.allow-chips{display:flex;flex-wrap:wrap;gap:8px;padding:4px 18px 16px}
.allow-pill{
  display:inline-flex;align-items:center;gap:6px;min-height:44px;padding:0 4px 0 12px;
  border-radius:10px;background:var(--bg-surface);border:1px solid var(--border-subtle);font-size:13px;
}
.allow-pill button{
  display:inline-flex;align-items:center;justify-content:center;
  min-width:44px;min-height:44px;background:none;border:none;color:var(--muted);
  font-size:16px;border-radius:8px;
}
.allow-pill button:hover{color:var(--danger-fg);background:var(--danger-bg)}
.toast{
  position:fixed;bottom:20px;right:24px;padding:11px 18px;border-radius:10px;
  background:var(--accent);color:var(--on-accent);font-size:13.5px;font-weight:600;
  opacity:0;transition:opacity .18s;pointer-events:none;
}
.toast.show{opacity:1}
@media (max-width:640px){
  .control-row{flex-direction:column;align-items:stretch;gap:12px;padding:14px}
  .allow-form{width:100%}
  .allow-chips{padding:4px 14px 14px}
}
</style></head>
<body>
__HEADER__
<main class="wrap">
  <div class="page-head">
    <h1>Pengaturan Kurowser</h1>
  </div>
  <div id="api-err" class="err-banner" role="alert"></div>

  <h2 class="sec-title">Tema & Tampilan</h2>
  <div class="sec-sub">Tema Gelap</div>
  <div class="themes-grid" id="presets-dark">__DARK_CARDS__</div>
  <div class="sec-sub">Tema Terang</div>
  <div class="themes-grid" id="presets-light">__LIGHT_CARDS__</div>

  <div class="card">
    <div class="control-row">
      <div class="control-info">
        <div class="row-title">Warna Aksen</div>
        <div class="row-sub">Menyesuaikan sorotan tab aktif, tombol utama, dan cincin fokus</div>
      </div>
      <div class="chips">
        __CHIPS_HTML__
        <label class="sr-only" for="accent-picker">Pilih warna aksen kustom</label>
        <input id="accent-picker" class="color-picker" type="color" value="__CURRENT_ACCENT__">
        <button id="accent-reset" class="btn btn-sm" type="button">Reset Bawaan</button>
      </div>
    </div>

    <div class="control-row">
      <div class="control-info">
        <label for="search-eng" class="row-title">Mesin Pencari Default</label>
        <div class="row-sub">Digunakan saat menulis kata pencarian di address bar</div>
      </div>
      <select id="search-eng" class="sel-box">
        <option value="google">Google</option>
        <option value="duckduckgo">DuckDuckGo</option>
      </select>
    </div>

    <div class="control-row">
      <div class="control-info">
        <label for="max-tabs" class="row-title">Batas Tab Aktif</label>
        <div class="row-sub">Tab lain akan tidur otomatis dan dimuat ulang saat dipilih kembali</div>
      </div>
      <select id="max-tabs" class="sel-box">
        <option value="2">2 Tab (Hemat Memori)</option>
        <option value="3">3 Tab (Seimbang)</option>
        <option value="4">4 Tab (Multitasking)</option>
      </select>
    </div>

    <div class="control-row">
      <div class="control-info">
        <div id="lbl-hover-sb" class="row-title">Melebarkan sidebar otomatis saat hover</div>
        <div class="row-sub">Sidebar menyusut ke rail ramping secara default dan melebar saat kursor diarahkan ke kiri</div>
      </div>
      <button id="toggle-hover-sb" class="switch-btn" type="button" aria-labelledby="lbl-hover-sb" aria-pressed="true">
        <span class="switch-track" aria-hidden="true"></span>
      </button>
    </div>
  </div>

  <h2 class="sec-title">__SVG_SHIELD__ Kurowser Shields <span id="shields-status-badge" class="status-text"></span></h2>
  <div class="card">
    <div class="control-row">
      <div class="control-info">
        <div id="lbl-adblock" class="row-title">Blokir Iklan & Tracker</div>
        <div class="row-sub">Memblokir script iklan, analytics, dan tracker pihak ketiga (__TOTAL_BLOCKED__ diblokir total)</div>
      </div>
      <button id="toggle-adblock" class="switch-btn" type="button" aria-labelledby="lbl-adblock" aria-pressed="false">
        <span class="switch-track" aria-hidden="true"></span>
      </button>
    </div>

    <div class="control-row">
      <div class="control-info">
        <div id="lbl-cosmetic" class="row-title">Sembunyikan Elemen Iklan</div>
        <div class="row-sub">Merapikan container banner kosong agar halaman tetap bersih</div>
      </div>
      <button id="toggle-cosmetic" class="switch-btn" type="button" aria-labelledby="lbl-cosmetic" aria-pressed="false">
        <span class="switch-track" aria-hidden="true"></span>
      </button>
    </div>

    <div class="control-row">
      <div class="control-info">
        <label for="allow-input" class="row-title">Situs Dikecualikan</label>
        <div class="row-sub">Shields nonaktif otomatis pada domain di bawah ini</div>
      </div>
      <form id="allow-form" class="allow-form">
        <input id="allow-input" class="allow-input" placeholder="contoh.com" autocomplete="off">
        <button id="allow-submit" class="btn btn-sm btn-accent" type="submit">Tambah</button>
      </form>
    </div>
    <div id="allow-chips" class="allow-chips"></div>
  </div>

  <h2 class="sec-title">Pembaruan Aplikasi</h2>
  <div class="card">
    <div class="control-row">
      <div class="control-info">
        <div id="lbl-autoupdate" class="row-title">Perbarui Kurowser otomatis</div>
        <div class="row-sub">Mengunduh pembaruan resmi dari GitHub Releases secara otomatis di latar belakang (diverifikasi SHA-256) dan menerapkan saat Kurowser ditutup.</div>
      </div>
      <button id="toggle-autoupdate" class="switch-btn" type="button" aria-labelledby="lbl-autoupdate" aria-pressed="false">
        <span class="switch-track" aria-hidden="true"></span>
      </button>
    </div>

    <div class="control-row">
      <div class="control-info">
        <div id="upd-title" class="row-title">Versi Kurowser</div>
        <div id="upd-sub" class="row-sub">Memeriksa status pembaruan...</div>
      </div>
      <div style="display:flex;gap:8px;flex-wrap:wrap;">
        <button id="btn-check-update" class="btn btn-sm" type="button">Periksa Pembaruan</button>
        <button id="btn-apply-update" class="btn btn-sm btn-accent" type="button" style="display:none">Perbarui &amp; Mulai Ulang</button>
      </div>
    </div>
  </div>
</main>
<div id="toast" class="toast" role="status" aria-live="polite">Pengaturan disimpan</div>

<script nonce="__CSP_NONCE__">
const API_TOKEN = __TOKEN_JSON__;
let state = __STATE_JSON__;
let allowlist = __ALLOWLIST_JSON__;
let updateState = __UPDATE_JSON__;

const errEl = document.getElementById('api-err');
function showErr(msg) {
  errEl.textContent = msg;
  errEl.classList.add('open');
}
function clearErr() {
  errEl.textContent = '';
  errEl.classList.remove('open');
}

document.getElementById('search-eng').value = state.search_engine;
document.getElementById('max-tabs').value = String(state.max_live_tabs);

function renderSidebarHoverUi() {
  const btn = document.getElementById('toggle-hover-sb');
  if (!btn) return;
  const on = state.sidebar_hover_expand !== false;
  btn.classList.toggle('on', on);
  btn.setAttribute('aria-pressed', String(on));
}

function showToast(msg) {
  const t = document.getElementById('toast');
  if (msg) t.textContent = msg;
  t.classList.add('show');
  setTimeout(() => t.classList.remove('show'), 1500);
}

function renderShieldUi() {
  const abBtn = document.getElementById('toggle-adblock');
  const cosBtn = document.getElementById('toggle-cosmetic');
  const badge = document.getElementById('shields-status-badge');
  abBtn.classList.toggle('on', !!state.adblock_enabled);
  abBtn.setAttribute('aria-pressed', String(!!state.adblock_enabled));
  cosBtn.classList.toggle('on', !!state.adblock_cosmetic);
  cosBtn.setAttribute('aria-pressed', String(!!state.adblock_cosmetic));
  badge.textContent = state.adblock_enabled ? 'Aktif' : 'Nonaktif';
  badge.classList.toggle('off', !state.adblock_enabled);

  const box = document.getElementById('allow-chips');
  box.innerHTML = '';
  if (!allowlist.length) {
    const span = document.createElement('span');
    span.className = 'row-sub';
    span.textContent = 'Belum ada situs yang dikecualikan.';
    box.appendChild(span);
    return;
  }
  allowlist.forEach(host => {
    const pill = document.createElement('span');
    pill.className = 'allow-pill';
    const lbl = document.createElement('span');
    lbl.textContent = host;
    const del = document.createElement('button');
    del.type = 'button';
    del.textContent = '×';
    del.setAttribute('aria-label', 'Hapus ' + host + ' dari pengecualian');
    del.title = 'Hapus dari pengecualian';
    del.addEventListener('click', async () => {
      clearErr();
      del.disabled = true;
      try {
        const res = await fetch('/api/shields/allowlist/remove', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN },
          body: JSON.stringify({ host })
        });
        if (!res.ok) throw new Error('HTTP ' + res.status);
        const out = await res.json();
        if (Array.isArray(out.allowlist)) allowlist = out.allowlist;
        renderShieldUi();
        showToast('Pengecualian dihapus');
      } catch {
        del.disabled = false;
        showErr('Gagal menghapus domain pengecualian. Silakan coba lagi.');
      }
    });
    pill.appendChild(lbl);
    pill.appendChild(del);
    box.appendChild(pill);
  });
}

async function save(reloadAfter = true) {
  clearErr();
  try {
    const res = await fetch('/api/settings', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN },
      body: JSON.stringify(state)
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    showToast('Pengaturan disimpan');
    if (reloadAfter) setTimeout(() => location.reload(), 180);
    return true;
  } catch {
    showErr('Gagal menyimpan pengaturan. Silakan coba lagi.');
    return false;
  }
}

document.querySelectorAll('.theme-card').forEach(btn => {
  btn.addEventListener('click', async () => {
    const prevPreset = state.theme_preset;
    const prevAccent = state.custom_accent_hex;
    state.theme_preset = btn.dataset.preset;
    state.custom_accent_hex = null;
    if (!(await save(true))) {
      state.theme_preset = prevPreset;
      state.custom_accent_hex = prevAccent;
    }
  });
});

document.querySelectorAll('.chip[data-accent]').forEach(chip => {
  chip.addEventListener('click', async () => {
    const prev = state.custom_accent_hex;
    state.custom_accent_hex = chip.dataset.accent;
    if (!(await save(true))) state.custom_accent_hex = prev;
  });
});

document.getElementById('accent-reset').addEventListener('click', async () => {
  const prev = state.custom_accent_hex;
  state.custom_accent_hex = null;
  if (!(await save(true))) state.custom_accent_hex = prev;
});

document.getElementById('accent-picker').addEventListener('change', async e => {
  const prev = state.custom_accent_hex;
  state.custom_accent_hex = e.target.value;
  if (!(await save(true))) state.custom_accent_hex = prev;
});

document.getElementById('search-eng').addEventListener('change', async e => {
  const prev = state.search_engine;
  state.search_engine = e.target.value;
  if (!(await save(true))) {
    state.search_engine = prev;
    e.target.value = prev;
  }
});

document.getElementById('max-tabs').addEventListener('change', async e => {
  const prev = state.max_live_tabs;
  state.max_live_tabs = parseInt(e.target.value, 10) || 3;
  if (!(await save(true))) {
    state.max_live_tabs = prev;
    e.target.value = String(prev);
  }
});

document.getElementById('toggle-hover-sb').addEventListener('click', async () => {
  const prev = state.sidebar_hover_expand !== false;
  state.sidebar_hover_expand = !prev;
  renderSidebarHoverUi();
  if (!(await save(false))) {
    state.sidebar_hover_expand = prev;
    renderSidebarHoverUi();
  }
});

document.getElementById('toggle-adblock').addEventListener('click', async () => {
  const prev = state.adblock_enabled;
  state.adblock_enabled = !prev;
  renderShieldUi();
  if (!(await save(false))) {
    state.adblock_enabled = prev;
    renderShieldUi();
  }
});

document.getElementById('toggle-cosmetic').addEventListener('click', async () => {
  const prev = state.adblock_cosmetic;
  state.adblock_cosmetic = !prev;
  renderShieldUi();
  if (!(await save(false))) {
    state.adblock_cosmetic = prev;
    renderShieldUi();
  }
});

document.getElementById('allow-form').addEventListener('submit', async e => {
  e.preventDefault();
  clearErr();
  const inp = document.getElementById('allow-input');
  const sub = document.getElementById('allow-submit');
  const host = inp.value.trim();
  if (!host) return;
  sub.disabled = true;
  try {
    const res = await fetch('/api/shields/allowlist/add', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN },
      body: JSON.stringify({ host })
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    const out = await res.json();
    if (Array.isArray(out.allowlist)) allowlist = out.allowlist;
    inp.value = '';
    renderShieldUi();
    showToast('Situs ditambahkan ke pengecualian');
  } catch {
    showErr('Gagal menambahkan domain pengecualian. Silakan coba lagi.');
  } finally {
    sub.disabled = false;
  }
});

function renderUpdateUi() {
  const autoBtn = document.getElementById('toggle-autoupdate');
  autoBtn.classList.toggle('on', !!state.auto_update);
  autoBtn.setAttribute('aria-pressed', String(!!state.auto_update));

  const titleEl = document.getElementById('upd-title');
  const subEl = document.getElementById('upd-sub');
  const chkBtn = document.getElementById('btn-check-update');
  const applyBtn = document.getElementById('btn-apply-update');

  const modeLabel = updateState.installed ? 'Terinstal' : 'Portabel';
  titleEl.textContent = 'Kurowser v' + (updateState.current_version || '1.1.1') + ' (' + modeLabel + ')';

  const st = updateState.status || 'idle';
  applyBtn.style.display = st === 'ready' ? '' : 'none';
  chkBtn.disabled = st === 'checking' || st === 'downloading';

  if (st === 'checking') {
    subEl.textContent = 'Memeriksa rilis terbaru di GitHub...';
  } else if (st === 'downloading') {
    subEl.textContent = 'Mengunduh & memverifikasi Kurowser v' + (updateState.version || '') + '...';
  } else if (st === 'ready') {
    subEl.textContent = 'Kurowser v' + (updateState.version || '') + ' siap (SHA-256 terverifikasi). Akan diterapkan otomatis saat ditutup atau klik tombol di kanan.';
  } else if (st === 'up_to_date') {
    subEl.textContent = 'Sudah menggunakan versi terbaru.';
  } else if (st === 'failed') {
    subEl.textContent = updateState.error || 'Gagal memeriksa pembaruan.';
  } else {
    subEl.textContent = 'Pembaruan resmi dicek otomatis dari GitHub Releases.';
  }
}

let pollTimer = null;
async function refreshUpdateStatus() {
  try {
    const res = await fetch('/api/update/status', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN },
      body: '{}'
    });
    if (!res.ok) return;
    updateState = await res.json();
    renderUpdateUi();
    if (updateState.status === 'checking' || updateState.status === 'downloading') {
      if (!pollTimer) {
        pollTimer = setInterval(refreshUpdateStatus, 1100);
      }
    } else if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  } catch {}
}

document.getElementById('toggle-autoupdate').addEventListener('click', async () => {
  const prev = state.auto_update;
  state.auto_update = !prev;
  renderUpdateUi();
  if (!(await save(false))) {
    state.auto_update = prev;
    renderUpdateUi();
  }
});

document.getElementById('btn-check-update').addEventListener('click', async () => {
  clearErr();
  updateState.status = 'checking';
  renderUpdateUi();
  try {
    const res = await fetch('/api/update/check', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN },
      body: '{}'
    });
    if (!res.ok) throw new Error('HTTP ' + res.status);
    updateState = await res.json();
    renderUpdateUi();
    if (!pollTimer) pollTimer = setInterval(refreshUpdateStatus, 1100);
  } catch {
    showErr('Gagal memulai pemeriksaan pembaruan.');
  }
});

document.getElementById('btn-apply-update').addEventListener('click', async () => {
  clearErr();
  const btn = document.getElementById('btn-apply-update');
  btn.disabled = true;
  try {
    await fetch('/api/update/apply', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json', 'X-Kurowser-Token': API_TOKEN },
      body: '{}'
    });
  } catch {
    btn.disabled = false;
    showErr('Gagal menerapkan pembaruan.');
  }
});

renderSidebarHoverUi();
renderShieldUi();
renderUpdateUi();
if (updateState.status === 'checking' || updateState.status === 'downloading') {
  pollTimer = setInterval(refreshUpdateStatus, 1100);
}
</script></body></html>"#;

    tpl.replace("__CSS__", &css)
        .replace("__HEADER__", &header)
        .replace("__DARK_CARDS__", &dark_cards)
        .replace("__LIGHT_CARDS__", &light_cards)
        .replace("__CHIPS_HTML__", &chips_html)
        .replace("__CURRENT_ACCENT__", &current_accent)
        .replace("__SVG_SHIELD__", SVG_SHIELD)
        .replace("__TOTAL_BLOCKED__", &total_blocked.to_string())
        .replace("__CSP_NONCE__", csp_nonce)
        .replace("__TOKEN_JSON__", &token_json)
        .replace("__STATE_JSON__", &state_json)
        .replace("__ALLOWLIST_JSON__", &allowlist_json)
        .replace("__UPDATE_JSON__", &update_json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_pages_meet_structure_and_copy_invariants() {
        for preset in [ThemePreset::Graphite, ThemePreset::Paper] {
            let mut data = BrowserData::default();
            data.settings.theme_preset = preset;
            let pages = [
                ("start", render_start_page(&data, "testnonce")),
                ("history", render_history_page(&data, "testnonce")),
                ("bookmarks", render_bookmarks_page(&data, "testnonce")),
                ("downloads", render_downloads_page(&data, "testnonce")),
                ("settings", render_settings_page(&data, "testnonce")),
            ];
            for (name, html) in pages {
                assert!(
                    !html.contains('\u{2014}'),
                    "page {name} contains forbidden em dash"
                );
                assert!(html.contains(r#"<meta name="viewport""#), "{name} missing viewport");
                assert!(html.contains("<main"), "{name} missing main landmark");
                assert!(html.contains(":focus-visible"), "{name} missing focus-visible");
                assert!(
                    html.contains("prefers-reduced-motion"),
                    "{name} missing reduced motion rule"
                );
                assert!(html.contains(r#"role="alert""#), "{name} missing alert container");
            }
        }
    }
}
