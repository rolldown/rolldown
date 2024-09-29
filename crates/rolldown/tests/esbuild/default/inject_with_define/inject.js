export let both = 'inject'
export let first = 'TEST FAILED!'
export let second = 'success (identifier)'

let both2 = 'inject'
let first2 = 'TEST FAILED!'
let second2 = 'success (dot name)'
export {
	both2 as 'bo.th',
	first2 as 'fir.st',
	second2 as 'seco.nd',
}