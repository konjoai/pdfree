import AppKit
import Foundation

/// One fillable field to draw on the canvas, from `pdfree-core`'s label-aware
/// `fillable_fields` scan: either a real `AcroForm` widget or a box/line kept
/// because it had a nearby text label. Carries the signature/initials
/// classification (so the canvas styles signing fields distinctly and never
/// opens a text caret on them — Core UX Principle #3) and the matched label
/// (for tooltip/accessibility text). The classification comes from the engine,
/// not a Swift-side heuristic, so every shell (macOS/web/Tauri/iOS) agrees.
struct FieldOverlayBox: Identifiable {
    let id = UUID()
    let box: DetectedBox
    let signatureKind: SignatureFieldKind
    /// The `AcroForm` field name backing this overlay, when it came from a
    /// real widget (`nil` for a label-detected box on a flat form).
    let fieldName: String?
    /// The human-readable label the field was matched to — shown as the
    /// overlay's accessibility/tooltip text so an icon-only affordance always
    /// announces what it's for (CLAUDE.md UX research on labelless controls).
    let label: String?

    var isSignature: Bool { signatureKind != .none }
}

enum PageViewMode {
    case single
    case continuous
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
    /// Bumped only by explicit page navigation (`goToPage` and everything
    /// that calls it — thumbnail taps, search jumps, outline taps, prev/next).
    /// Continuous-scroll's own scroll-position tracking sets `pageIndex`
    /// directly without going through `goToPage`, so it never bumps this —
    /// that's what lets `ContinuousScrollView` tell "the user asked to jump
    /// to page N" apart from "scrolling revealed that page N is now on
    /// screen" and only react to the former (reacting to both would fight
    /// the user's own scroll, snapping back to a page's top mid-drag).
    @Published private(set) var pageJumpToken = 0
    @Published var pageImage: NSImage?
    @Published var pagePointSize: CGSize = .zero
    /// Single-page (paged, always fit-to-page-on-load) or continuous-scroll
    /// (every page stacked vertically, fit-to-width). See CLAUDE.md's
    /// continuous-scroll research note — this mode is strictly additive: it
    /// doesn't change single-page mode's own fit-to-page-on-load behavior.
    @Published var pageViewMode: PageViewMode = .single
    @Published var formFieldsList: [FormField] = []
    @Published var annotationsList: [AnnotationInfo] = []
    /// The label-aware fillable fields on the current page (see
    /// `pdfree_core::fields::fillable_fields`) — every `AcroForm` widget plus
    /// every label-detected box — computed once per page load and presented
    /// up front. This is exactly what `PageCanvasView` draws.
    @Published var fieldOverlays: [FieldOverlayBox] = []
    /// The document's outline/table-of-contents tree, if it has one — empty
    /// for the common case of a PDF with no bookmarks. Loaded once per
    /// document alongside `formFieldsList`/`annotationsList`.
    @Published private(set) var documentOutline: [Bookmark] = []
    @Published var errorMessage: String?
    @Published var isBusy = false
    @Published var fileURL: URL?
    @Published private(set) var recentFiles: [URL] = []
    @Published private(set) var savedSignatures: [SavedSignature] = []
    /// Every run matching the current search query, in page order — see
    /// `search(query:)`. Empty whenever the search bar is closed or the
    /// query has no matches.
    @Published private(set) var searchMatches: [SearchMatch] = []
    /// Index into `searchMatches` of the currently highlighted/jumped-to
    /// match, or `nil` when there are no matches (including "haven't
    /// searched yet").
    @Published var currentSearchMatchIndex: Int?
    /// Whether `undo()`/`redo()` currently have a snapshot to restore —
    /// drives the Edit-menu/keyboard-shortcut enablement.
    @Published private(set) var canUndo = false
    @Published private(set) var canRedo = false

    /// Bumped on every `search(query:)` call. A background search result
    /// whose captured token no longer matches is stale (a newer search, or a
    /// document change, superseded it) and is dropped.
    private var searchToken = 0

