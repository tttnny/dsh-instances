<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import type { LauncherUpdateInfo, LogLevel, ThemeMode } from '@/api/types'
import { SUPPORTED_LOCALES } from '@/i18n'
import { useLauncherStore } from '@/stores/launcher'
import { SHORTCUT_DOCS } from '@/shortcuts'

const { t } = useI18n()
const store = useLauncherStore()

const THEME_OPTIONS = computed<{ value: ThemeMode; label: string }[]>(() => [
  { value: 'light', label: t('settings.theme.light') },
  { value: 'dark', label: t('settings.theme.dark') },
  { value: 'system', label: t('settings.theme.system') },
])

const LOG_LEVEL_OPTIONS = computed<{ value: LogLevel; label: string }[]>(() => [
  { value: 'debug', label: t('settings.logLevel.debug') },
  { value: 'info', label: t('settings.logLevel.info') },
  { value: 'warn', label: t('settings.logLevel.warn') },
  { value: 'error', label: t('settings.logLevel.error') },
])

// --- In-app shortcut reference (t4/t14): rendered from SHORTCUT_DOCS ---------

const shortcutRows = computed<{ label: string; keys: string[]; native: boolean }[]>(() =>
  SHORTCUT_DOCS.map((d) => ({ label: t(d.labelKey), keys: d.keys, native: d.native ?? false })),
)

// --- General settings -------------------------------------------------------

async function patchSettings(patch: Parameters<typeof api.updateSettings>[0]) {
  try {
    store.settings = await api.updateSettings(patch)
    Message.success(t('settings.saved'))
  } catch (e) {
    Message.error(String(e))
  }
}

async function onThemeChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ theme: String(value) as ThemeMode })
}

async function onLogLevelChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ log_level: String(value) as LogLevel })
}

// --- Launcher update check (GitHub releases) --------------------------------

const launcherVersion = ref('')
const checkingUpdate = ref(false)
const updateInfo = ref<LauncherUpdateInfo | null>(null)
/** Update channel: "dev" (includes prereleases) or "release" (stable only). */
const updateChannel = ref<'dev' | 'release'>('dev')

const UPDATE_CHANNEL_OPTIONS = computed<{ value: 'dev' | 'release'; label: string }[]>(() => [
  { value: 'dev', label: t('settings.update.channel.dev') },
  { value: 'release', label: t('settings.update.channel.release') },
])

onMounted(async () => {
  try {
    launcherVersion.value = await api.getLauncherVersion()
  } catch {
    launcherVersion.value = '?'
  }
  try {
    dataDir.value = await api.getLauncherDirectory()
  } catch {
    dataDir.value = ''
  }
})

async function onCheckUpdate() {
  checkingUpdate.value = true
  try {
    updateInfo.value = await api.checkLauncherUpdate(updateChannel.value)
    if (updateInfo.value.up_to_date) Message.success(t('settings.update.upToDate'))
  } catch (e) {
    Message.error(String(e))
  } finally {
    checkingUpdate.value = false
  }
}

async function onUpdateChannelChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  const channel = String(value) === 'release' ? 'release' : 'dev'
  updateChannel.value = channel
  // A different channel invalidates the previous result; only a fresh check
  // is meaningful for the new channel.
  updateInfo.value = null
}

// --- Data directory ---------------------------------------------------------

const dataDir = ref('')

async function onOpenDataDir() {
  try {
    const dir = await api.openLauncherDirectory()
    dataDir.value = dir
  } catch (e) {
    Message.error(String(e))
  }
}

async function onOpenLauncherLog() {
  try {
    const path = await api.openLauncherLog()
    Message.success(t('settings.logOpened', { path }))
  } catch (e) {
    Message.error(String(e))
  }
}

async function onLocaleChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ locale: String(value) })
}

async function onTrayChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ minimize_to_tray: Boolean(value) })
}

async function onAutostartChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ autostart: Boolean(value) })
}

// --- SKILL source repos (issue #10) ---------------------------------------------

const newSkillRepo = ref('')
const skillRepoBusy = ref(false)

async function onAddSkillRepo() {
  const url = newSkillRepo.value.trim()
  if (!url) return
  if (store.settings.skill_repos.includes(url)) {
    Message.warning(t('settings.skillRepoExists'))
    return
  }
  skillRepoBusy.value = true
  try {
    await patchSettings({ skill_repos: [...store.settings.skill_repos, url] })
    newSkillRepo.value = ''
  } finally {
    skillRepoBusy.value = false
  }
}

async function onRemoveSkillRepo(url: string) {
  await patchSettings({ skill_repos: store.settings.skill_repos.filter((r) => r !== url) })
}

