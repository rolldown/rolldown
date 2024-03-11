import { Bundler } from '../binding'
import {
  normalizeInputOptions,
  type InputOptions,
} from '../options/input-options'
import { createInputOptionsAdapter } from '../options/input-options-adapter'

export async function createBundler(
  inputOptions: InputOptions,
): Promise<Bundler> {
  // Convert `InputOptions` to `NormalizedInputOptions`.
  const normalizedInputOptions = await normalizeInputOptions(inputOptions)
  // Convert `NormalizedInputOptions` to `BindingInputOptions`
  const bindingInputOptions = createInputOptionsAdapter(
    normalizedInputOptions,
    inputOptions,
  )
  return new Bundler(bindingInputOptions)
}
