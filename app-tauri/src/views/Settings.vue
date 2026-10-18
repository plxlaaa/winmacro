<script setup lang="ts">
import { ref, onMounted } from "vue";
import { getVersion } from "@tauri-apps/api/app";
import { enable as enableAutostart, disable as disableAutostart, isEnabled as autostartEnabled } from "@tauri-apps/plugin-autostart";
import { api, type Settings } from "../api";
import {
  checkForUpdate,
  updateChecking,
  updateError,
  updateInfo,
  showUpdateDialog,
} from "../updater";

const emit = defineEmits<{ (e: "saved"): void }>();

const s = ref<Settings>({
  resolution_mode: "auto",
  screen_width: 1920,
  screen_height: 1080,
  wait_seconds: 3,
  hide_on_play: true,
  record_mouse_move: true,
  auto_check_updates: true,
  close_to_tray: true,
  trajectory_simplify: "medium",
  use_window_anchor: true,
});
const saved = ref(false);
const screenText = ref("—");
const appVersion = ref("");
const isLatest = ref(false);
const autostartOn = ref(false);

async function toggleAutostart() {
  try {
    if (autostartOn.value) {
      await enableAutostart();
    } else {
      await disableAutostart();
    }
  } catch (e) {
    alert(String(e));
    autostartOn.value = !autostartOn.value;
  }
}

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

async function manualCheck() {
  isLatest.value = false;
  const u = await checkForUpdate();
  if (u) {
    showUpdateDialog.value = true;
  } else if (!updateError.value) {
    isLatest.value = true;
  }
}

onMounted(async () => {
  s.value = await api.getSettings();
  getVersion()
    .then((v) => (appVersion.value = v))
    .catch(() => (appVersion.value = "未知"));
  api.screenSize()
    .then(([w, h]) => (screenText.value = w ? `${w}×${h}` : "未知"))
    .catch(() => (screenText.value = "未知"));
  autostartEnabled()
    .then((v) => (autostartOn.value = v))
    .catch(() => (autostartOn.value = false));
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
          <div class="form-label">轨迹精度</div>
          <div class="form-hint">
            录制完成后对鼠标移动路径做 Douglas-Peucker 折线简化（仅影响体积与速度，不影响点击命中）。
            <span style="color: var(--muted)">
              当前：{{
                s.trajectory_simplify === "off" ? "不简化" :
                s.trajectory_simplify === "low" ? "极致（0.5 px）" :
                s.trajectory_simplify === "high" ? "粗略（4 px）" :
                "推荐（1.5 px）"
              }}
            </span>
          </div>
        </div>
        <div class="form-control">
          <select
            :value="s.trajectory_simplify"
            @change="s.trajectory_simplify = ($event.target as HTMLSelectElement).value as Settings['trajectory_simplify']"
          >
            <option value="off">不简化</option>
            <option value="low">极致（0.5 px）</option>
            <option value="medium">推荐（1.5 px）</option>
            <option value="high">粗略（4 px）</option>
          </select>
        </div>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">窗口锚定回放</div>
          <div class="form-hint">
            回放时优先按录制时的目标窗口定位（窗口移动了脚本也跟着走）。窗口已关闭则回退到全屏缩放。
            关闭后始终使用 v2.4 的全屏映射（旧行为）。
          </div>
        </div>
        <label class="switch">
          <input v-model="s.use_window_anchor" type="checkbox" />
          <span class="track"></span>
          <span class="thumb"></span>
        </label>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">自动检查更新</div>
          <div class="form-hint">
            启动 3 秒后静默检查一次 GitHub Releases；发现新版本时弹窗提示，也可在下方手动检查。
          </div>
        </div>
        <label class="switch">
          <input v-model="s.auto_check_updates" type="checkbox" />
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

    <div class="stat-card" style="margin-bottom: 22px">
      <div class="form-row">
        <div>
          <div class="form-label">关闭按钮最小化到托盘</div>
          <div class="form-hint">
            点窗口 ✕ 时隐藏到系统托盘（F10 热键录制仍可用），托盘右键菜单可退出。关闭则真正退出程序。
          </div>
        </div>
        <label class="switch">
          <input v-model="s.close_to_tray" type="checkbox" />
          <span class="track"></span>
          <span class="thumb"></span>
        </label>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">开机自动启动</div>
          <div class="form-hint">登录 Windows 后自动运行 WinMacro 并驻留托盘，随时可热键执行脚本。</div>
        </div>
        <label class="switch">
          <input v-model="autostartOn" type="checkbox" @change="toggleAutostart" />
          <span class="track"></span>
          <span class="thumb"></span>
        </label>
      </div>

      <div class="form-row">
        <div>
          <div class="form-label">应用更新</div>
          <div class="form-hint">
            当前版本 v{{ appVersion || "…" }}。更新包经 minisign 签名校验后静默安装，全程无需浏览器。
          </div>
        </div>
        <div class="form-control">
          <button class="btn" :disabled="updateChecking" @click="manualCheck">
            {{ updateChecking ? "检查中…" : "检查更新" }}
          </button>
          <span v-if="isLatest && !updateInfo" style="color: var(--ok)">✓ 已是最新版本</span>
          <span v-if="updateInfo" style="color: var(--accent)">
            可更新到 v{{ updateInfo.version }}
          </span>
          <span v-if="updateError" style="color: var(--danger); font-size: 12px">
            {{ updateError }}
          </span>
        </div>
      </div>
    </div>

    <button class="btn primary" @click="save">保存设置</button>
    <span v-if="saved" style="color: var(--ok); margin-left: 12px">✓ 已保存</span>
  </div>
</template>
