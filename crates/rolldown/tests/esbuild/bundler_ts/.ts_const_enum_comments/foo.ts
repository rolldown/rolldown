import { Foo } from "./bar";
const enum Bar {
	"%/*" = 1,
	"*/%" = 2,
}
console.log({
	'should have comments': [
		Foo["%/*"],
		Bar["%/*"],
	],
	'should not have comments': [
		Foo["*/%"],
		Bar["*/%"],
	],
});