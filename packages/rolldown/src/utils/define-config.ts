import type { ConfigExport } from '../types/config-export';
import type { RolldownOptions } from '../types/rolldown-options';

export function defineConfig(config: RolldownOptions): RolldownOptions;
export function defineConfig(config: RolldownOptions[]): RolldownOptions[];
export function defineConfig(config: ConfigExport): ConfigExport;
export function defineConfig(config: ConfigExport): ConfigExport {
  return config;
}
