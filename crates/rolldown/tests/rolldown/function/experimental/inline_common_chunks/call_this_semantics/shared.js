export function tag(strings) {
  globalThis.events.push(`tag:${this === undefined}`);
  return strings[0];
}

export function optionalCall() {
  globalThis.events.push(`optional:${this === undefined}`);
}
