<script setup lang="ts">
import { ref, onMounted } from "vue";
import { api, type Settings } from "../api";

const emit = defineEmits<{ (e: "saved"): void }>();

const s = ref<Settings>({
  resolution_mode: "auto",
  screen_width: 1920,
  screen_height: 1080,
  wait_seconds: 3,
  hide_on_play: true,
  record_mouse_move: true,
});
const saved = ref(false);
const screenText = ref("—");

const RESOLUTIONS: Array<[number, number, string]> = [
  [1280, 720, "HD 720p"],
  [1366, 768, "HD"],
  [1600, 900, "HD+"],
  [1920, 1080, "Full HD"],
  [2560, 1440, "2K QHD"],
  [3840, 2160, "4K UHD"],
];

function applyPreset(value: string) {
  const [w, h] = value.split("x").map(Number);
  if (w && h) {
    s.value.screen_width = w;
    s.value.screen_height = h;
  }
}

async function save() {
  try {
    await api.setSettings(s.value);
    saved.value = true;
    setTimeout(() => (saved.value = false), 1800);
    emit("saved");
  } catch (e) {
    alert(String(e));
  }
}

onMounted(async () => {
  s.value = await api.getSettings();
  api.screenSize()
    .then(([w, h]) => (screenText.value = w ? `${w}×${h}` : "未知"))
    .catch(() => (screenText.value = "未知"));
});
</script>

<template>
  <div>
    <div class="page-title">设置</div>
    <div class="page-sub">所有设置立即持久化到 ~/.winmacro/settings.json，并自动兼容旧版本数据。</div>

    <div class="stat-card" style="margin-bottom: 22px">
      <div class="form-row">
        <div>
          <div class="form-label">开始脚本执行前等待秒数</div>
          <div class="form-hint">
            点击「执行选中」后先倒计时这段时间，再开始回放。给足切换窗口、定位目标的时间。0 为立即执行。
          </div>
        </div>
        <div class="form-control">
          <input v-model.number="s.wait_seconds" type="number" min="0" max="60" />
          <span style="color: var(--muted)">秒</span>
        </div>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">脚本开始执行后关闭界面</div>
          <div class="form-hint">
            回放开始时自动隐藏 WinMacro 窗口，结束或按 F11 停止后自动恢复。避免界面挡住被操作的目标程序。
          </div>
        </div>
        <label class="switch">
          <input v-model="s.hide_on_play" type="checkbox" />
          <span class="track"></span>
          <span class="thumb"></span>
        </label>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">记录鼠标轨迹</div>
          <div class="form-hint">
            关闭后只记录点击、滚轮、键盘等操作，忽略纯移动——脚本更精简，回放时直接跳到每个点击位置。
          </div>
        </div>
        <label class="switch">
          <input v-model="s.record_mouse_move" type="checkbox" />
          <span class="track"></span>
          <span class="thumb"></span>
        </label>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">录制分辨率</div>
          <div class="form-hint">
            回放时把录制坐标映射到当前屏幕（当前：{{ screenText }}）。换显示器分辨率后回放依然落点准确。
          </div>
        </div>
        <div class="form-control">
          <select
            :value="s.resolution_mode"
            @change="s.resolution_mode = ($event.target as HTMLSelectElement).value"
          >
            <option value="auto">自动识别（推荐）</option>
            <option value="manual">手动指定</option>
          </select>
          <template v-if="s.resolution_mode === 'manual'">
            <select @change="applyPreset(($event.target as HTMLSelectElement).value)">
              <option value="" disabled selected>常用分辨率…</option>
              <option v-for="[w, h, name] in RESOLUTIONS" :key="name" :value="`${w}x${h}`">
                {{ w }}×{{ h }}（{{ name }}）
              </option>
            </select>
            <input v-model.number="s.screen_width" type="number" style="min-width: 76px" />
            <span style="color: var(--muted)">×</span>
            <input v-model.number="s.screen_height" type="number" style="min-width: 76px" />
          </template>
        </div>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">快捷键</div>
          <div class="form-hint">全局生效，录制中的热键本身不会写进脚本。</div>
        </div>
        <div style="color: var(--muted); display: flex; gap: 14px">
          <span><span class="hint-kbd">F10</span> 开始 / 结束录制</span>
          <span><span class="hint-kbd">F11</span> 停止回放</span>
        </div>
      </div>
    </div>

    <button class="btn primary" @click="save">保存设置</button>
    <span v-if="saved" style="color: var(--ok); margin-left: 12px">✓ 已保存</span>
  </div>
</template>
