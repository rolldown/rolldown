// These should not cause us to crash
/* @__NO_SIDE_EFFECTS__ */ declare function f1(y) { sideEffect(y) }
/* @__NO_SIDE_EFFECTS__ */ declare const f2 = function (y) { sideEffect(y) }
/* @__NO_SIDE_EFFECTS__ */ declare const f3 = (y) => { sideEffect(y) }
declare const f4 = /* @__NO_SIDE_EFFECTS__ */ function (y) { sideEffect(y) }
declare const f5 = /* @__NO_SIDE_EFFECTS__ */ (y) => { sideEffect(y) }
namespace ns {
	/* @__NO_SIDE_EFFECTS__ */ export declare function f1(y) { sideEffect(y) }
	/* @__NO_SIDE_EFFECTS__ */ export declare const f2 = function (y) { sideEffect(y) }
	/* @__NO_SIDE_EFFECTS__ */ export declare const f3 = (y) => { sideEffect(y) }
	export declare const f4 = /* @__NO_SIDE_EFFECTS__ */ function (y) { sideEffect(y) }
	export declare const f5 = /* @__NO_SIDE_EFFECTS__ */ (y) => { sideEffect(y) }
}
