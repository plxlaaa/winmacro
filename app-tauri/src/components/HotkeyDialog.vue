<script setup lang="ts">
import { ref, onMounted } from "vue";

const props = defineProps<{ name: string; current: string | null }>();
const emit = defineEmits<{
  (e: "close"): void;
  (e: "save", hotkey: string | null): void;
}>();

const rootEl = ref<HTMLElement | null>(null);
onMounted(() => rootEl.value?.focus());

const captured = ref<string | null>(props.current);
const error = ref("");

const PRETTY: Record<string, string> = {
  Control: "Ctrl",
  Alt: "Alt",
  Shift: "Shift",
  Meta: "Win",
};

function prettyCombo(spec: string | null): string {
  if (!spec) return "未设置";
  return spec
    .split("+")
    .map((part) => PRETTY[part] ?? part)
    .join(" + ");
}

function onKeydown(ev: KeyboardEvent) {
  ev.preventDefault();
  ev.stopPropagation();

  if (ev.key === "Escape") {
    emit("close");
    return;
  }
  if (ev.key === "Backspace" || ev.key === "Delete") {
    captured.value = null;
    error.value = "";
    return;
  }
  if (ev.key === "Enter") {
    onSave();
    return;
  }

  const mods: string[] = [];
  if (ev.ctrlKey) mods.push("Control");
  if (ev.altKey) mods.push("Alt");
  if (ev.shiftKey) mods.push("Shift");
  if (ev.metaKey) mods.push("Meta");

  // 主键（e.code：KeyA / Digit1 / F2 / ArrowUp ...）
  const code = ev.code;
  if (!code || code === "ControlLeft" || code === "ControlRight"
      || code === "AltLeft" || code === "AltRight"
      || code === "ShiftLeft" || code === "ShiftRight"
      || code === "MetaLeft" || code === "MetaRight") {
    return; // 只按了修饰键，等待主键
  }

  if (mods.length === 0) {
    error.value = "请加上 Ctrl / Alt / Shift / Win 修饰键，避免误触";
    return;
  }
  // F10/F11 是保留键
  if (code === "F10" || code === "F11") {
    error.value = "F10 / F11 是保留热键（录制 / 停止回放）";
    return;
  }
  error.value = "";
  captured.value = [...mods, code].join("+");
}

function onSave() {
  emit("save", captured.value);
}
</script>

<template>
  <div class="dialog-mask" @keydown="onKeydown" tabindex="0" ref="rootEl">
    <div class="dialog" style="max-width: 460px">
      <div class="dialog-title">设置脚本热键</div>
      <div class="dialog-sub">{{ name }}</div>

      <div class="hk-box">
        <div class="hk-value">{{ prettyCombo(captured) }}</div>
        <div class="hk-hint">直接按下组合键（如 Ctrl + Alt + 1）替换；退格键清除</div>
      </div>
      <div v-if="error" class="hk-error">{{ error }}</div>

      <div class="dialog-actions">
        <button class="btn danger" v-if="captured" @click="captured = null">清除</button>
        <span style="flex: 1"></span>
        <button class="btn" @click="emit('close')">取消</button>
        <button class="btn primary" @click="onSave">保存</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.hk-box {
  margin: 16px 0 6px;
  background: var(--bg);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  padding: 22px;
  text-align: center;
}
.hk-value {
  font-size: 20px;
  font-weight: 700;
  color: var(--accent);
  font-family: Consolas, monospace;
}
.hk-hint {
  margin-top: 10px;
  font-size: 12px;
  color: var(--faint);
}
.hk-error {
  color: var(--danger);
  font-size: 12.5px;
  margin: 6px 0;
}
</style>
