import { platform, Text, View } from "nsh";
import { currentWindow, titleBar } from "nsh/desktop";
import { ActivityIndicator, FirmwareActions, SerialActions, SidebarToggle, WorkbenchModes } from "chipdock/native";

export function WorkbenchToolbar(props: { mode: string; modal: boolean }) {
  const window = currentWindow();
  const active = () => (window.state()?.focused ?? true) && !props.modal;
  const mac = platform.os === "macos";
  return (
    <View class="relative z-[1] shrink-0 flex-row items-center gap-3 border-b border-foreground/10"
      desktop={{ windowDrag: true }} style={{
        height: 52,
        paddingLeft: mac ? 95 : Math.max(80, titleBar().left),
        paddingRight: mac ? 9 : 9 + titleBar().right,
      }}>
      <WorkbenchModes />
      <If when={props.mode === "serial"}><SidebarToggle /></If>
      <Text class={active() ? "font-bold text-foreground" : "font-bold text-toolbar-title-inactive"}
        style={{ paddingLeft: 1, fontSize: 15 }}>{props.mode === "flash" ? "固件烧录" : "串口连接"}</Text>
      <View class="flex-1" />
      <If when={props.mode === "flash"}><ActivityIndicator /></If>
      <If when={props.mode === "flash"} else={<SerialActions />}><FirmwareActions /></If>
    </View>
  );
}
