// Used by __tests__/initial-error.spec.ts: the spec swaps the slot comment
// below for an unterminated string before starting its own server, so that
// server's first build fails.
export const value = 'initial-error: ok';

document.querySelector('.initial-error').textContent = value;

/* @syntax-error-slot */
