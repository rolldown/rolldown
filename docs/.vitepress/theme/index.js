import DefaultTheme from 'vitepress/theme'
import './custom.css'
import { GroupIconComponent } from 'vitepress-plugin-group-icons/client'

export default {
  ...DefaultTheme,
  enhanceApp({ app }) {
    app.use(GroupIconComponent, {
      homebrew: 'logos:homebrew',
      cargo: 'vscode-icons:file-type-cargo',
    })
  },
}
