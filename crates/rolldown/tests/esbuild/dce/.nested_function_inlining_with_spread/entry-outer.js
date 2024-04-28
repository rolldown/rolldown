import {
	empty1,
	empty2,
	empty3,

	identity1,
	identity2,
	identity3,
} from './inner.js'

check(
	empty1(),
	empty2(args),
	empty3(...args),

	identity1(),
	identity2(args),
	identity3(...args),
)