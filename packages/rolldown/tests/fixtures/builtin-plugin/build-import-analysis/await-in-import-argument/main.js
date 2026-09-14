async function resolveSpecifier() {
  return './lib.js';
}

export const a = await import(await resolveSpecifier());

export const b = await import(await resolveSpecifier()).then((m) => m.foo);
