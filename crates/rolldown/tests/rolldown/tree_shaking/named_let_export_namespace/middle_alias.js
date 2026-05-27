import * as api from './api_alias';

// `let` with no write references is observationally non-reassignable —
// `SymbolRefFlags::IsNotReassigned` is set and the namespace rewire applies,
// matching the `const` case.
export let res = api;
