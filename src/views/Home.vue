<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Message, Notification } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'
import type { DshInstance } from '@/api/types'
import launcherDefaultIcon from '@/assets/launcher-icon.png'
import NewInstanceDialog from '@/components/NewInstanceDialog.vue'

const router = useRouter()
const { t } = useI18n()
const store = useLauncherStore()

// --- Instance icons -----------------------------------------------------------

const iconMap = ref<Record<string, string | null>>({})

async function loadIcons() {
  const next: Record<string, string | null> = {}
  for (const inst of store.instances) {
    if (!inst.icon) {
      next[inst.id] = null
      continue
    }
    try {
      next[inst.id] = await api.readInstanceIcon(inst.id)
    } catch {
      next[inst.id] = null
    }
  }
  iconMap.value = next
}

watch(
  () => store.instances.map((i) => `${i.id}:${i.icon ?? ''}`).join(','),
  loadIcons,
  { immediate: true },
)

onMounted(() => {
  ensureProfiles()
})

// --- Per-card profile state ---------------------------------------------------

const profilesById = ref<Record<string, string[]>>({})
const profileSel = ref<Record<string, string | undefined>>({})
const profilesLoading = ref<Record<string, boolean>>({})
const restarting = ref<Record<string, boolean>>({})

async function loadProfilesFor(inst: DshInstance) {
  if (profilesLoading.value[inst.id]) return
  profilesLoading.value[inst.id] = true
  try {
    const list = await api.listProfiles(inst.home_id)
    profilesById.value[inst.id] = list
    const keep = profileSel.value[inst.id]
    if (keep && list.includes(keep)) return
    profileSel.value[inst.id] =
      (inst.last_profile && list.includes(inst.last_profile) && inst.last_profile) ||
      (inst.default_profile && list.includes(inst.default_profile) && inst.default_profile) ||
      list[0] ||
      undefined
    if (list.length === 0) {
      Message.warning(t('home.noProfile'))
    }
  } catch (e) {
    Message.error(t('common.operationFailed', { msg: String(e) }))
  } finally {
    profilesLoading.value[inst.id] = false
  }
}

function ensureProfiles() {
  for (const inst of store.instances) {
    if (!(inst.id in profilesById.value)) void loadProfilesFor(inst)
  }
}

watch(
  () => store.instances.map((i) => i.id).join(','),
  () => ensureProfiles(),
  { immediate: true },
)

function touchLastUsed(id: string) {
  if (store.settings.last_instance_id === id) return
  void api.updateSettings({ last_instance_id: id }).then((s) => {
    store.settings = s
  })
}

// --- Card helpers -------------------------------------------------------------

const query = ref('')

const filteredInstances = computed(() => {
  const q = query.value.trim().toLowerCase()
  if (!q) return store.instances
  return store.instances.filter((i) => i.name.toLowerCase().includes(q))
})

function versionOf(inst: DshInstance): string {
  return store.versionById(inst.version_id)?.version ?? inst.version_id
}

function homeNameOf(inst: DshInstance): string {
  return store.homeById(inst.home_id)?.name ?? inst.home_id
}

function sharedHome(inst: DshInstance): boolean {
  return store.instances.filter((i) => i.home_id === inst.home_id).length > 1
}

function statusOf(id: string) {
  return store.statusOf(id)
}

function canStart(inst: DshInstance): boolean {
  const st = statusOf(inst.id).state
  return (
    !!profileSel.value[inst.id] &&
    st !== 'starting' &&
    st !== 'running' &&
    !restarting.value[inst.id] &&
    !!store.versionById(inst.version_id)
  )
}

// --- Start / stop / open -----------------------------------------------------

async function reportHealth(instanceId: string, profile: string) {
  try {
    const report = await api.checkInstanceHealth(instanceId, profile)
    for (const f of report.findings.slice(0, 3)) {
      const content = `${t('home.health.prefix')}${f.message}`
      if (f.level === 'error') Notification.error({ title: t('home.health.errorTitle'), content, duration: 0, closable: true })
      else Notification.warning({ title: t('home.health.warnTitle'), content, duration: 8000, closable: true })
    }
  } catch {
    // Advisory only
  }
}

