import SwiftUI

/// Continuous-scroll page view mode: every page stacked vertically at
/// fit-to-width, replacing page-by-page navigation with plain scrolling. This
/// is strictly a reading/navigation mode — fill/sign/annotate/overlay-text
/// interactions stay single-page-only (see `PDFDocumentStore.pageViewMode`'s
/// doc comment); a solid, well-tested reading mode beats a half-working
/// interactive one. Lazy: `LazyVStack` only materializes pages near the
/// visible area, and each one's image is rendered on demand.
struct ContinuousScrollView: View {
    @ObservedObject var store: PDFDocumentStore
    let viewportWidth: CGFloat

    static let scrollSpace = "pdfree.continuousScroll"

    var body: some View {
        ScrollViewReader { proxy in
            ScrollView {
                LazyVStack(spacing: 16) {
                    ForEach(0..<store.pageCount, id: \.self) { index in
                        ContinuousPageRow(store: store, index: index, viewportWidth: viewportWidth)
                            .id(index)
                    }
                }
                .padding(.vertical, 16)
            }
            .coordinateSpace(name: Self.scrollSpace)
            .onPreferenceChange(PageTopOffsetKey.self) { offsets in
                if let nearest = Self.nearestPageToTop(offsets: offsets), nearest != store.pageIndex {
                    // Direct assignment, not `goToPage` — scrolling itself
                    // shouldn't count as an explicit "jump" (see
                    // `pageJumpToken`'s doc comment), or every scroll tick
                    // would trigger `proxy.scrollTo` below and fight the
                    // user's own drag.
                    store.pageIndex = nearest
                }
            }
            .onAppear {
                proxy.scrollTo(store.pageIndex, anchor: .top)
            }
            .onChange(of: store.pageJumpToken) { _ in
                withAnimation(.easeInOut(duration: 0.2)) {
                    proxy.scrollTo(store.pageIndex, anchor: .top)
                }
            }
        }
    }

    /// The page whose top has scrolled up to (or just past) the top of the
    /// viewport is "current" — the entry with the largest offset that's
    /// still `<= threshold`. Falls back to whichever entry is closest to 0
    /// if every visible page's top is still below the threshold (e.g. right
    /// after opening, before any scrolling has happened). Pure and unit
    /// tested directly (`ContinuousScrollViewTests`) — no view rendering or
    /// PDFium involved.
    static func nearestPageToTop(offsets: [UInt16: CGFloat], threshold: CGFloat = 80) -> UInt16? {
        let atOrAboveThreshold = offsets.filter { $0.value <= threshold }
        if let best = atOrAboveThreshold.max(by: { $0.value < $1.value }) {
            return best.key
        }
        return offsets.min(by: { abs($0.value) < abs($1.value) })?.key
    }
}

private struct PageTopOffsetKey: PreferenceKey {
    static var defaultValue: [UInt16: CGFloat] = [:]
    static func reduce(value: inout [UInt16: CGFloat], nextValue: () -> [UInt16: CGFloat]) {
        value.merge(nextValue()) { _, new in new }
    }
}

private struct ContinuousPageRow: View {
    @ObservedObject var store: PDFDocumentStore
    let index: UInt16
    let viewportWidth: CGFloat

    var body: some View {
        Group {
            if let image = store.continuousPageImage(at: index, viewportWidth: viewportWidth) {
                Image(nsImage: image)
                    .resizable()
                    .aspectRatio(contentMode: .fit)
                    .frame(width: viewportWidth)
            } else {
                Color.white
                    .frame(width: viewportWidth, height: placeholderHeight)
            }
        }
        .shadow(color: .black.opacity(0.45), radius: 14, y: 8)
        .background(
            GeometryReader { geo in
                Color.clear.preference(
                    key: PageTopOffsetKey.self,
                    value: [index: geo.frame(in: .named(ContinuousScrollView.scrollSpace)).minY]
                )
            }
        )
    }

    /// Best-guess placeholder height before this page's own image has
    /// loaded: the cached point size if any page's been measured yet (most
    /// PDFs share one page size), else a Letter-ratio fallback.
    private var placeholderHeight: CGFloat {
        if let size = store.cachedPageSize(at: index), size.width > 0 {
            return viewportWidth * size.height / size.width
        }
        return viewportWidth * 11 / 8.5
    }
}
