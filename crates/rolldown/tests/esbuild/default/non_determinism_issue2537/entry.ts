export function aap(noot: boolean, wim: number) {
	let mies = "teun"
	if (noot) {
		function vuur(v: number) {
			return v * 2
		}
		function schaap(s: number) {
			return s / 2
		}
		mies = vuur(wim) + schaap(wim)
	}
	return mies
}