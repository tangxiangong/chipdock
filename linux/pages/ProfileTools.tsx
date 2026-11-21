import { signal, Text, token, View } from "nsh";
import { ProfileAction, ProfileName, ProfileSession } from "chipdock/native";

function SummaryRow(props: { title: string; value: string }) {
  return <View class="flex-row" style={{ gap: 14 }}>
    <Text class="text-muted-foreground" style={{ width: 24.5, fontSize: token("caption-size") }}>{props.title}</Text>
    <Text class="flex-1 min-w-0" style={{ fontSize: token("caption-size") }}>{props.value}</Text>
  </View>;
}

export function ProfileTools() {
  const summary = signal({ target: "", wire: "", speed: "", erase: "" });
  return <ProfileSession onSummaryChange={summary.set} class="p-6 leading-[1.2] text-(length:--body-size)"
    style={{ width: 400, gap: 18 }}>
    <View style={{ gap: 3 }}>
      <Text class="font-bold">保存烧录配置</Text>
      <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>保存探针、目标芯片、连接参数与烧录选项。</Text>
    </View>
    <ProfileName />
    <View class="p-3 bg-foreground/5" style={{ borderRadius: 8, gap: 6 }}>
      <SummaryRow title="目标" value={summary().target || "自动识别"} />
      <SummaryRow title="接口" value={`${summary().wire} · ${summary().speed} kHz`} />
      <SummaryRow title="擦除" value={summary().erase === "full" ? "整片擦除" : summary().erase === "skip" ? "跳过擦除" : "按需擦除"} />
    </View>
    <View class="flex-row justify-end gap-2"><ProfileAction /><ProfileAction save /></View>
  </ProfileSession>;
}
