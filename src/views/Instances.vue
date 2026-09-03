<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'
import type { DshInstance, InstanceState } from '@/api/types'
import ModpackImportDialog from '@/components/ModpackImportDialog.vue'

const router = useRouter()
const { t } = useI18n()
const store = useLauncherStore()

const modpackImportVisible = ref(false)

const columns = computed(() => [
  { title: t('instances.table.name'), slotName: 'name', width: 170 },
  { title: t('instances.table.version'), slotName: 'version', width: 120 },
  { title: t('instances.table.home'), slotName: 'home', width: 140 },
  { title: t('instances.table.profile'), slotName: 'profile', width: 80 },
  { title: t('instances.table.status'), slotName: 'status', width: 90 },
  {
    title: t('instances.table.actions'),
    slotName: 'actions',
    width: 190,
    align: 'center' as const,
  },
])

// --- Instance icons (issue #8): resolved lazily per instance -------------------

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

function stateColor(state: InstanceState): string {
  switch (state) {
    case 'running':
      return 'green'
    case 'starting':
      return 'orange'
    case 'exited':
      return 'red'
    default:
      return 'gray'
  }
}

async function onDelete(id: string, name: string) {
  try {
    await api.deleteInstance(id)
    await store.refreshInstances()
    Message.success(t('instances.deleted'))
  } catch (e) {
    Message.error(String(e))
  }
}

// --- Copy instance ---------------------------------------------------------

const copySource = ref<DshInstance | null>(null)
const copyName = ref('')
const copyNewHome = ref(false)
const copying = ref(false)

function openCopy(inst: DshInstance) {
  copySource.value = inst
  copyName.value = `${inst.name} 副本`
  copyNewHome.value = false
}

function closeCopy() {
  copySource.value = null
  copyName.value = ''
  copyNewHome.value = false
}

const copyValid = computed(() => copyName.value.trim().length > 0 && !!copySource.value)

async function confirmCopy() {
  if (!copySource.value || !copyValid.value) return
  copying.value = true
  try {
    const created = await api.copyInstance({
      source_id: copySource.value.id,
      name: copyName.value.trim(),
      new_home: copyNewHome.value,
    })
    await store.refreshInstances()
    Message.success(t('instances.copied', { name: created.name }))
    closeCopy()
  } catch (e) {
    Message.error(String(e))
  } finally {
    copying.value = false
  }
}

function copyUrl(url: string) {
  navigator.clipboard?.writeText(url)
  Message.success(t('common.copied'))
}

// Opens the running instance URL in the system browser (new tab in preview).
async function onOpenBrowser(id: string) {
  try {
    await api.openInstanceWindow(id)
  } catch (e) {
    Message.error(String(e))
  }
}

// --- DSH_HOME management (moved here from Settings: a HOME only makes
// sense next to the instances that use it) ------------------------------------

const newHomeName = ref('')
const newHomePath = ref('')

/** How many instances reference a HOME (drives the delete guard hint). */
function homeUsedBy(id: string): number {
  return store.instances.filter((i) => i.home_id === id).length
}

async function onPickDir() {
  if (api.isTauri) {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog')
      const dir = await open({ directory: true, multiple: false })
      if (typeof dir === 'string') newHomePath.value = dir
    } catch (e) {
      Message.error(String(e))
    }
  } else {
    Message.info(t('instances.browserPickHint'))
  }
}

async function onAddHome() {
  try {
    await api.createHome(newHomeName.value, newHomePath.value)
    newHomeName.value = ''
    newHomePath.value = ''
    await store.refreshHomes()
    Message.success(t('settings.saved'))
  } catch (e) {
    Message.error(String(e))
  }
}

async function onRemoveHome(id: string) {
  try {
    await api.removeHome(id)
    await store.refreshHomes()
  } catch (e) {
    Message.error(String(e))
  }
}

