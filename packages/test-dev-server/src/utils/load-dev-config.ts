import nodePath from 'node:path';
import nodeUrl from 'node:url';
import type { DevConfig } from './define-dev-config.js';

/** Load `dev.config.mjs` from the given directory (absolute path). */
export async function loadDevConfig(dir: string): Promise<DevConfig> {
  const exports = await import(nodeUrl.pathToFileURL(nodePath.join(dir, 'dev.config.mjs')).href);
  return exports.default;
}
