import { Scroll, signal, Text, token, View, type JSX } from "nsh";
import { Button, Switch } from "nsh/base";
import {
  SerialBaud, SerialChoice, SerialConnectButton, SerialPortPicker, WorkbenchIcon,
  type SerialFormState, type SerialPageHostRef, type SerialOutputLine,
} from "chipdock/native";

function Section(props: { title: string; children: JSX.Element }) {
  return (
    <View style={{ gap: 10 }}>
      <Text class="font-semibold" style={{ fontSize: token("body-size"), paddingHorizontal: 30 }}>{props.title}</Text>
      {props.children}
    </View>
  );
}

function Group(props: { children: JSX.Element }) {
  return <View class="mx-5 bg-group" style={{ borderRadius: 10, paddingHorizontal: 10 }}>{props.children}</View>;
}

type Parameter = "bits" | "stop" | "parity" | "flow";
function ParameterRow(props: { label: string; field: Parameter; last?: boolean }) {
  const measured = signal(280);
  const stacked = () => measured() < 240;
  return (
    <View class="border-foreground/10" onLayout={({ bounds }) => {
      if (Math.abs(bounds.size.width - measured()) > 0.25) measured.set(bounds.size.width);
    }} style={{
      minHeight: stacked() ? 65 : 40, borderBottomWidth: props.last ? 0 : 1,
      flexDirection: stacked() ? "column" : "row", alignItems: stacked() ? "flex-start" : "center",
      paddingTop: stacked() ? 11 : 0, gap: stacked() ? 8 : 20,
    }}>
      <Text class="shrink-0" style={{ width: 39, height: 16, fontSize: token("body-size") }}>{props.label}</Text>
      <View class={stacked() ? "min-w-0 w-full" : "min-w-0 flex-1"} style={{ height: stacked() ? 20 : "auto" }}>
        <SerialChoice field={props.field} />
      </View>
    </View>
  );
}

function FormRow(props: { label: string; last?: boolean; children: JSX.Element }) {
  return (
    <View class="flex-row items-center gap-3 border-foreground/10" style={{ minHeight: 40, borderBottomWidth: props.last ? 0 : 1 }}>
      <Text style={{ fontSize: token("body-size") }}>{props.label}</Text>
      <View class="flex-1" />{props.children}
    </View>
  );
}

function Option(props: { label: string; active: boolean; checked: boolean; disabled: boolean; onChange: (checked: boolean) => void; last?: boolean }) {
  return (
    <FormRow label={props.label} last={props.last}>
      <Switch checked={props.checked} disabled={props.disabled} accessibilityLabel={props.label}
        onChange={({ checked }) => props.onChange(checked)}
        trackClass={props.active
          ? "w-[42px] h-[22px] p-[3px] rounded-full bg-foreground/18 checked:bg-primary transition-colors duration-120"
          : "w-[42px] h-[22px] p-[3px] rounded-full bg-foreground/18 checked:bg-selection-inactive transition-colors duration-120"}
        thumbClass="w-[26px] h-[16px] rounded-full bg-primary-foreground checked:translate-x-[10px] transition-transform duration-180" />
    </FormRow>
  );
}

function OutputLine(props: { line: SerialOutputLine; value: boolean; disabled: boolean; host: () => SerialPageHostRef }) {
  return (
    <FormRow label={props.line === "rts" ? "RTS" : "DTR"}>
      <View class="flex-row bg-secondary" style={{ padding: 2, borderRadius: 7 }}>
        <Each of={[false, true]} key={value => value ? "set" : "clear"}>{level =>
          <Button selected={props.value === level()} disabled={props.disabled} onPress={() => props.host().setLine(props.line, level())}
            class="selected:bg-foreground/15" style={{ height: 20, paddingHorizontal: 10, borderRadius: 5 }}>
            <Text style={{ fontSize: token("caption-size") }}>{level() ? "置位" : "清除"}</Text>
          </Button>}
        </Each>
      </View>
    </FormRow>
  );
}

export function SerialConnection(props: { state: SerialFormState; host: () => SerialPageHostRef }) {
  return (
    <View class="flex-1 min-h-0">
      <Scroll class="flex-1">
        <View class="py-5" style={{ gap: 30 }}>
          <Section title="设备">
            <Group>
              <View class="flex-row items-center gap-1" style={{ minHeight: 40 }}>
                <View class="flex-1 min-w-0"><SerialPortPicker /></View>
                <Button accessibilityLabel="刷新串口（⌘R）" disabled={props.state.portsBusy}
                  onPress={() => props.host().refreshPorts()} hitSlop={5}
                  class="p-0 items-center justify-center text-foreground/70 focus-visible:outline-2 focus-visible:outline-ring"
                  style={{ width: 11.5, height: 22 }}>
                  <WorkbenchIcon name="refresh" style={{ width: 14, height: 14 }} />
                </Button>
              </View>
            </Group>
          </Section>
          <Section title="参数">
            <Group>
              <View class="flex-row items-center border-b border-foreground/10" style={{ minHeight: 36, gap: 20 }}>
                <Text class="shrink-0" style={{ width: 39, fontSize: token("body-size") }}>波特率</Text><SerialBaud />
              </View>
              <ParameterRow field="bits" label="数据位" /><ParameterRow field="stop" label="停止位" />
              <ParameterRow field="parity" label="校验位" /><ParameterRow field="flow" label="流控制" last />
            </Group>
            <If when={!props.state.baudValid}>
              <Text class="text-destructive" style={{ paddingHorizontal: 30, fontSize: token("small-size") }}>波特率须为正整数</Text>
            </If>
          </Section>
          <Section title="选项">
            <Group>
              <Option label="启动时连接" active={props.state.activeWindow} checked={props.state.autoConnect}
                disabled={props.state.autoConnectLocked} onChange={v => props.host().setAutoConnect(v)} />
              <Option label="掉线后重连" active={props.state.activeWindow} checked={props.state.reconnect}
                disabled={props.state.optionsLocked} onChange={v => props.host().setReconnect(v)} last />
            </Group>
          </Section>
          <If when={props.state.controlLines}>
            <Section title="控制线">
              <Group>
                <OutputLine line="rts" value={props.state.rts} disabled={props.state.rtsLocked} host={props.host} />
                <OutputLine line="dtr" value={props.state.dtr} disabled={props.state.dtrLocked} host={props.host} />
              </Group>
            </Section>
          </If>
        </View>
      </Scroll>
      <View class="p-3 border-t border-foreground/10"><SerialConnectButton /></View>
    </View>
  );
}
