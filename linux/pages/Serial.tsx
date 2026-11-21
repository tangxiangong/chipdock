import { signal, View, windowSize } from "nsh";
import { ResizablePanel, ResizablePanelGroup } from "nsh/base";
import { SerialPageHost, SerialError, SerialSessions, SerialStatus, SerialTerminal, type SerialFormState, type SerialPageHostRef } from "chipdock/native";
import { SerialConnection } from "./SerialConnection";
import { SerialComposer } from "./SerialComposer";

export function Serial(props: { sidebar: boolean; composer: boolean }) {
  let host: SerialPageHostRef;
  const state = signal<SerialFormState>({
    commandMode: false, activeWindow: true, baudValid: true, portsBusy: true, optionsLocked: true,
    autoConnectLocked: true, autoConnect: false, reconnect: false, controlLines: false, rts: false, dtr: false,
    rtsLocked: true, dtrLocked: true, sendCharacters: 0, sendJob: false,
  });
  return (
    <SerialPageHost ref={ref => { host = ref; }} onFormChange={event => state.set(event.state)}
      class="flex-1" style={{ minHeight: 633, marginTop: windowSize().width < 997 ? -16.5 : 0 }}>
      <SerialSessions />
      <ResizablePanelGroup class="flex-1 min-h-0" handleClass="bg-border" handleAccessibilityLabel="调整连接区宽度">
        <ResizablePanel size={340} minSize={220} maxSize={340} visible={props.sidebar}><SerialConnection state={state()} host={() => host} /></ResizablePanel>
        <ResizablePanel size={820} minSize={440}>
          <View class="flex-1 min-h-0" style={{ paddingLeft: props.sidebar ? 1 : 0 }}>
            <SerialError />
            <ResizablePanelGroup axis="vertical" class="flex-1 min-h-0" handleClass="bg-border"
              handleStyle={{ transform: [{ translateY: -1 }] }} handleAccessibilityLabel="调整终端与发送区高度">
              <ResizablePanel size={348} minSize={180}>
                <View class="flex-1 min-h-0" style={{ paddingBottom: props.composer ? 1 : 0 }}><SerialTerminal /></View>
              </ResizablePanel>
              <ResizablePanel size={320} minSize={164} maxSize={320} visible={props.composer}><SerialComposer state={state()} host={() => host} /></ResizablePanel>
            </ResizablePanelGroup>
          </View>
        </ResizablePanel>
      </ResizablePanelGroup>
      <SerialStatus />
    </SerialPageHost>
  );
}
