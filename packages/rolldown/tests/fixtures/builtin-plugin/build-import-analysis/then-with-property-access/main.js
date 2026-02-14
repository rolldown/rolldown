const a = await import('./lib').then((m) => m.foo);
const b = await import('./lib').then((m) => m.bar);

export { a, b };
