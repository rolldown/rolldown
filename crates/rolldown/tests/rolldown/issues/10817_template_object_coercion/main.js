// Preserve the source flag order to exercise runtime regexp coercion.
// oxfmt-ignore
export const text = `${/foo/ig}`;
export const result = text || '';

const originalObjectToString = Object.prototype.toString;
Object.prototype.toString = () => 'custom object';
export const objectText = `${{}}`;
export const objectResult = objectText || '';
Object.prototype.toString = originalObjectToString;

const originalArrayToString = Array.prototype.toString;
Array.prototype.toString = () => 'custom array';
export const arrayText = `${[1, 2]}`;
export const arrayResult = arrayText || '';
Array.prototype.toString = originalArrayToString;

export const bigintText = `${0xffn}`;
export const bigintResult = bigintText || '';

// oxfmt-ignore
export const compoundText = `${/foo/ig + ''}`;
export const compoundResult = compoundText || '';

// oxfmt-ignore
const coerced = /foo/ig + '';
export const cachedText = `${coerced}`;
export const cachedResult = cachedText || '';
