export const SECOND = 1000;
export const NAME = 'rolldown';
// A template literal initializer is a string constant too.
export const EMPTY = ``;
export const GREETING = `hello ${1}`;
// Unused derived constants that coerce a constant declared in this module.
export const MINUTE = 60 * SECOND;
export const LABEL = `${NAME}!`;
export const NEGATED = -SECOND;
export const TOTAL = SECOND + 1;
`${EMPTY}`;
`${GREETING}`;
// `LABEL` is a template whose substitution is a constant of this module.
`${LABEL}`;

// The follow-up case in the issue: a template literal constant coerced twice.
const a = ``;
`${a}`;
`${a}`;
