globalThis.load = [() => import('./barrel.js').then((m) => m.NAME), () => import('./page.js')];
