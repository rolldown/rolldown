let count = 0;

export function reset() {
  count = 0;
}

export function inc() {
  count += 1;
}

// This creates live binding for `default` export.
export { count as default }
