import type { DevOptions } from '../types/dev-options';
import type { NormalizedDevOptions } from '../types/normalized-dev-options';

export function normalizeDevOptions(options: DevOptions): NormalizedDevOptions {
  return {
    platform: options.platform ?? 'browser',
  };
}
