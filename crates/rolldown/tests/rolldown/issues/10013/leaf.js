import { first, second } from 'dep';

// The entries only pull the re-exported `other`, never `value`. Under
// `strictExecutionOrder`, the unused `value = first(second)` body and its
// external `dep` import must both stay out of the shared chunk (#10013).
export { other } from './helper.js';

export const value = first(second);
