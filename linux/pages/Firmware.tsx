import { Canvas, Text, token, View, windowSize } from "nsh";
import { ResizablePanel, ResizablePanelGroup } from "nsh/base";
import { ChooseFirmware, WorkbenchIcon, FirmwareHistory } from "chipdock/native";

import { FirmwareDocument } from "./FirmwareDocument";
import { FlashOptions } from "./FlashOptions";
import { TargetDialog } from "./TargetDialog";

function EmptyDocument() {
  return (
    <View class="flex-1" style={{ gap: 22 }}>
      <View class="relative flex-1 py-6 gap-2 items-center justify-center" style={{ minHeight: 170, borderRadius: 10 }}>
        <Canvas class="absolute inset-0" style={{ pointerEvents: "none" }} draw={(ctx, size) => {
          ctx.strokeStyle = token("foreground");
          ctx.globalAlpha = 0.175;
          ctx.lineWidth = 1.5;
          ctx.setLineDash([6, 4]);
          ctx.beginPath();
          ctx.roundRect(0.75, 0.75, Math.max(0, size.width - 1.5), Math.max(0, size.height - 1.5), 9.25);
          ctx.stroke();
        }} />
        <View class="mb-1 text-muted-foreground" style={{ width: 36, height: 36 }}>
          <WorkbenchIcon name="drop-doc" style={{ width: 36, height: 36 }} />
        </View>
        <Text style={{ fontSize: 15, fontWeight: "500" }}>将固件拖到这里</Text>
        <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>支持 ELF、AXF、HEX 与 BIN 文件</Text>
        <View class="mt-2"><ChooseFirmware /></View>
      </View>
      <FirmwareHistory />
    </View>
  );
}

export function Firmware(props: { empty: boolean; targetSetup: boolean }) {
  return (
    <View class="flex-1 min-h-0">
      <ResizablePanelGroup axis="vertical" class="flex-1" handleClass="bg-border"
        handleAccessibilityLabel="调整固件文档和烧录选项高度">
        <ResizablePanel minSize={260}>
          <View class="flex-1 min-h-0 py-6" style={{ paddingHorizontal: Math.max(32, windowSize().width * 0.05), gap: 20 }}>
            <If when={props.empty} else={<FirmwareDocument />}><EmptyDocument /></If>
          </View>
        </ResizablePanel>
        <ResizablePanel size={300} minSize={260} maxSize={420}><FlashOptions /></ResizablePanel>
      </ResizablePanelGroup>
      <If when={props.targetSetup}><TargetDialog /></If>
    </View>
  );
}
