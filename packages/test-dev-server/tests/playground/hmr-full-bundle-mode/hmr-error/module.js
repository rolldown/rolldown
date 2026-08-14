// Used by __tests__/hmr-error.spec.ts: the spec swaps the slot comment below
// for an unterminated string so the HMR update fails; a page refresh then
// triggers a full rebuild.
export const value = 'hmr-error: ok';

document.querySelector('.hmr-error').textContent = value;

/* @syntax-error-slot */
