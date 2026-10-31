import 'ext';
import data from 'ext' with { type: 'json' };
export * from 'ext' with { type: 'json' };
export const lazy = () => import('ext', { with: { type: 'json' } });
export const lazyAssert = () => import('ext', { assert: { type: 'json' } });
export const computed = (name) => import(name, { assert: { type: 'json' } });
export { data };
