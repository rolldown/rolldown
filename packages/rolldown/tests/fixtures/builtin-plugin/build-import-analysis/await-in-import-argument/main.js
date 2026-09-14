async function resolveSpecifier() {
  return './lib.js';
}

// The dynamic import argument contains an `await`. The preload thunk must be
// `async`, otherwise the `await` is stranded in a non-async arrow and the
// emitted chunk is a syntax error.
export const a = await import(await resolveSpecifier());

// Same, via `.then()` -- the non-destructuring wrapping path.
export const b = await import(await resolveSpecifier()).then((m) => m.foo);
