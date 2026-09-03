<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useLauncherStore } from '@/stores/launcher'
import type { ThemeMode } from '@/api/types'
import { api } from '@/api'
import { Message } from '@arco-design/web-vue'
import ModpackImportDialog from '@/components/ModpackImportDialog.vue'
import PluginFileImportDialog from '@/components/PluginFileImportDialog.vue'
import {
  MENU_NAVIGATE_EVENTS,
  MENU_REFRESH_EVENTS,
  handleAppKeydown,
  resolveMenuRoute,
  type ShortcutRoute,
} from '@/shortcuts'

const route = useRoute()
const router = useRouter()
const { t, locale } = useI18n()
const store = useLauncherStore()
const isTauri = api.isTauri

// --- Theme: light / dark / follow system -------------------------------------

const themeMedia = window.matchMedia('(prefers-color-scheme: dark)')

/** Applies the effective Arco theme to <body> (body[arco-theme='dark']). */
function applyTheme(mode: ThemeMode) {
  const dark = mode === 'dark' || (mode === 'system' && themeMedia.matches)
  if (dark) {
    document.body.setAttribute('arco-theme', 'dark')
  } else {
    document.body.removeAttribute('arco-theme')
  }
}

function onSystemThemeChange() {
  if (store.settings.theme === 'system') applyTheme('system')
}

onMounted(async () => {
  await store.init()
  locale.value = store.settings.locale || 'zh-CN'
  // Apply the persisted theme early (before init resolves the settings may
  // still be defaults; the watch below re-applies on any change).
  applyTheme(store.settings.theme || 'system')
  themeMedia.addEventListener('change', onSystemThemeChange)
  // If Node.js is missing, guide the user to install it before anything else.
  if (!store.runtime?.node?.installed && route.name !== 'setup') {
    router.push({ name: 'setup' })
  }
  await setupModpackEntryPoints()
  window.addEventListener('keydown', onAppKeydown)
  await setupMenuListeners()
})

// --- In-app shortcuts (t4/t9): Cmd/Ctrl+1/2/3/4, Cmd+, K/R, Esc --------------

function goShortcut(name: ShortcutRoute) {
  if (name === 'instances' && route.name === 'instance-edit') return
  const target =
    name === 'instances' ? 'instances' : name === 'download' ? 'download-create' : name
  if (route.name === target) return
  void router.push({ name: target }).catch(() => undefined)
}

function refreshShortcut() {
  void Promise.allSettled([store.refreshInstances(), store.refreshTasks(), store.checkRuntime()])
}

function backShortcut() {
  if (route.name === 'home') return
  router.back()
}

function onAppKeydown(e: KeyboardEvent) {
  if (handleAppKeydown(e, { go: goShortcut, refresh: refreshShortcut, back: backShortcut })) {
    e.preventDefault()
  }
}

let unlistenMenu: (() => void)[] = []

/** Backend menu events (t3 Rust menu) drive the same routes as the shortcuts. */
async function setupMenuListeners() {
  if (!isTauri) return
  try {
    const { listen } = await import('@tauri-apps/api/event')
    for (const name of MENU_NAVIGATE_EVENTS) {
      const un = await listen<unknown>(name, (event) => {
        const target = resolveMenuRoute(event.payload)
        if (target) goShortcut(target)
      })
      unlistenMenu.push(un)
    }
    for (const name of MENU_REFRESH_EVENTS) {
      const un = await listen(name, () => refreshShortcut())
      unlistenMenu.push(un)
    }
    // H3: Help-menu actions have no route target — handle them here instead
    // of leaving the clicks dead. Same semantics as the tray counterpart.
    unlistenMenu.push(await listen('check-update', () => void onMenuCheckUpdate()))
    unlistenMenu.push(await listen('open-help', () => void onMenuOpenHelp()))
  } catch {
    // Event bridge unavailable (browser preview); shortcuts still work.
  }
}

let checkingMenuUpdate = false

/**
 * Native menu → 检查更新 (H3 fix): same semantics as the tray counterpart
 * (`check_update_from_tray`) — open the release page when an update exists,
 * otherwise toast up-to-date. Failures surface as an error toast so the
 * click never looks dead.
 */
