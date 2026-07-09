const BINDING_MISMATCH_CODE = 'ERR_ROLLDOWN_BINDING_MISMATCH';

export interface BindingMismatchTaggedError extends Error {
  readonly code: typeof BINDING_MISMATCH_CODE;
}

export class BindingMismatchError extends TypeError implements BindingMismatchTaggedError {
  readonly code = BINDING_MISMATCH_CODE;
}

export function isBindingMismatchError(error: unknown): error is BindingMismatchTaggedError {
  if ((typeof error !== 'object' || error === null) && typeof error !== 'function') return false;
  try {
    return Reflect.get(error, 'code') === BINDING_MISMATCH_CODE;
  } catch {
    return false;
  }
}

export function markBindingMismatchError<T extends Error>(
  error: T,
): T & BindingMismatchTaggedError {
  return Object.assign(error, {
    code: BINDING_MISMATCH_CODE,
  } satisfies Pick<BindingMismatchTaggedError, 'code'>);
}
