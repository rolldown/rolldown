import { getConfig } from './helpers.js';

// This module depends on entry-chunk helpers, creating a vendor->entry edge.
// It is NOT imported directly by the entry. Instead, shared.js imports it.
export const TIMEOUT_FROM_VENDOR = getConfig('TIMEOUT', 300000);

