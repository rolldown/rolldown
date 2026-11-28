export const lazy = () => import('./y.js');
const { load } = await import('./zz-e.js');
export const value = await load();
