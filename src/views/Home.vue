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

// --- Instance icons (issue #8): each card avatar follows its instance --------

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
// Each card owns its Profile dropdown; selection prefers last-used, then the
// instance default, then the first available — same order as before.

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

function subtitleOf(inst: DshInstance): string {
  const v = versionOf(inst)
  const p = profileSel.value[inst.id] ?? '—'
  return `${v} · ${p}`
}

// --- Start / stop / open ---------------------------------------------------

async function reportHealth(instanceId: string, profile: string) {
  try {
    const report = await api.checkInstanceHealth(instanceId, profile)
    for (const f of report.findings.slice(0, 3)) {
      const content = `${t('home.health.prefix')}${f.message}`
      if (f.level === 'error') Notification.error({ title: t('home.health.errorTitle'), content, duration: 0, closable: true })
      else Notification.warning({ title: t('home.health.warnTitle'), content, duration: 8000, closable: true })
    }
  } catch {
    // A failed preflight must never affect the launch.
  }
}

async function onStart(inst: DshInstance) {
  const profile = profileSel.value[inst.id]
  if (!profile || restarting.value[inst.id]) return
  try {
    await api.startInstance(inst.id, profile)
    touchLastUsed(inst.id)
    Message.success(t('home.started'))
    // Dependency-tree preflight: advisory only, never blocks the launch. A
    // duplicated core copy in the profile silently breaks every tool call at
    // runtime, so surface it here instead of leaving users to dig through logs.
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

// Restart with the card's selected profile (falling back to the running one).
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
      // Stopped but not started: report the state first so the user knows
      // a manual start is the way back, then the underlying reason.
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

// Opens the running instance URL in the system browser (new tab in preview).
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

// --- Card overflow: settings / open dir / view log ----------------------------

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
    <!-- Toolbar: search + entry points -->
    <div class="home-toolbar">
      <a-input
        v-model="query"
        :placeholder="t('home.searchPlaceholder')"
        allow-clear
        class="home-search"
      />
      <span class="home-count">{{ t('home.instanceCount', { count: filteredInstances.length }) }}</span>
      <span class="home-spacer" />
      <a-button @click="goManage">{{ t('home.instanceList') }}</a-button>
      <a-button type="primary" @click="goNew">{{ t('home.newInstance') }}</a-button>
    </div>

    <!-- Empty state -->
    <div v-if="store.instances.length === 0" class="dl-card home-empty">
      <a-empty :description="t('instances.emptyDesc')">
        <template #image>
          <div class="empty-title">{{ t('instances.emptyTitle') }}</div>
        </template>
        <a-button type="primary" @click="goNew">{{ t('home.newInstance') }}</a-button>
      </a-empty>
    </div>
    <div v-else-if="filteredInstances.length === 0" class="dl-card home-empty">
      <a-empty :description="t('common.loading')" />
    </div>

    <!-- Instance card wall -->
    <div v-else class="instance-grid">
      <div v-for="inst in filteredInstances" :key="inst.id" class="dl-card instance-card">
        <div class="card-head">
          <div class="instance-avatar">
            <img :src="iconMap[inst.id] ?? launcherDefaultIcon" alt="" />
          </div>
          <div class="card-title-block">
            <div class="card-name" :title="inst.name">{{ inst.name }}</div>
            <div class="card-meta">{{ versionOf(inst) }} · {{ homeNameOf(inst) }}</div>
          </div>
          <a-tag
            :color="statusOf(inst.id).state === 'running' ? 'green' : statusOf(inst.id).state === 'starting' ? 'orange' : 'gray'"
            size="small"
          >
            {{ t(`home.status.${statusOf(inst.id).state}`) }}
          </a-tag>
        </div>

        <div v-if="sharedHome(inst)" class="card-shared">
          <a-tooltip :content="t('home.sharedHomeWarning')">
            <a-tag color="orangered" size="small">{{ t('home.sharedHome') }}</a-tag>
          </a-tooltip>
        </div>

        <div class="card-profile-row">
          <span class="field-label">{{ t('home.profile') }}</span>
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
          <a-button
            size="mini"
            type="text"
            :loading="profilesLoading[inst.id]"
            @click="loadProfilesFor(inst)"
          >
            ⟳
          </a-button>
        </div>

        <div v-if="statusOf(inst.id).state === 'running' && statusOf(inst.id).url" class="card-url">
          <a-link class="url-link" :title="statusOf(inst.id).url!" @click="onOpenBrowser(inst)">
            {{ statusOf(inst.id).url }}
          </a-link>
          <a-button size="mini" type="text" class="url-copy" @click="copyUrl(statusOf(inst.id).url!)">
            {{ t('common.copy') }}
          </a-button>
        </div>

        <div class="card-actions">
          <template v-if="statusOf(inst.id).state !== 'running' && !restarting[inst.id]">
            <a-button
              type="primary"
              long
              :disabled="!canStart(inst)"
              :loading="statusOf(inst.id).state === 'starting'"
              @click="onStart(inst)"
            >
              {{ statusOf(inst.id).state === 'starting' ? t('home.starting') : t('home.start') }}
            </a-button>
            <div v-if="subtitleOf(inst)" class="card-sub">{{ subtitleOf(inst) }}</div>
          </template>
          <template v-else-if="statusOf(inst.id).state === 'running'">
            <a-button type="primary" long @click="onOpenBrowser(inst)">
              {{ t('home.openWindow') }}
            </a-button>
            <div class="stop-row">
              <a-button
                status="danger"
                class="stop-half"
                :disabled="restarting[inst.id]"
                @click="onStop(inst)"
              >
                {{ t('home.stop') }}
              </a-button>
              <a-button
                class="stop-half"
                :loading="restarting[inst.id]"
                :disabled="restarting[inst.id]"
                @click="onRestart(inst)"
              >
                {{ t('home.restart') }}
              </a-button>
            </div>
          </template>
          <template v-else>
            <!-- Restart in flight: hold a disabled loading slot so progress
                 stays visible instead of flipping back mid-flight. -->
            <a-button type="primary" long disabled :loading="true">
              {{ t('home.restart') }}
            </a-button>
          </template>
        </div>

        <div class="card-foot">
          <a-button size="small" type="text" @click="goSettings(inst)">
            {{ t('instances.table.edit') }}
          </a-button>
          <a-button
            size="small"
            type="text"
            :loading="terminalBusy[inst.id]"
            @click="onOpenTerminal(inst)"
          >
            {{ t('home.openTerminal') }}
          </a-button>
          <span class="foot-spacer" />
          <a-button
            size="small"
            type="text"
            :loading="dirBusy[inst.id]"
            @click="onOpenDirectory(inst)"
          >
            {{ t('instanceEdit.openDirectory') }}
          </a-button>
          <a-button
            size="small"
            type="text"
            :loading="logBusy[inst.id]"
            @click="onViewLog(inst)"
          >
            {{ t('instanceEdit.viewLog') }}
          </a-button>
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
  gap: 16px;
}

.home-toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.home-search {
  width: 260px;
}

.home-count {
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
}

.home-spacer {
  flex: 1;
}

.home-empty {
  text-align: center;
  padding: 48px 24px;
}

.empty-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-1);
}

