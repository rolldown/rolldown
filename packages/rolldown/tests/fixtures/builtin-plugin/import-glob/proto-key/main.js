export const lazy = import.meta.glob('./files/*.js', {
  with: { tag: 'lazy', __proto__: 'string' },
});
export const lazyComputed = import.meta.glob('./files/*.js', {
  with: { tag: 'lazy-computed', ['__proto__']: 'string' },
});
export const eager = import.meta.glob('./files/*.js', {
  eager: true,
  with: { tag: 'eager', __proto__: 'string' },
});
export const eagerComputed = import.meta.glob('./files/*.js', {
  eager: true,
  ['with']: { tag: 'eager-computed', ['__proto__']: 'string' },
});