async function onMenuCheckUpdate() {
  if (checkingMenuUpdate) return
  checkingMenuUpdate = true
  try {
    const info = await api.checkLauncherUpdate('dev')
    if (info.up_to_date) {
      Message.success(t('settings.update.upToDate'))
    } else {
      Message.info(t('settings.update.available', { version: info.latest ?? '' }))
      if (info.url) await api.openExternal(info.url)
    }
  } catch (e) {
    Message.error(String(e))
  } finally {
    checkingMenuUpdate = false
  }
}

/** Native menu → 使用文档 (H3 fix): open the project README in the browser. */
async function onMenuOpenHelp() {
  try {
    await api.openExternal('https://github.com/dsh-plugins/dsh-launcher')
  } catch (e) {
    Message.error(String(e))
  }
}

// --- Modpack entry points: .dspack/.tgz drag-drop + dsh-launcher://pack?url= ---

const modpackImportVisible = ref(false)
const modpackImportSource = ref('')
const pluginFileVisible = ref(false)
const pluginFilePath = ref('')
let unlistenDrag: (() => void) | undefined
let unlistenDeepLink: (() => void) | undefined

function openModpackImport(source: string) {
  modpackImportSource.value = source
  modpackImportVisible.value = true
}

function openPluginFileImport(path: string) {
  pluginFilePath.value = path
  pluginFileVisible.value = true
}

/** A dropped .dspack is always a modpack; a .tgz may be a legacy modpack or
 * a plugin tarball — probe the manifest and route accordingly. */
async function handleDroppedPack(path: string) {
  if (/\.dspack$/i.test(path)) {
    openModpackImport(path)
    return
  }
  try {
    await api.readModpackManifest(path)
    openModpackImport(path)
  } catch {
    openPluginFileImport(path)
  }
}

/** Instance id when the drop happened on an instance editor page. */
const dropInstanceId = computed(() =>
  route.name === 'instance-edit' ? String(route.params.id ?? '') : undefined,
)

async function setupModpackEntryPoints() {
  if (!isTauri) return
  // Cold start via protocol: the deep link arrived in argv before the
  // webview could listen; pull it now.
  const pending = await api.pendingDeepLink()
  if (pending) handleDeepLink(pending)
  const { getCurrentWebview } = await import('@tauri-apps/api/webview')
  unlistenDrag = await getCurrentWebview().onDragDropEvent((event) => {
    if (event.payload.type !== 'drop') return
    const path = event.payload.paths.find((p) => /\.(dspack|tgz)$/i.test(p))
    if (path) void handleDroppedPack(path)
  })
  const { listen } = await import('@tauri-apps/api/event')
  unlistenDeepLink = await listen<string>('deep-link', (event) => handleDeepLink(event.payload))
}

/** dsh-launcher://pack?url=<tgz> → modpack import; dsh-launcher://launch → start instance. */
function handleDeepLink(raw: string) {
  try {
    const u = new URL(raw)
    if (u.protocol !== 'dsh-launcher:') return
    if (u.host === 'pack') {
      const packUrl = u.searchParams.get('url')
      if (packUrl) openModpackImport(packUrl)
    } else if (u.host === 'launch') {
      void launchFromDeepLink(u)
    }
  } catch {
    // Not a URL we understand; ignore.
  }
}

/** dsh-launcher://launch?instance=<name|id>&profile=<name> (issue #9). */
async function launchFromDeepLink(u: URL) {
  const ref = u.searchParams.get('instance')
  if (!ref) return
  const inst = store.instances.find((i) => i.id === ref || i.name === ref)
  if (!inst) {
    Message.error(t('modpackLaunch.instanceNotFound', { name: ref }))
    return
  }
  try {
    const state = store.statusOf(inst.id).state
    if (state !== 'running' && state !== 'starting') {
      const profile = u.searchParams.get('profile') || inst.default_profile || 'web'
      await api.startInstance(inst.id, profile)
    }
    // start_instance returns right after spawn; the web URL (and the
    // open-in-browser readiness check) only exist once the instance is running.
    await openBrowserWhenReady(inst.id)
  } catch (e) {
    Message.error(String(e))
  }
}

