if (globalThis.never) var VALUE = 1n;
const ALIAS = VALUE;

// Two reads keep the pre-scan DCE from replacing `ALIAS` with `VALUE`.
1n * ALIAS;
1n * ALIAS;
