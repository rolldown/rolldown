//! These should all have "no side effects"
x([
	/* #__NO_SIDE_EFFECTS__ */ y => y,
	/* #__NO_SIDE_EFFECTS__ */ () => {},
	/* #__NO_SIDE_EFFECTS__ */ (y) => (y),
	/* #__NO_SIDE_EFFECTS__ */ async y => y,
	/* #__NO_SIDE_EFFECTS__ */ async () => {},
	/* #__NO_SIDE_EFFECTS__ */ async (y) => (y),
])