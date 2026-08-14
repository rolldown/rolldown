import * as fs from 'node:fs';
import * as path from 'node:path';
import { ESBUILD_SNAPSHOTS_URL } from '../urls.js';

export const SNAPSHOT_FILES = [
  'snapshots_css.txt',
  'snapshots_dce.txt',
  'snapshots_default.txt',
  'snapshots_glob.txt',
  'snapshots_importphase.txt',
  'snapshots_importstar.txt',
  'snapshots_importstar_ts.txt',
  'snapshots_loader.txt',
  'snapshots_lower.txt',
  'snapshots_packagejson.txt',
  'snapshots_splitting.txt',
  'snapshots_ts.txt',
  'snapshots_tsconfig.txt',
  'snapshots_yarnpnp.txt',
] as const;

export type SnapshotFileName = (typeof SNAPSHOT_FILES)[number];

const SNAPSHOTS_DIR = path.resolve(import.meta.dirname, '../../../tmp/esbuild-tests/snapshots');
const CACHE_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours

function isCacheFresh(filePath: string): boolean {
  if (!fs.existsSync(filePath)) return false;
  const mtime = fs.statSync(filePath).mtimeMs;
  return Date.now() - mtime < CACHE_TTL_MS;
}

async function downloadSnapshot(filename: SnapshotFileName): Promise<string> {
  const url = `${ESBUILD_SNAPSHOTS_URL}/${filename}`;
  console.log(`Downloading ${filename}...`);

  const response = await fetch(url);
  if (!response.ok) {
    // If download fails and we have a stale cache, use it
    const filePath = path.join(SNAPSHOTS_DIR, filename);
    if (fs.existsSync(filePath)) {
      console.warn(
        `Download failed (${response.status} ${response.statusText}), using stale cache for ${filename}`,
      );
      return fs.readFileSync(filePath, 'utf-8');
    }
    throw new Error(`Failed to download ${filename}: ${response.status} ${response.statusText}`);
  }

  const content = await response.text();
  return content;
}

export async function ensureSnapshot(
  filename: SnapshotFileName,
  options: { force?: boolean } = {},
): Promise<string> {
  const filePath = path.join(SNAPSHOTS_DIR, filename);

  // Skip download if cache is fresh (less than 24h old) and not forced
  if (!options.force && isCacheFresh(filePath)) {
    return fs.readFileSync(filePath, 'utf-8');
  }

  const content = await downloadSnapshot(filename);

  fs.mkdirSync(SNAPSHOTS_DIR, { recursive: true });
  fs.writeFileSync(filePath, content);
  console.log(`Saved ${filename}`);

  return content;
}
