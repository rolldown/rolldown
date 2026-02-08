// With strictExecutionOrder, modules are wrapped with lazy initializers.
// Circular chunk imports should not cause TDZ errors at runtime.
export const state = {};

