// Only `let` is in scope: another module can read a `var` before this module runs.
export var legacy = false;

export function enable_legacy() {
  legacy = true;
}
