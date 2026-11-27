export function imports() {
  return Promise.all([
    import('./module.js', { with: { type: 'json' }, with: {} }),
    import('./module.js', { assert: { type: 'json' }, assert: { type: 'text' } }),
    import('./module.js', { assert: { type: 'json' }, with: { type: 'text' } }),
    import('./module.js', { ['with']: { type: 'text' }, assert: { type: 'json' } }),
  ]);
}

export function loadExternal() {
  return import('data:application/json,42', {
    assert: { type: 'text' },
    ['assert']: { type: 'json' },
  });
}

export function loadWithAccessors(specifier) {
  return import(specifier, {
    get assert() {
      return { type: 'json' };
    },
    set assert(value) {},
  });
}

export function loadWithBoth(specifier) {
  return import(specifier, { assert: { type: 'text' }, ['with']: { type: 'json' } });
}
