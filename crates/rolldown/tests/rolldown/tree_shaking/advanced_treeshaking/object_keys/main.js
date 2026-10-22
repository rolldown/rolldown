// A read of a symbol key with no getter has no effect.
const key = Symbol();
const symbols = { [key]: 'DEAD_symbol' };
symbols[key];

// A `delete` on an object that nothing reads has no effect.
const deleted = { a: 'DEAD_delete_value' };
delete deleted.a;

// A later definition of a key replaces the property.
const overwritten = { a: 'DEAD_overwrite', a: 1 };
export function main() {
  return overwritten.a;
}
