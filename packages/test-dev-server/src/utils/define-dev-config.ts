import type { BuildOptions } from 'rolldown';
import { DevOptions } from '../types/dev-options';

export interface DevConfig {
  build?: BuildOptions;
  dev?: DevOptions;
}

export function defineDevConfig(config: DevConfig): DevConfig {
  return config;
}
