// Four awaits so init_deep cannot complete before outer code resumes
// if it is not properly awaited.
await Promise.resolve();
await Promise.resolve();
await Promise.resolve();
await Promise.resolve();
globalThis.__deepReady = true;
export const value = 'deep-value';
