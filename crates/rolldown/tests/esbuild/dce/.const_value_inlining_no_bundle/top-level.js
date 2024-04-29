// These should be kept because they are top-level and tree shaking is not enabled
const n_keep = null
const u_keep = undefined
const i_keep = 1234567
const f_keep = 123.456
const s_keep = ''

// Values should still be inlined
console.log(
	// These are doubled to avoid the "inline const/let into next statement if used once" optimization
	n_keep, n_keep,
	u_keep, u_keep,
	i_keep, i_keep,
	f_keep, f_keep,
	s_keep, s_keep,
)