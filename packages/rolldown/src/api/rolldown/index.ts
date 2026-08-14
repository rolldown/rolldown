import type { InputOptions } from '../../options/input-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { validateOption } from '../../utils/validator';
import { RolldownBuild } from './rolldown-build';

/**
 * The API compatible with Rollup's `rollup` function.
 *
 * Unlike Rollup, the module graph is not built until the methods of the bundle object are called.
 *
 * @param input The input options object.
 * @returns A Promise that resolves to a bundle object.
 *
 * @example
 * ```js
 * import { rolldown } from 'rolldown';
 *
 * let bundle, failed = false;
 * try {
 *   bundle = await rolldown({
 *     input: 'src/main.js',
 *   });
 *   await bundle.write({
 *     format: 'esm',
 *   });
 * } catch (e) {
 *   console.error(e);
 *   failed = true;
 * }
 * if (bundle) {
 *   await bundle.close();
 * }
 * process.exitCode = failed ? 1 : 0;
 * ```
 *
 * @category Programmatic APIs
 */
// `async` here is intentional to be compatible with `rollup.rollup`.
export const rolldown = async (input: InputOptions): Promise<RolldownBuild> => {
  validateOption('input', input);
  const inputOptions = await PluginDriver.callOptionsHook(input);
  return new RolldownBuild(inputOptions);
};
