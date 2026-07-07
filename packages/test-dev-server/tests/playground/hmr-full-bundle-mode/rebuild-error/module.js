// Used by __tests__/rebuild-error.spec.ts. Not self-accepting on purpose:
// editing it forces a full rebuild, which the flag plugin in dev.config.mjs
// then fails.
export const value = 'rebuild-error: ok';

document.querySelector('.rebuild-error').textContent = value;
