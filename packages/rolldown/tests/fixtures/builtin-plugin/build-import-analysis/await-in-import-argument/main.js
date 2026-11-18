async function resolveSpecifier() {
  return './lib.js';
}

export async function load() {
  return await import(await resolveSpecifier());
}

export async function loadThen() {
  return await import(await resolveSpecifier()).then((m) => m.foo);
}
