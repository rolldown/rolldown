import type { InputOptions } from 'rolldown';
import { rolldown } from 'rolldown';
import type { TestConfig } from './types';

export async function compileFixture(fixturePath: string, config: TestConfig) {
  const inputOptions: InputOptions = {
    input: 'main.js',
    cwd: fixturePath,
    ...config.config,
  };
  inputOptions.checks ??= {};
  inputOptions.checks.pluginTimings ??= false;
  const build = await rolldown(inputOptions);
  if (Array.isArray(config.config?.output)) {
    const outputs = [];
    for (const output of config.config.output) {
      outputs.push(await build.write(output));
    }
    return outputs;
  }
  const outputOptions = config.config?.output ?? {};
  return await build.write(outputOptions);
}
