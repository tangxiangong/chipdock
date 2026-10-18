import SwiftUI

struct FirmwareLayoutView: View {
    var usage: FirmwareUsage
    var regions: [MemoryRegionLayout]

    /// Only the Flash regions the firmware touches, so distant OTP or system
    /// memory regions do not squeeze the firmware into a sliver.
    private var flashRegions: [MemoryRegionLayout] {
        let flash = regions.filter { $0.kind == .nvm && !$0.isAlias }
        guard let low = usage.startAddress, let high = usage.endAddress else { return flash }
        return flash.filter { $0.start < high && $0.end > low }
    }
    private var start: UInt64 { min(usage.startAddress ?? 0, flashRegions.map(\.start).min() ?? usage.startAddress ?? 0) }
    private var end: UInt64 { max(usage.endAddress ?? 1, flashRegions.map(\.end).max() ?? usage.endAddress ?? 1) }
    private var capacity: UInt64 { flashRegions.reduce(0) { $0 + $1.size } }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            GeometryReader { geometry in
                let span = Double(max(end - start, 1))
                ZStack(alignment: .leading) {
                    Rectangle().fill(Theme.well)
                    ForEach(Array(usage.segments.enumerated()), id: \.offset) { _, segment in
                        Rectangle()
                            .fill(Color.accentColor)
                            .frame(width: max(geometry.size.width * Double(segment.size) / span, 2))
                            .offset(x: geometry.size.width * Double(segment.start - start) / span)
                    }
                }
                .clipShape(RoundedRectangle(cornerRadius: 3))
            }
            .frame(height: 8)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel("固件写入布局")
            .accessibilityValue("\(usage.segments.count) 个区段，共 \(byteCountLabel(usage.usedBytes))")
            HStack {
                Text(addressLabel(start))
                Spacer()
                if capacity > 0 {
                    Text("Flash \(byteCountLabel(capacity)) · 已用 \(Double(usage.usedBytes) / Double(capacity) * 100, specifier: "%.1f")%")
                        .foregroundStyle(usage.usedBytes > capacity ? Color.red : Color.secondary)
                }
                Spacer()
                Text(addressLabel(end > 0 ? end - 1 : 0))
            }
            .font(.caption.monospaced())
            .foregroundStyle(.secondary)
        }
    }
}
