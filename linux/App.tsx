import { effect, mount, signal, View } from "nsh";
import { startInterfaceUpdates } from "./ota/automatic";
import { WorkbenchHost, type WorkbenchHostRef } from "chipdock/native";
import { Firmware } from "./pages/Firmware";
import { Serial } from "./pages/Serial";
import { Settings } from "./pages/Settings";
import { WorkbenchToolbar } from "./components/WorkbenchToolbar";
import { ToolDialog } from "./components/ToolDialog";
import { Memory } from "./pages/Memory";
import { Logs } from "./pages/Logs";
import { Plot } from "./pages/Plot";
import { UpdateWindow } from "./pages/UpdateWindow";

function App() {
  let host: WorkbenchHostRef;
  const ready = signal(false);
  effect(() => {
    if (ready()) startInterfaceUpdates();
  });
  const layout = signal({ mode: "flash", sidebar: true, composer: true, modal: false, targetSetup: false, empty: true });
  const dialog = signal<{ tool: string | null; nativeSheet: boolean }>({ tool: null, nativeSheet: false });
  return (
    <WorkbenchHost ref={ref => { host = ref; }} class="flex-1 bg-background text-foreground"
      onStateChanged={state => { layout.set(state); ready.set(true); }}
      onToolChange={dialog.set} renderSettings={version => <Settings version={version} />}
      renderAuxiliary={kind => kind === "logs" ? <Logs /> : kind === "memory" ? <Memory /> : <Plot />}
      renderUpdate={surface => <UpdateWindow surface={surface} />}>
      <WorkbenchToolbar mode={layout().mode} modal={layout().modal} />
      <If when={layout().mode === "flash"} else={<Serial sidebar={layout().sidebar} composer={layout().composer} />}>
        <Firmware empty={layout().empty} targetSetup={layout().targetSetup} />
      </If>
      <If when={layout().modal}><View class="absolute inset-0 bg-modal-background-veil" style={{ pointerEvents: "none" }} /></If>
      <ToolDialog tool={dialog().tool} nativeSheet={dialog().nativeSheet} close={() => host.closeTool()} />
    </WorkbenchHost>
  );
}

mount(App);
