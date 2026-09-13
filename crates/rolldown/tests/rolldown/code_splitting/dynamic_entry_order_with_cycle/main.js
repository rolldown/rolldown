const { serverExports } = await import('./server.js');

// Throws `toNamespace is not a function` if the cycle is executed in the order
// no runtime can produce.
if (serverExports.name !== 'server') {
  throw new Error(`expected the namespace to be built, got ${serverExports.name}`);
}
console.log('ok');
