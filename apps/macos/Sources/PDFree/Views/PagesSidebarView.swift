import SwiftUI

/// The left thumbnail rail: a centered `PAGES` label, then one 88×114pt
/// thumbnail per page — the current page gets a green ring + shadow, others
/// a plain drop shadow. Drag to reorder; right-click (via the ellipsis menu)
/// to rotate or delete.
struct PagesSidebarView: View {
    @ObservedObject var store: PDFDocumentStore
    /// The page currently being dragged, if any — used only for the dimmed
    /// drag-source affordance. Deliberately *not* used to live-reorder the
    /// list on every hover: `store.movePages` round-trips the whole document
    /// through the FFI (re-serializing the PDF), which is too expensive to
    /// run on every drag-over event, so the actual reorder only happens once
    /// on drop.
    @State private var draggedIndex: UInt16?

    var body: some View {
        ScrollView {
            VStack(spacing: 13) {
                Text("PAGES")
                    .font(.system(size: 9, weight: .semibold))
                    .tracking(1)
                    .foregroundStyle(Theme.Color.textLow)
                    .padding(.top, 16)

                VStack(spacing: 16) {
                    ForEach(0..<store.pageCount, id: \.self) { index in
                        pageThumb(index)
                    }
                }
            }
            .padding(.bottom, 16)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Theme.Color.railBg)
        .overlay(Rectangle().fill(Theme.Color.hairlineFaint).frame(width: 1), alignment: .trailing)
    }

    @ViewBuilder
    private func pageThumb(_ index: UInt16) -> some View {
        let isCurrent = store.pageIndex == index

        VStack(spacing: 4) {
            ZStack {
                if let thumb = store.thumbnail(at: index) {
                    Image(nsImage: thumb)
                        .resizable()
                        .aspectRatio(contentMode: .fit)
                } else {
                    Color.white
                }
            }
            .frame(width: Theme.Metric.thumbnailSize.width, height: Theme.Metric.thumbnailSize.height)
            .background(Color.white)
            .clipShape(RoundedRectangle(cornerRadius: 3))
            .overlay(
                RoundedRectangle(cornerRadius: 3)
                    .stroke(Theme.Color.green, lineWidth: isCurrent ? 2 : 0)
            )
            .shadow(color: .black.opacity(isCurrent ? 0.5 : 0.4), radius: isCurrent ? 8 : 6, y: 3)
            .contextMenu {
                Button("Rotate 90° CW") { store.rotate(page: index, rotation: .clockwise90) }
                Button("Rotate 180°") { store.rotate(page: index, rotation: .clockwise180) }
                Button("Rotate 270° CW") { store.rotate(page: index, rotation: .clockwise270) }
                Divider()
                Button("Delete Page", role: .destructive) { store.deletePage(index) }
            }

            Text("\(index + 1)")
                .font(.system(size: 10, weight: isCurrent ? .semibold : .regular))
                .foregroundStyle(isCurrent ? Theme.Color.textHigh : Theme.Color.textMid2)
        }
        .opacity(draggedIndex == index ? 0.4 : 1)
        .contentShape(Rectangle())
        .onTapGesture { store.goToPage(index) }
        .onDrag {
            draggedIndex = index
            return NSItemProvider(object: NSString(string: String(index)))
        }
        .onDrop(
            of: [.text],
            delegate: PageReorderDropDelegate(targetIndex: index, draggedIndex: $draggedIndex) { from, to in
                store.movePages(fromOffsets: IndexSet(integer: Int(from)), toOffset: to)
            }
        )
    }
}

/// Reorders on drop only (see `draggedIndex`'s doc comment) — tracks which
/// page is being dragged so the source thumbnail can dim, but doesn't touch
/// the document until the user actually releases over a target.
private struct PageReorderDropDelegate: DropDelegate {
    let targetIndex: UInt16
    @Binding var draggedIndex: UInt16?
    let onDrop: (UInt16, Int) -> Void

    func dropUpdated(info: DropInfo) -> DropProposal? {
        DropProposal(operation: .move)
    }

    func performDrop(info: DropInfo) -> Bool {
        defer { draggedIndex = nil }
        guard let dragged = draggedIndex, dragged != targetIndex else { return false }
        // `movePages`/`List.onMove` convention: `toOffset` is the index in
        // the array *after* the dragged element is removed, so dropping
        // past the source shifts the target left by one.
        let toOffset = targetIndex > dragged ? Int(targetIndex) + 1 : Int(targetIndex)
        onDrop(dragged, toOffset)
        return true
    }
}
