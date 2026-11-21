import { signal, Text, token, View, windowSize, type JSX } from "nsh";
import { EraseModePicker, FlashButton, FlashOptionsHost, FlashToggle, type FlashState } from "chipdock/native";

function Caption(props: { children: string }) {
  return <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>{props.children}</Text>;
}
function OptionRow(props: { label: string; top?: boolean; children: JSX.Element }) {
  return <View class="flex-row items-start gap-[14px]">
    <Text class={`w-[52px] text-right text-muted-foreground ${props.top ? "mt-1" : ""}`}
      style={{ fontSize: token("body-size") }}>{props.label}</Text>
    {props.children}
  </View>;
}

export function FlashOptions() {
  const state = signal<FlashState>({
    connected: false, busy: false, empty: true, dryRun: false, target: "", chip: "", wire: "", speed: "", probe: "", erase: "",
  });
  const eraseHelp = () => {
    switch (state().erase) {
      case "full": return "写入前擦除整个 Flash，其他数据会丢失";
      case "skip": return "不擦除，目标区域须已擦除";
      default: return "仅擦除固件覆盖的扇区";
    }
  };
  const hint = () => state().busy ? "" : !state().connected ? "连接后即可烧录（⌘K）" : state().empty ? "选择固件后即可烧录" : "按 ⌘↩ 开始";
  const primary = () => state().busy ? "正在处理…" : !state().connected ? "连接目标" : state().dryRun ? "开始预检查" : `烧录到 ${state().chip}`;
  return <FlashOptionsHost onChange={event => state.set(event.state)}>
    <View class="py-6 gap-6" style={{ paddingHorizontal: Math.max(windowSize().width * 0.05, 32) }}>
      <View class="gap-4">
        <OptionRow label="目标" top>
          <View class="flex-1 mt-1 gap-[3px]">
            <View class="flex-row items-center gap-[6px]">
              <View class={`size-[7px] rounded-full ${state().connected ? "bg-success" : "bg-muted-foreground/40"}`} />
              <Text class="font-medium" style={{ fontSize: token("body-size") }}>{state().target}</Text>
              <If when={!state().connected}><Caption>未连接</Caption></If>
            </View>
            <Caption>{`${state().wire} · ${state().speed} kHz · ${state().probe || "自动选择探针"}`}</Caption>
          </View>
          <If when={state().connected}><FlashButton command="disconnect" label="断开连接" /></If>
          <FlashButton command="setup" label="更改…" />
        </OptionRow>
        <OptionRow label="擦除" top>
          <View class="gap-1"><View class="w-[100px]"><EraseModePicker /></View><Caption>{eraseHelp()}</Caption></View>
        </OptionRow>
        <OptionRow label="完成后">
          <View class="gap-[7px]"><FlashToggle option="verify" /><FlashToggle option="reset" /><FlashToggle option="dryRun" /></View>
        </OptionRow>
      </View>
      <View class="flex-row items-center gap-4"><Caption>{hint()}</Caption><View class="flex-1" />
        <FlashButton command="erase" label="擦除芯片…" /><FlashButton command="program" label={primary()} />
      </View>
    </View>
  </FlashOptionsHost>;
}
