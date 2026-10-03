export const SECOND = 1000;
export const NAME = 'rolldown';
// A template literal initializer is a string constant too.
export const EMPTY = ``;
export const GREETING = `hello ${1}`;
// These unused derived constants coerce a constant that this module declares.
export const MINUTE = 60 * SECOND;
export const LABEL = `${NAME}!`;
export const NEGATED = -SECOND;
export const TOTAL = SECOND + 1;
`${EMPTY}`;
`${GREETING}`;
// `LABEL` is a template whose substitution is a constant of this module.
`${LABEL}`;

// The follow-up case in the issue coerces a template literal constant twice.
const a = ``;
`${a}`;
`${a}`;
