import './cases/require'
import './cases/manual_reexport'
import './cases/deconflict_import_bindings'

if (import.meta.hot) {
  import.meta.hot.accept()
}