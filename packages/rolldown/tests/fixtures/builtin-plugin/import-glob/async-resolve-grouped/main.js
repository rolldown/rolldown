export const skipped = import.meta.glob(['#missing/*.js', '#features/*.js'], { eager: true });
export const other = import.meta.glob('#stateful/*.js', { eager: true });
export const unresolved = import.meta.glob('#stateful/*.js', { eager: true });
