import { Text, token, View } from "nsh";
import { Radio, RadioGroup } from "nsh/base";
import {
  SerialChecksum, SerialChoice, SerialCommandEditor, SerialInterval, SerialRepeat, SerialSendButton,
  SerialSendHistory, SerialSendProgress, SerialTextDraft, SerialToolsMenu,
  type SerialFormState, type SerialPageHostRef,
} from "chipdock/native";

function Caption(props: { value: string }) {
  return <Text class="text-muted-foreground" style={{ fontSize: token("caption-size") }}>{props.value}</Text>;
}

export function SerialComposer(props: { state: SerialFormState; host: () => SerialPageHostRef }) {
  return (
    <View class="flex-1 min-h-0">
      <View class="px-3 flex-row items-center border-b border-foreground/10" style={{ height: 37, gap: 10 }}>
        <RadioGroup class="flex-row px-0 bg-serial-control" style={{ borderRadius: 6 }}>
          <Each of={[false, true]} key={value => value ? "command" : "text"}>{mode =>
            <Radio checked={props.state.commandMode === mode()} onChange={({ checked }) => {
              if (checked) props.host().setCommandMode(mode());
            }}
              class="p-0 items-center justify-center hover:bg-accent active:bg-muted focus-visible:outline-2 focus-visible:outline-ring"
              classList={{
                "checked:bg-primary checked:text-primary-foreground": props.state.activeWindow,
                "checked:bg-composer-selection-inactive checked:text-foreground": !props.state.activeWindow,
              }}
              style={{ width: 42, height: 20, borderRadius: 5 }}>
              <Text style={{ fontSize: token("caption-size") }}>{mode() ? "命令" : "文本"}</Text>
            </Radio>}
          </Each>
        </RadioGroup>
        <If when={!props.state.commandMode}>
          <View class="flex-row items-center gap-2">
            <View style={{ width: 81.5 }}><SerialChoice field="format" /></View>
            <View style={{ width: 72.5 }}><SerialChoice field="ending" /></View>
            <SerialChecksum />
          </View>
        </If>
        <View class="flex-1 min-w-2" /><SerialToolsMenu />
      </View>
      <If when={props.state.commandMode} else={<SerialTextDraft />}><SerialCommandEditor /></If>
      <View class="px-3 py-2 flex-row items-center border-t border-foreground/10" style={{ minHeight: 40, gap: 10 }}>
        <If when={!props.state.commandMode}>
          <View class="flex-row items-center" style={{ gap: 10 }}>
            <SerialSendHistory /><View class="mx-1 bg-foreground/10" style={{ width: 1, height: 14 }} />
            <Caption value="每" /><SerialInterval /><Caption value="ms" /><SerialRepeat />
          </View>
        </If>
        <If when={props.state.sendJob}><SerialSendProgress /></If>
        <View class="flex-1 min-w-1" />
        <If when={!props.state.commandMode && props.state.sendCharacters > 0}><Caption value={`${props.state.sendCharacters} 字符`} /></If>
        <SerialSendButton />
      </View>
    </View>
  );
}
