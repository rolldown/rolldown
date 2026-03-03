const a = (await import('./lib')).foo;
const b = (await import('./lib')).bar;

export { a, b };