/** Waits for the instance to report `running` with a URL, then opens it in the system browser. */
async function openBrowserWhenReady(id: string) {
  const deadline = Date.now() + 120_000
  for (;;) {
    const st = store.statusOf(id)
    if (st.state === 'running' && st.url) {
      await api.openInstanceWindow(id)
      return
    }
    if (st.state === 'exited' || Date.now() > deadline) {
      // Last attempt: surface the backend's own error if it is not ready.
      await api.openInstanceWindow(id)
      return
    }
    await new Promise((r) => setTimeout(r, 500))
  }
}

onUnmounted(() => {
  themeMedia.removeEventListener('change', onSystemThemeChange)
  window.removeEventListener('keydown', onAppKeydown)
  unlistenDrag?.()
  unlistenDeepLink?.()
  unlistenMenu.forEach((un) => un())
  unlistenMenu = []
})

watch(
  () => store.settings.locale,
  (v) => {
    if (v) locale.value = v
  },
)

watch(
  () => store.settings.theme,
  (v) => {
    if (v) applyTheme(v)
  },
)

// --- Sidebar navigation -------------------------------------------------------

type NavKey = 'home' | 'instances' | 'plugins' | 'tasks' | 'settings'

const siderCollapsed = ref(localStorage.getItem('dsh-launcher.siderCollapsed') === '1')

function toggleSider() {
  siderCollapsed.value = !siderCollapsed.value
  try {
    localStorage.setItem('dsh-launcher.siderCollapsed', siderCollapsed.value ? '1' : '0')
  } catch {
    // Private mode etc: collapse state is best-effort.
  }
}

/** Maps every route onto one highlighted sidebar entry. */
const navSelected = computed<NavKey>(() => {
  const name = route.name as string
  if (name === 'instances' || name === 'instance-edit' || name === 'modpack-export') return 'instances'
  // The create-instance wizard lives inside 实例管理 (no separate entry).
  if (name === 'download-create' || name === 'download-name' || name === 'download') return 'instances'
  if (name === 'download-plugins' || name === 'plugin-version' || name === 'plugin-install') {
    return 'plugins'
  }
  if (name === 'tasks') return 'tasks'
  if (name === 'settings') return 'settings'
  return 'home'
})

function navGo(key: NavKey) {
  switch (key) {
    case 'home':
      void router.push({ name: 'home' }).catch(() => undefined)
      break
    case 'instances':
      if (route.name !== 'instances' && route.name !== 'instance-edit') {
        void router.push({ name: 'instances' }).catch(() => undefined)
      } else if (route.name !== 'instances') {
        void router.push({ name: 'instances' }).catch(() => undefined)
      }
      break
    case 'plugins':
      void router.push({ name: 'download-plugins' }).catch(() => undefined)
      break
    case 'tasks':
      void router.push({ name: 'tasks' }).catch(() => undefined)
      break
    case 'settings':
      void router.push({ name: 'settings' }).catch(() => undefined)
      break
  }
}

const runningInstanceCount = computed(
  () => store.instances.filter((i) => store.statusOf(i.id).state === 'running').length,
)

/** Slim header title for the current route. */
const pageTitle = computed(() => {
  const name = route.name as string
  switch (name) {
    case 'home':
      return t('nav.home')
    case 'instances':
      return t('instances.title')
    case 'instance-edit':
      return route.params.id ? t('instanceEdit.titleEdit') : t('instanceEdit.titleNew')
    case 'modpack-export':
      return t('exportPack.title')
    case 'download-create':
    case 'download-name':
    case 'download':
      return t('download.createInstance')
    case 'download-plugins':
    case 'plugin-version':
    case 'plugin-install':
      return t('download.plugins')
    case 'tasks':
      return t('tasks.title')
    case 'settings':
      return t('settings.title')
    case 'setup':
      return t('setup.title')
    default:
      return t('app.title')
  }
})

/** Wizard-like pages get an explicit back affordance next to the title. */
const showBack = computed(() => {
  const name = route.name as string
  return (
    name === 'instance-edit' ||
    name === 'modpack-export' ||
    name === 'download-name' ||
    name === 'plugin-version' ||
    name === 'plugin-install'
  )
})

