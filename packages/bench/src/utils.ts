import nodePath from 'node:path';
import nodeUrl from 'node:url';

import type { BenchSuite } from './types';

const dirname = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url));

export const REPO_ROOT = nodePath.join(dirname, '../../..');

export const PROJECT_ROOT = nodePath.join(dirname, '..');

export function defineSuite(config: BenchSuite): BenchSuite {
  return config;
}
