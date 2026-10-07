import './cases/require';
import './cases/manual_reexport';
import './cases/deconflict_import_bindings';
import './cases/string_import_name';
import './cases/imported_callee_this';

if (import.meta.hot) {
  import.meta.hot.accept();
}
