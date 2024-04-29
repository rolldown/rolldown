const enum NonIntegerNumberToString {
	SUPPORTED = '' + 1,
	UNSUPPORTED = '' + 1.5,
}
console.log(
	NonIntegerNumberToString.SUPPORTED,
	NonIntegerNumberToString.UNSUPPORTED,
)

const enum OutOfBoundsNumberToString {
	SUPPORTED = '' + 1_000_000_000,
	UNSUPPORTED = '' + 1_000_000_000_000,
}
console.log(
	OutOfBoundsNumberToString.SUPPORTED,
	OutOfBoundsNumberToString.UNSUPPORTED,
)

const enum TemplateExpressions {
	// TypeScript enums don't handle any of these
	NULL = '' + null,
	TRUE = '' + true,
	FALSE = '' + false,
	BIGINT = '' + 123n,
}
console.log(
	TemplateExpressions.NULL,
	TemplateExpressions.TRUE,
	TemplateExpressions.FALSE,
	TemplateExpressions.BIGINT,
)