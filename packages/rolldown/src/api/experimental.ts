import type { InputOptions } from '../options/input-options';
import { PluginDriver } from '../plugin/plugin-driver';
import { RolldownBuild } from './rolldown/rolldown-build';

export { freeExternalMemory } from '../types/external-memory-handle';

/**
 * This is an experimental API. It's behavior may change in the future.
 *
 * Calling this API will only execute the scan stage of rolldown.
 */
export const scan = async (input: InputOptions): Promise<void> => {
  const inputOptions = await PluginDriver.callOptionsHook(input);
  const build = new RolldownBuild(inputOptions);
  try {
    await build.scan();
  } finally {
    await build.close();
  }
};
