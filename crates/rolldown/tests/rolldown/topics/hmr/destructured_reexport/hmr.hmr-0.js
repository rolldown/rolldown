const feature = {
  selectA: 'a',
  selectB: 'b',
  selectC: 'c',
};

export const { selectA: aliasA, selectB: aliasB, selectC: aliasC } = feature;

export const plain = 'plain-updated';

console.log(aliasA, aliasB, aliasC, plain);

import.meta.hot.accept();
