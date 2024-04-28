export const foo = 123 // Inlining should be prevented by the cycle
export function bar() {
	return foo
}
export { baz } from './circular-re-export-cycle'