.instance-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
  gap: 16px;
  padding-bottom: 24px;
}

.instance-card {
  display: flex;
  flex-direction: column;
  gap: 12px;
  margin-top: 0 !important;
}

.card-head {
  display: flex;
  align-items: center;
  gap: 12px;
  min-width: 0;
}

.instance-avatar {
  width: 48px;
  height: 48px;
  border-radius: 12px;
  background: linear-gradient(135deg, #165dff, #722ed1);
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  flex-shrink: 0;
  user-select: none;

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
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-meta {
  font-size: 12px;
  color: var(--color-text-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-shared {
  margin-top: -6px;
}

.card-profile-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.field-label {
  font-size: 12px;
  color: var(--color-text-3);
  flex-shrink: 0;
}

.card-profile-select {
  flex: 1;
  min-width: 0;
}

.card-url {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  min-width: 0;

  .url-link {
    flex: 1 1 auto;
    min-width: 0;
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .url-copy {
    flex-shrink: 0;
  }
}

.card-actions {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: auto;
}

.card-sub {
  font-size: 12px;
  color: var(--color-text-3);
  text-align: center;
}

.stop-row {
  display: flex;
  gap: 10px;

  .stop-half {
    flex: 1;
  }
}

.card-foot {
  display: flex;
  align-items: center;
  gap: 4px;
  border-top: 1px solid var(--color-border-1);
  padding-top: 8px;
}

.foot-spacer {
  flex: 1;
}
</style>
