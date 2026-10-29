export let calls = 0;
export function helper(tag) {
  calls += 1;
  return tag + calls;
}
