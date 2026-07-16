// TEMPORARY (remove together with vendor/emnapi, see vendor/emnapi/README.md).
//
// Shared integrity helpers binding the vendored archives to the emnapi npm
// package contents they were built from. `build.mjs` records the hashes in
// `manifest.json` at generation time; `install.mjs` re-verifies them on every
// machine (including every CI lane that consumes the archives) before copying
// the archives into `node_modules/emnapi/lib`.
import { createHash } from 'node:crypto';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

export function hashFile(path) {
  return `sha512-${createHash('sha512').update(readFileSync(path)).digest('base64')}`;
}

function walk(root, relative, out) {
  for (const entry of readdirSync(join(root, relative)).sort()) {
    const rel = `${relative}/${entry}`;
    if (statSync(join(root, rel)).isDirectory()) {
      walk(root, rel, out);
    } else {
      out.push(rel);
    }
  }
}

// Every file that can influence the archive contents: all shipped sources and
// headers plus the gyp build description (whose `emnapi` target defines the
// source list `build.mjs` mirrors).
export function collectSourceHashes(emnapiRoot) {
  const files = ['emnapi.gyp'];
  walk(emnapiRoot, 'src', files);
  walk(emnapiRoot, 'include', files);
  const hashes = {};
  for (const file of files) {
    hashes[file] = hashFile(join(emnapiRoot, file));
  }
  return hashes;
}

// Minimal `ar` member listing (System V/GNU flavor as emitted by `llvm-ar`),
// recorded in the manifest so archive-content changes show up in review.
export function listArchiveMembers(path) {
  const buffer = readFileSync(path);
  if (buffer.subarray(0, 8).toString('latin1') !== '!<arch>\n') {
    throw new Error(`${path} is not an ar archive`);
  }
  const members = [];
  let longNames = null;
  let offset = 8;
  while (offset + 60 <= buffer.length) {
    const header = buffer.subarray(offset, offset + 60).toString('latin1');
    const size = Number.parseInt(header.slice(48, 58).trim(), 10);
    if (Number.isNaN(size)) {
      throw new Error(`${path}: corrupt ar member header at offset ${offset}`);
    }
    let name = header.slice(0, 16).trim();
    const body = buffer.subarray(offset + 60, offset + 60 + size);
    if (name === '//') {
      longNames = body.toString('latin1');
    } else if (name.startsWith('/') && name !== '/') {
      const start = Number.parseInt(name.slice(1), 10);
      const end = longNames.indexOf('\n', start);
      name = longNames.slice(start, end === -1 ? undefined : end).replace(/\/$/, '');
      members.push(name);
    } else if (name !== '/' && name !== '') {
      members.push(name.replace(/\/$/, ''));
    }
    offset += 60 + size + (size % 2);
  }
  return members;
}
