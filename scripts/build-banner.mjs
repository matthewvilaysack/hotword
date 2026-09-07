// Generates the banner. site/banner.svg has the wordmark and taglines as
// paths (so it needs no fonts), a deep-blue-to-orange ground, and a CSS glitch
// that tears the wordmark every few seconds. site/banner.png is one torn frame
// rasterized for social previews. Fonts are fetched once into .cache/fonts.
// Run: bun scripts/build-banner.mjs
import { mkdir, readFile, writeFile, stat } from "node:fs/promises";
import opentype from "opentype.js";
import { Resvg } from "@resvg/resvg-js";

const W = 1280, H = 640;
const FONTS = {
  display: { family: "Big+Shoulders+Display:wght@900", file: ".cache/fonts/bigshoulders-900.ttf" },
  mono: { family: "IBM+Plex+Mono:wght@400", file: ".cache/fonts/plexmono-400.ttf" },
};

async function fetchFont({ family, file }) {
  try { await stat(file); return file; } catch {}
  const css = await (await fetch(`https://fonts.googleapis.com/css2?family=${family}`, { headers: { "User-Agent": "curl" } })).text();
  const url = css.match(/url\((https:[^)]+\.ttf)\)/)?.[1];
  if (!url) throw new Error(`no ttf url for ${family}`);
  await mkdir(".cache/fonts", { recursive: true });
  await writeFile(file, Buffer.from(await (await fetch(url)).arrayBuffer()));
  return file;
}

async function load(spec) {
  const buf = await readFile(await fetchFont(spec));
  return opentype.parse(buf.buffer.slice(buf.byteOffset, buf.byteOffset + buf.byteLength));
}

const path = (font, text, x, y, size, spacing = 0) =>
  font.getPath(text, x, y, size, { letterSpacing: spacing / size, kerning: true }).toPathData(1);

const display = await load(FONTS.display);
const mono = await load(FONTS.mono);

const word = path(display, "hotword", 88, 400, 300, -3);
const line1 = path(mono, "Say the phrase. The shell runs first.", 92, 470, 30);
const line2 = path(mono, "Phrase-triggered and session-start workflows for terminal coding agents.", 92, 520, 20);
const prompt = path(mono, "$ claude  >  ", 92, 586, 20);
const promptW = mono.getAdvanceWidth("$ claude  >  ", 20);
const phrase = path(mono, "review pr 42", 92 + promptW, 586, 20);
const urlText = "github.com/matthewvilaysack/hotword";
const url = path(mono, urlText, 1188 - mono.getAdvanceWidth(urlText, 20), 586, 20);

const ANIM = `    /* One tear every six seconds, hard cuts, then rest. */
    .a { animation: ta 6s steps(1, end) infinite; }
    .b { animation: tb 6s steps(1, end) infinite; }
    .c { animation: tc 6s steps(1, end) infinite; }
    .d { animation: td 6s steps(1, end) infinite; }
    .base { animation: shake 6s steps(1, end) infinite; }
    .sweep { animation: sweep 9s linear infinite; }
    @keyframes ta { 0%, 70% { transform: translateX(0); opacity: 0; } 71% { transform: translateX(-22px); opacity: .9; } 74% { transform: translateX(14px); } 77% { transform: translateX(-8px); } 80%, 100% { transform: translateX(0); opacity: 0; } }
    @keyframes tb { 0%, 72% { transform: translateX(0); opacity: 0; } 73% { transform: translateX(26px); opacity: .9; } 76% { transform: translateX(-12px); } 79%, 100% { transform: translateX(0); opacity: 0; } }
    @keyframes tc { 0%, 71% { transform: translateX(0); opacity: 0; } 72% { transform: translateX(16px); opacity: .9; } 75% { transform: translateX(-20px); } 78% { transform: translateX(6px); } 81%, 100% { transform: translateX(0); opacity: 0; } }
    @keyframes td { 0%, 74% { transform: translateX(0); opacity: 0; } 75% { transform: translateX(-30px); opacity: .9; } 77% { transform: translateX(10px); } 79%, 100% { transform: translateX(0); opacity: 0; } }
    @keyframes shake { 0%, 71% { transform: translate(0,0); } 72% { transform: translate(-3px,1px); } 75% { transform: translate(2px,-1px); } 78% { transform: translate(-1px,0); } 80%, 100% { transform: translate(0,0); } }
    @keyframes sweep { 0% { transform: translateY(-40px); } 100% { transform: translateY(${H + 40}px); } }
    @media (prefers-reduced-motion: reduce) { .a, .b, .c, .d, .base, .sweep { animation: none; } .a, .b, .c, .d { opacity: .9; } }`;

