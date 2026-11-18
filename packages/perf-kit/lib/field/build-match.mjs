// Match the scripts a deployed site served to the files of a local build, so a
// field run can use that build's sourcemaps and module graph. Production rarely
// serves its maps; a local build of the same commit with the same settings often
// emits byte-identical chunks. Identity is decided by content, never by name:
// - exact:          same path, same SHA-256
// - renamed:        another path in the build has the same SHA-256
// - debug-id:       the served file names a //# debugId that a local map carries
// - different-build: the path exists locally with other content (wrong commit,
//                   settings or bundler version - the usual cause)
// - not-in-build:   no local file has this path or content

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const sha256 = (text) => crypto.createHash('sha256').update(text).digest('hex');
// A line of its own, often followed by the //# sourceMappingURL line.
const DEBUG_ID_RE = /^\/\/# debugId=([0-9a-fA-F-]{36})[ \t]*$/m;

function walk(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (entry.name === 'node_modules' || entry.name.startsWith('.')) continue;
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) walk(full, out);
    else if (/\.[mc]?js$/.test(entry.name)) out.push(full);
  }
  return out;
}

/** Hash every emitted script of a build once. Maps are read lazily, on a match. */
export function indexBuild(dir) {
  const byPath = new Map();
  const bySha = new Map();
  const byDebugId = new Map();
  for (const file of walk(dir)) {
    const rel = path.relative(dir, file).replaceAll('\\', '/');
    const code = fs.readFileSync(file, 'utf8');
    const sha = sha256(code);
    byPath.set(rel, sha);
    if (!bySha.has(sha)) bySha.set(sha, rel);
    const debugId = code.match(DEBUG_ID_RE)?.[1]?.toLowerCase();
    if (debugId) byDebugId.set(debugId, rel);
  }
  return { dir, byPath, bySha, byDebugId };
}

/** Decide which local file (if any) a served script is. `file` has no leading slash. */
export function matchScript(index, { file, body }) {
  if (!index) return { status: 'no-build' };
  const sha = sha256(body);
  if (index.byPath.get(file) === sha) return { status: 'exact', local: file };
  const renamed = index.bySha.get(sha);
  if (renamed) return { status: 'renamed', local: renamed };
  const debugId = body.match(DEBUG_ID_RE)?.[1]?.toLowerCase();
  if (debugId && index.byDebugId.has(debugId)) {
    return { status: 'debug-id', local: index.byDebugId.get(debugId) };
  }
  return { status: index.byPath.has(file) ? 'different-build' : 'not-in-build' };
}

/** Code + sourcemap of a matched local file, in the shape attributeChunks expects. */
export function readMatched(index, match) {
  if (!match?.local) return null;
  const file = path.join(index.dir, match.local);
  const mapFile = `${file}.map`;
  if (!fs.existsSync(mapFile)) return null;
  return {
    code: fs.readFileSync(file, 'utf8'),
    map: JSON.parse(fs.readFileSync(mapFile, 'utf8')),
  };
}

/** Counts and bytes per match status - how much of the page the build explains. */
export function matchSummary(matches) {
  const summary = { scripts: 0, bytes: 0, matchedScripts: 0, matchedBytes: 0, byStatus: {} };
  for (const { status, bytes } of matches) {
    summary.scripts += 1;
    summary.bytes += bytes;
    summary.byStatus[status] = (summary.byStatus[status] ?? 0) + 1;
    if (status === 'exact' || status === 'renamed' || status === 'debug-id') {
      summary.matchedScripts += 1;
      summary.matchedBytes += bytes;
    }
  }
  return summary;
}