    /// Whole-document byte snapshots for undo/redo. Every `pdfree-core`
    /// mutation already takes whole-document bytes in and returns
    /// whole-document bytes out (see `mutate()`), so a bounded stack of past
    /// snapshots is the natural fit — no rope/operation-log machinery needed,
    /// since the engine never mutates incrementally in place. Capped at
    /// `maxUndoDepth` entries; realistic document sizes (a few MB) make even
    /// that many snapshots a non-issue in memory.
    private var undoStack: [Data] = []
    private var redoStack: [Data] = []
    private let maxUndoDepth = 20

    private var thumbnailCache: [UInt16: NSImage] = [:]
    /// Thumbnail page indices with a background render in flight, so the same
    /// page isn't queued twice while the sidebar re-renders.
    private var pendingThumbnails: Set<UInt16> = []
    /// Continuous-scroll mode's per-page images — fit-to-width (not
    /// fit-to-page, since these stack vertically and scroll), separate from
    /// `thumbnailCache`'s low-res sidebar renders. Populated lazily as
    /// `ContinuousScrollView`'s `LazyVStack` brings each page on screen.
    private var continuousImageCache: [UInt16: NSImage] = [:]
    private var pendingContinuousImages: Set<UInt16> = []
    /// Per-page fillable-field scan and page-size results, cached because
    /// neither changes when only the zoom/DPI changes — so a window resize
    /// re-renders the image without re-running the (expensive) field scan.
    /// Cleared on open and on every mutation, since those can change geometry.
    private var overlaysCache: [UInt16: [FieldOverlayBox]] = [:]
    private var pageSizeCache: [UInt16: CGSize] = [:]

    /// Serial queue every `PDFium`-backed FFI call runs on, off the main
    /// thread — so opening, rendering, scanning, and mutating never freeze the
    /// UI. It's *serial* on purpose: `PDFium` isn't safe to bind/drive from two
    /// threads at once (see `pages.rs`' "never two live bindings" note), so
    /// operations are queued rather than run concurrently.
    private let ffiQueue = DispatchQueue(label: "ai.konjo.pdfree.ffi", qos: .userInitiated)
    /// Bumped whenever the document itself changes (open/mutate/close). A
    /// background result whose captured token no longer matches is stale and
    /// dropped, so a slow load can't clobber a newer document's state.
    private var docToken = 0
    /// Bumped on every render request (page change, resize, open, mutate).
    /// Same staleness guard as `docToken`, but for the page image/overlays —
    /// rapid page flips apply only the newest render's result, no flicker.
    private var renderToken = 0
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

    /// Open a document. The parse happens off the main thread so the picker
    /// dismissing and the window staying responsive don't wait on it; the
    /// currently-open document (if any) stays on screen until the new one is
    /// ready, and a parse failure surfaces an error without tearing it down.
    /// `viewportSize` is deliberately *not* reset here — it's a property of
    /// the canvas, which doesn't change just because the document did, so
    /// keeping it means the first render already fits-to-page instead of
    /// briefly falling back to a fixed DPI.
    func openReplacing(data: Data, url: URL?) {
        isBusy = true
        docToken += 1
        let token = docToken
        ffiQueue.async {
            do {
                let doc = try PdfDocument.fromBytes(data: data)
                DispatchQueue.main.async {
                    guard token == self.docToken else { return }
                    self.data = data
                    self.document = doc
                    self.fileURL = url
                    self.pageIndex = 0
                    self.pageImage = nil
                    self.fieldOverlays = []
                    self.formFieldsList = []
                    self.annotationsList = []
                    self.documentOutline = []
                    self.thumbnailCache.removeAll()
                    self.pendingThumbnails.removeAll()
                    self.continuousImageCache.removeAll()
                    self.pendingContinuousImages.removeAll()
                    self.overlaysCache.removeAll()
                    self.pageSizeCache.removeAll()
                    self.clearSearch()
                    self.clearUndoHistory()
                    self.isBusy = false
                    self.rememberRecent(url)
                    self.refreshAfterLoad()
                }
            } catch {
                DispatchQueue.main.async {
                    guard token == self.docToken else { return }
                    self.isBusy = false
                    self.errorMessage = self.describe(error)
                }
            }
        }
    }

