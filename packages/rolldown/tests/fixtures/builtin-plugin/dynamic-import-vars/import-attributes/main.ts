export function relative(name: string) {
  return import(`./files/${name}.js`, { with: { type: 'text' as const } as const } as const);
}

export function alias(name: string) {
  return import(`@/${name}.js?raw`, { with: { type: 'text', 'custom-key': 'a"b\\c\n' } });
}

export function withoutAttributes(name: string) {
  return import(`./files/${name}.js`);
}

export function repeatedWith(name: string) {
  return import(`./files/${name}.js`, { with: { type: 'json' }, with: {} });
}

export function legacyAssert(name: string) {
  return import(`@/${name}.js`, { /* 😀 */ assert: { type: 'text' } });
}

export function withAndAssert(name: string) {
  return import(`./files/${name}.js`, { ['with']: { type: 'text' }, assert: { type: 'json' } });
}

export function malformedAttribute(name: string) {
  return import(`@/${name}.js?malformed`, { with: { type: 'text', malformed: 123 } });
}
