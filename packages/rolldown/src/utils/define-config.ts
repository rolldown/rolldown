import type { RolldownOptions } from '../types/rolldown-options';
import type { MaybePromise } from '../types/utils';

/**
 * Type for `default export` of `rolldown.config.js` file.
 * @category Config
 */
export type ConfigExport = RolldownOptions | RolldownOptions[] | RolldownOptionsFunction;
/** @category Config */
export type RolldownOptionsFunction = (
  commandLineArguments: Record<string, any>,
) => MaybePromise<RolldownOptions | RolldownOptions[]>;

/**
 * A helper to define a rolldown configuration with type hints.
 *
 * @example
 * ```js [rolldown.config.js]
 * import { defineConfig } from 'rolldown';
 *
 * export default defineConfig({
 *   input: 'src/main.js',
 *   output: {
 *     file: 'bundle.js',
 *   },
 * });
 * ```
 *
 * @category Config
 */
export function defineConfig(config: RolldownOptions): RolldownOptions;
export function defineConfig(config: RolldownOptions[]): RolldownOptions[];
export function defineConfig(config: RolldownOptionsFunction): RolldownOptionsFunction;
export function defineConfig(config: ConfigExport): ConfigExport;
export function defineConfig(config: ConfigExport): ConfigExport {
  return config;
}
