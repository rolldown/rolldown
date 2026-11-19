globalThis.load = [
  () => import('./barrel.js').then((m) => [m.default, m.__proto__, m.NAME, m.again]),
  () => import('./page.js'),
];
