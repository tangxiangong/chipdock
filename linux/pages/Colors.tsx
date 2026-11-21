import { signal, Text, token, View } from "nsh";
import {
  AddColorRule, ColorPresets, ColorRuleAction, ColorRulesHost, ColorRuleTable,
  TransmitColorPicker, WorkbenchIcon, type ColorRulesHostProps,
} from "chipdock/native";

type ColorState = Parameters<NonNullable<ColorRulesHostProps["onState"]>>[0];

function EmptyColors() {
  return <View class="absolute inset-0 items-center justify-center">
    <View class="items-center" style={{ height: 121 }}>
      <WorkbenchIcon name="palette" class="text-empty-symbol" style={{ width: 44, height: 44, marginBottom: 18.25 }} />
      <Text class="font-bold text-empty-foreground" style={{ fontSize: 15, lineHeight: 19 }}>没有着色规则</Text>
      <Text class="text-empty-foreground" style={{ marginTop: 6, fontSize: 15, lineHeight: 19 }}>
        接收数据匹配关键字时按指定颜色显示。
      </Text>
    </View>
  </View>;
}

function ColorForm(props: { state: ColorState }) {
  return <>
    <View class="flex-row items-center gap-2">
      <ColorPresets /><AddColorRule /><View class="flex-1" />
      <Text style={{ fontSize: token("body-size") }}>TX 颜色</Text><TransmitColorPicker />
    </View>
    <ColorRuleTable><If when={props.state.empty}><EmptyColors /></If></ColorRuleTable>
    <If when={props.state.failure}>
      <View class="flex-row items-start gap-2">
        <WorkbenchIcon name="warning-fill" class="text-destructive" />
        <Text class="text-destructive" style={{ fontSize: token("body-size") }}>{props.state.failure}</Text>
      </View>
    </If>
    <View class="flex-row items-center justify-end gap-2"><ColorRuleAction command="cancel" /><ColorRuleAction command="save" /></View>
  </>;
}

export function ColorRules() {
  const state = signal<ColorState | null>(null);
  return <ColorRulesHost onState={state.set} class="p-5 gap-3" style={{ width: 620, height: 420 }}>
    <If when={state()}>{draft =>
      <If when={draft().loadError} else={<ColorForm state={draft()} />}>{error => <>
        <Text class="text-destructive" style={{ fontSize: token("body-size") }}>{`着色规则无效：${error()}`}</Text>
        <View class="flex-1" /><ColorRuleAction command="done" />
      </>}</If>
    }</If>
  </ColorRulesHost>;
}
