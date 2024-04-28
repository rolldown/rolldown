export const foo = 123 // Inlining should be prevented by the cycle
export function bar() {
	return foo
}
import './circular-import-cycle'