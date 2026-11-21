import { Text, View, token } from "nsh";
import {
  DownloadProgress, ReleaseNotes, UpdateButton, UpdateIcon, UpdateTitle,
  type UpdatePresentation,
} from "chipdock/native";

function IconSlot() {
  return <View class="items-center justify-center" style={{ width: 64, height: 64 }}>
    <UpdateIcon />
  </View>;
}

function ProgressPage(props: { phase: UpdatePresentation["phase"] }) {
  const heading = () => {
    if (props.phase === "downloading") return "正在下载更新…";
    if (props.phase === "installing") return "正在安装更新…";
    return "正在检查更新…";
  };
  return <View class="flex-1 min-h-0">
    <UpdateTitle class="px-5 justify-center" style={{ height: 32 }}>
      <Text class="font-semibold" style={{ fontSize: token("body-size") }}>软件更新</Text>
    </UpdateTitle>
    <View class="flex-1 px-5 flex-row gap-2" style={{ paddingTop: 4, paddingBottom: 20 }}>
      <IconSlot />
      <View class="flex-1 min-w-0" style={{ paddingTop: 10, gap: 14 }}>
        <Text class="font-bold" style={{ fontSize: token("body-size") }}>{heading()}</Text>
        <DownloadProgress />
        <View class="flex-row justify-end"><UpdateButton command="cancel" label="取消" /></View>
      </View>
    </View>
  </View>;
}

function AvailablePage(props: { surface: UpdatePresentation }) {
  return <View class="flex-1 min-h-0">
    <View desktop={{ windowDrag: true }} style={{ height: 32 }} />
    <View class="flex-1 min-h-0 px-5 pb-5 gap-3">
      <View class="flex-row gap-2" style={{ height: 64 }}>
        <IconSlot />
        <View class="flex-1 min-w-0" style={{ paddingTop: 13, gap: 8 }}>
          <Text class="font-semibold" style={{ fontSize: token("body-size") }}>新版本的ChipDock已经发布</Text>
          <Text style={{ fontSize: token("caption-size") }}>
            {`ChipDock ${props.surface.version}可供下载，您现在的版本是${props.surface.currentVersion}。要现在下载吗？`}
          </Text>
        </View>
      </View>
      <ReleaseNotes />
      <View class="flex-row items-center" style={{ height: 28 }}>
        <UpdateButton command="skip" label="跳过这个版本" />
        <View class="flex-1" />
        <UpdateButton command="install" label="安装更新" />
      </View>
    </View>
  </View>;
}

export function UpdateWindow(props: { surface: UpdatePresentation }) {
  return <If when={props.surface.phase === "available"} else={<ProgressPage phase={props.surface.phase} />}>
    <AvailablePage surface={props.surface} />
  </If>;
}
