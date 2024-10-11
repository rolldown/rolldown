let tests = {
	// Exponentiation operator
	0: a ** b ** c,
	1: (a ** b) ** c,

	// Exponentiation assignment operator
	2: a **= b,
	3: a.b **= c,
	4: a[b] **= c,
	5: a().b **= c,
	6: a()[b] **= c,
	7: a[b()] **= c,
	8: a()[b()] **= c,

	// These all should not need capturing (no object identity)
	9: a[0] **= b,
	10: a[false] **= b,
	11: a[null] **= b,
	12: a[void 0] **= b,
	13: a[123n] **= b,
	14: a[this] **= b,

	// These should need capturing (have object identitiy)
	15: a[/x/] **= b,
	16: a[{}] **= b,
	17: a[[]] **= b,
	18: a[() => {}] **= b,
	19: a[function() {}] **= b,
}