    /// Back to the empty state.
    func closeDocument() {
        docToken += 1
        renderToken += 1
        data = nil
        document = nil
        pageImage = nil
        pagePointSize = .zero
        formFieldsList = []
        annotationsList = []
        fieldOverlays = []
        documentOutline = []
        fileURL = nil
        pageIndex = 0
        isBusy = false
        thumbnailCache.removeAll()
        pendingThumbnails.removeAll()
        continuousImageCache.removeAll()
        pendingContinuousImages.removeAll()
        overlaysCache.removeAll()
        pageSizeCache.removeAll()
        clearSearch()
        clearUndoHistory()
        viewportSize = .zero
    }

    func goToPage(_ index: UInt16) {
        guard index < pageCount else { return }
        pageIndex = index
        pageJumpToken += 1
        renderCurrentPage()
    }

    /// A page thumbnail. Returns the cached image immediately if present;
    /// otherwise kicks a background render on `ffiQueue` (never `PDFium` on the
    /// main thread) and returns `nil` for now — the sidebar refreshes once the
    /// render lands in the cache. In-flight renders are deduped so scrolling
    /// the rail doesn't queue the same page repeatedly.
    func thumbnail(at index: UInt16) -> NSImage? {
        if let cached = thumbnailCache[index] { return cached }
        guard let data, !pendingThumbnails.contains(index) else { return nil }
        pendingThumbnails.insert(index)
        let token = docToken
        let dpi = thumbnailDPI
        ffiQueue.async {
            let png = try? renderPage(pdfBytes: data, index: index, dpi: UInt32(dpi))
            let image = png.flatMap { NSImage(data: $0) }
            DispatchQueue.main.async {
                self.pendingThumbnails.remove(index)
                guard token == self.docToken, let image else { return }
                self.thumbnailCache[index] = image
                // thumbnailCache isn't @Published (it's a plain cache), so nudge
                // observers to re-read it now that this page is ready.
                self.objectWillChange.send()
            }
        }
        return nil
    }

    /// A page image for continuous-scroll mode, fit to `viewportWidth` rather
    /// than fit-to-page (these stack vertically and scroll, so only the width
    /// needs to match the column). Same lazy/cached/deduped shape as
    /// `thumbnail(at:)`. Also opportunistically fills `pageSizeCache` so
    /// `ContinuousPageRow`'s placeholder sizing has a real aspect ratio as
    /// soon as any page's size has been read once.
    func continuousPageImage(at index: UInt16, viewportWidth: CGFloat) -> NSImage? {
        if let cached = continuousImageCache[index] { return cached }
        guard let data, viewportWidth > 0, !pendingContinuousImages.contains(index) else { return nil }
        pendingContinuousImages.insert(index)
        let token = docToken
        let cachedSize = pageSizeCache[index]
        let fallback = fallbackDPI
        ffiQueue.async {
            let size = cachedSize ?? (try? pageSize(pdfBytes: data, index: index))
                .map { CGSize(width: CGFloat($0.width), height: CGFloat($0.height)) }
            let dpi = Self.fitWidthDPI(pageWidth: size?.width, viewportWidth: viewportWidth, fallback: fallback).rounded()
            let png = try? renderPage(pdfBytes: data, index: index, dpi: UInt32(dpi))
            let image = png.flatMap { NSImage(data: $0) }
            DispatchQueue.main.async {
                self.pendingContinuousImages.remove(index)
                guard token == self.docToken else { return }
                if let size { self.pageSizeCache[index] = size }
                guard let image else { return }
                self.continuousImageCache[index] = image
                self.objectWillChange.send()
            }
        }
        return nil
    }

