let count = 0;

export function reset() {
  count = 0;
}

export function inc() {
  count += 1;
}

// `export default [expr]` doesn't create live binding for `default` export.
export default count
