globalThis.events.push('S3');
export const base = { n: 3 };
export function tag(strings, ...values) {
  return (this === undefined ? 'U' : 'B') + strings.join('|') + values.join(',');
}
