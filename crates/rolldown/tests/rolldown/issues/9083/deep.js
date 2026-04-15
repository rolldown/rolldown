// Has top-level await — value is assigned after two awaits so a missing
// await through the export-star barrel leaves downstream init incomplete.
await Promise.resolve();
await Promise.resolve();

export const value = 'hello';
