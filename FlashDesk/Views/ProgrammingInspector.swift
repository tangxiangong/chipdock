import SwiftUI

struct ProgrammingInspector: View {
    @Bindable var store: WorkbenchStore

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Label("烧录设置", systemImage: "slider.horizontal.3")
                .font(.headline).padding(18)
            Divider()
            Form {
                if store.isBin {
                    Section("二进制") {
                        TextField("基地址", text: $store.binBaseAddress)
                            .font(.system(.body, design: .monospaced))
                        if parseAddress(store.binBaseAddress) == nil {
                            Text("请输入有效基地址").font(.caption).foregroundStyle(.red)
                        }
                    }
                }
                Section("擦除策略") {
                    Picker("擦除策略", selection: $store.eraseStrategy) {
                        ForEach(EraseStrategy.allCases) { Text($0.title).tag($0) }
                    }
                    .pickerStyle(.radioGroup).labelsHidden()
                }
                Section("写入行为") {
                    Toggle("写入后校验", isOn: $store.verify)
                    Toggle("完成后复位", isOn: $store.resetAfter)
                    Toggle("仅预检查", isOn: $store.dryRun)
                }
            }
            .formStyle(.grouped)
            .disabled(store.isBusy)
            Divider()
            Button("保存配置…", systemImage: "square.and.arrow.down") { store.showSaveProfile = true }
                .buttonStyle(.borderless).padding(16).disabled(store.isBusy)
        }
    }
}
