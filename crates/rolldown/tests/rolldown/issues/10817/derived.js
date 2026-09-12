import { SECOND, NAME, EMPTY, LABEL as BANG } from './constants.js';

// Unused derived constants that coerce an imported constant.
export const MINUTE = 60 * SECOND;
export const LABEL = `${NAME}!`;
// A bare expression statement that only coerces an imported constant.
60 * SECOND;
`${NAME}`;
`${EMPTY}`;
`${BANG}`;

export const used = 'used';
