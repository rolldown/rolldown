globalThis.log.push('shared');

export const label = 'shared';

export function mark(entry) {
  globalThis.log.push(entry);
}
