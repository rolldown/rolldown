import type { InputOptions } from '../options/input-options';
import type { OutputOptions } from '../options/output-options';
import type { RolldownOutput } from '../types/rolldown-output';
import { rolldown } from './rolldown';

/**
 * The options for {@linkcode build} function.
 *
 * @experimental
 * @category Programmatic APIs
 */
export type BuildOptions = InputOptions & {
  /**
   * Write the output to the file system
   *
   * @default true
   */
  write?: boolean;
  output?: OutputOptions;
};

/**
 * Build a single output.
 *
 * @param options The build options.
 * @returns A Promise that resolves to the build output.
 */
async function build(options: BuildOptions): Promise<RolldownOutput>;
/**
 * Build multiple outputs __sequentially__.
 *
 * @param options The build options.
 * @returns A Promise that resolves to the build outputs for each option.
 */
async function build(options: BuildOptions[]): Promise<RolldownOutput[]>;
/**
 * The API similar to esbuild's `build` function.
 *
 * @example
 * ```js
 * import { build } from 'rolldown';
 *
 * const result = await build({
 *   input: 'src/main.js',
 *   output: {
 *     file: 'bundle.js',
 *   },
 * });
 * console.log(result);
 * ```
 *
 * @experimental
 * @category Programmatic APIs
 */
async function build(
  options: BuildOptions | BuildOptions[],
): Promise<RolldownOutput | RolldownOutput[]> {
  if (Array.isArray(options)) {
    return Promise.all(options.map((opts) => build(opts)));
  } else {
    const { output, write = true, ...inputOptions } = options;
    const build = await rolldown(inputOptions);
    try {
      if (write) {
        return await build.write(output);
      } else {
        return await build.generate(output);
      }
    } finally {
      await build.close();
    }
  }
}

export { build };
