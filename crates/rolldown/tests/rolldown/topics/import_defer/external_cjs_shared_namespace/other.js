// Another entry chunk with its own deferred import of the external.
import defer * as ns from 'ext-shared';

export const getO = () => ns;
