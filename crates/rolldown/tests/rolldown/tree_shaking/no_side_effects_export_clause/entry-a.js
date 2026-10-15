import { declared } from './declared.js';
import { listed } from './listed.js';
import { alias } from './aliased.js';
import { arrow } from './arrow.js';
import defaultIdentifier from './default-identifier.js';
import defaultSpecifier from './default-specifier.js';
import { reassigned } from './reassigned.js';

declared('removed: export function');
listed('removed: export { fn }');
alias('removed: export { fn as alias }');
arrow('removed: const fn = () => {} + export { fn }');
defaultIdentifier('removed: export default fn');
defaultSpecifier('removed: export { fn as default }');
const [first] = listed('removed: destructured result');

reassigned('kept: reassigned after the annotation');
export const result = listed('kept: result is used');
