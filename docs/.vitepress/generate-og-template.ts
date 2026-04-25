import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

const DIR = import.meta.dirname;
const TEMPLATE_PATH = path.join(DIR, 'og-template.svg');
const FONT_PATH = path.join(DIR, 'fonts', 'Inter-VariableFont_opsz,wght.ttf');
const CACHE_DIR = path.join(DIR, 'cache');
const OUTPUT_PATH = path.join(CACHE_DIR, 'og-template.generated.svg');

let cachedPath: string | undefined;

/**
 * Produce a copy of `og-template.svg` with the Inter font embedded as a
 * base64 `@font-face` data URL, so OG image rendering via `sharp`/librsvg is
 * deterministic on any OS (including Linux CI where Inter is not installed
 * system-wide). Returns the absolute path of the generated SVG.
 *
 * The Inter font is licensed under SIL OFL 1.1. See ./fonts/OFL.txt.
 */
export function prepareOgTemplateWithFont(): string {
  if (cachedPath) return cachedPath;

  const svg = readFileSync(TEMPLATE_PATH, 'utf-8');
  const fontBase64 = readFileSync(FONT_PATH).toString('base64');

  // Multiple `<defs>` are valid in SVG, so we inject ours right after the
  // opening `<svg ...>` tag without touching the existing `<defs>` block.
  const fontFace =
    `<defs><style type="text/css">` +
    `@font-face{` +
    `font-family:'Inter';` +
    `font-style:normal;` +
    `font-weight:100 900;` +
    `src:url('data:font/ttf;base64,${fontBase64}') format('truetype');` +
    `}` +
    `</style></defs>`;

  const injected = svg.replace(/(<svg\b[^>]*>)/, `$1${fontFace}`);

  mkdirSync(CACHE_DIR, { recursive: true });
  writeFileSync(OUTPUT_PATH, injected, 'utf-8');

  cachedPath = OUTPUT_PATH;
  return cachedPath;
}
