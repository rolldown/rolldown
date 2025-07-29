import nodePath from 'node:path';
import nodeUrl from 'node:url';
import { DevConfig } from './define-dev-config.js';

export async function loadDevConfig(): Promise<DevConfig> {
  const exports = await import(
    nodeUrl.pathToFileURL(nodePath.join(process.cwd(), 'dev.config.mjs'))
      .href
  );
  return exports.default;
}
