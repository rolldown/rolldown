export const short1 = ''

export let short2 = ''

export function short3() {}

export class short4 {}

export let nonShort1 = ''
// Trigger a re-assignment
nonShort1 = ''

export function nonShort2() {}
// Trigger a re-assignment
nonShort2 = () => {}