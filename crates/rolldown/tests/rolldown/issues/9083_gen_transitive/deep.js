// Four awaits: with the broken ordering (init_deep() without await in
// barrel) the outer code resumes before M1d runs, so __deepReady is
// still undefined when _test.mjs runs.
await Promise.resolve();
await Promise.resolve();
await Promise.resolve();
await Promise.resolve();
globalThis.__deepReady = true;
export const tlaValue = 'hello';
