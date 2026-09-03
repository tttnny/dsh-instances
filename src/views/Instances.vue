<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Message } from '@arco-design/web-vue'
import { api } from '@/api'
import { useLauncherStore } from '@/stores/launcher'
import type { DshInstance, InstanceState } from '@/api/types'
import NewInstanceDialog from '@/components/NewInstanceDialog.vue'

const router = useRouter()
const { t } = useI18n()
const store = useLauncherStore()

const newVisible = ref(false)

const columns = computed(() => [
  { title: t('instances.table.name'), slotName: 'name', width: 170 },
  { title: t('instances.table.version'), slotName: 'version', width: 120 },
  { title: t('instances.table.home'), slotName: 'home', width: 190 },
  { title: t('instances.table.profile'), slotName: 'profile', width: 110 },
  { title: t('instances.table.status'), slotName: 'status', width: 110 },
  {
    title: t('instances.table.actions'),
    slotName: 'actions',
    width: 200,
    align: 'right' as const,
  },
])

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

async function onDelete(id: string, name: string) {
  try {
    await api.deleteInstance(id)
    await store.refreshInstances()
    Message.success(t('instances.deleted'))
  } catch (e) {
    Message.error(String(e))
  }
}

// --- Copy instance -----------------------------------------------------------

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

async function onOpenBrowser(id: string) {
  try {
    await api.openInstanceWindow(id)
  } catch (e) {
    Message.error(String(e))
  }
}
</script>

<template>
  <div class="dl-page instances-page">
    <div class="dl-card instances-card">
      <div class="dl-card-title">
        <div class="title-with-count">
          <h3>{{ t('instances.title') }}</h3>
          <span class="count-badge tnum">{{ store.instances.length }}</span>
        </div>
        <div class="dl-toolbar">
          <button class="mac-primary-btn" @click="newVisible = true">
            <svg viewBox="0 0 16 16" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
              <line x1="8" y1="3" x2="8" y2="13" />
              <line x1="3" y1="8" x2="13" y2="8" />
            </svg>
            <span>{{ t('instances.newInstance') }}</span>
          </button>
        </div>
      </div>

      <a-table
        :columns="columns"
        :data="store.instances"
        :pagination="false"
        row-key="id"
        :scroll="{ x: 790 }"
        class="apple-styled-table"
      >
        <template #name="{ record }">
          <div class="inst-name-cell">
            <img v-if="iconMap[record.id]" :src="iconMap[record.id]!" class="inst-avatar" alt="" />
            <img v-else src="@/assets/launcher-icon.png" class="inst-avatar" alt="" />
            <span class="cell-title" :title="record.name">{{ record.name }}</span>
          </div>
        </template>

        <template #version="{ record }">
          <span
            class="cell-text tnum"
            :title="store.versionById(record.version_id)?.version ?? record.version_id"
          >
            {{ store.versionById(record.version_id)?.version ?? record.version_id }}
          </span>
        </template>

        <template #home="{ record }">
          <a-tooltip :content="store.homeById(record.home_id)?.path">
            <span class="cell-text home-link">
              {{ store.homeById(record.home_id)?.name ?? record.home_id }}
            </span>
          </a-tooltip>
        </template>

        <template #profile="{ record }">
          <span class="profile-chip" :title="record.last_profile ?? record.default_profile ?? ''">
            {{ record.last_profile ?? record.default_profile ?? '—' }}
          </span>
        </template>

        <template #status="{ record }">
          <div class="status-cell">
            <span :class="['apple-status-dot', store.statusOf(record.id).state]">
              {{ t(`home.status.${store.statusOf(record.id).state}`) }}
            </span>
            <div v-if="store.statusOf(record.id).url" class="status-url-pill">
              <span class="url-click tnum" @click="onOpenBrowser(record.id)">
                {{ store.statusOf(record.id).url }}
              </span>
              <button class="mini-copy" :title="t('common.copy')" @click="copyUrl(store.statusOf(record.id).url!)">
                <svg viewBox="0 0 16 16" width="10" height="10" fill="none" stroke="currentColor" stroke-width="1.6">
                  <rect x="5" y="5" width="8" height="8" rx="1.5" />
                  <path d="M3 11V3a1 1 0 0 1 1-1h8" />
                </svg>
              </button>
            </div>
          </div>
        </template>

        <template #actions="{ record }">
          <div class="table-actions">
            <button
              class="mac-action-pill"
              :title="t('instances.table.edit')"
              @click="router.push({ name: 'instance-edit', params: { id: record.id } })"
            >
              {{ t('instances.table.edit') }}
            </button>
            <button
              class="mac-action-pill"
              :title="t('instances.table.copy')"
              @click="openCopy(record)"
            >
              {{ t('instances.table.copy') }}
            </button>
            <a-popconfirm
              :content="t('instances.confirmDelete', { name: record.name })"
              @ok="onDelete(record.id, record.name)"
            >
              <button class="mac-action-pill danger" :title="t('instances.table.delete')">
                {{ t('instances.table.delete') }}
              </button>
            </a-popconfirm>
          </div>
        </template>

        <template #empty>
          <div class="table-empty-block">
            <div class="empty-title">{{ t('instances.emptyTitle') }}</div>
            <div class="empty-desc">{{ t('instances.emptyDesc') }}</div>
            <button class="mac-primary-btn" style="margin-top: 12px" @click="newVisible = true">
              {{ t('instances.newInstance') }}
            </button>
          </div>
        </template>
      </a-table>
    </div>

    <!-- Copy Instance Dialog -->
    <a-modal
      :visible="!!copySource"
      :title="t('instances.copyTitle')"
      :ok-text="t('instanceEdit.save')"
      :cancel-text="t('instanceEdit.cancel')"
      :ok-button-props="{ disabled: !copyValid, loading: copying }"
      modal-class="apple-modal"
      @ok="confirmCopy"
      @cancel="closeCopy"
    >
      <a-form layout="vertical" :model="{}">
        <a-form-item :label="t('instances.copyNameLabel')" required>
          <a-input v-model="copyName" :placeholder="t('instances.copyNamePlaceholder')" />
        </a-form-item>
        <a-form-item :label="t('instances.copyHomeLabel')">
          <div class="apple-segmented">
            <button
              type="button"
              :class="{ active: !copyNewHome }"
              @click="copyNewHome = false"
            >
              {{ t('instances.copyHomeReuse') }}
            </button>
            <button
              type="button"
              :class="{ active: copyNewHome }"
              @click="copyNewHome = true"
            >
              {{ t('instances.copyHomeNew') }}
            </button>
          </div>
        </a-form-item>
        <p class="copy-hint-text">{{ t('instances.copyHint') }}</p>
      </a-form>
    </a-modal>

    <NewInstanceDialog v-model:visible="newVisible" />
  </div>
