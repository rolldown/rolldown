console.log('b start');
if (globalThis.rejectSibling) throw new Error('b failed');
globalThis.release();
await 0;
console.log('b end');