async function onStart(inst: DshInstance) {
  const profile = profileSel.value[inst.id]
  if (!profile || restarting.value[inst.id]) return
  try {
    await api.startInstance(inst.id, profile)
    touchLastUsed(inst.id)
    Message.success(t('home.started'))
    void reportHealth(inst.id, profile)
  } catch (e) {
    Message.error(String(e))
  }
}

async function onStop(inst: DshInstance) {
  if (restarting.value[inst.id]) return
  try {
    await api.stopInstance(inst.id)
    Message.success(t('home.stopped'))
  } catch (e) {
    Message.error(String(e))
  }
}

async function onRestart(inst: DshInstance) {
  if (restarting.value[inst.id]) return
  const profile = profileSel.value[inst.id] ?? statusOf(inst.id).profile ?? undefined
  if (!profile) {
    Message.warning(t('home.noProfile'))
    return
  }
  restarting.value[inst.id] = true
  try {
    try {
      await api.stopInstance(inst.id)
    } catch (e) {
      Message.error(String(e))
      return
    }
    try {
      await api.startInstance(inst.id, profile)
    } catch (e) {
      Message.warning(t('home.stopped'))
      Message.error(String(e))
      return
    }
    Message.success(t('home.started'))
    void reportHealth(inst.id, profile)
  } finally {
    restarting.value[inst.id] = false
  }
}

async function onOpenBrowser(inst: DshInstance) {
  try {
    await api.openInstanceWindow(inst.id)
    touchLastUsed(inst.id)
  } catch (e) {
    Message.error(String(e))
  }
}

function copyUrl(url: string) {
  navigator.clipboard?.writeText(url)
  Message.success(t('common.copied'))
}

// --- Card overflow actions ----------------------------------------------------

function goSettings(inst: DshInstance) {
  void router.push({ name: 'instance-edit', params: { id: inst.id } }).catch(() => undefined)
}

const dirBusy = ref<Record<string, boolean>>({})
const logBusy = ref<Record<string, boolean>>({})

