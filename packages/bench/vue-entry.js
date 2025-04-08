import { createApp } from 'vue';
import { createRouter, createWebHistory } from 'vue-router';

const router = createRouter({
  history: createWebHistory(),
});
const app = createApp({});

app.use(router);
app.mount('#app');
