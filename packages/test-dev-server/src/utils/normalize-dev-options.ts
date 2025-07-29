import { DevOptions } from '../types/dev-options';
import { NormalizedDevOptions } from '../types/normalized-dev-options';

export function normalizeDevOptions(options: DevOptions): NormalizedDevOptions {
  return {
    platform: options.platform ?? 'browser',
  };
}
