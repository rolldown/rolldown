export function getShared() {
  return 'shared';
}

globalThis.sharedEvaluated = (globalThis.sharedEvaluated ?? 0) + 1;
