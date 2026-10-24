import AppKit
import SwiftUI

/// Actual file bytes at their load addresses; no target-memory read is issued.
struct FirmwareDataView: View {
    var preview: FirmwarePreview

    private static let dataFont = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 12) {
                Text("固件内容").font(.callout)
                Spacer(minLength: 0)
                Text("\(addressLabel(preview.startAddress)) – \(addressLabel(endAddress))")
                    .font(.caption.monospaced())
                    .foregroundStyle(.secondary)
                    .lineLimit(1).truncationMode(.middle)
            }
            .help("固件文件起始最多 4 KiB 的实际数据；·· 表示文件未写入的地址")
            GeometryReader { geometry in
                let width = max(geometry.size.width - 24, 0)
                let characterWidth = ceil(Self.dataFont.maximumAdvancement.width)
                let addressWidth = CGFloat(max(addressLabel(endAddress).count, 10)) * characterWidth
                let columns = width >= addressWidth + 12 + 47 * characterWidth ? 16
                    : (width >= addressWidth + 12 + 23 * characterWidth ? 8 : 4)
                let hexWidth = CGFloat(columns * 3 - 1) * characterWidth
                let asciiWidth = CGFloat(columns) * characterWidth
                let showsASCII = width >= addressWidth + hexWidth + 24 + asciiWidth
                VStack(spacing: 0) {
                    HStack(spacing: 12) {
                        Text("地址").frame(width: addressWidth, alignment: .leading)
                        Text("十六进制").frame(width: hexWidth, alignment: .leading)
                        if showsASCII { Text("ASCII") }
                        Spacer(minLength: 0)
                    }
                    .font(.caption).foregroundStyle(.secondary)
                    .padding(.horizontal, 12).padding(.vertical, 6)
                    .background(Theme.well)
                    Divider()
                    ScrollView {
                        LazyVStack(alignment: .leading, spacing: 2) {
                            ForEach(Array(stride(from: 0, to: preview.bytes.count, by: columns)), id: \.self) { offset in
                                let bytes = Array(preview.bytes[offset..<min(offset + columns, preview.bytes.count)])
                                HStack(spacing: 12) {
                                    Text(addressLabel(preview.startAddress + UInt64(offset)))
                                        .foregroundStyle(.secondary)
                                        .fixedSize(horizontal: true, vertical: false)
                                        .frame(width: addressWidth, alignment: .leading)
                                    Text(hex(bytes))
                                        .fixedSize(horizontal: true, vertical: false)
                                        .frame(width: hexWidth, alignment: .leading)
                                    if showsASCII {
                                        Text(ascii(bytes))
                                            .foregroundStyle(.secondary)
                                            .fixedSize(horizontal: true, vertical: false)
                                            .frame(width: asciiWidth, alignment: .leading)
                                    }
                                }
                            }
                        }
                        .font(Font(Self.dataFont))
                        .lineLimit(1)
                        .padding(.horizontal, 12).padding(.vertical, 4)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .textSelection(.enabled)
                    }
                    .scrollBounceBehavior(.basedOnSize)
                }
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .overlay {
                    RoundedRectangle(cornerRadius: 6)
                        .strokeBorder(Color.primary.opacity(0.12), lineWidth: 1)
                }
            }
            .frame(minHeight: 64)
        }
    }

    private var endAddress: UInt64 {
        preview.startAddress + UInt64(max(preview.bytes.count - 1, 0))
    }

    private func hex(_ bytes: [UInt8?]) -> String {
        bytes.map { $0.map { String(format: "%02X", $0) } ?? "··" }.joined(separator: " ")
    }

    private func ascii(_ bytes: [UInt8?]) -> String {
        String(bytes.map { value in
            guard let value, (32...126).contains(value) else { return Character("·") }
            return Character(UnicodeScalar(value))
        })
    }
}
