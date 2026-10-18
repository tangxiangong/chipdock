import SwiftUI

struct SaveProfileSheet: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var saving = false

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            VStack(alignment: .leading, spacing: 3) {
                Text("保存烧录配置").font(.headline)
                Text("保存探针、目标芯片、连接参数与烧录选项。").font(.callout).foregroundStyle(.secondary)
            }
            TextField("配置名称", text: $name).textFieldStyle(.roundedBorder)
            Grid(alignment: .leading, horizontalSpacing: 14, verticalSpacing: 6) {
                GridRow { Text("目标").foregroundStyle(.secondary); Text(store.targetName) }
                GridRow { Text("接口").foregroundStyle(.secondary); Text("\(store.wireProtocol.title) · \(store.speed) kHz") }
                GridRow { Text("擦除").foregroundStyle(.secondary); Text(store.eraseStrategy.title) }
            }
            .font(.callout)
            .padding(12)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.well, in: RoundedRectangle(cornerRadius: 8, style: .continuous))
            HStack {
                Spacer()
                Button("取消", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button("保存") {
                    saving = true
                    Task { await store.saveProfile(name: name); saving = false }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || saving)
            }
        }
        .padding(24)
        .frame(width: 400)
        .onAppear { name = store.effectiveChip.isEmpty ? "烧录配置" : store.effectiveChip }
    }
}
