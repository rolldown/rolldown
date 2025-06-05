import type { BindingHmrOutput, BindingHmrOutputPatch } from '../binding';
import { normalizeErrors } from './error';

export function transformHmrPatchOutput(
  output: BindingHmrOutput,
): BindingHmrOutputPatch {
  handleHmrPatchOutputErrors(output);
  const { patch } = output;
  return patch!;
}

function handleHmrPatchOutputErrors(output: BindingHmrOutput): void {
  const rawErrors = output.errors;
  if (rawErrors.length > 0) {
    throw normalizeErrors(rawErrors);
  }
}
