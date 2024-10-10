using x = y
switch (foo) {
	case 0: using c = d
	default: using e = f
}

async function foo() {
	using x = y
	switch (foo) {
		case 0: using c = d
		default: using e = f
	}
	switch (foo) {
		case 0: await using c = d
		default: using e = f
	}
}