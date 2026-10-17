import SwiftUI

struct SaveProfileSheet: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.dismiss) private var dismiss
    @State private var name = ""
    @State private var saving = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            Text("保存烧录配置").font(.title2.weight(.semibold))
            TextField("配置名称", text: $name).textFieldStyle(.roundedBorder)
            LabeledContent("目标", value: store.targetName).foregroundStyle(.secondary)
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
        .frame(width: 380)
        .onAppear { name = store.effectiveChip.isEmpty ? "烧录配置" : store.effectiveChip }
    }
}
