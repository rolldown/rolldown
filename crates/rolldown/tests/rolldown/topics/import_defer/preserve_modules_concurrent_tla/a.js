console.log('a start');
await new Promise((resolve) => (globalThis.release = resolve));
console.log('a end');
