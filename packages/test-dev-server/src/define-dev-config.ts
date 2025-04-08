import type { BuildOptions } from 'rolldown';

export interface ServeOptions {}

export interface DevConfig {
  build?: BuildOptions;
  serve?: ServeOptions;
}

export function defineDevConfig(config: DevConfig): DevConfig {
  return config;
}
