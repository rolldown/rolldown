export const app = import('./app.js').then((m) => ({
  done: m.done,
  n: m.n,
}));
