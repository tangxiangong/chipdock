import { signal, Text, token, View } from "nsh";
import {
  FirmwareBytes, FirmwareDocumentHost, FirmwareFileMenu, FirmwareMemoryMap,
  WorkbenchIcon, type FirmwareDetails,
} from "chipdock/native";

function Metric(props: { label: string; value: string; mono?: boolean }) {
  return <View style={{ gap: 3 }}>
    <Text class="text-muted-foreground" style={{ fontSize: token("small-size") }}>{props.label}</Text>
    <Text class={props.mono ? "font-mono" : ""} style={{ fontSize: token("body-size") }}>{props.value}</Text>
  </View>;
}

const fileColors: Record<string, string> = {
  ELF: "bg-firmware-elf/14 text-firmware-elf",
  HEX: "bg-firmware-tint/14 text-firmware-tint",
  BIN: "bg-firmware-bin/14 text-firmware-bin",
};

export function FirmwareDocument() {
  const details = signal<FirmwareDetails>({
    name: "", directory: "", format: "", size: "", segments: "", range: "", previewRange: "", analyzing: false,
  });
  return <FirmwareDocumentHost class="flex-1 min-h-0 gap-[20px]" onChange={event => details.set(event.details)}>
    <View class="flex-row items-center gap-[14px]">
      <View class={`size-[44px] rounded-[10px] items-center justify-center ${fileColors[details().format] ?? "bg-muted-foreground/14 text-muted-foreground"}`}>
        <WorkbenchIcon name="file" style={{ width: 22, height: 22 }} />
      </View>
      <View class="flex-1 min-w-0 gap-[3px]">
        <Text class="font-semibold" style={{ fontSize: 17 }}>{details().name}</Text>
        <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>{details().directory}</Text>
      </View>
      <FirmwareFileMenu />
    </View>
    <If when={details().format} else={
      <View>
        <If when={details().error != null} else={<View><If when={details().analyzing}>
          <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>正在分析固件…</Text>
        </If></View>}>
          <View selectable class="flex-row items-start gap-2 p-[10px] rounded-[6px] bg-firmware-tint/8">
            <WorkbenchIcon name="warning-fill" class="text-firmware-tint size-[14px]" />
            <Text class="flex-1 min-w-0" style={{ fontSize: token("caption-size") }}>{details().error}</Text>
          </View>
        </If>
      </View>
    }>
      <View class="flex-1 min-h-0 gap-[20px]">
        <FirmwareMemoryMap />
        <View class="flex-1 min-h-[64px] gap-2">
          <View class="flex-row items-center">
            <Text style={{ fontSize: token("caption-size") }}>固件内容</Text><View class="flex-1" />
            <Text class="font-mono text-muted-foreground" style={{ fontSize: token("small-size") }}>{details().previewRange}</Text>
          </View>
          <FirmwareBytes />
        </View>
        <View class="flex-row items-start flex-wrap gap-[36px]">
          <Metric label="格式" value={details().format} /><Metric label="写入大小" value={details().size} />
          <Metric label="区段" value={details().segments} /><Metric label="地址范围" mono value={details().range} />
        </View>
      </View>
    </If>
  </FirmwareDocumentHost>;
}
