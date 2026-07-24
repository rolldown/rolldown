// Side-effectful sibling re-exported by the same barrel. Its top-level event keeps the barrel a
// real wrapped execution dependency instead of a fully flattened re-exporter.
(globalThis.__events ??= []).push('sibling');

export const vSib = { value: 3 };