    /// The DPI that renders a page at exactly `viewportWidth` wide — the
    /// fit-width counterpart to `fitDPI`'s fit-both-dimensions math, used by
    /// continuous-scroll mode where height is free to scroll off-screen.
    private static func fitWidthDPI(pageWidth: CGFloat?, viewportWidth: CGFloat, fallback: Float) -> Float {
        guard let pageWidth, pageWidth > 0, viewportWidth > 0 else { return fallback }
        let dpi = Float(viewportWidth) / Float(pageWidth) * 72
        return dpi > 0 ? dpi : fallback
    }

    /// The cached PDF-point page size, if known — `ContinuousPageRow` uses
    /// this for placeholder aspect ratio before its own image has loaded.
    func cachedPageSize(at index: UInt16) -> CGSize? {
        pageSizeCache[index]
    }

    // MARK: - Mutations

    /// Apply an operation that transforms the current bytes into new bytes,
    /// then reload every derived piece of state (document handle, thumbnails,
    /// current page render, form fields, annotations) from the result.
    ///
    /// The transform *and* the reparse run off the main thread on `ffiQueue`,
    /// so a heavy op (merge, sign, big overlay) doesn't freeze the UI — the
    /// busy spinner shows meanwhile, and the result is published on main.
    func mutate(_ label: String, _ op: @escaping (Data) throws -> Data) {
        guard let data else { return }
        isBusy = true
        docToken += 1
        renderToken += 1
        let token = docToken
        ffiQueue.async {
            do {
                let newData = try op(data)
                let newDoc = try PdfDocument.fromBytes(data: newData)
                let pageCount = newDoc.pageCount()
                DispatchQueue.main.async {
                    guard token == self.docToken else { return }
                    // Every successful mutation is a new undo checkpoint — the
                    // bytes just about to be replaced are what a subsequent
                    // undo() restores. A fresh edit also forks away from
                    // whatever redo history existed (standard editor
                    // behavior: redo only replays undos, not arbitrary future
                    // states).
                    self.pushUndoSnapshot(data)
                    self.redoStack.removeAll()
                    self.data = newData
                    self.document = newDoc
                    self.thumbnailCache.removeAll()
                    self.pendingThumbnails.removeAll()
                    self.continuousImageCache.removeAll()
                    self.pendingContinuousImages.removeAll()
                    self.overlaysCache.removeAll()
                    self.pageSizeCache.removeAll()
                    // A mutation can shift or remove the very text a search
                    // match's bounding box pointed at — stale highlights are
                    // worse than none, so clear rather than risk that.
                    self.clearSearch()
                    self.updateUndoRedoFlags()
                    if self.pageIndex >= pageCount {
                        self.pageIndex = pageCount > 0 ? pageCount - 1 : 0
                    }
                    self.isBusy = false
                    self.refreshAfterLoad()
                }
            } catch {
                DispatchQueue.main.async {
                    guard token == self.docToken else { return }
                    self.isBusy = false
                    self.errorMessage = "\(label) failed: \(self.describe(error))"
                }
            }
        }
    }

    // MARK: - Undo / redo

    /// Reverts to the byte snapshot just before the last mutation, if any.
    func undo() {
        guard let previous = undoStack.popLast(), let current = data else { return }
        redoStack.append(current)
        updateUndoRedoFlags()
        restoreSnapshot(previous)
    }

    /// Re-applies the mutation just undone, if any.
    func redo() {
        guard let next = redoStack.popLast(), let current = data else { return }
        undoStack.append(current)
        updateUndoRedoFlags()
        restoreSnapshot(next)
    }

    private func pushUndoSnapshot(_ snapshot: Data) {
        undoStack.append(snapshot)
        if undoStack.count > maxUndoDepth {
            undoStack.removeFirst(undoStack.count - maxUndoDepth)
        }
    }

    private func updateUndoRedoFlags() {
        canUndo = !undoStack.isEmpty
        canRedo = !redoStack.isEmpty
    }

    private func clearUndoHistory() {
        undoStack.removeAll()
        redoStack.removeAll()
        canUndo = false
        canRedo = false
    }

