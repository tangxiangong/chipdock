import SwiftUI
import UniformTypeIdentifiers

struct MemoryView: View {
    @Bindable var store: WorkbenchStore
    @State private var showExporter = false

    var body: some View {
        VStack(alignment: .leading, spacing: 20) {
            HStack(alignment: .firstTextBaseline) {
                Text("内存读取").font(.title2.weight(.semibold))
                Spacer()
                Menu {
                    Button("擦除整个芯片…", role: .destructive) { store.confirmErase = true }
                        .disabled(store.isBusy || store.connection == nil)
                } label: { Image(systemName: "ellipsis.circle") }
                .menuStyle(.borderlessButton)
                .fixedSize()
                .help("芯片操作")
            }
            HStack(alignment: .bottom, spacing: 12) {
                VStack(alignment: .leading, spacing: 7) {
                    Text("起始地址").font(.caption).foregroundStyle(.secondary)
                    TextField("0x08000000", text: $store.memoryAddress)
                        .font(.system(.body, design: .monospaced))
                        .frame(minWidth: 140)
                        .accessibilityLabel("起始地址")
                }
                VStack(alignment: .leading, spacing: 7) {
                    Text("字节数").font(.caption).foregroundStyle(.secondary)
                    Picker("字节数", selection: $store.memoryLength) {
                        ForEach([64, 128, 256, 512, 1024, 2048, 4096], id: \.self) { Text(String($0)).tag(UInt32($0)) }
                    }
                    .labelsHidden()
                    .frame(width: 95)
                }
                Button("读取", systemImage: "arrow.down.doc") { Task { await store.readMemory() } }
                    .buttonStyle(.borderedProminent)
                    .disabled(!store.canReadMemory)
                    .help("读取内存（⇧⌘R）")
            }
            if parseAddress(store.memoryAddress) == nil {
                Label("请输入有效地址", systemImage: "exclamationmark.triangle")
                    .font(.caption).foregroundStyle(.red)
            }
            memoryContents
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            if !store.memoryRegions.isEmpty { memoryMap }
            if let event = store.latestEvent, event.kind == .erase { JobProgressView(event: event, isBusy: store.isBusy) }
        }
        .padding(28)
        .fileExporter(isPresented: $showExporter, document: ExportDocument(data: store.memoryResult?.data ?? Data()),
                      contentType: .data, defaultFilename: "memory-\(addressLabel(store.memoryResult?.address)).bin") { result in
            if case .failure(let error) = result { store.report(error, operation: "导出内存") }
        }
    }

    @ViewBuilder
    private var memoryContents: some View {
        if let result = store.memoryResult {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    Text("\(addressLabel(result.address)) · \(result.length) 字节")
                        .font(.caption.monospaced()).foregroundStyle(.secondary)
                    Spacer()
                    Button("导出…", systemImage: "square.and.arrow.up") { showExporter = true }
                        .disabled(store.isBusy)
                }
                ScrollView([.horizontal, .vertical]) {
                    LazyVStack(alignment: .leading, spacing: 5) {
                        HStack(spacing: 20) {
                            Text("地址").frame(width: 90, alignment: .leading)
                            Text((0..<16).map { String(format: "%02X", $0) }.joined(separator: " "))
                            Text("ASCII")
                        }
                        .foregroundStyle(.tertiary)
                        .padding(.bottom, 7)
                        ForEach(store.memoryLines) { line in
                            HStack(spacing: 20) {
                                Text(addressLabel(line.address)).foregroundStyle(.secondary)
                                    .frame(width: 90, alignment: .leading)
                                Text(line.hex.padding(toLength: 47, withPad: " ", startingAt: 0))
                                Text(line.ascii).foregroundStyle(.secondary)
                            }
                        }
                    }
                    .font(.system(size: 11, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(16)
                }
                .background(.quaternary.opacity(0.2), in: RoundedRectangle(cornerRadius: 8))
            }
        } else {
            ContentUnavailableView(store.connection == nil ? "连接目标以读取内存" : "选择地址并读取内存", systemImage: "memorychip")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.quaternary.opacity(0.15), in: RoundedRectangle(cornerRadius: 12))
        }
    }

    private var memoryMap: some View {
        DisclosureGroup("内存映射") {
            ScrollView {
                VStack(spacing: 10) {
                    ForEach(Array(store.memoryRegions.enumerated()), id: \.offset) { _, region in
                        Button {
                            store.memoryAddress = addressLabel(region.start)
                            store.memoryLength = 256
                        } label: {
                            HStack {
                                Label(region.name ?? region.kind.title, systemImage: region.kind == .nvm ? "internaldrive" : "memorychip")
                                    .frame(minWidth: 90, alignment: .leading)
                                Text(addressLabel(region.start)).font(.caption.monospaced()).foregroundStyle(.secondary)
                                Spacer()
                                Text(byteCountLabel(region.size)).monospacedDigit().foregroundStyle(.secondary)
                            }
                            .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .disabled(store.isBusy)
                        .help("将起始地址设为 \(addressLabel(region.start))")
                    }
                }
                .padding(.top, 10)
            }
            .frame(maxHeight: 160)
        }
        .font(.callout)
    }
}

extension MemoryRegionKind {
    var title: String {
        switch self {
        case .nvm: "Flash"
        case .ram: "RAM"
        case .generic: "存储区域"
        }
    }
}
