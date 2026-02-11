import { TIMEOUT_FROM_VENDOR } from './vendor_dep.js';

// This module has no dependencies outside the vendor group. The entry imports it,
// creating an entry->vendor edge. Together with vendor_dep.js, this produces a
// direct chunk cycle that requires the safety net to avoid TDZ at runtime.
export const TIMEOUT = TIMEOUT_FROM_VENDOR;

