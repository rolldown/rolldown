// ESM module that exports module.exports
export const foo = 'foo';
export const bar = 'bar';
export const moduleExports = { value: 'external module' };
export { moduleExports as 'module.exports' };
