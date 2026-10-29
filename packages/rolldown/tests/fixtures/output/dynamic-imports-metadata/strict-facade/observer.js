globalThis.events ??= [];
globalThis.events.push('observer:' + Boolean(globalThis.ready));
console.log('observer eval', Boolean(globalThis.ready));
