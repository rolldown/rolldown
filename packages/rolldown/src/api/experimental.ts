import { BindingBundler } from '../binding';
import type { InputOptions } from '../options/input-options';
import { PluginDriver } from '../plugin/plugin-driver';
import { createBundlerImpl } from '../utils/create-bundler';
import { handleOutputErrors } from '../utils/transform-to-rollup-output';

/**
 * This is an experimental API. It's behavior may change in the future.
 *
 * Calling this API will only execute the scan stage of rolldown.
 */
export const experimental_scan = async (input: InputOptions): Promise<void> => {
  const inputOptions = await PluginDriver.callOptionsHook(input);
  const { impl: bundler, stopWorkers } = await createBundlerImpl(
    new BindingBundler(),
    inputOptions,
    {},
  );
  const output = await bundler.scan();
  handleOutputErrors(output);
  await stopWorkers?.();
};
