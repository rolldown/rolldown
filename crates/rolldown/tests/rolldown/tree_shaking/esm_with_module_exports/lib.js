// ESM module with multiple exports including "module.exports"
export const used = 'used value';
export const unused1 = 'unused1 value';
export const unused2 = 'unused2 value';

// This export is specifically for require() compatibility
export const moduleExports = { used };

// Alternative export name that matches the runtime check
export { moduleExports as "module.exports" };
