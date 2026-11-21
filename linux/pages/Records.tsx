import { signal, Text, token, View } from "nsh";
import { Input, Tab, Tabs } from "nsh/base";
import { type RecordDirection, SerialRecordAction, SerialRecordSheet, SerialRecordTable, WorkbenchIcon } from "chipdock/native";

const directions = [
  { key: "all", label: "全部" },
  { key: "receive", label: "接收" },
  { key: "send", label: "发送" },
] as const;

function DirectionPicker(props: { direction: RecordDirection; select: (key: RecordDirection) => void }) {
  return <Tabs class="flex-row items-center p-0 gap-0 rounded-(--radius) bg-tools-tab" style={{ width: 150, height: 24 }}>
    <Each of={directions} key={direction => direction.key}>{(direction, index) =>
      <Tab selected={props.direction === direction().key} onPress={() => props.select(direction().key)}
        class="relative p-0 items-center justify-center rounded-(--radius) selected:bg-primary selected:text-primary-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring"
        style={{ width: 50, height: 24 }}>
        <If when={index() > 0 && props.direction !== direction().key && props.direction !== directions[index() - 1]?.key}>
          <View class="absolute left-0 bg-tools-tab-divider" style={{ top: 5, bottom: 5, width: 1 }} />
        </If>
        <Text style={{ fontSize: token("body-size") }}>{direction().label}</Text>
      </Tab>}
    </Each>
  </Tabs>;
}

export function Records() {
  const query = signal("");
  const direction = signal<RecordDirection>("all");
  const count = signal(0);
  const focused = signal(false);
  return <SerialRecordSheet query={query()} direction={direction()} onCountChange={event => count.set(event.count)}
    class="p-5 gap-3" style={{ width: 920, height: 480 }}>
    <View class="flex-row items-center gap-2">
      <View class="flex-row items-center flex-1 rounded-full bg-record-search-well"
        style={{ gap: 3, height: 24, paddingHorizontal: focused() ? 5.5 : 5, borderWidth: focused() ? 0.5 : 1,
          borderColor: focused() ? token("record-search-focus-border") : token("input"),
          outlineWidth: focused() ? 3 : 0, outlineColor: token("record-search-ring") }}>
        <WorkbenchIcon name="search" class="text-muted-foreground" style={{ width: 15, height: 15 }} />
        <Input accessibilityLabel="筛选文本、HEX 或来源" placeholder="筛选文本、HEX 或来源" autoFocus
          onFocus={() => focused.set(true)} onBlur={() => focused.set(false)}
          value={query()} onChange={({ value }) => query.set(value)} class="flex-1 min-w-0"
          style={{ fontSize: token("body-size"), caretColor: token("primary"), placeholderColor: token("muted-foreground") }} />
      </View>
      <DirectionPicker direction={direction()} select={value => direction.set(value)} />
      <Text class="text-muted-foreground tabular-nums" style={{ fontSize: token("body-size") }}>{`${count()} 条`}</Text>
    </View>
    <SerialRecordTable />
    <View class="flex-row items-center gap-2">
      <SerialRecordAction command="copy" /><SerialRecordAction command="locate" />
      <View class="flex-1" /><SerialRecordAction command="close" />
    </View>
  </SerialRecordSheet>;
}
