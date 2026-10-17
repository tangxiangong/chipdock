import SwiftUI

struct FirmwareLayoutView: View {
    var usage: FirmwareUsage
    var regions: [MemoryRegionLayout]

    private var flashRegions: [MemoryRegionLayout] { regions.filter { $0.kind == .nvm && !$0.isAlias } }
    private var start: UInt64 { min(usage.startAddress ?? 0, flashRegions.map(\.start).min() ?? usage.startAddress ?? 0) }
    private var end: UInt64 { max(usage.endAddress ?? 1, flashRegions.map(\.end).max() ?? usage.endAddress ?? 1) }
    private var capacity: UInt64 { flashRegions.reduce(0) { $0 + $1.size } }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            HStack {
                Label("写入布局", systemImage: "rectangle.split.3x1").font(.headline)
                Spacer()
                if capacity > 0 {
                    Text("\(Double(usage.usedBytes) / Double(capacity) * 100, specifier: "%.1f")%")
                        .font(.system(.callout, design: .monospaced)).foregroundStyle(.secondary)
                }
            }
            GeometryReader { geometry in
                ZStack(alignment: .leading) {
                    RoundedRectangle(cornerRadius: 5).fill(.quaternary)
                    ForEach(Array(usage.segments.enumerated()), id: \.offset) { _, segment in
                        let span = Double(max(end - start, 1))
                        RoundedRectangle(cornerRadius: 3)
                            .fill(Color.accentColor)
                            .frame(width: max(geometry.size.width * Double(segment.size) / span, 2))
                            .offset(x: geometry.size.width * Double(segment.start - start) / span)
                    }
                }
            }
            .frame(height: 18)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("固件写入布局")
            .accessibilityValue("\(usage.segments.count) 个区段，共 \(byteCountLabel(usage.usedBytes))")
            HStack {
                Text(addressLabel(start))
                Spacer()
                Text(addressLabel(end > 0 ? end - 1 : 0))
            }
            .font(.caption.monospaced()).foregroundStyle(.tertiary)
            HStack(spacing: 18) {
                Label("写入 \(byteCountLabel(usage.usedBytes))", systemImage: "rectangle.fill")
                    .foregroundStyle(Color.accentColor)
                if capacity > 0 {
                    Text("Flash \(byteCountLabel(capacity))").foregroundStyle(.secondary)
                }
            }
            .font(.caption)
        }
    }
}
