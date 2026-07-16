import SwiftUI

/// Recursive outline/table-of-contents tree for `store.documentOutline`.
/// Tapping any row with a page jumps there via `store.goToPage`; rows with no
/// page (a heading-only bookmark) are inert labels.
struct OutlineTreeView: View {
    @ObservedObject var store: PDFDocumentStore

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 2) {
                ForEach(Array(store.documentOutline.enumerated()), id: \.offset) { _, node in
                    OutlineRow(node: node, depth: 0, store: store)
                }
            }
            .padding(.horizontal, 8)
            .padding(.vertical, 12)
        }
    }
}

private struct OutlineRow: View {
    let node: Bookmark
    let depth: Int
    @ObservedObject var store: PDFDocumentStore
    @State private var isExpanded = true

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 4) {
                if !node.children.isEmpty {
                    Button {
                        withAnimation(.easeInOut(duration: 0.12)) { isExpanded.toggle() }
                    } label: {
                        Image(systemName: "chevron.right")
                            .font(.system(size: 8, weight: .semibold))
                            .rotationEffect(.degrees(isExpanded ? 90 : 0))
                            .foregroundStyle(Theme.Color.textLow)
                            .frame(width: 12, height: 12)
                    }
                    .buttonStyle(.plain)
                } else {
                    Color.clear.frame(width: 12, height: 12)
                }

                Text(node.title)
                    .font(.system(size: 12))
                    .foregroundStyle(Theme.Color.textHigh)
                    .lineLimit(1)
                    .truncationMode(.tail)

                Spacer(minLength: 0)
            }
            .padding(.vertical, 4)
            .padding(.leading, CGFloat(depth) * 14)
            .contentShape(Rectangle())
            .onTapGesture {
                if let page = node.page {
                    store.goToPage(page)
                }
            }

            if isExpanded {
                ForEach(Array(node.children.enumerated()), id: \.offset) { _, child in
                    OutlineRow(node: child, depth: depth + 1, store: store)
                }
            }
        }
    }
}
