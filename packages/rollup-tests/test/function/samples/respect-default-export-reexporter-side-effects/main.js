import { foo } from './reexporter-chain-1.js';

assert.deepStrictEqual(foo, {
	chain2: 'modified',
	chain3: 'modified',
	chain4: 'modified',
	chain5: 'modified'
});
