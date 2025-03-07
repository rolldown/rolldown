console.log('should not be removed')
export type T = number
// also export value T to ensure rolldown will not emit module is not export `xxx` diagnostic
export const T = 10000
