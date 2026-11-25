import value from './files/dep.js' with { tag: 'static', __proto__: 'string' };

export { value };
export const dynamic = () =>
  import('./files/dep.js', { with: { tag: 'dynamic', __proto__: 'string' } });
export const dynamicComputed = () =>
  import('./files/dep.js', { with: { tag: 'dynamic-computed', ['__proto__']: 'string' } });
