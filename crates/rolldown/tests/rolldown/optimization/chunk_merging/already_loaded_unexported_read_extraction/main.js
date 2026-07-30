export const done = import('./app.js').then((app) => {
  console.log(app.missing);
  return app.done;
});
