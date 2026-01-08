import type { RolldownOptions } from '../types/rolldown-options';
import type { MaybePromise } from '../types/utils';

/**
 * Type for `default export` of `rolldown.config.js` file.
 */
export type ConfigExport = RolldownOptions | RolldownOptions[] | RolldownOptionsFunction;
export type RolldownOptionsFunction = (
  commandLineArguments: Record<string, any>,
) => MaybePromise<RolldownOptions | RolldownOptions[]>;

export function defineConfig(config: RolldownOptions): RolldownOptions;
export function defineConfig(config: RolldownOptions[]): RolldownOptions[];
export function defineConfig(config: RolldownOptionsFunction): RolldownOptionsFunction;
export function defineConfig(config: ConfigExport): ConfigExport;
export function defineConfig(config: ConfigExport): ConfigExport {
  return config;
}
