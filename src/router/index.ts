import { createRouter, createWebHashHistory } from 'vue-router'

const router = createRouter({
  // Hash history keeps the Tauri webview happy without server rewrites.
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'home', component: () => import('@/views/Home.vue') },
    { path: '/instances', name: 'instances', component: () => import('@/views/Instances.vue') },
    { path: '/instances/:id', name: 'instance-edit', component: () => import('@/views/InstanceEdit.vue') },
    { path: '/homes', name: 'homes', component: () => import('@/views/Homes.vue') },
    { path: '/profiles', name: 'profiles', component: () => import('@/views/Profiles.vue') },
    { path: '/plugins', name: 'plugins', component: () => import('@/views/Plugins.vue') },
    { path: '/versions', name: 'versions', component: () => import('@/views/Versions.vue') },
    { path: '/settings', name: 'settings', component: () => import('@/views/Settings.vue') },
    { path: '/tasks', name: 'tasks', component: () => import('@/views/Tasks.vue') },
    { path: '/setup', name: 'setup', component: () => import('@/views/Setup.vue') },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})

export default router