function onHeaderBack() {
  router.back()
}

const appWindow = (() => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  return (window as any)?.__TAURI_INTERNALS__ ? loadWindowApi() : null
})()

async function loadWindowApi() {
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  return getCurrentWindow()
}

// Manual drag: native data-tauri-drag-region only works on elements carrying
// the attribute, which leaves gaps undraggable.
async function onHeaderMouseDown(e: MouseEvent) {
  if (!isTauri || e.button !== 0) return
  const el = e.target as HTMLElement | null
  if (el?.closest('.nav-item, a, button, input, [data-no-drag]')) return
  const w = await appWindow
  w?.startDragging()
}
</script>

<template>
  <a-layout class="app-shell">
    <!-- Tauri Overlay 模式下红绿灯悬浮在左上角：单独留一条拖拽栏，
         下方整排内容整体下移，不再跟红绿灯挤在同一行。 -->
    <div v-if="isTauri" class="traffic-bar" @mousedown="onHeaderMouseDown" />
    <div class="app-body">
    <aside class="app-sider" :class="{ collapsed: siderCollapsed }">
      <div class="sider-brand" @mousedown="onHeaderMouseDown">
        <span v-if="!siderCollapsed" class="sider-title">{{ t('app.title') }}</span>
        <button class="sider-collapse" :title="t('nav.toggleSider')" @click="toggleSider">
          {{ siderCollapsed ? '»' : '«' }}
        </button>
      </div>

      <nav class="sider-nav">
        <button
          class="nav-item"
          :class="{ active: navSelected === 'home' }"
          :title="siderCollapsed ? t('nav.home') : ''"
          @click="navGo('home')"
        >
          <span class="nav-icon">▶</span>
          <span v-if="!siderCollapsed" class="nav-label">{{ t('nav.home') }}</span>
        </button>
        <button
          class="nav-item"
          :class="{ active: navSelected === 'instances' }"
          :title="siderCollapsed ? t('nav.instances') : ''"
          @click="navGo('instances')"
        >
          <span class="nav-icon">🗂</span>
          <span v-if="!siderCollapsed" class="nav-label">{{ t('nav.instances') }}</span>
          <a-tag v-if="!siderCollapsed && runningInstanceCount > 0" size="small" color="green">
            {{ runningInstanceCount }}
          </a-tag>
        </button>

        <button
          class="nav-item"
          :class="{ active: navSelected === 'plugins' }"
          :title="siderCollapsed ? t('nav.plugins') : ''"
          @click="navGo('plugins')"
        >
          <span class="nav-icon">🧩</span>
          <span v-if="!siderCollapsed" class="nav-label">{{ t('nav.plugins') }}</span>
        </button>

        <button
          class="nav-item"
          :class="{ active: navSelected === 'tasks' }"
          :title="siderCollapsed ? t('nav.tasks') : ''"
          @click="navGo('tasks')"
        >
          <span class="nav-icon">⏱</span>
          <span v-if="!siderCollapsed" class="nav-label">{{ t('nav.tasks') }}</span>
          <a-badge
            v-if="!siderCollapsed && store.runningTaskCount > 0"
            :count="store.runningTaskCount"
          />
          <span v-if="siderCollapsed && store.runningTaskCount > 0" class="nav-dot" />
        </button>
        <button
          class="nav-item"
          :class="{ active: navSelected === 'settings' }"
          :title="siderCollapsed ? t('nav.settings') : ''"
          @click="navGo('settings')"
        >
          <span class="nav-icon">⚙</span>
          <span v-if="!siderCollapsed" class="nav-label">{{ t('nav.settings') }}</span>
        </button>
      </nav>

      <div v-if="!siderCollapsed" class="sider-footer">
        <span v-if="store.runningTaskCount > 0" class="sider-task-hint">
          {{ t('tasks.runningHint', { count: store.runningTaskCount }) }}
        </span>
        <span v-else class="sider-task-idle">{{ t('tasks.idleHint') }}</span>
      </div>
    </aside>

    <a-layout class="app-main">
      <a-layout-header class="app-header" @mousedown="onHeaderMouseDown">
        <button v-if="showBack" class="header-back-btn" @click="onHeaderBack">←</button>
        <span class="header-title">{{ pageTitle }}</span>
        <a-tag v-if="!isTauri" size="small" color="orange">{{ t('app.mockBadge') }}</a-tag>
        <span class="header-spacer" />
        <button
          class="header-refresh"
          :title="t('common.refresh')"
          data-no-drag
          @click="refreshShortcut"
        >
          ⟳
        </button>
      </a-layout-header>
      <a-layout-content class="app-content">
        <a-scrollbar
          type="track"
          outer-style="height: 100%"
          style="height: 100%; overflow-y: auto"
        >
          <router-view />
        </a-scrollbar>
      </a-layout-content>
    </a-layout>
    </div>

    <ModpackImportDialog v-model:visible="modpackImportVisible" :initial-source="modpackImportSource" />
    <PluginFileImportDialog
      v-model:visible="pluginFileVisible"
      :file-path="pluginFilePath"
      :initial-instance-id="dropInstanceId"
    />
  </a-layout>
