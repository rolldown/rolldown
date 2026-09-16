globalThis.events.push('tla:start');
await Promise.resolve();
globalThis.events.push('tla:end');
export const value = 7;
