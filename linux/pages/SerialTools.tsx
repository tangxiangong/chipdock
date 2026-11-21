import { Scroll, signal, Text, token, View, type JSX } from "nsh";
import { Tab, Tabs } from "nsh/base";
import { SerialJobStatus, SerialToolSession, ToolCommandButton, ToolDone, ToolOutcome, ToolParameter } from "chipdock/native";

const diagnostics = [
  { id: "checksum", title: "发送校验", height: 456 },
  { id: "framing", title: "接收分帧", height: 411 },
  { id: "modbus", title: "Modbus RTU", height: 411 },
  { id: "capture", title: "持续采集", height: 411 },
];
const workflows = [
  { id: "packets", title: "常用包库", height: 505 },
  { id: "file", title: "发送文件", height: 392 },
  { id: "macro", title: "宏命令", height: 451 },
  { id: "replies", title: "自动应答", height: 414 },
];

function ToolTabs(props: { pages: typeof workflows; selected: string; width: number; select: (id: string) => void }) {
  return <Tabs class="self-start p-0 flex-row bg-tools-tab" style={{ height: 24, borderRadius: 7 }}
    indicatorClass="inset-y-0 bg-primary transition-transform" indicatorStyle={{ borderRadius: 6, transitionDuration: 240 }}>
    <Each of={props.pages} key={entry => entry.id}>{(entry, index) =>
      <Tab selected={entry().id === props.selected} onPress={() => props.select(entry().id)}
        class="relative items-center justify-center selected:text-primary-foreground"
        style={{ width: props.width, height: 24, borderRadius: 6 }}>
        <If when={index() > 0 && props.selected !== entry().id && props.selected !== props.pages[index() - 1].id}>
          <View class="absolute left-0 bg-tools-tab-divider" style={{ top: 5, width: 1, height: 14 }} />
        </If>
        <Text style={{ fontSize: token("body-size") }}>{entry().title}</Text>
      </Tab>}
    </Each>
  </Tabs>;
}

function Note(props: { message: string }) {
  return <View class="py-3 border-b border-foreground/[0.055]">
    <Text class="text-muted-foreground" style={{ fontSize: token("small-size"), height: 12, lineHeight: 12 }}>{props.message}</Text>
  </View>;
}

function EditorRow(props: { children: JSX.Element }) {
  return <View class="border-b border-foreground/[0.055]" style={{ paddingVertical: 10 }}>{props.children}</View>;
}

function Actions(props: { children: JSX.Element; single?: boolean }) {
  return <View class={props.single ? "items-start" : "flex-row items-center gap-2 flex-wrap"}
    style={{ paddingVertical: 10 }}>{props.children}</View>;
}

