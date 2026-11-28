// An uncalled dynamic request creates no static evaluation dependency.
export const load = () => import.defer('./dep.js');
export const x = 'independent';