    /// Reloads document state from an undo/redo snapshot — the same reload
    /// shape as `mutate()`'s success path, minus running a transform and minus
    /// touching the undo/redo stacks themselves (the caller already did that).
    private func restoreSnapshot(_ newData: Data) {
        isBusy = true
        docToken += 1
        renderToken += 1
        let token = docToken
        ffiQueue.async {
            guard let newDoc = try? PdfDocument.fromBytes(data: newData) else { return }
            let pageCount = newDoc.pageCount()
            DispatchQueue.main.async {
                guard token == self.docToken else { return }
                self.data = newData
                self.document = newDoc
                self.thumbnailCache.removeAll()
                self.pendingThumbnails.removeAll()
                self.continuousImageCache.removeAll()
                self.pendingContinuousImages.removeAll()
                self.overlaysCache.removeAll()
                self.pageSizeCache.removeAll()
                self.clearSearch()
                if self.pageIndex >= pageCount {
                    self.pageIndex = pageCount > 0 ? pageCount - 1 : 0
                }
                self.isBusy = false
                self.refreshAfterLoad()
            }
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

    // These read-only FFI queries run on `ffiQueue` and call back on main,
    // rather than blocking the caller — both to stay responsive and, just as
    // importantly, so they never drive `PDFium` on the main thread while a
    // render/scan is in flight on the queue (two concurrent bindings are
    // unsafe; see `ffiQueue`'s note).

    func splitExport(ranges: [PageRange], completion: @escaping ([Data]?) -> Void) {
        guard let data else { completion(nil); return }
        ffiQueue.async {
            do {
                let pieces = try splitDocument(pdfBytes: data, ranges: ranges)
                DispatchQueue.main.async { completion(pieces) }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = self.describe(error)
                    completion(nil)
                }
            }
        }
    }

    /// Password-protect the current document for export, off the main
    /// thread — this shells out to `qpdf` (see `pdfree_core::encrypt`'s
    /// module doc comment for why `PDFium` itself can't do this), so it
    /// never mutates `self.data`/the open document, only ever hands back a
    /// separate encrypted byte buffer for the caller to save.
    func exportEncrypted(password: String, completion: @escaping (Data?) -> Void) {
        guard let data else { completion(nil); return }
        ffiQueue.async {
            do {
                let encrypted = try encryptDocument(pdfBytes: data, userPassword: password, ownerPassword: nil)
                DispatchQueue.main.async { completion(encrypted) }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = self.describe(error)
                    completion(nil)
                }
            }
        }
    }

    /// The result of `extractDocumentText`: either the document's real,
    /// embedded text layer, or — when there wasn't one — OCR output for just
    /// the current page.
    enum TextExtractionResult {
        case documentText(String)
        case ocrCurrentPage(String)
    }

