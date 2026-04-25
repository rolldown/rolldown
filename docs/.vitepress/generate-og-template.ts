import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const DIR = fileURLToPath(new URL('.', import.meta.url));
const TEMPLATE_PATH = path.join(DIR, 'og-template.svg');
const FONT_PATH = path.join(DIR, 'fonts', 'Inter-VariableFont_opsz,wght.ttf');
const CACHE_DIR = path.join(DIR, 'cache');
const OUTPUT_PATH = path.join(CACHE_DIR, 'og-template.generated.svg');

interface CacheEntry {
  path: string;
  templateMtimeMs: number;
  fontMtimeMs: number;
}

let cache: CacheEntry | undefined;

/**
 * Produce a copy of `og-template.svg` with the Inter font (SIL OFL 1.1,
 * see ./fonts/OFL.txt) embedded as a base64 `@font-face`, so OG image
 * rendering via `sharp`/librsvg is deterministic on hosts without Inter
 * installed system-wide (e.g. Linux CI).
 */
export function prepareOgTemplateWithFont(): string {
  const templateMtimeMs = statSync(TEMPLATE_PATH).mtimeMs;
  const fontMtimeMs = statSync(FONT_PATH).mtimeMs;

  if (
    cache &&
    cache.templateMtimeMs === templateMtimeMs &&
    cache.fontMtimeMs === fontMtimeMs &&
    existsSync(cache.path)
  ) {
    return cache.path;
  }

  const svg = readFileSync(TEMPLATE_PATH, 'utf-8');
  const fontBase64 = readFileSync(FONT_PATH).toString('base64');

  const fontFace =
    `<defs><style type="text/css">` +
    `@font-face{` +
    `font-family:'Inter';` +
    `font-style:normal;` +
    `font-weight:100 900;` +
    `src:url('data:font/ttf;base64,${fontBase64}') format('truetype');` +
    `}` +
    `</style></defs>`;

  // SVG allows multiple <defs>; inject after the root tag to leave the
  // original <defs> (filter definitions) untouched.
  const injected = svg.replace(/(<svg\b[^>]*>)/, `$1${fontFace}`);

  mkdirSync(CACHE_DIR, { recursive: true });
  writeFileSync(OUTPUT_PATH, injected, 'utf-8');

  cache = { path: OUTPUT_PATH, templateMtimeMs, fontMtimeMs };
  return cache.path;
}
