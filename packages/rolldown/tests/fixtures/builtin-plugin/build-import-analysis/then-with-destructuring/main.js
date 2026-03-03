const a = await import('./lib').then(({ foo }) => foo);
const b = await import('./lib').then(({ bar }) => bar);

export { a, b };
