import { View, token } from "nsh";
import { Dialog } from "nsh/base";
import { ProfileTools } from "../pages/ProfileTools";
import { ProtocolEditor } from "../pages/Protocol";
import { ColorRules } from "../pages/Colors";
import { Records } from "../pages/Records";
import { SerialTools } from "../pages/SerialTools";

function ToolContent(props: { tool: string }) {
  switch (props.tool) {
    case "protocol": return <ProtocolEditor />;
    case "colors": return <ColorRules />;
    case "records": return <Records />;
    case "profile": return <ProfileTools />;
    default: return <SerialTools initial={props.tool} />;
  }
}

export function ToolDialog(props: { tool: string | null; nativeSheet: boolean; close: () => void }) {
  return <Each of={props.tool ? [props.tool] : []} key={tool => tool}>{tool =>
    <Dialog onCancel={props.close} class="flex-1 items-center justify-center text-foreground text-(length:--body-size) leading-[1.2]"
      style={{ backgroundColor: props.nativeSheet ? "transparent" : token("scrim") }}>
      <View class="bg-background overflow-hidden" style={{ borderRadius: 16 }}><ToolContent tool={tool()} /></View>
    </Dialog>}
  </Each>;
}
