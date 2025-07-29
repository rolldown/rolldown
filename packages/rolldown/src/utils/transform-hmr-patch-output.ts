import type { BindingHmrOutput, BindingHmrUpdate } from '../binding';
import { normalizeErrors } from './error';

export function transformHmrPatchOutput(
  output: BindingHmrOutput,
): BindingHmrUpdate | undefined {
  handleHmrPatchOutputErrors(output);
  const { patch } = output;
  return patch ?? undefined;
}

function handleHmrPatchOutputErrors(output: BindingHmrOutput): void {
  const rawErrors = output.errors;
  if (rawErrors.length > 0) {
    throw normalizeErrors(rawErrors);
  }
}
