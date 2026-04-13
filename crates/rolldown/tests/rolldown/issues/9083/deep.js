// Has top-level await — value is assigned AFTER two awaits so that
// missing await on init_middle() in barrel causes manager to be
// created with value===undefined (and setup() then throws).
await Promise.resolve();
await Promise.resolve();
export const value = 'hello';
