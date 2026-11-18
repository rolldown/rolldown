// Another module imports the writer. Nothing calls that module's function, but the rule
// does not follow references through other modules.
export let cross = false;

export function enable_cross() {
  cross = true;
}
