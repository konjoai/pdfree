import AppKit
import Foundation

/// A scanned box (see `DetectedBox`) paired with whatever we could work out
/// about the named `AcroForm` field underneath it, so the canvas can style
/// signature/initials fields distinctly from ordinary fill boxes (Core UX
/// Principles: "signature/initials fields are special-cased"). The
/// signature/initials classification itself comes from `pdfree-core`
/// (`FormField.signatureKind`), not a Swift-side heuristic, so every shell
/// (macOS/web/Tauri/iOS) agrees on it.
struct FieldOverlayBox: Identifiable {
    let id = UUID()
    let box: DetectedBox
    let signatureKind: SignatureFieldKind
    let fieldName: String?

    var isSignature: Bool { signatureKind != .none }
}

/// Owns the current PDF's bytes and the parsed `PdfDocument` handle, and is
/// the single place every `pdfree-ffi` mutation flows through: each one takes
/// the current bytes, produces new bytes, and this store reloads from the
/// result. That keeps the FFI's "operate on whole-document byte buffers"
/// contract (see docs/api.md) from leaking into every view.
///
/// No document is loaded until `openReplacing` is called — the app starts on
/// the empty state (Core UX Principles / design handoff: "the drop surface
/// IS the window", never auto-load a bundled sample).
@MainActor
final class PDFDocumentStore: ObservableObject {
    @Published private(set) var data: Data?
    @Published private(set) var document: PdfDocument?
    @Published var pageIndex: UInt16 = 0
    @Published var pageImage: NSImage?
    @Published var pagePointSize: CGSize = .zero
    @Published var formFieldsList: [FormField] = []
    @Published var annotationsList: [AnnotationInfo] = []
    /// Every fillable box (drawn rectangle or ruled-line table cell) on the
    /// current page, scanned once per page load — presented up front rather
    /// than guessed one click at a time.
    @Published var detectedBoxes: [DetectedBox] = []
    /// `detectedBoxes` merged with `formFieldsList`'s page/rect, classified
    /// normal vs. signature — what `PageCanvasView` actually draws.
    @Published var fieldOverlays: [FieldOverlayBox] = []
    @Published var errorMessage: String?
    @Published var isBusy = false
    @Published var fileURL: URL?
    @Published private(set) var recentFiles: [URL] = []
    @Published private(set) var savedSignatures: [SavedSignature] = []

    private var thumbnailCache: [UInt16: NSImage] = [:]
    /// Per-page box scan (`boxesOnPage`) and page-size results, cached because
    /// neither changes when only the zoom/DPI changes — so a window resize
    /// re-renders the image without re-running the (expensive) vector scan.
    /// Cleared on open and on every mutation, since those can change geometry.
    private var boxesCache: [UInt16: [DetectedBox]] = [:]
    private var pageSizeCache: [UInt16: CGSize] = [:]
    /// Available canvas size, in pixels, that the current page should fit
    /// inside — set by the canvas view via `updateViewport` on load and on
    /// every resize (Core UX Principles: default view = whole page visible,
    /// always). Falls back to `fallbackDPI` until the canvas reports a real
    /// size.
    private var viewportSize: CGSize = .zero
    private let fallbackDPI: Float = 150
    private let thumbnailDPI: Float = 60

    private static let recentFilesKey = "PDFree.recentFiles"
    private static let signerNameKey = "PDFree.signerName"

