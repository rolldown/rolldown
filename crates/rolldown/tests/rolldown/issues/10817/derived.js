import { SECOND, NAME, EMPTY, LABEL as BANG } from './constants.js';

// These unused derived constants coerce an imported constant.
export const MINUTE = 60 * SECOND;
export const LABEL = `${NAME}!`;
// This bare expression statement only coerces an imported constant.
60 * SECOND;
`${NAME}`;
`${EMPTY}`;
`${BANG}`;

export const used = 'used';