// --- Proxy settings -----------------------------------------------------------

async function onProxyEnabledChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ proxy_enabled: Boolean(value) })
}

async function onProxyApplyDshChange(value: string | number | boolean | Record<string, unknown> | (string | number | boolean | Record<string, unknown>)[]) {
  await patchSettings({ proxy_apply_dsh: Boolean(value) })
}

// Text fields save on blur / Enter so typing is not interrupted.
const proxyUrl = ref(store.settings.proxy_url ?? '')
const proxyPort = ref(store.settings.proxy_port ?? 7890)
const noProxy = ref(store.settings.no_proxy ?? '')
watch(
  () => [store.settings.proxy_url, store.settings.proxy_port, store.settings.no_proxy] as const,
  ([url, port, np]) => {
    const u = String(url ?? '')
    if (u !== proxyUrl.value) proxyUrl.value = u
    const p = Number(port ?? 7890)
    if (p !== proxyPort.value) proxyPort.value = p
    const n = String(np ?? '')
    if (n !== noProxy.value) noProxy.value = n
  },
)

async function onProxyFieldsSave() {
  const patch: Parameters<typeof api.updateSettings>[0] = {}
  const url = proxyUrl.value.trim()
  if (url && url !== store.settings.proxy_url) patch.proxy_url = url
  if (proxyPort.value && proxyPort.value !== store.settings.proxy_port) patch.proxy_port = proxyPort.value
  const np = noProxy.value.trim()
  if (np !== (store.settings.no_proxy ?? '')) patch.no_proxy = np
  if (Object.keys(patch).length > 0) await patchSettings(patch)
}

// --- Section anchor nav: long page, jump instead of blind scrolling ----------

const sections = computed(() => [
  { key: 'general', label: t('settings.general') },
  { key: 'shortcuts', label: t('settings.shortcuts.title') },
  { key: 'proxy', label: t('settings.proxy.title') },
  { key: 'skillRepos', label: t('settings.skillRepos.title') },
  { key: 'update', label: t('settings.update.title') },
  { key: 'dataDir', label: t('settings.dataDir.title') },
])

const sectionEls = ref<Record<string, HTMLElement | null>>({})

function setSectionRef(key: string, el: unknown) {
  sectionEls.value[key] = (el as HTMLElement | null) ?? null
}

function scrollToSection(key: string) {
  sectionEls.value[key]?.scrollIntoView({ behavior: 'smooth', block: 'start' })
}
</script>