async function onOpenDirectory(inst: DshInstance) {
  dirBusy.value[inst.id] = true
  try {
    const path = await api.openInstanceDirectory(inst.id)
    Message.success(t('instanceEdit.dirOpened', { path }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    dirBusy.value[inst.id] = false
  }
}

async function onViewLog(inst: DshInstance) {
  logBusy.value[inst.id] = true
  try {
    const path = await api.openInstanceLog(inst.id)
    Message.success(t('instanceEdit.logOpened', { path }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    logBusy.value[inst.id] = false
  }
}

function goNew() {
  newVisible.value = true
}

const newVisible = ref(false)

const terminalBusy = ref<Record<string, boolean>>({})

async function onOpenTerminal(inst: DshInstance) {
  terminalBusy.value[inst.id] = true
  try {
    const label = await api.openInstanceTerminal(inst.id)
    Message.success(t('home.terminalOpened', { label }))
  } catch (e) {
    Message.error(String(e))
  } finally {
    terminalBusy.value[inst.id] = false
  }
}

function goManage() {
  void router.push({ name: 'instances' }).catch(() => undefined)
}
</script>

<template>
  <div class="dl-page home-page">
    <!-- Apple-grade Toolbar -->
    <div class="home-toolbar">
      <div class="apple-search-wrapper">
        <svg class="search-icon" viewBox="0 0 16 16" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8">
          <circle cx="7" cy="7" r="4.5" />
          <line x1="10.5" y1="10.5" x2="14" y2="14" stroke-linecap="round" />
        </svg>
        <input
          v-model="query"
          type="text"
          class="apple-search-input"
          :placeholder="t('home.searchPlaceholder')"
        />
        <button v-if="query" class="search-clear-btn" @click="query = ''">×</button>
      </div>

      <span class="home-count tnum">{{ t('home.instanceCount', { count: filteredInstances.length }) }}</span>
      <span class="home-spacer" />

      <button class="mac-secondary-btn" @click="goManage">
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
          <line x1="2" y1="4" x2="14" y2="4" />
          <line x1="2" y1="8" x2="14" y2="8" />
          <line x1="2" y1="12" x2="14" y2="12" />
        </svg>
        <span>{{ t('home.instanceList') }}</span>
      </button>

      <button class="mac-primary-btn" @click="goNew">
        <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <line x1="8" y1="3" x2="8" y2="13" />
          <line x1="3" y1="8" x2="13" y2="8" />
        </svg>
        <span>{{ t('home.newInstance') }}</span>
      </button>
    </div>

    <!-- Empty state -->
    <div v-if="store.instances.length === 0" class="dl-card home-empty">
      <div class="empty-icon-wrap">
        <svg viewBox="0 0 48 48" width="44" height="44" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="6" y="8" width="36" height="32" rx="4" />
          <line x1="6" y1="18" x2="42" y2="18" />
          <circle cx="12" cy="13" r="1.5" fill="currentColor" />
          <circle cx="17" cy="13" r="1.5" fill="currentColor" />
          <circle cx="22" cy="13" r="1.5" fill="currentColor" />
          <line x1="24" y1="26" x2="24" y2="34" />
          <line x1="20" y1="30" x2="28" y2="30" />
        </svg>
      </div>
      <div class="empty-title">{{ t('instances.emptyTitle') }}</div>
      <div class="empty-desc">{{ t('instances.emptyDesc') }}</div>
      <button class="mac-primary-btn" style="margin-top: 14px;" @click="goNew">
        {{ t('home.newInstance') }}
      </button>
    </div>

    <div v-else-if="filteredInstances.length === 0" class="dl-card home-empty">
      <div class="empty-desc">{{ t('home.noMatchingInstances') }}</div>
    </div>

    <!-- Instance Card Wall -->
    <div v-else class="instance-grid">
      <div
        v-for="inst in filteredInstances"
        :key="inst.id"
        class="dl-card instance-card"
      >
        <!-- Card Header -->
        <div class="card-head">
          <div class="instance-avatar">
            <img :src="iconMap[inst.id] ?? launcherDefaultIcon" alt="" />
          </div>
          <div class="card-title-block">
            <div class="card-name" :title="inst.name">{{ inst.name }}</div>
            <div class="card-meta tnum">{{ versionOf(inst) }} · {{ homeNameOf(inst) }}</div>
          </div>
          <!-- Apple Status Indicator -->
          <div class="status-indicator-wrap">
            <span :class="['apple-status-dot', statusOf(inst.id).state]">
              {{ t(`home.status.${statusOf(inst.id).state}`) }}
            </span>
          </div>
        </div>

        <!-- Shared Home Indicator -->
        <div v-if="sharedHome(inst)" class="card-shared-badge">
          <a-tooltip :content="t('home.sharedHomeWarning')">
            <span class="shared-chip">
              <svg viewBox="0 0 12 12" width="11" height="11" fill="none" stroke="currentColor" stroke-width="1.6">
                <circle cx="6" cy="6" r="4.5" />
                <line x1="6" y1="3.5" x2="6" y2="6.5" />
                <circle cx="6" cy="8.5" r="0.5" fill="currentColor" />
              </svg>
              {{ t('home.sharedHome') }}
            </span>
          </a-tooltip>
        </div>

        <!-- Profile Selection Row -->
        <div class="card-profile-row">
          <span class="field-label">{{ t('home.profile') }}</span>
          <div class="profile-select-capsule">
            <a-select
              v-model="profileSel[inst.id]"
              :placeholder="t('home.selectProfile')"
              :loading="profilesLoading[inst.id]"
              size="small"
              class="card-profile-select"
              allow-clear
              @change="touchLastUsed(inst.id)"
            >
              <a-option v-for="p in profilesById[inst.id] ?? []" :key="p" :value="p">{{ p }}</a-option>
            </a-select>
            <button
              class="profile-refresh-btn"
              :title="t('common.refresh')"
              :disabled="profilesLoading[inst.id]"
              @click="loadProfilesFor(inst)"
            >
              <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                <path d="M13.5 8A5.5 5.5 0 1 1 12 4.1L14 2" />
                <polyline points="14 5.5 14 2 10.5 2" />
              </svg>
            </button>
          </div>
        </div>

        <!-- Running URL Banner -->
        <div v-if="statusOf(inst.id).state === 'running' && statusOf(inst.id).url" class="card-url-banner">
          <button class="url-chip" @click="onOpenBrowser(inst)">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <path d="M7 9l5-5m0 0H8m4 0v4" />
              <rect x="3" y="6" width="7" height="7" rx="1.5" />
            </svg>
            <span class="url-text tnum">{{ statusOf(inst.id).url }}</span>
          </button>
          <button class="url-copy-btn" :title="t('common.copy')" @click="copyUrl(statusOf(inst.id).url!)">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <rect x="5" y="5" width="8" height="8" rx="1.5" />
              <path d="M3 11V3a1 1 0 0 1 1-1h8" />
            </svg>
          </button>
        </div>

        <!-- Primary Action Buttons -->
        <div class="card-main-actions">
          <template v-if="restarting[inst.id]">
            <button class="action-btn secondary-action is-busy" disabled>
              <span class="mini-spinner" />
              <span>{{ t('home.restart') }}</span>
            </button>
          </template>

          <template v-else-if="statusOf(inst.id).state === 'running'">
            <button class="action-btn primary-action active-open" @click="onOpenBrowser(inst)">
              <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <polygon points="5 3 13 8 5 13 5 3" />
              </svg>
              <span>{{ t('home.openBrowser') }}</span>
            </button>
            <button class="action-btn secondary-action" @click="onRestart(inst)">
              <span>{{ t('home.restart') }}</span>
            </button>
            <button class="action-btn danger-action" @click="onStop(inst)">
              <span>{{ t('home.stop') }}</span>
            </button>
          </template>

          <template v-else-if="statusOf(inst.id).state === 'starting'">
            <button class="action-btn secondary-action is-busy" disabled>
              <span class="mini-spinner" />
              <span>{{ t('home.status.starting') }}</span>
            </button>
            <button class="action-btn danger-action" @click="onStop(inst)">
              <span>{{ t('home.stop') }}</span>
            </button>
          </template>

          <template v-else>
            <button
              class="action-btn primary-action"
              :disabled="!canStart(inst)"
              @click="onStart(inst)"
            >
              <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <polygon points="5 3 13 8 5 13 5 3" />
              </svg>
              <span>{{ t('home.start') }}</span>
            </button>
          </template>
        </div>

        <!-- Card Bottom Utility Bar -->
        <div class="card-bottom-bar">
          <button class="mac-micro-btn" :title="t('home.openTerminal')" :disabled="terminalBusy[inst.id]" @click="onOpenTerminal(inst)">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <polyline points="4 5 7 8 4 11" />
              <line x1="8" y1="12" x2="12" y2="12" />
            </svg>
            <span>{{ t('home.terminal') }}</span>
          </button>

          <button class="mac-micro-btn" :title="t('instanceEdit.openDir')" :disabled="dirBusy[inst.id]" @click="onOpenDirectory(inst)">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <path d="M2 4a1.5 1.5 0 0 1 1.5-1.5h3l2 2H13a1.5 1.5 0 0 1 1.5 1.5v6a1.5 1.5 0 0 1-1.5 1.5H3.5A1.5 1.5 0 0 1 2 12V4z" />
            </svg>
            <span>{{ t('instanceEdit.openDir') }}</span>
          </button>

          <button class="mac-micro-btn" :title="t('instanceEdit.viewLog')" :disabled="logBusy[inst.id]" @click="onViewLog(inst)">
            <svg viewBox="0 0 16 16" width="12" height="12" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <rect x="3" y="2" width="10" height="12" rx="1.5" />
              <line x1="6" y1="6" x2="10" y2="6" />
              <line x1="6" y1="9" x2="10" y2="9" />
            </svg>
            <span>{{ t('instanceEdit.viewLog') }}</span>
          </button>

          <span class="flex-spacer" />

          <button class="mac-micro-btn icon-only" :title="t('home.cardSettings')" @click="goSettings(inst)">
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round">
              <circle cx="8" cy="8" r="2.5" />
              <path d="M8 1.5v1.2M8 13.3v1.2M1.5 8h1.2M13.3 8h1.2M3.4 3.4l.8.8M11.8 11.8l.8.8M3.4 12.6l.8-.8M11.8 4.2l.8-.8" />
            </svg>
          </button>
        </div>
      </div>
    </div>

    <NewInstanceDialog v-model:visible="newVisible" />
  </div>
</template>

<style lang="scss" scoped>
.home-page {
  display: flex;
  flex-direction: column;
  gap: 20px;
}

// macOS Toolbar
.home-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
}

.apple-search-wrapper {
  position: relative;
  display: flex;
  align-items: center;
  width: 250px;

  .search-icon {
    position: absolute;
    left: 10px;
    color: var(--color-text-3);
    pointer-events: none;
  }

  .apple-search-input {
    width: 100%;
    height: 30px;
    padding: 0 28px 0 30px;
    font-size: 13px;
    border-radius: 8px;
    border: 1px solid var(--apple-card-border);
    background: var(--apple-card-bg);
    color: var(--color-text-1);
    outline: none;
    transition: all 0.16s ease;

    &:focus {
      border-color: rgb(var(--primary-6));
      box-shadow: 0 0 0 2px rgb(var(--primary-6) / 20%);
    }

    &::placeholder {
      color: var(--color-text-4);
    }
  }

  .search-clear-btn {
    position: absolute;
    right: 8px;
    border: none;
    background: transparent;
    color: var(--color-text-3);
    cursor: pointer;
    font-size: 14px;
    padding: 2px 4px;
    line-height: 1;

    &:hover {
      color: var(--color-text-1);
    }
  }
}

.home-count {
  font-size: 12px;
  color: var(--color-text-3);
  font-weight: 500;
}

.home-spacer {
  flex: 1;
}

// Apple Buttons
.mac-primary-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 14px;
  font-size: 13px;
  font-weight: 500;
  border-radius: 8px;
  border: none;
  background: rgb(var(--primary-6));
  color: #fff;
  cursor: pointer;
  box-shadow: 0 1px 3px rgb(var(--primary-6) / 30%);
  transition: all 0.16s ease;

  &:hover {
    filter: brightness(1.06);
    box-shadow: 0 2px 8px rgb(var(--primary-6) / 45%);
  }

  &:active {
    transform: scale(var(--apple-active-scale));
  }
}

.mac-secondary-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 30px;
  padding: 0 12px;
  font-size: 13px;
  font-weight: 500;
  border-radius: 8px;
  border: 1px solid var(--apple-card-border);
  background: var(--apple-card-bg);
  color: var(--color-text-2);
  cursor: pointer;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  transition: all 0.16s ease;

  &:hover {
    background: var(--apple-group-bg);
    color: var(--color-text-1);
  }

  &:active {
    transform: scale(var(--apple-active-scale));
  }
}

// Empty State
.home-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 56px 24px;
  text-align: center;
}

