export const loaded = import('./data.json', { with: { type: 'json' } }).then(
  (mod) => mod.default.value,
);