    /// Extracts the document's real text layer; if that comes back empty (a
    /// strong signal this is a scanned image with no text layer at all),
    /// falls back to running OCR on the current page's rendered image
    /// instead. Without this fallback, "Extract Text" on a scanned PDF would
    /// just silently return nothing — the whole point of OCR support.
    /// Scoped to the current page for the OCR path (not the whole document)
    /// since OCR-ing every page synchronously on demand would be slow for a
    /// large scan; the current page is what's already on screen.
    func extractDocumentText(ocrLanguage: String = "eng", completion: @escaping (TextExtractionResult?) -> Void) {
        guard let data else { completion(nil); return }
        let page = pageIndex
        ffiQueue.async {
            let text = (try? toText(pdfBytes: data)) ?? nil
            if let text, !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                DispatchQueue.main.async { completion(.documentText(text)) }
                return
            }
            guard let png = try? renderPage(pdfBytes: data, index: page, dpi: 300) else {
                DispatchQueue.main.async {
                    self.errorMessage = "No text found in this document."
                    completion(nil)
                }
                return
            }
            do {
                let ocrText = try aiOcrRecognize(pagePng: png, language: ocrLanguage)
                DispatchQueue.main.async { completion(.ocrCurrentPage(ocrText)) }
            } catch {
                DispatchQueue.main.async {
                    self.errorMessage = self.describe(error)
                    completion(nil)
                }
            }
        }
    }

    func textRun(atPage page: UInt16, x: Float, y: Float, completion: @escaping (TextRun?) -> Void) {
        guard let data else { completion(nil); return }
        ffiQueue.async {
            let run = (try? textRunAtPoint(pdfBytes: data, page: page, x: x, y: y)) ?? nil
            DispatchQueue.main.async { completion(run) }
        }
    }

    /// Search the whole document for `query` (case-insensitive), off the
    /// main thread. An empty query just clears any existing results rather
    /// than round-tripping through the FFI for nothing. On success, jumps to
    /// the first match's page automatically — same "search should just take
    /// you there" expectation as Preview/Safari's find bar.
    func search(query: String) {
        guard let data, !query.isEmpty else {
            searchMatches = []
            currentSearchMatchIndex = nil
            return
        }
        searchToken += 1
        let token = searchToken
        ffiQueue.async {
            let matches = (try? findText(pdfBytes: data, query: query, caseSensitive: false)) ?? []
            DispatchQueue.main.async {
                guard token == self.searchToken else { return }
                self.searchMatches = matches
                self.currentSearchMatchIndex = matches.isEmpty ? nil : 0
                if let first = matches.first, first.page != self.pageIndex {
                    self.goToPage(first.page)
                }
            }
        }
    }

    /// Clear search results without touching the query text itself — used
    /// when the search bar closes, so its highlight disappears immediately.
    func clearSearch() {
        searchToken += 1
        searchMatches = []
        currentSearchMatchIndex = nil
    }

    /// Advance to the next (or, wrapping, the first) match and jump to its
    /// page if it's not the current one.
    func goToNextSearchMatch() {
        guard !searchMatches.isEmpty else { return }
        let next = ((currentSearchMatchIndex ?? -1) + 1) % searchMatches.count
        currentSearchMatchIndex = next
        let match = searchMatches[next]
        if match.page != pageIndex { goToPage(match.page) }
    }

    /// Step back to the previous (or, wrapping, the last) match and jump to
    /// its page if it's not the current one.
    func goToPreviousSearchMatch() {
        guard !searchMatches.isEmpty else { return }
        let previous = ((currentSearchMatchIndex ?? 0) - 1 + searchMatches.count) % searchMatches.count
        currentSearchMatchIndex = previous
        let match = searchMatches[previous]
        if match.page != pageIndex { goToPage(match.page) }
    }

    /// The smallest already-scanned field box enclosing a point, if any —
    /// used to resolve a double-click into a specific box (snapping to a
    /// detected field) without another FFI round trip. Falls through to a
    /// fixed-size box in the caller when nothing here contains the point.
    func boxContaining(x: Float, y: Float) -> DetectedBox? {
        let tolerance: Float = 1.5
        return fieldOverlays
            .map(\.box)
            .filter {
                x >= $0.x - tolerance && x <= $0.x + $0.width + tolerance
                    && y >= $0.y - tolerance && y <= $0.y + $0.height + tolerance
            }
            .min { $0.width * $0.height < $1.width * $1.height }
    }

    /// The smallest overlay (if any) whose box contains a point — the single
    /// hit-test the canvas uses to resolve a click into a field to fill or
    /// sign, over the label-aware `fieldOverlays` list.
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

    /// Reload the whole-document state (form-field and annotation lists, both
    /// needed cross-page for the sign hop and counts) off the main thread,
    /// then render the current page. Called on open and after every mutation.
    private func refreshAfterLoad() {
        guard let data else { return }
        let token = docToken
        ffiQueue.async {
            let fields = (try? formFields(pdfBytes: data)) ?? []
            let annos = (try? listAnnotations(pdfBytes: data)) ?? []
            let toc = (try? outline(pdfBytes: data)) ?? []
            DispatchQueue.main.async {
                guard token == self.docToken else { return }
                self.formFieldsList = fields
                self.annotationsList = annos
                self.documentOutline = toc
            }
        }
        renderCurrentPage()
    }

    /// Render the current page and compute its fillable-field overlays off the
    /// main thread, publishing both when ready. The DPI is read on the main
    /// thread (it depends on the live viewport), but the rasterization and the
    /// field scan — the heavy parts — run on `ffiQueue`. Overlays are cached
    /// per page (they don't depend on zoom), so a resize re-renders the image
    /// without re-scanning.
    private func renderCurrentPage() {
        guard let data else { return }
        let page = pageIndex
        let viewport = viewportSize
        let cachedSize = pageSizeCache[page]
        let cachedOverlays = overlaysCache[page]
        let fallback = fallbackDPI
        renderToken += 1
        let token = renderToken

        ffiQueue.async {
            // Fit-to-page DPI: read the page's point size (cached, or via a
            // bytes-based FFI call on this same queue — never `PDFium` on the
            // main thread) and fit it to the live viewport. Rounded once,
            // up front, and reused for both the render call (which only
            // takes an integer DPI) and the pixel-to-point math below —
            // previously the render call truncated the fractional DPI via
            // `UInt32(dpi)` while the point-size math kept the untruncated
            // value, so the two silently drifted apart and field overlays
            // rendered slightly offset from the boxes/fields they highlight.
            let size = cachedSize ?? (try? pageSize(pdfBytes: data, index: page))
                .map { CGSize(width: CGFloat($0.width), height: CGFloat($0.height)) }
            let dpi = Self.fitDPI(pageSize: size, viewport: viewport, fallback: fallback).rounded()

            let png = try? renderPage(pdfBytes: data, index: page, dpi: UInt32(dpi))
            let image = png.flatMap { NSImage(data: $0) }
            let overlays: [FieldOverlayBox]
            if let cachedOverlays {
                overlays = cachedOverlays
            } else {
                let scanned = (try? fillableFields(pdfBytes: data, page: page)) ?? []
                overlays = Self.overlays(from: scanned)
            }
            DispatchQueue.main.async {
                guard token == self.renderToken else { return }
                if let size { self.pageSizeCache[page] = size }
                if let image {
                    self.pageImage = image
                    let ptsPerPixel = CGFloat(72.0 / dpi)
                    self.pagePointSize = CGSize(
                        width: image.size.width * ptsPerPixel,
                        height: image.size.height * ptsPerPixel
                    )
                }
                self.overlaysCache[page] = overlays
                self.fieldOverlays = overlays
            }
        }
    }

    /// Map the engine's label-aware `FillableField`s into the canvas overlay
    /// type. Pure and thread-agnostic, so it can run on `ffiQueue`.
    private static func overlays(from fields: [FillableField]) -> [FieldOverlayBox] {
        fields.map { field in
            FieldOverlayBox(
                box: DetectedBox(
                    page: field.page, x: field.x, y: field.y,
                    width: field.width, height: field.height
                ),
                signatureKind: field.signatureKind,
                fieldName: field.fieldName,
                label: field.label
            )
        }
    }

    /// The DPI that renders a page as large as possible while still fitting
    /// entirely inside `viewport` — the engine-shared `fitToPageDpi` math (see
    /// `renderer::fit_to_page` in `pdfree-core`), so this shell's default zoom
    /// matches every other platform's. Pure/thread-agnostic so it can run on
    /// `ffiQueue`; falls back to `fallback` when the size or viewport is
    /// unknown.
    private static func fitDPI(pageSize size: CGSize?, viewport: CGSize, fallback: Float) -> Float {
        guard let size, size.width > 0, size.height > 0,
              viewport.width > 0, viewport.height > 0
        else { return fallback }

        let dpi = fitToPageDpi(
            pageWidthPts: Float(size.width),
            pageHeightPts: Float(size.height),
            viewportWidthPx: Float(viewport.width),
            viewportHeightPx: Float(viewport.height)
        )
        return dpi > 0 ? dpi : fallback
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
