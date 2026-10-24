import SwiftUI
import UniformTypeIdentifiers

struct ExportDocument: FileDocument {
    static var readableContentTypes: [UTType] { [.data, .plainText] }
    var data: Data

    init(data: Data) { self.data = data }
    init(configuration: ReadConfiguration) throws {
        data = configuration.file.regularFileContents ?? Data()
    }
    func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {
        FileWrapper(regularFileWithContents: data)
    }
}

/// A save-panel request owned by the window where the export was initiated.
struct PendingExport {
    var data: Data
    var contentType: UTType
    var filename: String
    var operation: String
}

private struct ExportPresentation: ViewModifier {
    @Binding var request: PendingExport?
    @State private var failure: String?

    func body(content: Content) -> some View {
        content
            .fileExporter(isPresented: Binding(
                get: { request != nil }, set: { if !$0 { request = nil } }
            ), document: ExportDocument(data: request?.data ?? Data()),
               contentType: request?.contentType ?? .data, defaultFilename: request?.filename) { result in
                if case .failure(let error) = result { failure = error.localizedDescription }
            }
            .alert("导出失败", isPresented: Binding(
                get: { failure != nil }, set: { if !$0 { failure = nil } }
            )) {
                Button("好", role: .cancel) { failure = nil }
            } message: { Text(failure ?? "") }
    }
}

extension View {
    func exportPresentation(_ request: Binding<PendingExport?>) -> some View {
        modifier(ExportPresentation(request: request))
    }
}

private struct LogExportKey: FocusedValueKey { typealias Value = () -> Void }
private struct MemoryExportKey: FocusedValueKey { typealias Value = () -> Void }

extension FocusedValues {
    var logExport: (() -> Void)? {
        get { self[LogExportKey.self] }
        set { self[LogExportKey.self] = newValue }
    }
    var memoryExport: (() -> Void)? {
        get { self[MemoryExportKey.self] }
        set { self[MemoryExportKey.self] = newValue }
    }
}
