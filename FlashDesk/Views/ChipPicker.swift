import SwiftUI

struct ChipPicker: View {
    @Bindable var store: WorkbenchStore
    @Environment(\.dismiss) private var dismiss
    @State private var query = ""
    @State private var results: [String] = []
    @State private var selection: String?
    @State private var searching = false
    @State private var searchError: String?

    private var candidates: [String] { store.targetCandidates.map(\.name) }
    private var displayed: [String] { query.isEmpty ? candidates : results }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
                Text(candidates.isEmpty ? "选择芯片" : "选择识别到的目标").font(.headline)
                Text(candidates.isEmpty ? "搜索 probe-rs 支持的芯片型号。" : "目标无法唯一识别，请从候选型号中选择。")
                    .font(.callout).foregroundStyle(.secondary)
            }
            HStack(spacing: 8) {
                SearchField(prompt: "搜索芯片型号，例如 STM32F407", text: $query)
                    .accessibilityLabel("搜索芯片型号")
                if searching { ProgressView().controlSize(.small) }
            }
            if let error = searchError { Text(error).font(.caption).foregroundStyle(.red) }
            List(selection: $selection) {
                if query.isEmpty && candidates.isEmpty {
                    Label("自动识别", systemImage: "sparkle.magnifyingglass").tag("")
                }
                ForEach(displayed, id: \.self) { name in
                    Label(name, systemImage: "cpu").font(.body.monospaced()).tag(name)
                }
            }
            .listStyle(.bordered)
            .alternatingRowBackgrounds()
            .overlay {
                if !query.isEmpty && displayed.isEmpty && !searching && searchError == nil {
                    ContentUnavailableView.search(text: query)
                }
            }
            HStack {
                if !displayed.isEmpty { Text("\(displayed.count) 个目标").font(.caption).foregroundStyle(.secondary) }
                Spacer()
                Button("取消", role: .cancel) { dismiss() }.keyboardShortcut(.cancelAction)
                Button(candidates.isEmpty ? "选择" : "选择并连接") {
                    guard let selection else { return }
                    let reconnect = !candidates.isEmpty
                    store.chip = selection
                    dismiss()
                    if reconnect { Task { await store.connect() } }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(selection == nil || searching)
            }
        }
        .padding(24)
        .frame(width: 500, height: 480)
        .onAppear { selection = candidates.first ?? store.chip }
        .task(id: query) { await search() }
    }

    private func search() async {
        searchError = nil
        results = []
        guard !query.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { searching = false; return }
        let requestedQuery = query
        searching = true
        do {
            try await Task.sleep(for: .milliseconds(200))
            let found = try await store.rust.run { try $0.searchChips(query: requestedQuery, limit: 150) }
            guard !Task.isCancelled, query == requestedQuery else { return }
            results = found
            selection = found.first
            searching = false
        } catch is CancellationError { }
        catch {
            guard query == requestedQuery else { return }
            searchError = error.localizedDescription
            searching = false
        }
    }
}
