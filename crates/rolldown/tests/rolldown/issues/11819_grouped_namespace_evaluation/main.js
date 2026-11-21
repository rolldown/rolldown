if (Object.prototype.reproKey === 1) throw new Error('waiter stopped');
const target = await import('./target.js');
console.log(target.value);
