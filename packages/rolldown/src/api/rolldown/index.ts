import type { InputOptions } from '../../options/input-options';
import { PluginDriver } from '../../plugin/plugin-driver';
import { validateOption } from '../../utils/validator';
import { RolldownBuild } from './rolldown-build';

// `async` here is intentional to be compatible with `rollup.rollup`.
/** @category Programmatic APIs */
export const rolldown = async (input: InputOptions): Promise<RolldownBuild> => {
  validateOption('input', input);
  const inputOptions = await PluginDriver.callOptionsHook(input);
  return new RolldownBuild(inputOptions);
};
