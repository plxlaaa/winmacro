<script setup lang="ts">
import { computed } from "vue";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  updatePhase,
  updateProgress,
  updateError,
  downloadUpdate,
  fmtMB,
} from "../updater";

const props = defineProps<{ version: string; notes: string }>();
const emit = defineEmits<{ (e: "close"): void }>();

const pct = computed(() => {
  const p = updateProgress.value;
  if (!p || !p.total) return 0;
  return Math.min(100, (p.received / p.total) * 100);
});

const progressText = computed(() => {
  const p = updateProgress.value;
  if (!p) return "";
  const done = fmtMB(p.received);
  return p.total ? `${done} / ${fmtMB(p.total)}（${pct.value.toFixed(0)}%）` : done;
});

async function go() {
  try {
    await downloadUpdate();
  } catch {
    /* 错误已写入 updateError，模板展示 */
  }
}
</script>

<template>
  <div class="dialog-mask">
    <div class="dialog" style="max-width: 520px">
      <div class="dialog-title">🎉 发现新版本 v{{ props.version }}</div>

      <pre class="update-notes">{{ props.notes || "（本次更新无说明）" }}</pre>

      <div v-if="updatePhase === 'downloading'" class="update-progress">
        <div class="update-bar">
          <div class="update-bar-fill" :style="{ width: pct + '%' }"></div>
        </div>
        <div class="update-progress-text">{{ progressText }}</div>
      </div>

      <div v-if="updateError" class="update-error">{{ updateError }}</div>

      <div class="dialog-actions">
        <button
          v-if="updatePhase === 'idle'"
          class="btn primary"
          @click="go"
        >
          ⬇ 立即更新
        </button>
        <button
          v-if="updatePhase === 'ready'"
          class="btn primary"
          @click="relaunch()"
        >
          ↻ 重启并安装
        </button>
        <button
          v-if="updatePhase === 'idle'"
          class="btn"
          @click="emit('close')"
        >
          以后再说
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.update-notes {
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 220px;
  overflow-y: auto;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  padding: 14px;
  font-size: 13px;
  line-height: 1.8;
  color: var(--muted);
  margin: 14px 0;
  user-select: text;
}
.update-progress {
  margin: 14px 0;
}
.update-bar {
  height: 8px;
  border-radius: 4px;
  background: var(--card);
  border: 1px solid var(--border-strong);
  overflow: hidden;
}
.update-bar-fill {
  height: 100%;
  background: var(--accent);
  border-radius: 4px;
  transition: width 0.2s ease;
}
.update-progress-text {
  margin-top: 8px;
  color: var(--muted);
  font-size: 12.5px;
  text-align: center;
}
.update-error {
  color: var(--danger);
  font-size: 12.5px;
  margin: 8px 0;
  word-break: break-all;
}
</style>
