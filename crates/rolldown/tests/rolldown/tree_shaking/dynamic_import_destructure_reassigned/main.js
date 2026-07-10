let ns = await import('./lib.js');
ns = { used: 'x' };
const { used } = ns; // reads the reassigned object, not the namespace
console.log(used);