const render = (still) => `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="hotword: say the phrase, the shell runs first">
  <title>hotword</title>
  <defs>
    <linearGradient id="sky" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" stop-color="#0a1b5c"/>
      <stop offset="0.42" stop-color="#3b1f8f"/>
      <stop offset="0.66" stop-color="#7a2ea8"/>
      <stop offset="0.86" stop-color="#e2572a"/>
      <stop offset="1" stop-color="#ff8a1f"/>
    </linearGradient>
    <radialGradient id="glow" cx="0.82" cy="0.9" r="0.6">
      <stop offset="0" stop-color="#ffb347" stop-opacity="0.55"/>
      <stop offset="1" stop-color="#ffb347" stop-opacity="0"/>
    </radialGradient>
    <radialGradient id="deep" cx="0.1" cy="0.1" r="0.7">
      <stop offset="0" stop-color="#061043" stop-opacity="0.7"/>
      <stop offset="1" stop-color="#061043" stop-opacity="0"/>
    </radialGradient>
    <pattern id="scan" width="4" height="4" patternUnits="userSpaceOnUse">
      <rect width="4" height="1" fill="#000" fill-opacity="0.16"/>
    </pattern>
    <clipPath id="a"><rect x="0" y="228" width="${W}" height="26"/></clipPath>
    <clipPath id="b"><rect x="0" y="318" width="${W}" height="14"/></clipPath>
    <clipPath id="c"><rect x="0" y="372" width="${W}" height="34"/></clipPath>
    <clipPath id="d"><rect x="0" y="160" width="${W}" height="18"/></clipPath>
  </defs>
  <style>
    ${still ? "" : ANIM}
    .w { fill: #f6f2ff; }
    .s { opacity: .9; }
    .peach { fill: #ffd9a3; }
    .ice { fill: #9ad0ff; }
    .t { fill: #f6f2ff; }
    .dim { fill: #f6f2ff; fill-opacity: .78; }
    .hot { fill: #ffb347; }
  </style>
  <rect width="${W}" height="${H}" fill="url(#sky)"/>
  <rect width="${W}" height="${H}" fill="url(#deep)"/>
  <rect width="${W}" height="${H}" fill="url(#glow)"/>
  <rect width="${W}" height="${H}" fill="url(#scan)"/>
  <rect class="sweep" width="${W}" height="28" fill="#fff" fill-opacity="0.05" transform="translate(0 ${still ? 250 : 0})"/>
  <g class="a s peach" clip-path="url(#a)"${still ? ' transform="translate(-22 0)"' : ''}><path d="${word}"/></g>
  <g class="b s ice" clip-path="url(#b)"${still ? ' transform="translate(26 0)"' : ''}><path d="${word}"/></g>
  <g class="c s peach" clip-path="url(#c)"${still ? ' transform="translate(16 0)"' : ''}><path d="${word}"/></g>
  <g class="d s ice" clip-path="url(#d)"${still ? ' transform="translate(-30 0)"' : ''}><path d="${word}"/></g>
  <g class="base"${still ? ' transform="translate(-3 1)"' : ''}><path class="w" d="${word}"/></g>
  <path class="t" d="${line1}"/>
  <path class="dim" d="${line2}"/>
  <path class="t" d="${prompt}"/>
  <path class="hot" d="${phrase}"/>
  <path class="dim" d="${url}"/>
</svg>
`;
const animated = render(false);
await writeFile("site/banner.svg", animated);
const png = new Resvg(render(true), { fitTo: { mode: "width", value: W } }).render().asPng();
await writeFile("site/banner.png", png);
console.log(`site/banner.svg ${animated.length} bytes, site/banner.png ${png.length} bytes`);
