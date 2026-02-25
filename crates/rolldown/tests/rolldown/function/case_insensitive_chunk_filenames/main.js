// Entry that dynamically imports modules whose names differ only by case
import('./Edit.js').then((m) => console.log(m.name));
import('./lowercase/edit.js').then((m) => console.log(m.name));
