// Generates the demo app the harness measures and mutates, plus the marker-based
// defer/undefer codemod that implements the loop's "lazy-load this module" step.
//
// Design constraints that keep coverage-driven candidate detection honest:
// - All heavy weight sits INSIDE function bodies, never in top-level literals. V8
//   precise coverage marks a module's top level as executed the moment it evaluates,
//   so top-level data would look "used before paint" even when nothing reads it;
//   function-body weight cleanly separates "parsed" from "ran before paint".
// - The page is client-rendered: LCP is the hero <h1> painted by main.ts, so entry
//   chunk download + parse directly gates LCP.
// - Every feature reports readiness on window.__ready, so a measurement can assert
//   that a defer did not break the page (the guard) before accepting it.
// - hero_data is the trap: structurally deferrable (it has a marker block) but its
//   bytes execute before first paint — coverage must exclude it, not the harness.

import fs from 'node:fs';
import path from 'node:path';

// Feature registry: marker-block templates for baseline (static import) and
// deferred (dynamic import after first paint) forms. `kb` ~= generated source KB.
export const FEATURES = {
  hero_data: {
    kb: 25,
    seed: 505,
    baseline: [
      'import { heroSubtitle } from "./features/hero_data";',
      'setHeroSubtitle(heroSubtitle());',
      'readyFlags.hero_data = true;',
    ],
    deferred: [
      'void import("./features/hero_data").then((m) => {',
      '  setHeroSubtitle(m.heroSubtitle());',
      '  readyFlags.hero_data = true;',
      '});',
    ],
  },
  charts: {
    kb: 150,
    seed: 101,
    baseline: [
      'import { initCharts } from "./features/charts";',
      'scheduleInit("charts", initCharts);',
    ],
    deferred: [
      'scheduleInit("charts", () => import("./features/charts").then((m) => m.initCharts()));',
    ],
  },
  markdown: {
    kb: 110,
    seed: 202,
    baseline: [
      'import { initMarkdown } from "./features/markdown";',
      'scheduleInit("markdown", initMarkdown);',
    ],
    deferred: [
      'scheduleInit("markdown", () => import("./features/markdown").then((m) => m.initMarkdown()));',
    ],
  },
  analytics: {
    kb: 60,
    seed: 303,
    baseline: [
      'import { initAnalytics } from "./features/analytics";',
      'scheduleInit("analytics", initAnalytics);',
    ],
    deferred: [
      'scheduleInit("analytics", () => import("./features/analytics").then((m) => m.initAnalytics()));',
    ],
  },
  badges: {
    kb: 4,
    seed: 404,
    baseline: [
      'import { initBadges } from "./features/badges";',
      'scheduleInit("badges", initBadges);',
    ],
    deferred: [
      'scheduleInit("badges", () => import("./features/badges").then((m) => m.initBadges()));',
    ],
  },
};

export const FEATURE_NAMES = Object.keys(FEATURES);

