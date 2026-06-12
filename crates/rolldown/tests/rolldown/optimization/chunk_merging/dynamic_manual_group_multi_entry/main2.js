const [a, b] = await Promise.all([import('./a.js'), import('./b.js')]);

if (`${a.A}${b.B}` !== 'AB') {
  throw new Error('unexpected dynamic namespace result');
}