function Form(props: { page: string; failed: boolean }) {
  switch (props.page) {
    case "packets": return <>
      <Note message="每个包可设置 name、group、note、order、enabled、text、format。format 使用 hex 或 utf-8。" />
      <EditorRow><ToolParameter field="packets" label="常用包库" /></EditorRow>
      <ToolParameter field="selected" label="按发送顺序填写包名（逗号分隔，留空发送全部启用包）" />
      <ToolParameter field="interval" label="包间隔 (ms)" />
      <Actions>
        <ToolCommandButton command="packetsExample" label="示例" />
        <ToolCommandButton command="packetsImport" label="导入 JSON…" />
        <ToolCommandButton command="packetsSave" label="保存" />
        <ToolCommandButton command="packetsExport" label="导出 JSON…" />
        <ToolCommandButton command="batch" label="批量发送" />
      </Actions>
    </>;
    case "file": return <>
      <ToolParameter field="filePath" label="文件路径" />
      <ToolParameter field="chunk" label="块大小 (字节)" />
      <ToolParameter field="interval" label="块间隔 (ms)" />
      <Actions single><ToolCommandButton command="file" label="发送文件" /></Actions>
    </>;
    case "macro": return <>
      <Note message="步骤可包含 send、delayMs、wait 和 timeoutMs。匹配方式为 text、hex 或 regex。" />
      <EditorRow><ToolParameter field="macro" label="宏命令" /></EditorRow>
      <ToolParameter field="cycles" label="循环次数" />
      <Actions>
        <ToolCommandButton command="macro" label="运行宏" />
        <ToolCommandButton command="macroExport" label="导出结果 CSV…" />
      </Actions>
    </>;
    case "replies": return <>
      <Note message="按顺序匹配，支持 text、hex 或 regex。delayMs 用于延时，enabled 控制是否启用。" />
      <EditorRow><ToolParameter field="replies" label="自动应答" /></EditorRow>
      <Actions>
        <ToolCommandButton command="repliesExample" label="示例" />
        <ToolCommandButton command="replies" label="应用应答规则" />
      </Actions>
    </>;
    case "framing": return <>
      <ToolParameter field="framingMode" label="分帧方式" />
      <ToolParameter field="framingValue" label="分帧参数" />
      <Actions single><ToolCommandButton command="framing" label="应用" /></Actions>
    </>;
    case "modbus": return <>
      <ToolParameter field="station" label="站号" />
      <ToolParameter field="function" label="功能码" />
      <ToolParameter field="register" label="起始地址" />
      <ToolParameter field="modbusValue" label="数据" />
      <ToolParameter field="timeout" label="超时 (ms)" />
      <Actions>
        <ToolCommandButton command="modbus" label="生成请求" />
        <ToolCommandButton command="modbusSend" label="发送并等待响应" />
      </Actions>
    </>;
    case "capture": return <>
      <ToolParameter field="rawCapture" label="仅捕获原始 RX" />
      <ToolParameter field="captureFormat" label="日志格式" />
      <ToolParameter field="capturePath" label="新文件路径" />
      <ToolParameter field="rollMib" label="按大小滚动 (MiB)" />
      <Actions single><ToolCommandButton command="capture" label="开始采集" /></Actions>
      <If when={props.failed}><View style={{ paddingVertical: 10 }}><ToolOutcome failed /></View></If>
    </>;
    default: return <>
      <ToolParameter field="checksumInput" label="试算内容" />
      <ToolParameter field="format" label="编码" />
      <ToolParameter field="checksum" label="算法" />
      <ToolParameter field="prefix" label="跳过开头字节" />
      <ToolParameter field="suffix" label="跳过结尾字节" />
      <ToolParameter field="hexEnding" label="HEX 发送时允许附加行尾" />
      <Actions single><ToolCommandButton command="checksum" label="计算" /></Actions>
    </>;
  }
}

export function SerialTools(props: { initial: string }) {
  const page = signal(props.initial);
  const workflow = workflows.some(entry => entry.id === props.initial);
  const pages = workflow ? workflows : diagnostics;
  const empty = { result: false, error: false, labelHeight: 16, jobHeight: 0 };
  const reported = signal({ page: props.initial, ...empty });
  const feedback = () => reported().page === page() ? reported() : empty;
  function height() {
    const base = pages.find(entry => entry.id === page())?.height ?? 411;
    const status = feedback();
    if (workflow) return base + status.jobHeight + (status.result || status.error ? 12 + status.labelHeight : 0);
    if (status.result) return 562;
    if (status.error && page() !== "capture") return 411 + 14 + status.labelHeight;
    return base;
  }
  return <SerialToolSession page={page()} onFeedbackChange={reported.set} class="p-5"
    style={{ width: workflow ? 660 : 540, height: height(), gap: workflow ? 12 : 14 }}>
    <ToolTabs pages={pages} selected={page()} select={page.set} width={workflow ? 76 : 103} />
    <If when={!workflow}><View class="bg-foreground/10" style={{ height: 1 }} /></If>
    <Each of={[page()]} key={id => id}>{current =>
      <Scroll class="flex-1 min-h-0"><View class="p-5">
        <View class="bg-group" style={{ paddingHorizontal: 10, borderRadius: 10 }}>
          <Form page={current()} failed={feedback().error} />
        </View>
      </View></Scroll>}
    </Each>
    <If when={workflow}><SerialJobStatus /></If>
    <If when={feedback().result}><ToolOutcome diagnostic={!workflow} /></If>
    <If when={feedback().error && page() !== "capture"}><ToolOutcome failed /></If>
    <View class="flex-row justify-end"><ToolDone /></View>
  </SerialToolSession>;
}
