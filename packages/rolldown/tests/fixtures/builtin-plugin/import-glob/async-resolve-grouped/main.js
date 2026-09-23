export const skipped = import.meta.glob(['#missing/*.js', '#features/*.js'], { eager: true });
export const other = import.meta.glob('#other/*.js', { eager: true });
