// ESM module that exports module.exports alongside other exports
// module.exports should take priority when converted to CJS
export const namedExport = 'named';
export default { defaultValue: 'default' };

const moduleExports = { priorityValue: 'module.exports wins' };
export { moduleExports as 'module.exports' };
