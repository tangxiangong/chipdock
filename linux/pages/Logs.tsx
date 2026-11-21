import { platform, signal, Text, token, View } from "nsh";
import { LogActions, LogEntries } from "chipdock/native";

export function Logs() {
  const counts = signal({ visible: 0, errors: 0 });
  return (
    <View class="flex-1 min-h-0 bg-logs-background">
      <View desktop={{ windowDrag: true }} class="flex-row items-center gap-3"
        style={{ height: 52, paddingLeft: platform.os === "macos" ? 108 : 16, paddingRight: 9 }}>
        <View class="flex-1 min-w-0">
          <Text class="font-bold text-logs-title" style={{ fontSize: token("body-size") }}>活动日志</Text>
          <Text class="text-logs-subtitle" style={{ fontSize: 11 }}>
            {`${counts().visible} 条日志 · ${counts().errors} 条错误`}
          </Text>
        </View>
        <LogActions />
      </View>
      <LogEntries onSummary={counts.set} />
    </View>
  );
}
