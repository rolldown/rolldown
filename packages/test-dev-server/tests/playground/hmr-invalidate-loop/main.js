import './cycle-a.js';
import './plain.js';

// Accepting cycle-a here keeps the push walk from escalating through the
// entry to a full reload, so the invalidate chain below is what decides.
import.meta.hot?.accept('./cycle-a.js', () => {});
