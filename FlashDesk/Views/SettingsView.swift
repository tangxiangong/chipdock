import AppKit
import SwiftUI

struct SettingsView: View {
    var coreVersion: String
    @Bindable var updater: UpdateService
    @AppStorage("appearance") private var appearance = Appearance.system
    @AppStorage("defaultVerify") private var defaultVerify = true
    @AppStorage("defaultReset") private var defaultReset = true

    var body: some View {
        TabView {
            Form {
                Section("外观") {
                    Picker("主题", selection: $appearance) {
                        ForEach(Appearance.allCases) { Text($0.title).tag($0) }
                    }
                }
                Section("新会话的烧录默认值") {
                    Toggle("写入后校验", isOn: $defaultVerify)
                    Toggle("完成后复位", isOn: $defaultReset)
                }
            }
            .formStyle(.grouped)
            .tabItem { Label("通用", systemImage: "gearshape") }

            VStack(spacing: 14) {
                Image(nsImage: NSImage(named: NSImage.applicationIconName) ?? NSImage())
                    .resizable().frame(width: 76, height: 76)
                Text("FlashDesk").font(.title2.weight(.semibold))
                Text("版本 \(coreVersion)").foregroundStyle(.secondary)
                Text("SwiftUI · Rust · UniFFI").font(.caption).foregroundStyle(.secondary)
                HStack(spacing: 20) {
                    Link("源代码", destination: URL(string: "https://github.com/tangxiangong/flashdesk")!)
                    Link("发布记录", destination: URL(string: "https://github.com/tangxiangong/flashdesk/releases")!)
                    Button("检查更新…", action: updater.checkForUpdates)
                        .disabled(!updater.canCheckForUpdates)
                }
                .padding(.top, 4)
                Toggle("自动检查更新", isOn: $updater.automaticallyChecksForUpdates)
                    .toggleStyle(.checkbox)
                    .disabled(updater.unavailableReason != nil)
                Toggle("自动下载并安装更新", isOn: $updater.automaticallyDownloadsUpdates)
                    .toggleStyle(.checkbox)
                    .disabled(updater.unavailableReason != nil)
                if let reason = updater.unavailableReason {
                    Text(reason).font(.caption).foregroundStyle(.secondary)
                }
                Text("MIT OR Apache-2.0").font(.caption2).foregroundStyle(.secondary)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .tabItem { Label("关于", systemImage: "info.circle") }
        }
        .frame(width: 480, height: 330)
    }
}
