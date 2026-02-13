const a = await import('./lib1.js').then((m) => (console.log(m.foo), import('./lib2.js'))).then((m) => m.bar);

export { a };