<template>
  <div class="settings-layout">
    <aside class="settings-anchor">
      <button
        v-for="s in sections"
        :key="s.key"
        class="anchor-item"
        @click="scrollToSection(s.key)"
      >
        {{ s.label }}
      </button>
    </aside>
    <div class="dl-page settings-page">
    <div :ref="(el: unknown) => setSectionRef('general', el)" class="dl-card section-anchor">
      <div class="dl-card-title">
        <h3>{{ t('settings.general') }}</h3>
      </div>
      <a-form :model="store.settings" layout="vertical" class="settings-form">
        <a-form-item :label="t('settings.language')">
          <a-select
            :model-value="store.settings.locale"
            style="width: 220px"
            @change="onLocaleChange"
          >
            <a-option v-for="l in SUPPORTED_LOCALES" :key="l.value" :value="l.value">
              {{ l.label }}
            </a-option>
          </a-select>
        </a-form-item>
        <a-form-item :label="t('settings.theme.label')">
          <a-select
            :model-value="store.settings.theme"
            style="width: 220px"
            @change="onThemeChange"
          >
            <a-option v-for="o in THEME_OPTIONS" :key="o.value" :value="o.value">
              {{ o.label }}
            </a-option>
          </a-select>
        </a-form-item>
        <a-form-item>
          <a-switch :model-value="store.settings.minimize_to_tray" @change="onTrayChange" />
          <span class="switch-label">{{ t('settings.minimizeToTray') }}</span>
        </a-form-item>
        <a-form-item>
          <a-switch :model-value="store.settings.autostart" @change="onAutostartChange" />
          <span class="switch-label">{{ t('settings.autostart') }}</span>
        </a-form-item>
        <a-form-item :label="t('settings.logLevel.label')">
          <a-select
            :model-value="store.settings.log_level"
            style="width: 220px"
            @change="onLogLevelChange"
          >
            <a-option v-for="o in LOG_LEVEL_OPTIONS" :key="o.value" :value="o.value">
              {{ o.label }}
            </a-option>
          </a-select>
          <p class="settings-hint">{{ t('settings.logLevel.hint') }}</p>
        </a-form-item>
      </a-form>
    </div>

    <div :ref="(el: unknown) => setSectionRef('shortcuts', el)" class="dl-card section-anchor">
      <div class="dl-card-title">
        <h3>{{ t('settings.shortcuts.title') }}</h3>
      </div>
      <p class="settings-hint">{{ t('settings.shortcuts.desc') }}</p>
      <div class="shortcut-list">
        <div v-for="row in shortcutRows" :key="row.label" class="shortcut-row">
          <span class="shortcut-label">{{ row.label }}<a-tag v-if="row.native" size="small" class="shortcut-native">{{ t('settings.shortcuts.nativeTag') }}</a-tag></span>
          <span class="shortcut-keys">
            <kbd v-for="k in row.keys" :key="k" class="shortcut-kbd">{{ k }}</kbd>
          </span>
        </div>
      </div>
      <p class="settings-hint">{{ t('settings.shortcuts.note') }}</p>
    </div>

    <div :ref="(el: unknown) => setSectionRef('proxy', el)" class="dl-card section-anchor">
      <div class="dl-card-title">
        <h3>{{ t('settings.proxy.title') }}</h3>
      </div>
      <a-form :model="store.settings" layout="vertical" class="settings-form">
        <a-form-item>
          <a-switch :model-value="store.settings.proxy_enabled" @change="onProxyEnabledChange" />
          <span class="switch-label">{{ t('settings.proxy.enabled') }}</span>
          <p class="settings-hint">{{ t('settings.proxy.enabledHint') }}</p>
        </a-form-item>
        <a-form-item :label="t('settings.proxy.url')">
          <a-input
            v-model="proxyUrl"
            :disabled="!store.settings.proxy_enabled"
            placeholder="http://127.0.0.1"
            @blur="onProxyFieldsSave"
            @press-enter="onProxyFieldsSave"
          />
        </a-form-item>
        <a-form-item :label="t('settings.proxy.port')">
          <a-input-number
            v-model="proxyPort"
            :disabled="!store.settings.proxy_enabled"
            :min="1"
            :max="65535"
            style="width: 220px"
            @blur="onProxyFieldsSave"
          />
        </a-form-item>
        <a-form-item :label="t('settings.proxy.noProxy')">
          <a-input
            v-model="noProxy"
            :disabled="!store.settings.proxy_enabled"
            placeholder="127.0.0.1,localhost,::1"
            @blur="onProxyFieldsSave"
            @press-enter="onProxyFieldsSave"
          />
          <p class="settings-hint">{{ t('settings.proxy.noProxyHint') }}</p>
        </a-form-item>
        <a-form-item>
          <a-switch
            :model-value="store.settings.proxy_apply_dsh"
            :disabled="!store.settings.proxy_enabled"
            @change="onProxyApplyDshChange"
          />
          <span class="switch-label">{{ t('settings.proxy.applyDsh') }}</span>
          <p class="settings-hint">{{ t('settings.proxy.applyDshHint') }}</p>
        </a-form-item>
      </a-form>
    </div>

    <div :ref="(el: unknown) => setSectionRef('skillRepos', el)" class="dl-card section-anchor">
      <div class="dl-card-title">
        <h3>{{ t('settings.skillRepos.title') }}</h3>
      </div>
      <p class="settings-hint">{{ t('settings.skillRepos.hint') }}</p>
      <div class="skill-repo-add">
        <a-input
          v-model="newSkillRepo"
          :placeholder="t('settings.skillRepos.placeholder')"
          allow-clear
          @press-enter="onAddSkillRepo"
        />
        <a-button :loading="skillRepoBusy" :disabled="!newSkillRepo.trim()" @click="onAddSkillRepo">
          {{ t('settings.skillRepos.add') }}
        </a-button>
      </div>
      <a-list :data="store.settings.skill_repos" size="small">
        <template #item="{ item }">
          <a-list-item>
            <span class="skill-repo-url">{{ item }}</span>
            <template #actions>
              <a-button size="mini" status="danger" type="text" @click="onRemoveSkillRepo(item)">
                {{ t('instances.table.delete') }}
              </a-button>
            </template>
          </a-list-item>
        </template>
        <template #empty>
          <a-empty :description="t('settings.skillRepos.empty')" />
        </template>
      </a-list>
    </div>

    <div :ref="(el: unknown) => setSectionRef('update', el)" class="dl-card section-anchor">
      <div class="dl-card-title">
        <h3>{{ t('settings.update.title') }}</h3>
      </div>
      <div class="update-row">
        <span class="update-current">v{{ launcherVersion }}</span>
        <a-tag v-if="updateInfo?.channel === 'dev' || launcherVersion.includes('-dev.')" color="orange" size="small">
          {{ t('settings.update.devChannel') }}
        </a-tag>
        <a-select
          :model-value="updateChannel"
          class="update-channel-select"
          size="small"
          @change="onUpdateChannelChange"
        >
          <a-option v-for="o in UPDATE_CHANNEL_OPTIONS" :key="o.value" :value="o.value">
            {{ o.label }}
          </a-option>
        </a-select>
        <a-button size="small" :loading="checkingUpdate" @click="onCheckUpdate">
          {{ t('settings.update.check') }}
        </a-button>
      </div>
      <p class="settings-hint">{{ t('settings.update.channelHint') }}</p>
      <div v-if="updateInfo && !updateInfo.up_to_date" class="update-result">
        <a-alert type="info" :show-icon="true">
          {{ t('settings.update.available', { version: updateInfo.latest }) }}
          <template v-if="updateInfo.url">
            <a-link class="update-link" @click="api.openExternal(updateInfo.url!)">
              {{ t('settings.update.viewRelease') }}
            </a-link>
          </template>
        </a-alert>
      </div>
      <div v-else-if="updateInfo?.up_to_date" class="update-result">
        <span class="update-up-to-date">{{ t('settings.update.upToDate') }}</span>
      </div>
    </div>

    <div :ref="(el: unknown) => setSectionRef('dataDir', el)" class="dl-card section-anchor">
      <div class="dl-card-title">
        <h3>{{ t('settings.dataDir.title') }}</h3>
      </div>
      <p class="settings-hint">{{ t('settings.dataDir.hint') }}</p>
      <div class="update-row">
        <span class="data-dir-path" :title="dataDir">{{ dataDir || t('settings.dataDir.unknown') }}</span>
        <a-button size="small" @click="onOpenDataDir">{{ t('settings.dataDir.open') }}</a-button>
        <a-button size="small" @click="onOpenLauncherLog">{{ t('settings.dataDir.viewLog') }}</a-button>
      </div>
    </div>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.settings-layout {
  display: flex;
  align-items: flex-start;
  gap: 16px;
  max-width: calc(var(--dl-content-max) + 180px);
  margin: 0 auto;
  padding: 0 24px 0 0;
}

