const feature = {
  selectA: 'a',
  selectB: 'b',
  selectC: 'c',
};

// Destructured re-export — these aliases must survive into __exportAll
// in both the regular-finalizer Assets output and the HMR-stage patch.
export const { selectA: aliasA, selectB: aliasB, selectC: aliasC } = feature;

export const plain = 'plain';

console.log(aliasA, aliasB, aliasC, plain);

import.meta.hot.accept();
