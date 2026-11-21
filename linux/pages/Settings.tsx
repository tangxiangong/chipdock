import { signal, Text, token, View, type JSX } from "nsh";
import { Link, Tab, Tabs } from "nsh/base";
import { AboutIcon, AutomaticUpdate, CheckForUpdates, FlashDefault, ThemePicker, WorkbenchIcon } from "chipdock/native";

function SettingsGroup(props: { title: string; children: JSX.Element }) {
  return <View style={{ gap: 10 }}>
    <Text role="header" class="font-semibold" style={{ paddingHorizontal: 10, fontSize: token("body-size") }}>{props.title}</Text>
    <View class="bg-group" style={{ paddingHorizontal: 10, borderRadius: 10 }}>{props.children}</View>
  </View>;
}

function General() {
  return <View class="p-5" style={{ gap: 31 }}>
    <SettingsGroup title="外观">
      <View class="flex-row items-center justify-between" style={{ height: 37 }}>
        <Text style={{ fontSize: token("body-size") }}>主题</Text><ThemePicker />
      </View>
    </SettingsGroup>
    <SettingsGroup title="新会话的烧录默认值"><FlashDefault verify={true} /><FlashDefault verify={false} /></SettingsGroup>
  </View>;
}

function ProjectLink(props: { label: string; href: string }) {
  return <Link href={props.href} accessibilityLabel={props.label}
    class="text-primary hover:opacity-80 focus-visible:outline-2 focus-visible:outline-ring">
    <Text style={{ fontSize: token("body-size") }}>{props.label}</Text>
  </Link>;
}

function About(props: { version: string }) {
  return <View class="flex-1 min-h-0 items-center justify-center">
    <View class="w-full px-5 items-center" style={{ gap: 14 }}>
      <View class="items-center justify-center" style={{ width: 76, height: 76 }}><AboutIcon /></View>
      <Text class="font-semibold" style={{ fontSize: 17, height: 20.5 }}>ChipDock</Text>
      <Text class="text-muted-foreground" style={{ fontSize: token("body-size") }}>{`版本 ${props.version}`}</Text>
      <Text class="text-muted-foreground" style={{ fontSize: 10 }}>Rust · NSH</Text>
      <View class="mt-1 flex-row items-center" style={{ gap: 20 }}>
        <ProjectLink label="源代码" href="https://github.com/tangxiangong/chipdock" />
        <ProjectLink label="发布记录" href="https://github.com/tangxiangong/chipdock/releases" /><CheckForUpdates />
      </View>
      <AutomaticUpdate combined={false} /><AutomaticUpdate combined={true} />
      <Text class="font-medium text-muted-foreground" style={{ fontSize: token("small-size") }}>MIT OR Apache-2.0</Text>
    </View>
  </View>;
}

const tabs = [{ key: "general", label: "通用", icon: "gear" }, { key: "about", label: "关于", icon: "info" }];
export function Settings(props: { version: string }) {
  const tab = signal("general");
  const toolbarColor = () => tab() === "about" ? token("settings-toolbar") : token("background");
  return <View class="flex-1 min-h-0 bg-background text-foreground">
    <View class="shrink-0 items-center justify-center" desktop={{ windowDrag: true }}
      style={{ height: 32, paddingHorizontal: 80, backgroundColor: toolbarColor() }}>
      <Text class="font-bold text-foreground/70" style={{ fontSize: token("body-size") }}>{tab() === "general" ? "通用" : "关于"}</Text>
    </View>
    <Tabs class="shrink-0 flex-row items-start justify-center gap-0 border-border"
      style={{ height: 56, backgroundColor: toolbarColor(), borderBottomWidth: tab() === "general" ? 0 : 1 }}>
      <Each of={tabs} key={entry => entry.key}>{entry =>
        <Tab selected={tab() === entry().key} onPress={() => tab.set(entry().key)}
          class="items-center justify-center gap-1 text-muted-foreground selected:bg-background/0 selected:text-primary selected:border selected:border-foreground/10 hover:bg-accent active:bg-muted focus-visible:outline-2 focus-visible:outline-ring"
          style={{ width: 56, height: 48, borderRadius: 8 }} selectedStyle={{ glassEffect: "regular" }}>
          <WorkbenchIcon name={entry().icon} style={{ width: 22, height: 22 }} />
          <Text style={{ fontSize: token("caption-size") }}>{entry().label}</Text>
        </Tab>}
      </Each>
    </Tabs>
    <If when={tab() === "general"} else={<About version={props.version} />}><General /></If>
  </View>;
}
