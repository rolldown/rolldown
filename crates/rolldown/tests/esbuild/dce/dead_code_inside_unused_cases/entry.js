// Unknown test value
switch (x) {
	case 0: _ = require('./a'); break
	case 1: _ = require('./b'); break
}

// Known test value
switch (1) {
	case 0: _ = require('./FAIL-known-0'); break
	case 1: _ = require('./a'); break
	case 1: _ = require('./FAIL-known-1'); break
	case 2: _ = require('./FAIL-known-2'); break
}

// Check for "default"
switch (0) {
	case 1: _ = require('./FAIL-default-1'); break
	default: _ = require('./a'); break
}
switch (1) {
	case 1: _ = require('./a'); break
	default: _ = require('./FAIL-default'); break
}
switch (0) {
	case 1: _ = require('./FAIL-default-1'); break
	default: _ = require('./FAIL-default'); break
	case 0: _ = require('./a'); break
}

// Check for non-constant cases
switch (1) {
	case x: _ = require('./a'); break
	case 1: _ = require('./b'); break
	case x: _ = require('./FAIL-x'); break
	default: _ = require('./FAIL-x-default'); break
}

// Check for other kinds of jumps
for (const x of y)
	switch (1) {
		case 0: _ = require('./FAIL-continue-0'); continue
		case 1: _ = require('./a'); continue
		case 2: _ = require('./FAIL-continue-2'); continue
	}
x = () => {
	switch (1) {
		case 0: _ = require('./FAIL-return-0'); return
		case 1: _ = require('./a'); return
		case 2: _ = require('./FAIL-return-2'); return
	}
}

// Check for fall-through
switch ('b') {
	case 'a': _ = require('./FAIL-fallthrough-a')
	case 'b': _ = require('./a')
	case 'c': _ = require('./b'); break
	case 'd': _ = require('./FAIL-fallthrough-d')
}
switch ('b') {
	case 'a': _ = require('./FAIL-fallthrough-a')
	case 'b':
	case 'c': _ = require('./a')
	case 'd': _ = require('./b'); break
	case 'e': _ = require('./FAIL-fallthrough-e')
}