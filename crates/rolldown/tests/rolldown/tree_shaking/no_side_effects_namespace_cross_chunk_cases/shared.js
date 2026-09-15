/* @__NO_SIDE_EFFECTS__ */
export function fn() {
  console.log('fn side effect');
}

export const fnExpr = /* @__NO_SIDE_EFFECTS__ */ function () {
  console.log('fnExpr side effect');
};
