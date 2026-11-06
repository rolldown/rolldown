export const { ...direct } = await import('./direct.js');

// Keep the namespace binding from being inlined into the destructuring.
const ns = await import('./separate.js');
export const { ...separate } = ns;
export const { value: separateValue } = ns;
export const { second } = ns;
export const { third } = ns;

export const { excluded, ...mixed } = await import('./mixed.js');

// A local rest binding can still retain just its statically accessed member.
const { ...local } = await import('./local.js');
export const localValue = local.kept;
