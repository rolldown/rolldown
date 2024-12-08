import type { InputOptions } from '../options/input-options'
import { RolldownOptions } from '../types/rolldown-options'
import { BundlerWithStopWorker, createBundler } from '../utils/create-bundler'
import {
  handleOutputErrors,
  transformToRollupOutput,
} from '../utils/transform-to-rollup-output'

/**
 * This is an experimental API. It's behavior may change in the future.
 *
 * Calling this API will only execute the scan stage of rolldown.
 */
export const experimental_scan = async (input: InputOptions): Promise<void> => {
  const { bundler, stopWorkers } = await createBundler(input, {})
  const output = await bundler.scan()
  handleOutputErrors(output)
  await stopWorkers?.()
}

export const experimental_rebuild = async (
  options: RolldownOptions,
): Promise<RolldownRebuild> => {
  const inner = await createBundler(options, options.output ?? {})
  inner.bundler.setRebuildEnabled(true)
  return new RolldownRebuild(inner)
}

export class RolldownRebuild {
  constructor(private inner: BundlerWithStopWorker) {}

  async build() {
    const output = await this.inner.bundler.write()
    return transformToRollupOutput(output)
  }

  async close(): Promise<void> {
    await this.inner.stopWorkers?.()
    await this.inner.bundler.close()
  }
}
