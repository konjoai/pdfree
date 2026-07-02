import SwiftUI

/// Lets the user define inclusive, 0-based page ranges to split the current
/// document into separate files (`pages::split` — see docs/api.md).
struct SplitSheet: View {
    let pageCount: UInt16
    let onExport: ([PageRange]) -> Void
    let onCancel: () -> Void

    @State private var ranges: [RangeEntry] = [RangeEntry(start: 0, end: 0)]

    struct RangeEntry: Identifiable {
        let id = UUID()
        var start: Int
        var end: Int
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Split into Ranges").font(.headline)
            Text("0-based, inclusive page indices. This document has \(pageCount) page(s).")
                .font(.caption)
                .foregroundStyle(.secondary)
            ForEach($ranges) { $entry in
                HStack {
                    Stepper("Start: \(entry.start)", value: $entry.start, in: 0...maxIndex)
                    Stepper("End: \(entry.end)", value: $entry.end, in: 0...maxIndex)
                    Button(role: .destructive) {
                        ranges.removeAll { $0.id == entry.id }
                    } label: {
                        Image(systemName: "trash")
                    }
                }
            }
            Button("Add Range") { ranges.append(RangeEntry(start: 0, end: 0)) }
            HStack {
                Spacer()
                Button("Cancel") { onCancel() }
                Button("Export…") {
                    onExport(ranges.map { PageRange(start: UInt16($0.start), end: UInt16($0.end)) })
                }
                .keyboardShortcut(.defaultAction)
                .disabled(ranges.isEmpty)
            }
        }
        .padding()
        .frame(width: 420)
    }

    private var maxIndex: Int { max(0, Int(pageCount) - 1) }
}