    private static let auditDateFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        return formatter
    }()

    private static let deviceInfo: String = {
        let v = ProcessInfo.processInfo.operatingSystemVersion
        return "macOS \(v.majorVersion).\(v.minorVersion)"
    }()

    var hasDocument: Bool { document != nil }
    var pageCount: UInt16 { document?.pageCount() ?? 0 }
    var title: String { document?.title() ?? fileURL?.lastPathComponent ?? "Untitled" }

    /// The name baked into every signature's audit caption. Defaults to the
    /// macOS account's full name (so signing works with zero extra setup);
    /// persisted so an explicit override — once there's UI for one — sticks.
    var signerName: String {
        get { UserDefaults.standard.string(forKey: Self.signerNameKey) ?? NSFullUserName() }
        set { UserDefaults.standard.set(newValue, forKey: Self.signerNameKey) }
    }

    /// Signature/initials-kind fields across the whole document — what the
    /// sign flow hops between, in page then on-page reading order.
    var signatureFields: [FormField] {
        formFieldsList
            .filter { $0.signatureKind != .none }
            .sorted { ($0.page, $0.y) > ($1.page, $1.y) }
    }

    init() {
        loadSavedSignatures()
        loadRecentFiles()
    }

    func openReplacing(data: Data, url: URL?) {
        do {
            let doc = try PdfDocument.fromBytes(data: data)
            self.data = data
            document = doc
            fileURL = url
            pageIndex = 0
            thumbnailCache.removeAll()
            boxesCache.removeAll()
            pageSizeCache.removeAll()
            viewportSize = .zero
            rememberRecent(url)
            refreshAfterLoad()
        } catch {
            errorMessage = describe(error)
        }
    }

    /// Back to the empty state.
    func closeDocument() {
        data = nil
        document = nil
        pageImage = nil
        pagePointSize = .zero
        formFieldsList = []
        annotationsList = []
        detectedBoxes = []
        fieldOverlays = []
        fileURL = nil
        pageIndex = 0
        thumbnailCache.removeAll()
        boxesCache.removeAll()
        pageSizeCache.removeAll()
        viewportSize = .zero
    }

    func goToPage(_ index: UInt16) {
        guard index < pageCount else { return }
        pageIndex = index
        renderCurrentPage()
    }

    func thumbnail(at index: UInt16) -> NSImage? {
        if let cached = thumbnailCache[index] { return cached }
        guard let document, let png = try? document.renderPage(index: index, dpi: UInt32(thumbnailDPI))
        else { return nil }
        let image = NSImage(data: png)
        if let image { thumbnailCache[index] = image }
        return image
    }

    // MARK: - Mutations

    /// Apply an operation that transforms the current bytes into new bytes,
    /// then reload every derived piece of state (document handle, thumbnails,
    /// current page render, form fields, annotations) from the result.
    func mutate(_ label: String, _ op: (Data) throws -> Data) {
        guard let data else { return }
        isBusy = true
        defer { isBusy = false }
        do {
            let newData = try op(data)
            let newDoc = try PdfDocument.fromBytes(data: newData)
            self.data = newData
            document = newDoc
            thumbnailCache.removeAll()
            boxesCache.removeAll()
            pageSizeCache.removeAll()
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

    /// Stamps the signature image plus a small "Signed by … · …" caption
    /// beneath it — the lightweight local audit record (Core UX Principles:
    /// signer name + timestamp, not a certified/legal-grade chain of
    /// custody). Signer name defaults to the macOS account's full name so
    /// this needs no extra UI to work; device info is the OS version.
    func applySignature(pngData: Data, at placement: SignaturePlacement) {
        let audit = SignatureAudit(
            signerName: signerName,
            signedAt: Self.auditDateFormatter.string(from: Date()),
            deviceInfo: Self.deviceInfo
        )
        mutate("Place signature") {
            try placeSignatureWithAudit(pdfBytes: $0, imagePng: pngData, at: placement, audit: audit)
        }
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

    /// Insert a blank page (letter-size, 612x792pt) at the end of the document.
    func insertBlankPage() {
        mutate("Insert blank page") { bytes in
            let blank = try fromImage(imageBytes: Self.blankPagePNG(), dpi: 72)
            return try mergeDocuments(documents: [bytes, blank])
        }
    }

    func splitExport(ranges: [PageRange]) -> [Data]? {
        guard let data else { return nil }
        do {
            return try splitDocument(pdfBytes: data, ranges: ranges)
        } catch {
            errorMessage = describe(error)
            return nil
        }
    }

    func extractText() -> String? {
        guard let data else { return nil }
        do {
            return try toText(pdfBytes: data)
        } catch {
            errorMessage = describe(error)
            return nil
        }
    }

    func textRun(atPage page: UInt16, x: Float, y: Float) -> TextRun? {
        guard let data else { return nil }
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

    /// The overlay (if any) whose box contains a point. Hit-tests
    /// `fieldOverlays` directly rather than going through `boxContaining` —
    /// a signature field synthesized straight from its `FormField` rect
    /// (see `computeFieldOverlays`) has no entry in `detectedBoxes` at all,
    /// so it would never resolve to a click otherwise.
    func fieldOverlay(containingX x: Float, y: Float) -> FieldOverlayBox? {
        let tolerance: Float = 1.5
        return fieldOverlays
            .filter {
                x >= $0.box.x - tolerance && x <= $0.box.x + $0.box.width + tolerance
                    && y >= $0.box.y - tolerance && y <= $0.box.y + $0.box.height + tolerance
            }
            .min { $0.box.width * $0.box.height < $1.box.width * $1.box.height }
    }

    // MARK: - Saved signatures

    func saveSignature(pngData: Data, kind: SavedSignature.Kind) {
        let signature = SavedSignature(id: UUID(), pngData: pngData, kind: kind, createdAt: Date())
        savedSignatures.insert(signature, at: 0)
        persistSignature(signature)
    }

    func deleteSavedSignature(_ signature: SavedSignature) {
        savedSignatures.removeAll { $0.id == signature.id }
        try? FileManager.default.removeItem(at: signatureFile(for: signature.id))
        saveSignatureIndex()
    }

    // MARK: - Private

    private func refreshAfterLoad() {
        guard let data else { return }
        formFieldsList = (try? formFields(pdfBytes: data)) ?? []
        annotationsList = (try? listAnnotations(pdfBytes: data)) ?? []
        renderCurrentPage()
    }

    private func renderCurrentPage() {
        guard let document, let data else { return }
        // `PdfDocument.renderPage` only takes an integer DPI, but
        // `fitDPIForCurrentPage()` returns a fractional one — rounding once,
        // up front, and reusing that *exact* rounded value for both the
        // render call and `applyRenderedImage`'s pixel-to-point math is what
        // keeps field overlays aligned with the image actually on screen.
        // Previously the render call truncated the fractional DPI (via
        // `UInt32(dpi)`) while the overlay math kept the untruncated value,
        // so the two silently drifted apart — the actual cause of overlays
        // rendering slightly off from the boxes/fields they're highlighting.
        let dpi = (fitDPIForCurrentPage()).rounded()
        // Box detection is by far the heaviest per-page FFI call, and its
        // result is independent of zoom/DPI — so cache it and skip the
        // rescan on pure resizes (the common case during a window drag).
        // On a cache miss (first visit to this page), `pageView` renders
        // *and* scans boxes from a single bind + parse instead of two
        // separate ones (`renderPage` + `boxesOnPage`, each independently
        // binding PDFium and re-parsing the whole document from scratch) —
        // this is what made even a 1-page PDF slow to open and page
        // navigation slow to respond.
        do {
            if let cached = boxesCache[pageIndex] {
                let png = try document.renderPage(index: pageIndex, dpi: UInt32(dpi))
                applyRenderedImage(png, dpi: dpi)
                detectedBoxes = cached
            } else {
                let view = try pageView(pdfBytes: data, page: pageIndex, dpi: dpi)
                applyRenderedImage(view.png, dpi: dpi)
                boxesCache[pageIndex] = view.boxes
                detectedBoxes = view.boxes
            }
        } catch {
            errorMessage = describe(error)
        }
        fieldOverlays = computeFieldOverlays()
    }

    private func applyRenderedImage(_ png: Data, dpi: Float) {
        let image = NSImage(data: png)
        pageImage = image
        guard let image else { return }
        let ptsPerPixel = CGFloat(72.0 / dpi)
        pagePointSize = CGSize(
            width: image.size.width * ptsPerPixel,
            height: image.size.height * ptsPerPixel
        )
    }

    /// Pair each scanned box with the named field (if any) whose widget rect
    /// center falls inside it, so the canvas can tell a signature field from
    /// an ordinary one (Core UX Principles: never a text caret for signing).
    ///
    /// `boxesOnPage`'s vector-graphics scan can miss a real `AcroForm` field
    /// that has no closed/ruled box around it (e.g. a signature line that's
    /// just an underline) — since `FormField` now carries its own rect, any
    /// signature/initials field left unmatched still gets an overlay
    /// synthesized directly from that rect, so signing is never silently
    /// undiscoverable.
    private func computeFieldOverlays() -> [FieldOverlayBox] {
        let pageFields = formFieldsList.filter { $0.page == pageIndex }
        var matchedFieldNames = Set<String>()

        var overlays = detectedBoxes.map { box -> FieldOverlayBox in
            let tolerance: Float = 2
            let match = pageFields.first { field in
                let cx = field.x + field.width / 2
                let cy = field.y + field.height / 2
                return cx >= box.x - tolerance && cx <= box.x + box.width + tolerance
                    && cy >= box.y - tolerance && cy <= box.y + box.height + tolerance
            }
            if let match { matchedFieldNames.insert(match.name) }
            return FieldOverlayBox(box: box, signatureKind: match?.signatureKind ?? .none, fieldName: match?.name)
        }

        let unmatchedSignatureFields = pageFields.filter { $0.signatureKind != .none && !matchedFieldNames.contains($0.name) }
        overlays += unmatchedSignatureFields.map { field in
            FieldOverlayBox(
                box: DetectedBox(page: pageIndex, x: field.x, y: field.y, width: field.width, height: field.height),
                signatureKind: field.signatureKind,
                fieldName: field.name
            )
        }
        return overlays
    }

    /// The DPI that renders the current page as large as possible while
    /// still fitting entirely inside `viewportSize` — the engine-shared
    /// `fitToPageDpi` math (see `renderer::fit_to_page` in `pdfree-core`),
    /// so this shell's default zoom matches every other platform's.
    private func fitDPIForCurrentPage() -> Float {
        guard viewportSize.width > 0, viewportSize.height > 0,
              let size = cachedPageSize(pageIndex)
        else { return fallbackDPI }

        let dpi = fitToPageDpi(
            pageWidthPts: Float(size.width),
            pageHeightPts: Float(size.height),
            viewportWidthPx: Float(viewportSize.width),
            viewportHeightPx: Float(viewportSize.height)
        )
        return dpi > 0 ? dpi : fallbackDPI
    }

    /// The page's point size, cached so a resize doesn't re-cross the FFI on
    /// every intermediate drag size just to recompute fit-to-page DPI.
    private func cachedPageSize(_ index: UInt16) -> CGSize? {
        if let cached = pageSizeCache[index] { return cached }
        guard let document, let size = try? document.pageSize(index: index) else { return nil }
        let cgSize = CGSize(width: CGFloat(size.width), height: CGFloat(size.height))
        pageSizeCache[index] = cgSize
        return cgSize
    }

    /// Called by the canvas view on load and on every resize so the default
    /// zoom keeps fitting the whole page in the viewport (Core UX Principles:
    /// "never open zoomed in", "recompute on resize").
    func updateViewport(_ size: CGSize) {
        // Ignore no-op / negligible changes so window-drag resize doesn't
        // re-render on every intermediate pixel.
        guard abs(size.width - viewportSize.width) > 1 || abs(size.height - viewportSize.height) > 1
        else { return }
        viewportSize = size
        renderCurrentPage()
    }

    private func describe(_ error: Error) -> String {
        (error as? LocalizedError)?.errorDescription ?? "\(error)"
    }

    // MARK: - Recent files

    private func loadRecentFiles() {
        let paths = UserDefaults.standard.stringArray(forKey: Self.recentFilesKey) ?? []
        recentFiles = paths
            .map { URL(fileURLWithPath: $0) }
            .filter { FileManager.default.fileExists(atPath: $0.path) }
    }

    private func rememberRecent(_ url: URL?) {
        guard let url else { return }
        var files = recentFiles.filter { $0 != url }
        files.insert(url, at: 0)
        recentFiles = Array(files.prefix(5))
        UserDefaults.standard.set(recentFiles.map(\.path), forKey: Self.recentFilesKey)
    }

    // MARK: - Signature persistence

    /// `~/Library/Application Support/PDFree/signatures/` — a PNG per saved
    /// mark plus a small JSON index, loaded on launch so the returning-user
    /// popover (see `SignPopover`) appears from the first saved signature on.
    private var signaturesDirectory: URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
            .appendingPathComponent("PDFree/signatures", isDirectory: true)
        try? FileManager.default.createDirectory(at: base, withIntermediateDirectories: true)
        return base
    }

    private var signatureIndexURL: URL { signaturesDirectory.appendingPathComponent("index.json") }

    private func signatureFile(for id: UUID) -> URL {
        signaturesDirectory.appendingPathComponent("\(id.uuidString).png")
    }

    private struct SignatureIndexEntry: Codable {
        let id: UUID
        let kind: String
        let createdAt: Date
    }

    private func loadSavedSignatures() {
        guard let indexData = try? Data(contentsOf: signatureIndexURL),
              let entries = try? JSONDecoder().decode([SignatureIndexEntry].self, from: indexData)
        else { return }
        savedSignatures = entries.compactMap { entry in
            guard let png = try? Data(contentsOf: signatureFile(for: entry.id)),
                  let kind = SavedSignature.Kind(rawValue: entry.kind)
            else { return nil }
            return SavedSignature(id: entry.id, pngData: png, kind: kind, createdAt: entry.createdAt)
        }
    }

    private func persistSignature(_ signature: SavedSignature) {
        try? signature.pngData.write(to: signatureFile(for: signature.id))
        saveSignatureIndex()
    }

    private func saveSignatureIndex() {
        let entries = savedSignatures.map {
            SignatureIndexEntry(id: $0.id, kind: $0.kind.rawValue, createdAt: $0.createdAt)
        }
        if let indexData = try? JSONEncoder().encode(entries) {
            try? indexData.write(to: signatureIndexURL)
        }
    }

    /// A blank, opaque-white 612x792pt (US Letter @ 72dpi) page image, used
    /// by `insertBlankPage` — `pdfree-core` has no dedicated "blank page"
    /// primitive, but `convert::from_image` already turns any image into a
    /// single-page PDF sized to it, so a plain white PNG gets the same result.
    private static func blankPagePNG() -> Data {
        let size = NSSize(width: 612, height: 792)
        let image = NSImage(size: size)
        image.lockFocus()
        NSColor.white.setFill()
        NSRect(origin: .zero, size: size).fill()
        image.unlockFocus()
        guard let tiff = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:])
        else { return Data() }
        return png
    }
}
