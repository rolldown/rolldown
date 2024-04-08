import { Bundler } from '../binding'
import {
  normalizeInputOptions,
  type InputOptions,
} from '../options/input-options'
import { createInputOptionsAdapter } from '../options/input-options-adapter'
import {
  OutputOptions,
  normalizeOutputOptions,
} from '../options/output-options'

export async function createBundler(
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
): Promise<Bundler> {
  // Convert `InputOptions` to `NormalizedInputOptions`.
  const normalizedInputOptions = await normalizeInputOptions(inputOptions)
  const normalizedOutputOptions = normalizeOutputOptions(outputOptions)
  // Convert `NormalizedInputOptions` to `BindingInputOptions`
  const bindingInputOptions = createInputOptionsAdapter(
    normalizedInputOptions,
    inputOptions,
    normalizedOutputOptions,
  )
  return new Bundler(bindingInputOptions, normalizedOutputOptions)
}
