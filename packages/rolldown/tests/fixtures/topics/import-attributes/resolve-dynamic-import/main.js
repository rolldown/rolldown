export const lazy = () => import('./dep.js', { with: { kind: 'x' } });