// Deterministic PRNG so regenerated apps are byte-identical.
function mulberry32(seed) {
  return function next() {
    seed |= 0;
    seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const ALPHABET = 'abcdefghijklmnopqrstuvwxyz0123456789';

// ~1KB per block function: a string table plus a mixing loop. ASCII only, so V8
// coverage offsets (UTF-16 units) equal byte offsets in the emitted chunk.
function heavyFunctions(name, kb, seed) {
  const rand = mulberry32(seed);
  const parts = [];
  for (let i = 0; i < kb; i++) {
    const words = [];
    for (let w = 0; w < 24; w++) {
      let s = '';
      for (let c = 0; c < 24; c++) s += ALPHABET[Math.floor(rand() * ALPHABET.length)];
      words.push('"' + s + '"');
    }
    parts.push(
      `export function ${name}_block_${i}(x: number): number {\n`
      + `  const table = [${words.join(', ')}];\n`
      + '  let acc = x | 0;\n'
      + '  for (let i = 0; i < table.length; i++) {\n'
      + '    const s = table[i];\n'
      + '    for (let j = 0; j < s.length; j++) {\n'
      + `      acc = (acc * 31 + s.charCodeAt(j) + ${i}) | 0;\n`
      + '    }\n'
      + '  }\n'
      + '  return acc;\n'
      + '}\n',
    );
  }
  const calls = [];
  for (let i = 0; i < kb; i++) calls.push(`  acc = (acc + ${name}_block_${i}(acc)) | 0;`);
  parts.push(
    `export function ${name}_run_all(seed: number): number {\n`
    + '  let acc = seed | 0;\n'
    + `${calls.join('\n')}\n`
    + '  return acc;\n'
    + '}\n',
  );
  return parts.join('\n');
}

const INDEX_HTML = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<title>Quokka Analytics - demo storefront</title>
<style>
  body { margin: 0; font-family: system-ui, sans-serif; color: #1a2233; }
  nav { display: flex; gap: 24px; padding: 18px 32px; border-bottom: 1px solid #e3e8f0; }
  nav a { color: #45526b; text-decoration: none; }
  #hero { padding: 72px 32px 48px; max-width: 900px; }
  #hero-title { font-size: 52px; line-height: 1.1; margin: 0 0 16px; min-height: 114px; }
  #hero-subtitle { font-size: 20px; color: #45526b; margin: 0 0 24px; min-height: 26px; }
  #hero-cta { font-size: 18px; padding: 12px 28px; }
  .below { margin: 900px 32px 40px; }
  #chart-slot { display: flex; align-items: flex-end; gap: 3px; height: 120px; }
  #chart-slot .bar { width: 10px; background: #4a6cf7; }
  #md-slot { min-height: 40px; color: #45526b; }
  .badge { background: #e8ecff; padding: 2px 8px; border-radius: 10px; font-size: 12px; }
</style>
</head>
<body>
<div id="app"></div>
<script type="module" src="./main.js"></script>
</body>
</html>
`;

const BOOT_TS = `// Shared boot utilities: post-paint scheduling and per-feature readiness flags.
export const readyFlags: Record<string, boolean> =
  ((window as unknown as { __ready: Record<string, boolean> }).__ready = {});

/**
 * Run a side-band feature init strictly after first paint: window load, two
 * animation frames (paint has happened), then a tick. Both the static-import
 * baseline and the dynamic-import deferred form go through this same path, so a
 * measurement compares only download/parse cost, never init scheduling.
 */
export function scheduleInit(name: string, fn: () => unknown): void {
  readyFlags[name] = false;
  const run = () => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        setTimeout(() => {
          void Promise.resolve()
            .then(fn)
            .catch((err) => console.error("[init:" + name + "]", err))
            .finally(() => {
              readyFlags[name] = true;
            });
        }, 50);
      });
    });
  };
  if (document.readyState === "complete") run();
  else window.addEventListener("load", run, { once: true });
}
`;

function i18nTs() {
  return `// Locale layer. The string table is built INSIDE a function, but that function
// runs at startup (before first paint) because the hero needs translated copy.
// Coverage therefore reports this module as used-before-paint: not a defer candidate.
${heavyFunctions('i18n', 40, 606)}
const strings = new Map<string, string>();

export function initI18n(): void {
  const checksum = i18n_run_all(11);
  strings.set("hero.title", "Ship faster with Quokka Analytics");
  strings.set("hero.cta", "Start free trial");
  strings.set("footer.note", "chk:" + (checksum >>> 0).toString(16));
}

export function t(key: string): string {
  return strings.get(key) ?? key;
}
`;
}

const RENDER_TS = `// Critical-path rendering: builds the shell and paints the hero (the LCP element).
export type Translate = (key: string) => string;

export function renderShell(): void {
  const app = document.getElementById("app");
  if (!app) return;
  app.textContent = "";

  const nav = document.createElement("nav");
  for (const label of ["Quokka", "Docs", "Pricing", "Blog"]) {
    const a = document.createElement("a");
    a.href = "#";
    a.textContent = label;
    nav.appendChild(a);
  }

  const hero = document.createElement("section");
  hero.id = "hero";
  const title = document.createElement("h1");
  title.id = "hero-title";
  const subtitle = document.createElement("p");
  subtitle.id = "hero-subtitle";
  const cta = document.createElement("button");
  cta.id = "hero-cta";
  hero.append(title, subtitle, cta);

  const below = document.createElement("section");
  below.className = "below";
  const chartSlot = document.createElement("div");
  chartSlot.id = "chart-slot";
  const mdBtn = document.createElement("button");
  mdBtn.id = "md-btn";
  mdBtn.textContent = "Render changelog";
  const mdSlot = document.createElement("div");
  mdSlot.id = "md-slot";
  const badgeSlot = document.createElement("div");
  badgeSlot.id = "badge-slot";
  below.append(chartSlot, mdBtn, mdSlot, badgeSlot);

  app.append(nav, hero, below);
}

export function renderHero(t: Translate): void {
  const title = document.getElementById("hero-title");
  if (title) title.textContent = t("hero.title");
  const cta = document.getElementById("hero-cta");
  if (cta) cta.textContent = t("hero.cta");
}

export function setHeroSubtitle(text: string): void {
  const subtitle = document.getElementById("hero-subtitle");
  if (subtitle) subtitle.textContent = text;
}
`;

function featureTs(name) {
  const heavy = heavyFunctions(name, FEATURES[name].kb, FEATURES[name].seed);
  const inits = {
    hero_data: `export function heroSubtitle(): string {
  const n = hero_data_run_all(3);
  return "Handcrafted insights, checksum " + (n >>> 0).toString(16) + ", refreshed daily.";
}
`,
    charts: `export function initCharts(): void {
  const slot = document.getElementById("chart-slot");
  if (!slot) return;
  let acc = 7;
  for (let i = 0; i < 40; i++) {
    acc = Math.abs(charts_run_all(acc + i)) % 96;
    const bar = document.createElement("div");
    bar.className = "bar";
    bar.style.height = (4 + acc) + "px";
    slot.appendChild(bar);
  }
}
`,
    markdown: `export function initMarkdown(): void {
  const btn = document.getElementById("md-btn");
  const slot = document.getElementById("md-slot");
  if (!btn || !slot) return;
  btn.addEventListener("click", () => {
    // The heavy path runs only on user interaction, never during load.
    const n = markdown_run_all(42);
    slot.textContent = "rendered:" + (n >>> 0).toString(16);
  });
}
`,
    analytics: `export function initAnalytics(): void {
  const n = analytics_run_all(1234);
  document.documentElement.dataset.analytics = (n >>> 0).toString(16);
}
`,
    badges: `export function initBadges(): void {
  const slot = document.getElementById("badge-slot");
  if (!slot) return;
  const n = badges_run_all(9);
  const badge = document.createElement("span");
  badge.className = "badge";
  badge.textContent = "v" + (Math.abs(n) % 10) + "." + (Math.abs(n >> 4) % 10);
  slot.appendChild(badge);
}
`,
  };
  return heavy + '\n' + inits[name];
}

function featureBlock(name, mode) {
  return [
    `// <feature:${name}>`,
    ...FEATURES[name][mode],
    `// </feature:${name}>`,
  ].join('\n');
}

