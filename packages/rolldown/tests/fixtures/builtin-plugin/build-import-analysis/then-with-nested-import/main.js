const a = await import('./lib1.js').then(() => import('./lib2.js').then((m) => m.bar));

export { a };
