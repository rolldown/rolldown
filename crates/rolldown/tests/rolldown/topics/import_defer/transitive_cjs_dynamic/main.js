(globalThis.__extLog ??= []).push('main');

export const load = () => import.defer('./d.js');
