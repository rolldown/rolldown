import cjs from './cjs.cjs';

export * from './cjs.cjs';
cjs.foo = 'after';
cjs.then = (resolve) => resolve('assimilated');
