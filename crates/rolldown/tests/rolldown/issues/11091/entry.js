console.log('bar =', require('./req.js').bar.name);
export const { Foo } = await import('./lib.js');
console.log('Foo =', Foo);