.settings-anchor {
  position: sticky;
  top: 20px;
  width: 148px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 20px 0 20px 24px;
}

.anchor-item {
  text-align: left;
  border: none;
  background: transparent;
  color: var(--color-text-2);
  font-size: 13px;
  padding: 7px 10px;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;

  &:hover {
    background: var(--color-fill-2);
    color: rgb(var(--primary-6));
  }
}

.settings-page {
  flex: 1;
  min-width: 0;
  margin: 0;
  padding-left: 0;
}

.section-anchor {
  scroll-margin-top: 12px;
}
.skill-repo-add {
  display: flex;
  gap: 8px;
  margin: 12px 0;
}

.skill-repo-url {
  font-size: 13px;
  word-break: break-all;
}
.settings-form {
  max-width: 560px;
}

.switch-label {
  margin-left: 10px;
  color: var(--color-text-2);
}

.settings-hint {
  margin: 6px 0 0;
  font-size: 12px;
  color: var(--color-text-3);
}

.update-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}

.update-channel-select {
  width: 140px;
}

.data-dir-path {
  flex: 1;
  min-width: 0;
  font-size: 12px;
  color: var(--color-text-3);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.update-current {
  font-weight: 600;
}

.update-result {
  margin-top: 8px;
}

.update-link {
  margin-left: 8px;
}

.update-up-to-date {
  color: var(--color-text-3);
  font-size: 13px;
}

.shortcut-list {
  display: flex;
  flex-direction: column;
  max-width: 560px;
  margin: 12px 0;
  border: 1px solid var(--color-border-2);
  border-radius: 8px;
  overflow: hidden;
}

.shortcut-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 8px 12px;

  &:not(:last-child) {
    border-bottom: 1px solid var(--color-border-2);
  }
}

.shortcut-label {
  font-size: 13px;
  color: var(--color-text-1);
}

.shortcut-native {
  margin-left: 8px;
}

.shortcut-keys {
  display: flex;
  gap: 4px;
  flex-shrink: 0;
}

.shortcut-kbd {
  padding: 2px 8px;
  font-size: 12px;
  font-family: inherit;
  color: var(--color-text-2);
  background: var(--color-fill-2);
  border: 1px solid var(--color-border-2);
  border-bottom-width: 2px;
  border-radius: 6px;
  white-space: nowrap;
}
</style>
