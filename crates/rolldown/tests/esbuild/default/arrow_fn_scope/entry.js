tests = {
	0: ((x = y => x + y, y) => x + y),
	1: ((y, x = y => x + y) => x + y),
	2: ((x = (y = z => x + y + z, z) => x + y + z, y, z) => x + y + z),
	3: ((y, z, x = (z, y = z => x + y + z) => x + y + z) => x + y + z),
	4: ((x = y => x + y, y), x + y),
	5: ((y, x = y => x + y), x + y),
	6: ((x = (y = z => x + y + z, z) => x + y + z, y, z), x + y + z),
	7: ((y, z, x = (z, y = z => x + y + z) => x + y + z), x + y + z),
};