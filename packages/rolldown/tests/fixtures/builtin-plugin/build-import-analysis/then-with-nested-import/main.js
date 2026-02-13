const a = await import('./lib1.js').then((m) => import('./lib2.js'));

export { a };
