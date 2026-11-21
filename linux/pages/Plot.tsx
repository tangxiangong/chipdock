import { signal, Text, token, View } from "nsh";
import {
  PlotActions, PlotApply, PlotBinaryEditor, PlotDraftInput, PlotFieldPicker, PlotModePicker, PlotPage,
  PlotPreview, PlotTrace, WorkbenchIcon, type PlotPresentation,
} from "chipdock/native";

const chartColors = ["chart-1", "chart-2", "chart-3", "chart-4", "chart-5"] as const;

export function Plot() {
  const presentation = signal<PlotPresentation>({
    binary: false, result: "", dropped: 0, empty: true, sampling: true, names: [],
  });
  return (
    <PlotPage class="flex-1 min-h-0 bg-plot-background p-4 gap-3" onPresentationChange={({ state }) => presentation.set(state)}>
      <PlotActions />
      <View class="flex-row items-center gap-2">
        <PlotModePicker style={{ width: 180 }} />
        <PlotDraftInput field="fields" class="flex-1 min-w-0" />
        <PlotDraftInput field="template" class="flex-1 min-w-0" />
        <PlotApply />
      </View>
      <If when={presentation().binary}><PlotBinaryEditor /></If>
      <View class="flex-row items-center gap-2">
        <PlotDraftInput field="sample" class="flex-1 min-w-0" />
        <PlotPreview />
        <PlotFieldPicker style={{ width: 145 }} />
      </View>
      <If when={presentation().result}>
        <Text class="font-mono text-muted-foreground" style={{ fontSize: 10 }}>{presentation().result}</Text>
      </If>
      <If when={presentation().dropped > 0}>
        <Text class="text-warning" style={{ fontSize: token("caption-size") }}>{`已超出保留范围 ${presentation().dropped} 条记录`}</Text>
      </If>
      <View class="relative flex-1 min-h-0">
        <If when={!presentation().empty} else={
          <View class="absolute inset-0 items-center justify-center" style={{ paddingTop: 10.5, gap: 6 }}>
            <WorkbenchIcon name="plot-empty" class="text-plot-empty-symbol" style={{ width: 44, height: 44, marginBottom: 10 }} />
            <Text class="font-semibold text-muted-foreground" style={{ fontSize: 15 }}>暂无曲线数据</Text>
            <Text class="text-muted-foreground" style={{ fontSize: 15 }}>{presentation().sampling ? "收到可解析的数值后会实时绘制。" : "已暂停采样。"}</Text>
          </View>
        }><PlotTrace /></If>
      </View>
      <View class="gap-1">
        <Text class="text-muted-foreground" style={{ fontSize: 11 }}>样本</Text>
        <If when={!presentation().empty}>
          <View class="flex-row flex-wrap gap-4">
            <Each of={presentation().names} key={name => name}>{(name, index) =>
              <View class="flex-row items-center gap-1">
                <View class="rounded-full" style={{ width: 7, height: 7, backgroundColor: token(chartColors[index() % chartColors.length]) }} />
                <Text style={{ fontSize: token("caption-size") }}>{name()}</Text>
              </View>}
            </Each>
          </View>
        </If>
      </View>
    </PlotPage>
  );
}
