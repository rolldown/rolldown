import data from 'ext' with { type: 'json' };

export { data };
export const lazy = () => import('ext', { with: { type: 'json' } });
