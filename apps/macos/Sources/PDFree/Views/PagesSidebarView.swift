import SwiftUI

struct PagesSidebarView: View {
    @ObservedObject var store: PDFDocumentStore

    var body: some View {
        List {
            ForEach(0..<store.pageCount, id: \.self) { index in
                pageRow(index)
            }
            .onMove { store.movePages(fromOffsets: $0, toOffset: $1) }
        }
        .listStyle(.sidebar)
    }

    @ViewBuilder
    private func pageRow(_ index: UInt16) -> some View {
        HStack {
            if let thumb = store.thumbnail(at: index) {
                Image(nsImage: thumb)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: 56, height: 72)
                    .border(store.pageIndex == index ? Color.accentColor : Color.clear, width: 2)
            } else {
                Rectangle().fill(.quaternary).frame(width: 56, height: 72)
            }
            Text("\(index + 1)")
                .font(.callout)
            Spacer()
            Menu {
                Button("Rotate 90° CW") { store.rotate(page: index, rotation: .clockwise90) }
                Button("Rotate 180°") { store.rotate(page: index, rotation: .clockwise180) }
                Button("Rotate 270° CW") { store.rotate(page: index, rotation: .clockwise270) }
                Divider()
                Button("Delete Page", role: .destructive) { store.deletePage(index) }
            } label: {
                Image(systemName: "ellipsis.circle")
            }
            .menuStyle(.borderlessButton)
            .frame(width: 24)
        }
        .contentShape(Rectangle())
        .onTapGesture { store.goToPage(index) }
        .padding(.vertical, 2)
    }
}
