import { StateCode } from './state-codes.js';

// uses the imported binding internally
export const initialState = StateCode;

// re-exports the same imported binding for consumers of ./wrapper
export { StateCode } from './state-codes.js';
