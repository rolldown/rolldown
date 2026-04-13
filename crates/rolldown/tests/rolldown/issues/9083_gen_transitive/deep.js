// Has TLA — two awaits so the broken ordering is detectable
await Promise.resolve();
await Promise.resolve();
export const tlaValue = 'hello';
