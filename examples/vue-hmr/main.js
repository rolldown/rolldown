import { createApp } from 'vue'
import App from './App.vue'
import './foo' // Make sure rolldown inject runtime

createApp(App).mount('#app')
