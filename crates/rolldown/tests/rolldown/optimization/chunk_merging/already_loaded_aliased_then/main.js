export const done = import('./app.js').then(async (app) => {
  await app.done;
  return Object.keys(app).sort();
});
