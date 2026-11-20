import 'ext-bare';
import { value } from 'ext-named';
import * as extNs from 'ext-ns';
export * from 'ext-star';

globalThis.__extLog.push('d');
export const x = value;
export const nsValue = extNs.default.z;
