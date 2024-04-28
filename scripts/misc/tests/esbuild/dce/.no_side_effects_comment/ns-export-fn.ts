namespace ns {
	//! These should all have "no side effects"
	/* @__NO_SIDE_EFFECTS__ */ export function a() {}
	/* @__NO_SIDE_EFFECTS__ */ export function* b() {}
	/* @__NO_SIDE_EFFECTS__ */ export async function c() {}
	/* @__NO_SIDE_EFFECTS__ */ export async function* d() {}
}