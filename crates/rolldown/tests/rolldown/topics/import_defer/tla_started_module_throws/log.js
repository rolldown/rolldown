// The entry can reject, so the test reads the log from `globalThis`.
export const log = (globalThis.deferLog = []);
