function earlyReturn() {
	// This comes up when doing conditional compilation with "DropLabels"
	keep: {
		onlyWithKeep()
		return
	}
	onlyWithoutKeep()
}
function loop() {
	if (foo()) {
		keep: {
			bar()
			return;
		}
	}
}