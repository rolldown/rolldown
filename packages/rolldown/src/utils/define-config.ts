import type { ConfigExport } from '../types/config-export';
import type { RolldownOptions } from '../types/rolldown-options';
import type { RolldownOptionsFunction } from '../types/rolldown-options-function';

export function defineConfig(config: RolldownOptions): RolldownOptions;
export function defineConfig(config: RolldownOptions[]): RolldownOptions[];
export function defineConfig(
  config: RolldownOptionsFunction,
): RolldownOptionsFunction;
export function defineConfig(config: ConfigExport): ConfigExport;
export function defineConfig(config: ConfigExport): ConfigExport {
  return config;
}
