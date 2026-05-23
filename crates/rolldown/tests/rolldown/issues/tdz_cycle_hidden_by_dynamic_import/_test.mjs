import assert from 'node:assert';

await assert.rejects(
  () => import('./dist/main.js'),
  (error) =>
    error instanceof ReferenceError &&
    error.message.includes("Cannot access 'b' before initialization"),
);
