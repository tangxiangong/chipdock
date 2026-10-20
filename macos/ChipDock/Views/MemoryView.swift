import SwiftUI

struct MemoryView: View {
    @Bindable var store: WorkbenchStore

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 10) {
                Text("内存").font(.title3)
                Spacer()
                Button("导出…", systemImage: "square.and.arrow.up") { export() }
                    .labelStyle(.iconOnly).buttonStyle(.borderless)
                    .disabled(store.memoryResult == nil || store.isBusy)
                    .help("导出为二进制")
                Menu {
                    Button("擦除整个芯片…", systemImage: "trash", role: .destructive) {
                        store.showMemory = false
                        store.confirmErase = true
                    }
                    .disabled(!store.canErase)
                } label: { Image(systemName: "ellipsis.circle") }
                .menuStyle(.borderlessButton).menuIndicator(.hidden).fixedSize()
                .help("芯片操作")
            }
            .padding(.horizontal, 16)
            .padding(.top, 14)
            .padding(.bottom, 10)
            HStack(spacing: 8) {
                TextField("起始地址", text: $store.memoryAddress, prompt: Text("0x08000000"))
                    .textFieldStyle(.roundedBorder)
                    .font(Theme.mono)
                    .onSubmit { Task { await store.readMemory() } }
                    .labelsHidden()
                if parseAddress(store.memoryAddress) == nil {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundStyle(.red)
                        .help("请输入十六进制地址")
                        .accessibilityLabel("地址无效")
                }
                Picker("长度", selection: $store.memoryLength) {
                    ForEach([64, 128, 256, 512, 1024, 2048, 4096], id: \.self) { Text("\($0) B").tag(UInt32($0)) }
                }
                .labelsHidden().fixedSize()
                Button("读取", systemImage: "arrow.down.circle") { Task { await store.readMemory() } }
                    .disabled(!store.canReadMemory)
                    .help("读取内存（⇧⌘R）")
            }
            .padding(.horizontal, 16)
            .padding(.bottom, 12)
            if !store.memoryRegions.isEmpty {
                Divider()
                MemoryMapList(store: store)
                    .frame(height: min(CGFloat(store.memoryRegions.count) * 24 + 34, 160))
            }
            Divider()
            Group {
                if store.memoryResult != nil {
                    HexDump(lines: store.memoryLines)
                } else {
                    ContentUnavailableView {
                        Label("尚未读取", systemImage: "memorychip")
                    } description: {
                        Text(store.connection == nil ? "连接目标后可读取内存" : "输入地址后按回车或点击“读取”")
                    }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    private func export() {
        guard let result = store.memoryResult else { return }
        store.requestExport(PendingExport(data: result.data, contentType: .data,
                                          filename: "memory-\(addressLabel(result.address)).bin", operation: "导出内存"))
    }
}

/// Eight bytes per row keeps the popover narrow.
private struct HexDump: View {
    var lines: [MemoryLine]

    private static let width = 8

    private var rows: [MemoryLine] {
        guard let base = lines.first?.address else { return [] }
        let bytes = lines.flatMap(\.bytes)
        return stride(from: 0, to: bytes.count, by: Self.width).map { offset in
            MemoryLine(address: base + UInt64(offset), bytes: Array(bytes[offset..<min(offset + Self.width, bytes.count)]))
        }
    }

    var body: some View {
        ScrollView([.vertical, .horizontal]) {
            LazyVStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 12) {
                    Text("Offset    ")
                    Text((0..<Self.width).map { String(format: "%02X", $0) }.joined(separator: " "))
                    Text("ASCII")
                }
                .fixedSize()
                .foregroundStyle(.secondary)
                .padding(.vertical, 4)
                ForEach(rows) { line in
                    HStack(spacing: 12) {
                        Text(addressLabel(line.address)).foregroundStyle(.secondary)
                        hex(line.bytes)
                        Text(line.ascii).foregroundStyle(.secondary)
                    }
                    .fixedSize()
                }
            }
            .font(.system(size: 11, design: .monospaced))
            .padding(.horizontal, 16)
            .padding(.bottom, 8)
        }
        .textSelection(.enabled)
    }

    /// Erased (0xFF) and zero bytes are dimmed so meaningful data stands out.
    private func hex(_ bytes: [UInt8]) -> Text {
        (0..<Self.width).reduce(Text("")) { text, index in
            let separator = index == 0 ? "" : " "
            guard index < bytes.count else { return text + Text(separator + "  ") }
            let byte = bytes[index]
            let cell = Text(String(format: "%02X", byte))
                .foregroundStyle(byte == 0x00 || byte == 0xFF ? Color.secondary.opacity(0.5) : Color.primary)
            return text + Text(separator) + cell
        }
    }
}

/// Memory regions as a system list; selecting one jumps the read address to it.
private struct MemoryMapList: View {
    @Bindable var store: WorkbenchStore

    private var currentRegion: Binding<Int?> {
        Binding {
            guard let address = parseAddress(store.memoryAddress) else { return nil }
            return store.memoryRegions.firstIndex { address >= $0.start && address < $0.end }
        } set: { index in
            guard let index, store.memoryRegions.indices.contains(index) else { return }
            store.memoryAddress = addressLabel(store.memoryRegions[index].start)
        }
    }

    var body: some View {
        List(selection: currentRegion) {
            Section("内存映射") {
                ForEach(Array(store.memoryRegions.enumerated()), id: \.offset) { index, region in
                    HStack(spacing: 6) {
                        Rectangle().fill(region.kind.tint).frame(width: 3, height: 12)
                        Text(region.name ?? region.kind.title).lineLimit(1).truncationMode(.tail)
                        Spacer(minLength: 4)
                        Text(addressLabel(region.start)).font(.caption.monospaced()).foregroundStyle(.secondary)
                        Text(byteCountLabel(region.size)).font(.caption).foregroundStyle(.secondary)
                            .frame(width: 58, alignment: .trailing)
                    }
                    .help("将起始地址设为 \(addressLabel(region.start))")
                    .tag(index)
                }
            }
        }
        .listStyle(.plain)
        .scrollContentBackground(.hidden)
        .disabled(store.isBusy)
    }
}
