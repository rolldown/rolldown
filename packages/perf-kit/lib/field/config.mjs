// Field config: which deployed URL to load, how to ask the site for each build
// (variant), what to block, and which local build explains each variant.
//
// {
//   "url": "https://app.example.com/",
//   "variants": {
//     "base":      { "cookies": [{ "name": "version", "value": "stable" }], "build": "/path/to/dist" },
//     "candidate": { "query": { "preview": "pr-123" } }
//   },
//   "block": ["analytics.example.com"],   // URL substrings; the page cannot send these
//   "ignoreLcp": "#splash",               // LCP entries inside this selector do not count
//   "gate": "/api/items",                 // optional: the request the LCP waits for
//   "redact": ["tenant-[a-z0-9]+"]        // extra regexes removed from written URLs
// }
//
// A variant may set cookies, query parameters or both. Variant cookie values are
// treated as secrets: they are never printed or written to the report.

import fs from 'node:fs';
import path from 'node:path';

export function loadFieldConfig(file) {
  const resolved = path.resolve(file);
  if (!fs.existsSync(resolved)) throw new Error(`no field config at ${resolved}`);
  const config = JSON.parse(fs.readFileSync(resolved, 'utf8'));
  return validateFieldConfig(config, path.dirname(resolved));
}

export function validateFieldConfig(config, baseDir = process.cwd()) {
  let url;
  try {
    url = new URL(config.url);
  } catch {
    throw new Error('field config: "url" must be an absolute URL');
  }
  const names = Object.keys(config.variants ?? {});
  if (names.length < 1) throw new Error('field config: "variants" needs at least one entry');
  const variants = {};
  for (const name of names) {
    const v = config.variants[name] ?? {};
    for (const cookie of v.cookies ?? []) {
      if (!cookie?.name || typeof cookie.value !== 'string') {
        throw new Error(`field config: variant "${name}" has a cookie without name/value`);
      }
    }
    const target = new URL(url);
    for (const [key, value] of Object.entries(v.query ?? {})) target.searchParams.set(key, value);
    variants[name] = {
      name,
      cookies: v.cookies ?? [],
      url: target.toString(),
      build: v.build ? path.resolve(baseDir, v.build) : null,
    };
  }
  return {
    url: url.toString(),
    origin: url.origin,
    variants,
    block: config.block ?? [],
    ignoreLcp: config.ignoreLcp ?? null,
    gate: config.gate ?? null,
    redact: config.redact ?? [],
  };
}

/** What the report may say about the config: cookie names, never values. */
export function describeConfig(config, redact) {
  return {
    url: redact(config.url),
    variants: Object.fromEntries(
      Object.values(config.variants).map((v) => [
        v.name,
        {
          url: redact(v.url),
          cookies: v.cookies.map((c) => c.name),
          build: v.build,
        },
      ]),
    ),
    block: config.block,
    ignoreLcp: config.ignoreLcp,
    gate: config.gate,
  };
}
