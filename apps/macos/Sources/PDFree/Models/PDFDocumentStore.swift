import AppKit
import Foundation

/// Owns the current PDF's bytes and the parsed `PdfDocument` handle, and is
/// the single place every `pdfree-ffi` mutation flows through: each one takes
/// the current bytes, produces new bytes, and this store reloads from the
/// result. That keeps the FFI's "operate on whole-document byte buffers"
/// contract (see docs/api.md) from leaking into every view.
@MainActor
final class PDFDocumentStore: ObservableObject {
    @Published private(set) var data: Data
    @Published private(set) var document: PdfDocument
    @Published var pageIndex: UInt16 = 0
    @Published var pageImage: NSImage?
    @Published var pagePointSize: CGSize = .zero
    @Published var formFieldsList: [FormField] = []
    @Published var annotationsList: [AnnotationInfo] = []
    /// Every fillable box (drawn rectangle or ruled-line table cell) on the
    /// current page, scanned once per page load — presented up front rather
    /// than guessed one click at a time.
    @Published var detectedBoxes: [DetectedBox] = []
    @Published var errorMessage: String?
    @Published var isBusy = false
    @Published var fileURL: URL?

    private var thumbnailCache: [UInt16: NSImage] = [:]
    private let renderDPI: Float = 150
    private let thumbnailDPI: Float = 60

    var pageCount: UInt16 { document.pageCount() }
    var title: String { document.title() ?? fileURL?.lastPathComponent ?? "Untitled" }

    init(data: Data, url: URL?) {
        self.data = data
        self.fileURL = url
        guard let doc = try? PdfDocument.fromBytes(data: data) else {
            fatalError("PDFDocumentStore initialized with unparsable PDF bytes")
        }
        document = doc
        refreshAfterLoad()
    }

    func openReplacing(data: Data, url: URL?) {
        do {
            let doc = try PdfDocument.fromBytes(data: data)
            self.data = data
            document = doc
            fileURL = url
            pageIndex = 0
            thumbnailCache.removeAll()
            refreshAfterLoad()
        } catch {
            errorMessage = describe(error)
        }
    }

    func goToPage(_ index: UInt16) {
        guard index < pageCount else { return }
        pageIndex = index
        renderCurrentPage()
    }

    func thumbnail(at index: UInt16) -> NSImage? {
        if let cached = thumbnailCache[index] { return cached }
        guard let png = try? document.renderPage(index: index, dpi: UInt32(thumbnailDPI)) else { return nil }
        let image = NSImage(data: png)
        if let image { thumbnailCache[index] = image }
        return image
    }

    // MARK: - Mutations

    /// Apply an operation that transforms the current bytes into new bytes,
    /// then reload every derived piece of state (document handle, thumbnails,
    /// current page render, form fields, annotations) from the result.
    func mutate(_ label: String, _ op: (Data) throws -> Data) {
        isBusy = true
        defer { isBusy = false }
        do {
            let newData = try op(data)
            let newDoc = try PdfDocument.fromBytes(data: newData)
            data = newData
            document = newDoc
            thumbnailCache.removeAll()
            if pageIndex >= newDoc.pageCount() {
                pageIndex = newDoc.pageCount() - 1
            }
            refreshAfterLoad()
        } catch {
            errorMessage = "\(label) failed: \(describe(error))"
        }
    }

    func applyFormFill(_ values: [FieldFill]) {
        mutate("Fill form") { try formFill(pdfBytes: $0, values: values) }
    }

    func applyOverlay(_ overlay: TextOverlay) {
        mutate("Add text") { try overlayText(pdfBytes: $0, overlays: [overlay]) }
    }

    func applySignature(pngData: Data, at placement: SignaturePlacement) {
        mutate("Place signature") { try placeSignature(pdfBytes: $0, imagePng: pngData, at: placement) }
    }

    func applyAnnotation(_ annotation: Annotation) {
        mutate("Annotate") { try addAnnotations(pdfBytes: $0, annotations: [annotation]) }
    }

    func applyTextReplace(page: UInt16, find: String, replace: String) {
        mutate("Replace text") { try replaceText(pdfBytes: $0, page: page, find: find, replace: replace) }
    }

    func rotate(page: UInt16, rotation: Rotation) {
        mutate("Rotate page") { try rotatePage(pdfBytes: $0, page: page, rotation: rotation) }
    }

    func deletePage(_ index: UInt16) {
        guard pageCount > 1 else {
            errorMessage = "Can't delete the only page in the document."
            return
        }
        let remaining: [UInt16] = (0..<pageCount).filter { $0 != index }
        mutate("Delete page") { try extractPages(pdfBytes: $0, pages: remaining) }
    }

    func movePages(fromOffsets: IndexSet, toOffset: Int) {
        var order: [UInt16] = Array(0..<pageCount)
        order.move(fromOffsets: fromOffsets, toOffset: toOffset)
        mutate("Reorder pages") { try reorderPages(pdfBytes: $0, newOrder: order) }
    }

    func mergeAppending(_ otherData: Data) {
        mutate("Merge PDF") { try mergeDocuments(documents: [$0, otherData]) }
    }

    func insertImagePage(_ imageData: Data, dpi: Float = 150) {
        mutate("Insert image page") { bytes in
            let imagePdf = try fromImage(imageBytes: imageData, dpi: dpi)
            return try mergeDocuments(documents: [bytes, imagePdf])
        }
    }

    func splitExport(ranges: [PageRange]) -> [Data]? {
        do {
            return try splitDocument(pdfBytes: data, ranges: ranges)
        } catch {
            errorMessage = describe(error)
            return nil
        }
    }

    func extractText() -> String? {
        do {
            return try toText(pdfBytes: data)
        } catch {
            errorMessage = describe(error)
            return nil
        }
    }

    func textRun(atPage page: UInt16, x: Float, y: Float) -> TextRun? {
        do {
            return try textRunAtPoint(pdfBytes: data, page: page, x: x, y: y)
        } catch {
            errorMessage = describe(error)
            return nil
        }
    }

    /// The smallest already-scanned box (see `detectedBoxes`) enclosing a
    /// point, if any — used both to highlight-on-hover and to resolve a
    /// click/double-click into a specific box without another FFI round trip.
    func boxContaining(x: Float, y: Float) -> DetectedBox? {
        let tolerance: Float = 1.5
        return detectedBoxes
            .filter {
                x >= $0.x - tolerance && x <= $0.x + $0.width + tolerance
                    && y >= $0.y - tolerance && y <= $0.y + $0.height + tolerance
            }
            .min { $0.width * $0.height < $1.width * $1.height }
    }

    // MARK: - Private

    private func refreshAfterLoad() {
        renderCurrentPage()
        formFieldsList = (try? formFields(pdfBytes: data)) ?? []
        annotationsList = (try? listAnnotations(pdfBytes: data)) ?? []
    }

    private func renderCurrentPage() {
        do {
            let png = try document.renderPage(index: pageIndex, dpi: UInt32(renderDPI))
            let image = NSImage(data: png)
            pageImage = image
            if let image {
                let ptsPerPixel = CGFloat(72.0 / renderDPI)
                pagePointSize = CGSize(
                    width: image.size.width * ptsPerPixel,
                    height: image.size.height * ptsPerPixel
                )
            }
        } catch {
            errorMessage = describe(error)
        }
        detectedBoxes = (try? boxesOnPage(pdfBytes: data, page: pageIndex)) ?? []
    }

    private func describe(_ error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? "\(error)"
    }
}
