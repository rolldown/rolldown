// Note: Only remove "using" if it's null or undefined and not awaited

using null_remove = null
using null_keep = null
await using await_null_keep = null

// This has a side effect: throwing an error
using throw_keep = {}

using dispose_keep = { [Symbol.dispose]() { console.log('side effect') } }
await using await_asyncDispose_keep = { [Symbol.asyncDispose]() { console.log('side effect') } }

using undef_remove = undefined
using undef_keep = undefined
await using await_undef_keep = undefined

// Assume these have no side effects
const Symbol_dispose_remove = Symbol.dispose
const Symbol_asyncDispose_remove = Symbol.asyncDispose

console.log(
	null_keep,
	undef_keep,
)