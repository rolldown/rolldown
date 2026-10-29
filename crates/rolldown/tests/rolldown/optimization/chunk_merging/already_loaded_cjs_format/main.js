export const done = import('./app.js').then(async (app) => {
  // Wait for the dynamic import chain (main -> app -> lazy) to settle.
  await app.done;
  return Object.keys(app).sort();
});
