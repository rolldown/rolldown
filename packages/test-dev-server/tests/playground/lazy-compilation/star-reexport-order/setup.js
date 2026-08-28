const status = document.getElementById('star-reexport-order-status');

status.textContent = 'ready';

document.getElementById('star-reexport-order-btn').addEventListener('click', async () => {
  status.textContent = 'loading';
  try {
    const plain = await import('./entry-plain.js');
    const withImport = await import('./entry-with-import.js');
    status.textContent = `plain=${plain.foo} with-import=${withImport.foo}`;
  } catch (error) {
    status.textContent = `error: ${error.message}`;
  }
});