const homeColumns = computed(() => [
  { title: t('instances.homeName'), dataIndex: 'name', width: 180 },
  { title: t('instances.homePath'), dataIndex: 'path', ellipsis: true, tooltip: true },
  { title: t('instances.homeUsedBy'), slotName: 'usedBy', width: 140 },
  { title: t('instances.table.actions'), slotName: 'actions', width: 110, align: 'center' as const },
])
</script>

<template>
  <div class="dl-page">
    <div class="dl-card">
      <div class="dl-card-title">
        <h3>{{ t('instances.title') }}</h3>
        <div class="dl-toolbar">
          <a-button @click="modpackImportVisible = true">
            {{ t('modpack.importButton') }}
          </a-button>
          <a-button type="primary" @click="router.push({ name: 'download-create' })">
            {{ t('instances.newInstance') }}
          </a-button>
        </div>
      </div>

      <a-table
        :columns="columns"
        :data="store.instances"
        :pagination="false"
        row-key="id"
        :scroll="{ x: 790 }"
      >
        <template #name="{ record }">
          <span class="inst-name">
            <img v-if="iconMap[record.id]" :src="iconMap[record.id]!" class="inst-icon" alt="" />
            <img v-else src="@/assets/launcher-icon.png" class="inst-icon" alt="" />
            <span class="cell-ellipsis" :title="record.name">{{ record.name }}</span>
          </span>
        </template>
        <template #version="{ record }">
          <span
            class="cell-ellipsis"
            :title="store.versionById(record.version_id)?.version ?? record.version_id"
          >
            {{ store.versionById(record.version_id)?.version ?? record.version_id }}
          </span>
        </template>
        <template #home="{ record }">
          <a-tooltip :content="store.homeById(record.home_id)?.path">
            <span class="cell-ellipsis">{{
              store.homeById(record.home_id)?.name ?? record.home_id
            }}</span>
          </a-tooltip>
        </template>
        <template #profile="{ record }">
          <span class="cell-ellipsis" :title="record.last_profile ?? record.default_profile ?? ''">
            {{ record.last_profile ?? record.default_profile ?? '—' }}
          </span>
        </template>
        <template #status="{ record }">
          <div class="status-cell">
            <a-tag :color="stateColor(store.statusOf(record.id).state)">
              {{ t(`home.status.${store.statusOf(record.id).state}`) }}
            </a-tag>
            <template v-if="store.statusOf(record.id).url">
              <div class="status-url-row">
                <a-link
                  class="status-url"
                  :title="store.statusOf(record.id).url!"
                  @click="onOpenBrowser(record.id)"
                >
                  {{ store.statusOf(record.id).url }}
                </a-link>
                <a-button size="mini" type="text" @click="copyUrl(store.statusOf(record.id).url!)">
                  {{ t('common.copy') }}
                </a-button>
              </div>
            </template>
          </div>
        </template>
        <template #actions="{ record }">
          <a-space class="inst-actions" :size="4">
            <a-button
              size="mini"
              @click="router.push({ name: 'instance-edit', params: { id: record.id } })"
            >
              {{ t('instances.table.edit') }}
            </a-button>
            <a-button size="mini" @click="openCopy(record)">
              {{ t('instances.table.copy') }}
            </a-button>
            <a-popconfirm
              :content="t('instances.confirmDelete', { name: record.name })"
              @ok="onDelete(record.id, record.name)"
            >
              <a-button size="mini" status="danger">{{ t('instances.table.delete') }}</a-button>
            </a-popconfirm>
          </a-space>
        </template>
        <template #empty>
          <a-empty :description="t('instances.emptyDesc')">
            <template #image>
              <div class="empty-title">{{ t('instances.emptyTitle') }}</div>
            </template>
            <a-button type="primary" @click="router.push({ name: 'download-create' })">
              {{ t('instances.newInstance') }}
            </a-button>
          </a-empty>
        </template>
      </a-table>
    </div>

    <div class="dl-card">
      <div class="dl-card-title">
        <h3>{{ t('instances.homesTitle') }}</h3>
      </div>
      <p class="dl-card-desc">{{ t('instances.homesDesc') }}</p>

      <div class="home-add-row">
        <a-input v-model="newHomeName" :placeholder="t('instances.homeNamePlaceholder')" style="width: 200px" />
        <a-input v-model="newHomePath" :placeholder="t('instances.homePathPlaceholder')" class="home-path-input" />
        <a-button @click="onPickDir">{{ t('instances.pickDir') }}</a-button>
        <a-button type="primary" :disabled="!newHomeName.trim() || !newHomePath.trim()" @click="onAddHome">
          {{ t('instances.addHome') }}
        </a-button>
      </div>

      <a-table
        :columns="homeColumns"
        :data="store.homes"
        :pagination="false"
        row-key="id"
        :scroll="{ x: 640 }"
      >
        <template #usedBy="{ record }">
          <span class="home-used">{{ t('instances.homeUsedByCount', { count: homeUsedBy(record.id) }) }}</span>
        </template>
        <template #actions="{ record }">
          <a-popconfirm
            :content="t('instances.confirmDeleteHome', { name: record.name })"
            :disabled="homeUsedBy(record.id) > 0"
            @ok="onRemoveHome(record.id)"
          >
            <a-button size="small" status="danger" :disabled="homeUsedBy(record.id) > 0">
              {{ t('instances.deleteHome') }}
            </a-button>
          </a-popconfirm>
        </template>
        <template #empty>
          <a-empty :description="t('instances.homesEmpty')" />
        </template>
      </a-table>
    </div>

    <!-- Copy instance dialog: name it first, then duplicate on save -->
    <a-modal
      :visible="!!copySource"
      :title="t('instances.copyTitle')"
      :ok-text="t('instanceEdit.save')"
      :cancel-text="t('instanceEdit.cancel')"
      :ok-button-props="{ disabled: !copyValid, loading: copying }"
      @ok="confirmCopy"
      @cancel="closeCopy"
    >
      <a-form layout="vertical" :model="{}">
        <a-form-item :label="t('instances.copyNameLabel')" required>
          <a-input v-model="copyName" :placeholder="t('instances.copyNamePlaceholder')" />
        </a-form-item>
        <a-form-item :label="t('instances.copyHomeLabel')">
          <a-radio-group v-model="copyNewHome" type="button">
            <a-radio :value="false">{{ t('instances.copyHomeReuse') }}</a-radio>
            <a-radio :value="true">{{ t('instances.copyHomeNew') }}</a-radio>
          </a-radio-group>
        </a-form-item>
        <p class="copy-hint">{{ t('instances.copyHint') }}</p>
      </a-form>
    </a-modal>

    <ModpackImportDialog v-model:visible="modpackImportVisible" />
  </div>
</template>

<style lang="scss" scoped>
.inst-name {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  max-width: 100%;
}

.cell-ellipsis {
  display: block;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inst-name .cell-ellipsis {
  flex: 1 1 auto;
}

.inst-icon {
  width: 24px;
  height: 24px;
  border-radius: 6px;
  object-fit: cover;
  flex-shrink: 0;
}

.status-cell {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
  min-width: 0;
}

.status-url-row {
  display: flex;
  align-items: center;
  gap: 4px;
  min-width: 0;
  max-width: 100%;
}

.status-url {
  font-size: 12px;
  // Token-bearing URLs are long; ellipsize inside the table cell and keep
  // the full URL in the hover title / copy button.
  display: block;
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.inst-actions {
  flex-wrap: nowrap;
  white-space: nowrap;
}

.empty-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--color-text-1);
}

.copy-hint {
  margin: 0;
  font-size: 12px;
  color: var(--color-text-3);
}

.home-add-row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 16px;
}

.home-path-input {
  flex: 1;
}

.home-used {
  font-size: 12px;
  color: var(--color-text-3);
  white-space: nowrap;
}
</style>
