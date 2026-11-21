import { signal, Scroll, Text, token, View, windowSize, type JSX } from "nsh";
import { Button, Link, Switch } from "nsh/base";
import {
  TargetBaseInput, TargetCancel, TargetConnect, TargetDialogHost, TargetDisconnect, TargetFormHost,
  TargetIdentification, TargetProbePicker, TargetSpeedInput, TargetSpeedMenu, WorkbenchIcon,
  type TargetFormHostRef, type TargetFormState,
} from "chipdock/native";

function Section(props: { title: string; children: JSX.Element }) {
  return <View class="gap-2">
    <View class="h-[16px] px-[10px] justify-center"><Text class="font-bold" style={{ fontSize: token("body-size") }}>{props.title}</Text></View>
    {props.children}
  </View>;
}

function Group(props: { children: JSX.Element }) {
  return <View class="px-[10px] bg-group rounded-[10px]">{props.children}</View>;
}

function Row(props: { label: string; height: number; separator?: boolean; address?: boolean; baseLine?: boolean; children: JSX.Element }) {
  return <View class={`flex-row items-center gap-[6px] ${props.address ? (props.baseLine ? "border-b border-foreground/10" : "border-b border-background/0") : props.separator ? "border-b border-foreground/6" : ""}`}
    style={{ height: props.height }}>
    <Text class={props.address ? "font-mono" : ""} style={{ fontSize: token("body-size") }}>{props.label}</Text>
    {props.children}
  </View>;
}

function TargetForm() {
  let host: TargetFormHostRef;
  const state = signal<TargetFormState>({
    busy: false, connected: false, name: "", scanned: false, probeCount: 0,
    chip: "", wire: "", validSpeed: true, underReset: false, validBase: true, identified: false,
  });
  return <TargetFormHost ref={ref => { host = ref; }} class="w-[480px] gap-0" onChange={event => state.set(event.state)}>
    <View class="px-5 pt-5 gap-[2px]">
      <Text class="font-bold" style={{ fontSize: token("body-size") }}>目标配置</Text>
      <Text class={state().connected ? "text-success" : "text-muted-foreground"} style={{ fontSize: token("small-size") }}>
        {state().connected ? `已连接 ${state().name}` : "未连接"}
      </Text>
    </View>
    <Scroll style={{ maxHeight: Math.max(windowSize().height - 160, 320) }}>
      <View class="px-5 pt-6 gap-[30px]">
        <Section title="探针">
          <Group><Row label="调试探针" height={44}><View class="flex-1" /><TargetProbePicker />
            <Button disabled={state().busy} onPress={() => host.scan()} accessibilityLabel="扫描调试探针（⌘R）"
              class="w-[37.5px] h-[24px] rounded-[7px] items-center justify-center bg-button">
              <WorkbenchIcon name="refresh" class="size-[14px]" />
            </Button>
          </Row></Group>
          <If when={state().scanned}>
            <Text class={state().probeCount ? "text-muted-foreground" : "text-firmware-tint"} style={{ fontSize: token("small-size") }}>
              {state().probeCount ? `已发现 ${state().probeCount} 个调试探针。` : "未发现调试探针。请检查 USB 数据线；使用 Apple 芯片的 Mac 笔记本时，请先解锁并允许此配件连接。关闭其他调试工具后重新扫描。"}
            </Text>
            <If when={!state().probeCount}>
              <Link disabled={state().busy} href="https://support.apple.com/zh-cn/102282" class="text-primary">
                <Text style={{ fontSize: token("small-size") }}>USB 配件授权说明</Text>
              </Link>
            </If>
          </If>
        </Section>
        <Section title="目标芯片"><Group>
          <Row label="芯片" height={38.5} separator><View class="flex-1" />
            <Button disabled={state().busy} onPress={() => host.selectChip()} accessibilityLabel="选择芯片" class="h-[24px] flex-row items-center gap-1">
              <Text class="font-mono text-muted-foreground" style={{ fontSize: token("body-size") }}>{state().chip || "自动识别"}</Text>
              <WorkbenchIcon name="target-select" class="size-[10px] text-muted-foreground" />
            </Button>
          </Row>
          <Row label="协议" height={41} separator><View class="flex-1" />
            <View class="w-[112px] h-[24px] rounded-[6px] bg-control flex-row">
              <Each of={["swd", "jtag"]} key={wire => wire}>{wire =>
                <Button disabled={state().busy} selected={state().wire === wire()} onPress={() => host.setWire(wire())}
                  class="w-[56px] h-[24px] items-center justify-center rounded-[6px] selected:bg-primary selected:text-primary-foreground">
                  <Text style={{ fontSize: token("body-size") }}>{wire().toUpperCase()}</Text>
                </Button>}
              </Each>
            </View>
          </Row>
          <Row label="速度" height={37} separator><View class="flex-1" /><TargetSpeedInput />
            <Text class="text-muted-foreground" style={{ fontSize: token("body-size") }}>kHz</Text><TargetSpeedMenu />
          </Row>
          <If when={!state().validSpeed}>
            <View class="h-[36px] flex-row items-center gap-[12px] border-b border-foreground/10 text-destructive">
              <WorkbenchIcon name="warning" class="size-[13px]" /><Text style={{ fontSize: token("small-size") }}>速度须为 1–4294967295 kHz 的整数</Text>
            </View>
          </If>
          <Row label="复位下连接" height={37}><View class="flex-1" />
            <Switch checked={state().underReset} disabled={state().busy} onChange={({ checked }) => host.setUnderReset(checked)}
              accessibilityLabel="复位下连接" trackClass="w-[36px] h-[16px] p-[2px] rounded-full bg-foreground/18 checked:bg-primary"
              thumbClass="w-[20px] h-[12px] rounded-full bg-primary-foreground checked:translate-x-[12px]" />
          </Row>
        </Group></Section>
        <Section title="固件地址">
          <Group>
            <Row label="BIN 基地址" height={36} address baseLine={!state().validBase || state().inferred != null}><TargetBaseInput /></Row>
            <If when={!state().validBase} else={<View><If when={state().inferred != null}>
              <Row label="已识别地址" height={36}><View class="flex-1" />
                <Text class="text-muted-foreground" style={{ fontSize: token("body-size") }}>{state().inferred}</Text>
              </Row>
            </If></View>}>
              <View class="h-[36px] justify-center"><Text class="text-destructive" style={{ fontSize: token("body-size") }}>请输入有效的十六进制地址</Text></View>
            </If>
          </Group>
          <View class="px-[10px] mt-1"><Text class="text-muted-foreground" style={{ fontSize: token("small-size") }}>
            留空自动识别 ESP BIN；其他 BIN 可手动输入。ELF / HEX 使用文件内地址。
          </Text></View>
        </Section>
        <If when={state().identified}><TargetIdentification /></If>
      </View>
    </Scroll>
    <View class="px-6 pb-5 pt-[28px] flex-row items-center gap-2">
      <If when={state().connected}><TargetDisconnect /></If><View class="flex-1" /><TargetCancel /><TargetConnect />
    </View>
  </TargetFormHost>;
}

export function TargetDialog() {
  return <TargetDialogHost><TargetForm /></TargetDialogHost>;
}
