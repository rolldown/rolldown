export const state = { value: 'unset' };
export let phase = 'before';

export function mutate() {
  phase = 'after';
}