</template>

<style lang="scss" scoped>
.title-with-count {
  display: flex;
  align-items: center;
  gap: 8px;

  .count-badge {
    padding: 1px 7px;
    font-size: 11.5px;
    font-weight: 600;
    border-radius: 10px;
    background: var(--apple-group-bg);
    color: var(--color-text-3);
  }
}

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

// Table cell layouts
.inst-name-cell {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  max-width: 100%;

  .inst-avatar {
    width: 26px;
    height: 26px;
    border-radius: 7px;
    object-fit: cover;
    flex-shrink: 0;
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
  }

  .cell-title {
    font-weight: 600;
    color: var(--color-text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
}

.cell-text {
  font-size: 13px;
  color: var(--color-text-2);

  &.home-link {
    color: var(--color-text-3);
    &:hover {
      color: var(--color-text-1);
    }
  }
}

.profile-chip {
  padding: 2px 8px;
  font-size: 11.5px;
  border-radius: 6px;
  background: var(--apple-group-bg);
  color: var(--color-text-2);
  white-space: nowrap;
  display: inline-block;
}

.status-cell {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;

  .status-url-pill {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    background: var(--apple-group-bg);
    padding: 1px 6px;
    border-radius: 6px;
    max-width: 140px;

    .url-click {
      font-size: 11px;
      color: rgb(var(--primary-6));
      cursor: pointer;
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;

      &:hover {
        text-decoration: underline;
      }
    }

    .mini-copy {
      border: none;
      background: transparent;
      padding: 1px;
      color: var(--color-text-3);
      cursor: pointer;

      &:hover {
        color: var(--color-text-1);
      }
    }
  }
}

.table-actions {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  flex-shrink: 0;
}

.mac-action-pill {
  border: 1px solid var(--apple-card-border);
  background: var(--apple-card-bg);
  color: var(--color-text-2);
  border-radius: 6px;
  padding: 3px 9px;
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s ease;

  &:hover {
    background: var(--apple-group-bg);
    color: var(--color-text-1);
  }

  &.danger {
    color: rgb(var(--red-6));

    &:hover {
      background: rgb(var(--red-6) / 12%);
    }
  }

  &:active {
    transform: scale(var(--apple-active-scale));
  }
}

.table-empty-block {
  padding: 36px 16px;
  text-align: center;

  .empty-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--color-text-1);
  }

  .empty-desc {
    font-size: 12.5px;
    color: var(--color-text-3);
    margin-top: 4px;
  }
}

.copy-hint-text {
  margin: 8px 0 0;
  font-size: 12px;
  color: var(--color-text-3);
  line-height: 1.5;
}
</style>