function mainTs() {
  const blocks = FEATURE_NAMES.map((name) => featureBlock(name, 'baseline')).join('\n\n');
  return `import { readyFlags, scheduleInit } from "./boot";
import { initI18n, t } from "./i18n";
import { renderHero, renderShell, setHeroSubtitle } from "./render";

initI18n();
renderShell();
renderHero(t);

${blocks}

console.log("[app] booted");
`;
}

export function generateApp(appDir, { force = false } = {}) {
  const mainPath = path.join(appDir, 'src', 'main.ts');
  if (fs.existsSync(mainPath) && !force) {
    return { written: false, reason: 'app already exists (use --force to regenerate)' };
  }
  fs.rmSync(appDir, { recursive: true, force: true });
  fs.mkdirSync(path.join(appDir, 'src', 'features'), { recursive: true });
  fs.writeFileSync(path.join(appDir, 'index.html'), INDEX_HTML);
  fs.writeFileSync(path.join(appDir, 'src', 'boot.ts'), BOOT_TS);
  fs.writeFileSync(path.join(appDir, 'src', 'i18n.ts'), i18nTs());
  fs.writeFileSync(path.join(appDir, 'src', 'render.ts'), RENDER_TS);
  fs.writeFileSync(mainPath, mainTs());
  for (const name of FEATURE_NAMES) {
    fs.writeFileSync(path.join(appDir, 'src', 'features', `${name}.ts`), featureTs(name));
  }
  return { written: true };
}

/** Rewrite one feature's marker block in main.ts to its baseline or deferred form. */
export function setFeatureMode(appDir, feature, mode) {
  if (!FEATURES[feature]) {
    throw new Error(`unknown feature "${feature}" (known: ${FEATURE_NAMES.join(', ')})`);
  }
  if (mode !== 'baseline' && mode !== 'deferred') throw new Error(`bad mode "${mode}"`);
  const mainPath = path.join(appDir, 'src', 'main.ts');
  const src = fs.readFileSync(mainPath, 'utf8');
  const open = `// <feature:${feature}>`;
  const close = `// </feature:${feature}>`;
  const start = src.indexOf(open);
  const end = src.indexOf(close);
  if (start < 0 || end < 0) throw new Error(`feature block markers for "${feature}" not found in main.ts`);
  const next = src.slice(0, start) + featureBlock(feature, mode) + src.slice(end + close.length);
  if (next !== src) fs.writeFileSync(mainPath, next);
  return { changed: next !== src };
}

/** Parse main.ts marker blocks -> { feature: 'baseline' | 'deferred' }. */
export function featureModes(appDir) {
  const mainPath = path.join(appDir, 'src', 'main.ts');
  if (!fs.existsSync(mainPath)) return null;
  const src = fs.readFileSync(mainPath, 'utf8');
  const modes = {};
  for (const name of FEATURE_NAMES) {
    const open = `// <feature:${name}>`;
    const close = `// </feature:${name}>`;
    const start = src.indexOf(open);
    const end = src.indexOf(close);
    if (start < 0 || end < 0) {
      modes[name] = 'missing';
      continue;
    }
    const block = src.slice(start + open.length, end);
    modes[name] = block.includes(`import("./features/${name}")`) ? 'deferred' : 'baseline';
  }
  return modes;
}
