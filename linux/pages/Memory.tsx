import { signal, Text, View } from "nsh";
import { MemoryBytes, MemoryChipMenu, MemoryExport, MemoryAddress, MemoryLength, ReadMemory, WorkbenchIcon, MemoryRegions } from "chipdock/native";

export function Memory() {
  const invalid = signal(false);
  return (
    <View class="flex-1 min-h-0">
      <View class="px-4 flex-row items-center" style={{ paddingTop: 14, paddingBottom: 10, gap: 10 }}>
        <Text style={{ fontSize: 15 }}>内存</Text>
        <View class="flex-1" />
        <MemoryExport />
        <MemoryChipMenu />
      </View>
      <View class="px-4 pb-3 flex-row items-center gap-2">
        <MemoryAddress onValidityChange={event => invalid.set(event.invalid)} />
        <If when={invalid()}><WorkbenchIcon name="warning" class="text-destructive" accessibilityLabel="地址无效" /></If>
        <View style={{ width: 95.5 }}><MemoryLength /></View>
        <View style={{ width: 73 }}><ReadMemory /></View>
      </View>
      <MemoryRegions />
      <View class="bg-foreground/10" style={{ height: 1 }} />
      <MemoryBytes />
    </View>
  );
}
