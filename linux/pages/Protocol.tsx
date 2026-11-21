import { Scroll, signal, Text, token, View, type JSX } from "nsh";
import {
  ProtocolAction, ProtocolEditorHost, ProtocolFrame, ProtocolIdentity, ProtocolOptionField, ProtocolLengthToggle, ProtocolResponsePicker,
  ProtocolSidebar, ProtocolTrial, WorkbenchIcon, type ProtocolEditorHostProps,
} from "chipdock/native";

type EditorState = Parameters<NonNullable<ProtocolEditorHostProps["onState"]>>[0];
const cardContent = "p-[13px] bg-protocol-group rounded-(--radius-lg)";

function EditorCard(props: { title: string; symbol: string; children: JSX.Element }) {
  return <View style={{ gap: 3 }}>
    <View class="flex-row items-center gap-2 px-2">
      <WorkbenchIcon name={props.symbol} />
      <Text class="font-semibold" style={{ fontSize: token("body-size") }}>{props.title}</Text>
    </View>
    {props.children}
  </View>;
}

function RuleOptions(props: { receive: boolean }) {
  return <View class={`gap-3 ${cardContent}`}>
    <View class="flex-row items-start gap-4">
      <ProtocolOptionField field="checksumStart" label="校验起点" class="flex-1" />
      <ProtocolOptionField field="checksumEnd" label="校验终点 · 不包含" class="flex-1" />
    </View>
    <View class="flex-row gap-2">
      <ProtocolOptionField field="lengthField" label="长度字段 · 可选" class="flex-1" />
      <ProtocolOptionField field="trailer" label="固定帧尾 HEX · 可选" class="flex-1" />
    </View>
    <ProtocolLengthToggle />
    <If when={props.receive} else={
      <View class="flex-row items-start gap-4">
        <ProtocolResponsePicker class="flex-1" />
        <ProtocolOptionField field="timeout" label="等待超时 · ms" class="flex-1" />
      </View>
    }>
      <View class="gap-3">
        <ProtocolOptionField field="delimiter" label="文本分隔符 HEX · 可选" />
        <Text class="text-muted-foreground" style={{ fontSize: 10, lineHeight: 13 }}>
          {"文本解析时清空帧模板，显示模板使用 {text}。单帧最多 4096 字节。"}
        </Text>
      </View>
    </If>
  </View>;
}

function RuleForm(props: { receive: boolean }) {
  return <Scroll class="flex-1 min-w-0">
    <View class="p-5 gap-5">
      <ProtocolIdentity />
      <EditorCard title="帧配置" symbol="code"><ProtocolFrame class={cardContent} /></EditorCard>
      <EditorCard title={props.receive ? "解析选项" : "校验与反馈"} symbol="shield">
        <RuleOptions receive={props.receive} />
      </EditorCard>
      <EditorCard title="离线试算" symbol="play"><ProtocolTrial class={cardContent} /></EditorCard>
    </View>
  </Scroll>;
}

function EmptyEditor() {
  return <View class="flex-1 items-center justify-center gap-2 text-muted-foreground">
    <WorkbenchIcon name="add-rule" class="mb-3 text-empty-symbol" style={{ width: 44, height: 44 }} />
    <Text class="font-bold" style={{ fontSize: 15 }}>添加第一条规则</Text>
    <Text style={{ fontSize: 15 }}>为常用指令设置名称，或定义接收帧的解析方式。</Text>
    <View class="mt-2 flex-row gap-2">
      <ProtocolAction command="addSend" /><ProtocolAction command="addReceive" />
    </View>
  </View>;
}

export function ProtocolEditor() {
  const state = signal<EditorState | null>(null);
  return <ProtocolEditorHost onState={state.set}
    class="bg-protocol-background text-(length:--body-size) leading-[1.2]" style={{ width: 860, height: 650 }}>
    <If when={state()}>{editor => <>
      <View class="p-5 flex-row items-center gap-3 border-b border-foreground/10">
        <WorkbenchIcon name="terminal" class="text-primary"
          style={{ width: 24, height: 24, transform: [{ scaleY: 0.86 }, { translateY: 0.25 }] }} />
        <View class="flex-1 gap-1">
          <Text class="font-semibold" style={{ fontSize: 15 }}>命令与接收规则</Text>
          <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>定义发送指令，解析设备反馈</Text>
        </View>
        <Text class="text-muted-foreground" style={{ fontSize: token("small-size") }}>{`${editor().count} 条规则`}</Text>
      </View>
      <View class="flex-1 min-h-0 flex-row">
        <ProtocolSidebar />
        <If when={editor().selected} else={<EmptyEditor />}><RuleForm receive={editor().receive} /></If>
      </View>
      <If when={editor().failure}>
        <View class="px-5 py-2" style={{ maxHeight: 76 }}>
          <Text class="text-destructive" style={{ fontSize: token("caption-size") }}>{editor().failure}</Text>
        </View>
      </If>
      <View class="shrink-0 px-5 flex-row items-center gap-3 border-t border-foreground/10" style={{ height: 54 }}>
        <ProtocolAction command="import" /><ProtocolAction command="export" />
        <View class="flex-1" /><ProtocolAction command="cancel" /><ProtocolAction command="save" />
      </View>
    </>}</If>
  </ProtocolEditorHost>;
}
