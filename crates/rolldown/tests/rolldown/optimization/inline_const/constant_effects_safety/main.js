import { DEV, PROD, mutable, object } from './flags.js';

if ((console.log('TEST_EFFECT'), DEV)) console.log('DEAD');
if (DEV) console.log('DEAD');
else console.log('LIVE_ELSE');
if (PROD) console.log('LIVE_TRUE');
if (mutable) console.log('MUTABLE');
`${object}`;
