// Two deferred imports of one external in one file.
import defer * as one from 'ext-shared';
import defer * as two from 'ext-shared';

export const getC = () => [one, two];
