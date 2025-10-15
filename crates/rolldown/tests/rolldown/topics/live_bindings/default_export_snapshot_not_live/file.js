let x = 42;

export function inc() {
  x += 1;
}

// `export default [expr]` doesn't create live binding for `default` export.
// The value is captured at module evaluation time.
export default x;