.empty-icon-wrap {
  color: var(--color-text-4);
  margin-bottom: 12px;
}

.empty-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-1);
  margin-bottom: 4px;
}

.empty-desc {
  font-size: 13px;
  color: var(--color-text-3);
  max-width: 380px;
}

// Card Wall Grid
.instance-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 18px;
  align-items: stretch;
}

.instance-card {
  margin-top: 0 !important;
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 18px;
  border-radius: var(--dl-card-radius);

  &:hover {
    transform: translateY(-2px);
    box-shadow: var(--apple-card-hover-shadow);
  }
}

.card-head {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.instance-avatar {
  width: 44px;
  height: 44px;
  border-radius: 11px;
  background: linear-gradient(135deg, #165dff, #722ed1);
  box-shadow: 0 2px 8px rgb(0 0 0 / 12%);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex-shrink: 0;

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
}

.card-title-block {
  flex: 1;
  min-width: 0;
}

.card-name {
  font-size: 15px;
  font-weight: 600;
  letter-spacing: -0.015em;
  color: var(--color-text-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-meta {
  font-size: 12px;
  color: var(--color-text-3);
  margin-top: 2px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-indicator-wrap {
  flex-shrink: 0;
}

.card-shared-badge {
  .shared-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 1px 7px;
    font-size: 11px;
    border-radius: 6px;
    background: rgb(var(--orange-6) / 12%);
    color: rgb(var(--orange-6));
    font-weight: 500;
  }
}

// Profile Row
.card-profile-row {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 12.5px;

  .field-label {
    color: var(--color-text-3);
    font-weight: 500;
    flex-shrink: 0;
  }

  .profile-select-capsule {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .card-profile-select {
    flex: 1;
  }

  .profile-refresh-btn {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    border: 1px solid var(--apple-card-border);
    background: var(--apple-group-bg);
    color: var(--color-text-3);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.15s ease;

    &:hover:not(:disabled) {
      color: var(--color-text-1);
    }
  }
}

// URL Banner
.card-url-banner {
  display: flex;
  align-items: center;
  gap: 6px;
  background: var(--apple-group-bg);
  border-radius: 8px;
  padding: 4px 8px;
  font-size: 12px;

  .url-chip {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: transparent;
    color: rgb(var(--primary-6));
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    overflow: hidden;

    .url-text {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    &:hover {
      text-decoration: underline;
    }
  }

  .url-copy-btn {
    border: none;
    background: transparent;
    color: var(--color-text-3);
    cursor: pointer;
    padding: 3px;
    border-radius: 4px;

    &:hover {
      color: var(--color-text-1);
      background: var(--apple-group-bg);
    }
  }
}

// Card Actions
.card-main-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;

  .action-btn {
    flex: 1;
    height: 32px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 500;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: none;
    cursor: pointer;
    transition: all 0.16s ease;

    &.primary-action {
      background: rgb(var(--primary-6));
      color: #fff;
      box-shadow: 0 1px 3px rgb(var(--primary-6) / 25%);

      &:hover:not(:disabled) {
        filter: brightness(1.08);
      }

      &.active-open {
        background: rgb(var(--green-6));
        box-shadow: 0 1px 3px rgb(var(--green-6) / 25%);
      }

      &:disabled {
        opacity: 0.45;
        cursor: not-allowed;
      }
    }

    &.secondary-action {
      background: var(--apple-group-bg);
      color: var(--color-text-1);
      border: 1px solid var(--apple-card-border);

      &:hover:not(:disabled) {
        filter: brightness(0.96);
      }
    }

    &.danger-action {
      flex: 0 0 68px;
      background: rgb(var(--red-6) / 12%);
      color: rgb(var(--red-6));

      &:hover {
        background: rgb(var(--red-6) / 20%);
      }
    }

    &:active:not(:disabled) {
      transform: scale(var(--apple-active-scale));
    }
  }
}

.mini-spinner {
  width: 12px;
  height: 12px;
  border: 2px solid currentColor;
  border-right-color: transparent;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

// Bottom Bar
.card-bottom-bar {
  margin-top: auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding-top: 10px;
  border-top: 1px solid var(--apple-separator);

  .flex-spacer {
    flex: 1;
  }
}

.mac-micro-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  font-size: 11.5px;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--color-text-3);
  cursor: pointer;
  transition: all 0.15s ease;

  &:hover:not(:disabled) {
    background: var(--apple-group-bg);
    color: var(--color-text-1);
    border-color: var(--apple-card-border);
  }

  &.icon-only {
    padding: 5px;
  }

  &:active:not(:disabled) {
    transform: scale(var(--apple-active-scale));
  }
}
</style>
