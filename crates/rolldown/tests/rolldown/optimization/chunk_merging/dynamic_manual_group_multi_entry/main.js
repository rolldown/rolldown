const [a, b, c] = await Promise.all([import('./a.js'), import('./b.js'), import('./c.js')]);

if (`${a.A}${b.B}${c.C}` !== 'ABC') {
  throw new Error('unexpected dynamic namespace result');
}
