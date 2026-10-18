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

/// A save panel request raised from a popover. The panel is presented by the
/// window instead, because a transient popover closes as soon as it loses focus.
struct PendingExport {
    var data: Data
    var contentType: UTType
    var filename: String
    var operation: String
}