</template>

<style lang="scss" scoped>
.app-shell {
  height: 100%;
  display: flex;
  flex-direction: column;
}

// Tauri Overlay 红绿灯独占条：高度盖住悬浮的红绿灯（约 28px 外加
// 上下呼吸空间），整块可拖拽移动窗口。
.traffic-bar {
  height: 38px;
  flex-shrink: 0;
  -webkit-app-region: drag;
}

.app-body {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: row;
}

.app-sider {
  width: var(--dl-sider-width);
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: var(--color-bg-2);
  border-right: 1px solid var(--color-border-2);
  transition: width 0.15s ease;

  &.collapsed {
    width: var(--dl-sider-collapsed-width);

    .sider-brand {
      justify-content: center;
      padding: 0 8px;
    }

    .nav-item {
      justify-content: center;
      padding: 9px 0;
    }
  }
}

.sider-brand {
  display: flex;
  align-items: center;
  gap: 8px;
  height: var(--dl-header-height);
  padding: 0 12px;
  border-bottom: 1px solid var(--color-border-2);
  flex-shrink: 0;
  white-space: nowrap;
  overflow: hidden;
  // Draggable so the window can be moved from the brand area too.
  -webkit-app-region: drag;

  button {
    -webkit-app-region: no-drag;
  }
}

.sider-title {
  font-size: 14px;
  font-weight: 700;
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.sider-collapse {
  border: none;
  background: transparent;
  color: var(--color-text-3);
  cursor: pointer;
  border-radius: 6px;
  padding: 2px 6px;
  font-size: 13px;

  &:hover {
    background: var(--color-fill-2);
    color: rgb(var(--primary-6));
  }
}

.sider-nav {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 10px 8px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  padding: 9px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--color-text-2);
  font-size: 13px;
  cursor: pointer;
  text-align: left;
  position: relative;

  &:hover {
    background: var(--color-fill-2);
    color: var(--color-text-1);
  }

  &.active {
    background: rgb(var(--primary-6) / 10%);
    color: rgb(var(--primary-6));
    font-weight: 600;
  }
}

.nav-icon {
  width: 20px;
  text-align: center;
  font-size: 15px;
  flex-shrink: 0;
}

.nav-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.nav-dot {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: rgb(var(--primary-6));
}

.sider-footer {
  padding: 10px 12px;
  border-top: 1px solid var(--color-border-2);
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.app-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.app-content {
  flex: 1;
  min-height: 0;
  overflow: hidden;
}

.app-header {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  gap: 10px;
  height: var(--dl-header-height);
  padding: 0 16px;
  background: var(--color-bg-2);
  border-bottom: 1px solid var(--color-border-2);
}

.header-back-btn {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 16px;
  color: var(--color-text-2);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;

  &:hover {
    background: var(--color-fill-2);
    color: rgb(var(--primary-6));
  }
}

.header-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--color-text-1);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.header-spacer {
  flex: 1;
}

.header-refresh {
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 15px;
  color: var(--color-text-3);
  background: transparent;
  border: none;
  border-radius: 6px;
  cursor: pointer;

  &:hover {
    background: var(--color-fill-2);
    color: rgb(var(--primary-6));
  }
}
</style>
