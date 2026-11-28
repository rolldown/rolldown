export const eager = import.meta.glob('./files/*.js', {
  eager: true,
  with: { type: 'text', 'custom-key': 'eager' },
});
export const lazy = import.meta.glob('./files/*.js', {
  import: 'default',
  with: { type: 'text', 'custom-key': 'a"b\\c\n' },
});
