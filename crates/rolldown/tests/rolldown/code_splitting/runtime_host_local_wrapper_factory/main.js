const load = () => import('./lazy.cjs').then(({ default: value }) => value);
globalThis.results = Promise.all([load(), load()]);
throw new Error('entry failed');
