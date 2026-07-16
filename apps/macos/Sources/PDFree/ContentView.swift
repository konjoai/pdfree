import AppKit
import PDFKit
import SwiftUI
import UniformTypeIdentifiers

/// One modal task at a time — a sheet, keyed so SwiftUI can present it.
private enum ActiveSheet: Identifiable {
    /// The sign sheet anchored to a specific detected field (from the sign
    /// session — first-time draw/type/upload, or "Draw new"/"Type"/"Upload"
    /// from the returning-user popover).
    case signatureField(FormField, SignatureSheet.Tab)
    /// Manual fallback: Sign tool clicked somewhere with no detected
    /// signature field under the point.
    case signaturePoint(CGPoint)
    /// A label-detected signature line on a flat form (no backing `AcroForm`
    /// widget): sign directly into this box's rect.
    case signatureBox(DetectedBox)
    case note(CGPoint)
    case editText(TextRun)
    case fillForm
    case extractedText(String, viaOCR: Bool)
    case splitRanges
    case aiAssistant
    case exportPassword

    var id: String {
        switch self {
        case .signatureField(let field, let tab): return "signatureField-\(field.name)-\(tab.rawValue)"
        case .signaturePoint: return "signaturePoint"
        case .signatureBox(let box): return "signatureBox-\(box.x)-\(box.y)"
        case .note: return "note"
        case .editText: return "editText"
        case .fillForm: return "fillForm"
        case .extractedText: return "extractedText"
        case .splitRanges: return "splitRanges"
        case .aiAssistant: return "aiAssistant"
        case .exportPassword: return "exportPassword"
        }
    }
}

/// Tracks a hop-through-signature-fields session: every signature/initials
/// field in the document (starting from whichever one was clicked), which
/// ones are already placed, and therefore which field the popover/sheet is
/// currently anchored on.
private struct SignSessionState {
    let fields: [FormField]
    var completedNames: Set<String> = []

    var currentField: FormField? { fields.first { !completedNames.contains($0.name) } }
    var done: Bool { !fields.isEmpty && completedNames.count >= fields.count }
    var progress: (current: Int, total: Int) { (min(completedNames.count + 1, fields.count), fields.count) }
}

struct ContentView: View {
    @StateObject private var store = PDFDocumentStore()
    @State private var tool: Tool = .select
    @State private var activeSheet: ActiveSheet?
    @State private var inlineEditBox: DetectedBox?
    @State private var inlineEditText: String = ""
    @State private var signSession: SignSessionState?
    /// The last field a sign session was anchored on — kept around so the
    /// "done" popover has somewhere to stay pinned once `currentField`
    /// becomes nil (everything placed).
    @State private var lastSignAnchorField: FormField?
    /// True for a couple of seconds right after a signature is placed — the
    /// user should see their freshly placed mark clearly, with no "Sign
    /// here" box or popover in the way, before the session hops to the next
    /// field. Cleared by `commitPlacement`'s delayed advance.
    @State private var isPausingAfterPlacement = false
    /// Names of signature/initials fields whose "Sign here" affordance
    /// should stay hidden — set the instant a placement commits (so it
    /// disappears immediately, not just once the session formally advances
    /// past it) and cleared once the delayed advance folds it into
    /// `signSession.completedNames`.
    @State private var justSignedFieldNames: Set<String> = []
    @State private var isSearchVisible = false
    @State private var searchQuery = ""
    @FocusState private var isSearchFieldFocused: Bool

    /// Fallback box size (PDF points) when double-clicking finds no drawn
    /// rectangle or ruled-line cell to snap to.
    private static let defaultBoxSize = CGSize(width: 140, height: 18)

    var body: some View {
        VStack(spacing: 0) {
            titlebar
            Rectangle().fill(Color.black.opacity(0.4)).frame(height: 1)

            if !store.hasDocument {
                EmptyStateView(store: store, onOpen: openDocument)
            } else {
                HSplitView {
                    PagesSidebarView(store: store)
                        .frame(minWidth: Theme.Metric.railWidth, idealWidth: Theme.Metric.railWidth, maxWidth: Theme.Metric.railWidth)
                    canvasArea
                        .frame(minWidth: 420)
                    InspectorView(
                        store: store, tool: $tool,
                        onOpen: openDocument, onMerge: mergeDocument,
                        onInsertBlankPage: { store.insertBlankPage() }, onInsertImagePage: insertImage,
                        onSplit: { activeSheet = .splitRanges },
                        onRotate: { store.rotate(page: store.pageIndex, rotation: .clockwise90) },
                        onDelete: { store.deletePage(store.pageIndex) },
                        onExport: exportDocument,
                        onExportPasswordProtected: { activeSheet = .exportPassword },
                        onPrint: printDocument,
                        onUndo: { store.undo() }, onRedo: { store.redo() },
                        onSelectSign: { beginSigning(from: nil) },
                        onAskAI: { activeSheet = .aiAssistant },
                        onExtractText: extractDocumentText
                    )
                    .frame(minWidth: Theme.Metric.inspectorWidth, idealWidth: Theme.Metric.inspectorWidth, maxWidth: Theme.Metric.inspectorWidth)
                }
            }
        }
        .frame(minWidth: 900, minHeight: 640)
        .background(Theme.Color.panelBg)
        .preferredColorScheme(.dark)
        .onChange(of: store.pageIndex) { _ in commitInlineEdit() }
        .onChange(of: tool) { _ in commitInlineEdit() }
        .onChange(of: searchQuery) { store.search(query: $0) }
        .sheet(item: $activeSheet) { sheet in sheetView(for: sheet) }
        .alert("Error", isPresented: errorBinding) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(store.errorMessage ?? "")
        }
        .overlay(alignment: .top) {
            if store.isBusy {
                ProgressView().padding(6).background(.regularMaterial, in: Capsule())
                    .padding(.top, Theme.Metric.titlebarHeight + 8)
            }
        }
    }

    // MARK: - Titlebar

    private var titlebar: some View {
        ZStack {
            LinearGradient(colors: [Theme.Color.titlebarTop, Theme.Color.titlebarBottom], startPoint: .top, endPoint: .bottom)

            // Centered document title, padded clear of the corner marks.
            if store.hasDocument {
                Text(store.title)
                    .font(Theme.Font.titlebarTitle)
                    .foregroundStyle(Theme.Color.textRow)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .padding(.horizontal, 210)
            }

            HStack {
                Spacer()
                // Text wordmark only, centered over the inspector (tools)
                // column on the right — the document mark icon already
                // carries the branding on the empty-state hero, so repeating
                // it here would just be duplicate chrome.
                Wordmark(size: .small)
                    .frame(width: Theme.Metric.inspectorWidth)
            }
        }
        .frame(height: Theme.Metric.titlebarHeight)
    }

    // MARK: - Canvas

    private static let canvasPadding: CGFloat = Theme.Metric.canvasPagePadding

    @ViewBuilder
    private var canvasArea: some View {
        GeometryReader { geo in
            ZStack {
                RadialGradient(
                    colors: [Theme.Color.canvasTop, Theme.Color.canvasBottom],
                    center: .init(x: 0.5, y: 0), startRadius: 1, endRadius: 900
                )

                if store.hasDocument, store.pageViewMode == .continuous {
                    ContinuousScrollView(
                        store: store,
                        viewportWidth: max(geo.size.width - Self.canvasPadding * 2, 0)
                    )
                    .padding(.horizontal, Self.canvasPadding)

                    searchTriggerChip
                    pageNavBar
                } else if let image = store.pageImage {
                    ScrollView([.horizontal, .vertical]) {
                        PageCanvasView(
                            image: image,
                            pagePointSize: store.pagePointSize,
                            tool: tool,
                            fieldOverlays: visibleFieldOverlays,
                            onTap: handleTap,
                            onDrag: handleDrag,
                            onFreehandDrag: handleFreehandDrag,
                            onDoubleTap: handleDoubleTap,
                            inlineEditBox: inlineEditBox,
                            inlineEditText: $inlineEditText,
                            onCommitInlineEdit: commitInlineEdit,
                            onCancelInlineEdit: cancelInlineEdit,
                            signAnchorBox: signAnchorBox,
                            signOverlay: signOverlayView,
                            onSignBackgroundTap: (signSession?.done == true) ? dismissSign : nil,
                            searchHighlightBox: searchHighlightBox
                        )
                        .shadow(color: .black.opacity(0.55), radius: 25, y: 18)
                        .padding(Self.canvasPadding)
                    }
                    // Recreates the ScrollView (and resets its scroll offset)
                    // whenever the document or page changes — otherwise a
                    // stale offset from the previous page/document can leave
                    // the new one scrolled off-center even though it fits.
                    .id("\(store.fileURL?.path ?? "") #\(store.pageIndex)")

                    fieldCountChip
                    searchTriggerChip
                    pageNavBar
                } else {
                    ProgressView("Loading…").tint(.white)
                }
            }
            .overlay {
                // Scroll/swipe over the canvas turns pages (pass-through, so
                // clicks and drags still reach the page). Continuous-scroll
                // mode has its own native ScrollView handling scroll input
                // directly — layering this on top would fight it.
                if store.hasDocument, store.pageViewMode == .single {
                    ScrollPageFlipper(
                        onNext: { store.goToPage(store.pageIndex &+ 1) },
                        onPrev: { if store.pageIndex > 0 { store.goToPage(store.pageIndex &- 1) } }
                    )
                }
            }
            .overlay(alignment: .trailing) {
                if store.hasDocument, tool.isAnnotation {
                    AnnotationToolbar(tool: $tool)
                        .padding(.trailing, 16)
                        .transition(.move(edge: .trailing).combined(with: .opacity))
                }
            }
            .overlay(alignment: .top) {
                if isSearchVisible {
                    SearchBar(store: store, query: $searchQuery, isFocused: $isSearchFieldFocused, onClose: closeSearch)
                        .padding(.top, 14)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
            }
            .animation(.easeInOut(duration: 0.16), value: tool.isAnnotation)
            .animation(.easeInOut(duration: 0.15), value: isSearchVisible)
            .onAppear { updateViewport(geo.size) }
            .onChange(of: geo.size) { updateViewport($0) }
        }
    }

    /// Small circular trigger, mirroring `fieldCountChip`'s corner on the
    /// opposite side — carries the actual ⌘F shortcut, so the shortcut
    /// works as soon as a document is open without needing prior focus
    /// anywhere in particular.
    private var searchTriggerChip: some View {
        VStack {
            HStack {
                Spacer()
                Button {
                    isSearchVisible = true
                    isSearchFieldFocused = true
                } label: {
                    Image(systemName: "magnifyingglass")
                        .font(.system(size: 12, weight: .semibold))
                        .foregroundStyle(Theme.Color.textMid2)
                        .padding(9)
                        .background(.ultraThinMaterial, in: Circle())
                        .overlay(Circle().stroke(Color.white.opacity(0.09)))
                }
                .buttonStyle(.plain)
                .keyboardShortcut("f", modifiers: .command)
                .help("Find in Document (⌘F)")
                .padding(18)
            }
            Spacer()
        }
    }

    private func closeSearch() {
        isSearchVisible = false
        searchQuery = ""
        store.clearSearch()
    }

    /// The current match's box, converted for `PageCanvasView`, but only
    /// when that match is actually on the page being displayed right now.
    private var searchHighlightBox: DetectedBox? {
        guard let index = store.currentSearchMatchIndex, index < store.searchMatches.count else { return nil }
        let match = store.searchMatches[index]
        guard match.page == store.pageIndex else { return nil }
        return DetectedBox(page: match.page, x: match.x, y: match.y, width: match.width, height: match.height)
    }

    private var fieldCountChip: some View {
        VStack {
            HStack {
                if !store.fieldOverlays.isEmpty {
                    HStack(spacing: 7) {
                        Circle().fill(Theme.Color.green).frame(width: 6, height: 6)
                        Text(fieldCountLabel)
                    }
                    .font(Theme.Font.overlayChip)
                    .foregroundStyle(Theme.Color.greenChipText)
                    .padding(.horizontal, 11).padding(.vertical, 5)
                    .background(.ultraThinMaterial, in: Capsule())
                    .overlay(Capsule().stroke(Theme.Color.greenChipBorder))
                    .padding(18)
                }
                Spacer()
            }
            Spacer()
        }
    }

    /// "N fillable field(s) on this page" — reflects what's actually
    /// highlighted (label-aware `fieldOverlays`), which for a flat form with
    /// no `AcroForm` is the only honest count.
    private var fieldCountLabel: String {
        let n = store.fieldOverlays.count
        return "\(n) fillable field\(n == 1 ? "" : "s") on this page"
    }

    private var pageNavBar: some View {
        VStack {
            Spacer()
            HStack(spacing: 2) {
                Button { store.goToPage(store.pageIndex &- 1) } label: { Image(systemName: "chevron.left") }
                    .disabled(store.pageIndex == 0 || store.pageViewMode == .continuous)
                Text("\(store.pageIndex + 1) / \(store.pageCount)")
                    .font(Theme.Font.pageNav).monospacedDigit()
                    .foregroundStyle(Theme.Color.textHigh)
                    .padding(.horizontal, 6)
                Button { store.goToPage(store.pageIndex &+ 1) } label: { Image(systemName: "chevron.right") }
                    .disabled(store.pageIndex + 1 >= store.pageCount || store.pageViewMode == .continuous)

                Divider().frame(height: 12).padding(.horizontal, 4)

                Button {
                    store.pageViewMode = store.pageViewMode == .single ? .continuous : .single
                } label: {
                    Image(systemName: store.pageViewMode == .continuous ? "square.stack" : "doc")
                }
                .help(
                    store.pageViewMode == .continuous
                        ? "Switch to Single Page view" : "Switch to Continuous Scroll view"
                )
                .accessibilityLabel(store.pageViewMode == .continuous ? "Single Page view" : "Continuous Scroll view")
            }
            .buttonStyle(.plain)
            .foregroundStyle(Theme.Color.textMid2)
            .font(Theme.Font.pageNav)
            .padding(.horizontal, 10).padding(.vertical, 6)
            .background(.ultraThinMaterial, in: Capsule())
            .overlay(Capsule().stroke(Color.white.opacity(0.09)))
            .padding(.bottom, 18)
        }
    }

    private func updateViewport(_ size: CGSize) {
        let usable = CGSize(
            width: max(size.width - Self.canvasPadding * 2, 0),
            height: max(size.height - Self.canvasPadding * 2, 0)
        )
        store.updateViewport(usable)
    }

    // MARK: - Interaction

    private func handleTap(_ point: CGPoint) {
        switch tool {
        case .select:
            // Plain view mode — no field affordances, nothing to click.
            break
        case .fill:
            if let overlay = store.fieldOverlay(containingX: Float(point.x), y: Float(point.y)) {
                if overlay.isSignature {
                    beginSigningOverlay(overlay)
                } else if let field = matchingField(overlay), field.kind == .checkbox {
                    // Toggle in place — a checkbox has no text to type, so
                    // opening the inline text caret here (the previous
                    // behavior) let you "type into" a checkbox, which did
                    // nothing useful and looked broken.
                    commitInlineEdit()
                    let isChecked = field.value == "true"
                    store.applyFormFill([FieldFill(name: field.name, value: .checkbox(checked: !isChecked))])
                } else if let field = matchingField(overlay), field.kind != .text {
                    // Radio groups, dropdowns, list boxes: pdfium-render has
                    // no public setter for them yet (see forms.rs), so
                    // there's nothing to do inline — but don't open a text
                    // caret either, since typing into one doesn't set its
                    // real value. "Fill fields" in the inspector at least
                    // shows the field's current value read-only.
                    commitInlineEdit()
                } else if isEditing(overlay.box) {
                    // Same field already open (e.g. a click to reposition
                    // the caret) — leave the in-progress text alone rather
                    // than wiping it out from under the user.
                } else {
                    commitInlineEdit()
                    inlineEditText = ""
                    inlineEditBox = overlay.box
                }
            } else {
                // Clicked blank canvas while a field was open: save it
                // (Core UX Principles-adjacent — typed text should never be
                // lost to a click, only an explicit Escape cancels it).
                commitInlineEdit()
            }
        case .sign:
            if let overlay = store.fieldOverlay(containingX: Float(point.x), y: Float(point.y)),
               overlay.isSignature {
                beginSigningOverlay(overlay)
            } else if signSession == nil {
                activeSheet = .signaturePoint(point)
            }
        case .note:
            activeSheet = .note(point)
        case .overlayText:
            // Manual text box: drop an inline, WYSIWYG-fit editable field
            // right where the user clicked (same mechanism as the box-fill
            // inline editor), rather than a modal prompt. Save whatever was
            // in a previously open box first (a no-op if it was left empty).
            commitInlineEdit()
            inlineEditText = ""
            inlineEditBox = DetectedBox(
                page: store.pageIndex,
                x: Float(point.x) - Float(Self.defaultBoxSize.width / 2),
                y: Float(point.y) - Float(Self.defaultBoxSize.height / 2),
                width: Float(Self.defaultBoxSize.width),
                height: Float(Self.defaultBoxSize.height)
            )
        case .editText:
            store.textRun(atPage: store.pageIndex, x: Float(point.x), y: Float(point.y)) { run in
                if let run {
                    activeSheet = .editText(run)
                } else if store.errorMessage == nil {
                    store.errorMessage = "No text found at that point."
                }
            }
        case .highlight, .underline, .strikeout, .rectangle, .circle, .line, .arrow:
            break // handled by handleDrag
        case .ink:
            break // handled by handleFreehandDrag
        }
    }

    private func matchingField(_ overlay: FieldOverlayBox) -> FormField? {
        store.formFieldsList.first { $0.name == overlay.fieldName }
    }

    /// Whether `box` is the field currently open in the inline editor —
    /// used so a click that lands back on the field already being typed
    /// into doesn't reset its text.
    private func isEditing(_ box: DetectedBox) -> Bool {
        guard let inlineEditBox else { return false }
        return inlineEditBox.page == box.page && inlineEditBox.x == box.x && inlineEditBox.y == box.y
    }

    private func handleDrag(_ start: CGPoint, _ end: CGPoint) {
        let kind: AnnotationKind
        switch tool {
        case .highlight: kind = .highlight
        case .underline: kind = .underline
        case .strikeout: kind = .strikeOut
        case .rectangle: kind = .rectangle
        case .circle: kind = .circle
        case .line: kind = .line
        case .arrow: kind = .arrow
        default: return
        }

        if tool.isLineBased {
            // Line/Arrow need the actual drag direction, not a reordered
            // bounding box — a diagonal from bottom-left to top-right must
            // stay that diagonal, not become a rectangle's min/max corners.
            guard hypot(end.x - start.x, end.y - start.y) > 1 else { return }
            store.applyAnnotation(Annotation(
                page: store.pageIndex, kind: kind,
                x: 0, y: 0, width: 0, height: 0,
                color: nil, note: nil,
                points: [AnnotationPoint(x: Float(start.x), y: Float(start.y)),
                         AnnotationPoint(x: Float(end.x), y: Float(end.y))]
            ))
            return
        }

        let rect = CGRect(
            x: min(start.x, end.x), y: min(start.y, end.y),
            width: abs(end.x - start.x), height: abs(end.y - start.y)
        )
        guard rect.width > 1, rect.height > 1 else { return }
        store.applyAnnotation(Annotation(
            page: store.pageIndex, kind: kind,
            x: Float(rect.minX), y: Float(rect.minY),
            width: Float(rect.width), height: Float(rect.height),
            color: nil, note: nil, points: []
        ))
    }

    /// Commits a completed `.ink` freehand stroke — `points` are already in
    /// PDF-point space (`PageCanvasView` converts them before calling this).
    private func handleFreehandDrag(_ points: [CGPoint]) {
        guard points.count > 1 else { return }
        store.applyAnnotation(Annotation(
            page: store.pageIndex, kind: .ink,
            x: 0, y: 0, width: 0, height: 0,
            color: nil, note: nil,
            points: points.map { AnnotationPoint(x: Float($0.x), y: Float($0.y)) }
        ))
    }

    /// Double-click anywhere on the page: for tools other than Select (where
    /// a single click on a highlighted box already opens it), snap to the
    /// scanned box under the click, or fall back to a fixed-size box
    /// centered on the click, and drop an inline editable field there. This
    /// is the manual override for spots the scan didn't pick up as a box.
    private func handleDoubleTap(_ point: CGPoint) {
        let page = store.pageIndex
        let box = store.boxContaining(x: Float(point.x), y: Float(point.y))
            ?? DetectedBox(
                page: page,
                x: Float(point.x) - Float(Self.defaultBoxSize.width / 2),
                y: Float(point.y) - Float(Self.defaultBoxSize.height / 2),
                width: Float(Self.defaultBoxSize.width),
                height: Float(Self.defaultBoxSize.height)
            )
        if isEditing(box) { return }
        commitInlineEdit()
        inlineEditText = ""
        inlineEditBox = box
    }

    private func commitInlineEdit() {
        guard let box = inlineEditBox else { return }
        inlineEditBox = nil
        // Font size from the same TextFit call, over the same (untrimmed)
        // text, that PageCanvasView's live editor last rendered — the only
        // way the exported stamp is guaranteed to match what was on screen
        // the moment before committing (Core UX Principles: WYSIWYG).
        let fontSize = TextFit.fontSize(
            for: inlineEditText, boxWidthPts: CGFloat(box.width), boxHeightPts: CGFloat(box.height)
        )
        let text = inlineEditText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        let baselineY = CGFloat(box.y) + TextFit.baselineLift(fontSize: fontSize, boxHeightPts: CGFloat(box.height))
        store.applyOverlay(TextOverlay(page: box.page, x: box.x, y: Float(baselineY), text: text, fontSize: Float(fontSize)))
    }

    private func cancelInlineEdit() {
        inlineEditBox = nil
        inlineEditText = ""
    }

    // MARK: - Sign flow

    /// `pdfree-core` already classified this field (`FormField.signatureKind`)
    /// — this just maps that engine value onto the local `SavedSignature.Kind`
    /// used for saved-mark storage. Only called on fields already known to be
    /// signature/initials (from `store.signatureFields`), so `.none` here
    /// would mean a caller bug; fall back to the safer, more common case.
    private func signatureKind(for field: FormField) -> SavedSignature.Kind {
        field.signatureKind == .initials ? .initials : .signature
    }

    private func hasSavedMark(for field: FormField) -> Bool {
        store.savedSignatures.contains { $0.kind == signatureKind(for: field) }
    }

    /// Route a click on a signature-kind overlay to signing. An `AcroForm`
    /// signature widget (has a backing `FormField`) starts the rich hop
    /// session; a label-detected signature line on a flat form (no widget)
    /// opens a one-off sign sheet anchored to that box.
    private func beginSigningOverlay(_ overlay: FieldOverlayBox) {
        if let field = matchingField(overlay) {
            beginSigning(from: field)
        } else {
            activeSheet = .signatureBox(overlay.box)
        }
    }

    /// Every signature/initials field in the document, reordered so `field`
    /// (if given) comes first — starts (or continues) the hop session there.
    private func beginSigning(from field: FormField?) {
        var pending = store.signatureFields
        // No detected signature fields: leave the manual point-click
        // fallback in `handleTap`'s `.sign` case as the only path — don't
        // start a session or show an error just for selecting the tool.
        guard !pending.isEmpty else { return }
        if let field, let idx = pending.firstIndex(where: { $0.name == field.name }) {
            pending = Array(pending[idx...] + pending[..<idx])
        }
        signSession = SignSessionState(fields: pending)
        tool = .sign
        presentCurrentSignStep()
    }

    private func presentCurrentSignStep() {
        guard let field = signSession?.currentField else { return }
        lastSignAnchorField = field
        if store.pageIndex != field.page { store.goToPage(field.page) }
        activeSheet = hasSavedMark(for: field) ? nil : .signatureField(field, .draw)
    }

    /// `store.fieldOverlays` minus any signature/initials field already
    /// placed (in the current sign session, or in the last couple of
    /// seconds right after placement) — once a field's mark is down, its
    /// amber "Sign here" affordance should disappear rather than keep
    /// looking like it's still waiting to be signed.
    private var visibleFieldOverlays: [FieldOverlayBox] {
        let hidden = justSignedFieldNames.union(signSession?.completedNames ?? [])
        guard !hidden.isEmpty else { return store.fieldOverlays }
        return store.fieldOverlays.filter { overlay in
            guard let name = overlay.fieldName else { return true }
            return !hidden.contains(name)
        }
    }

    private var signAnchorBox: DetectedBox? {
        if isPausingAfterPlacement { return nil }
        guard let session = signSession else { return nil }
        let field = session.done ? lastSignAnchorField : session.currentField
        guard let field else { return nil }
        if !session.done {
            guard hasSavedMark(for: field) else { return nil }
        }
        return DetectedBox(page: field.page, x: field.x, y: field.y, width: field.width, height: field.height)
    }

    private var signOverlayView: AnyView? {
        if isPausingAfterPlacement { return nil }
        guard let session = signSession else { return nil }
        if session.done {
            return AnyView(
                SignPopover(
                    kind: .signature, savedSignatures: [], progress: session.progress, done: true,
                    onPlace: { _ in }, onDrawNew: {}, onType: {}, onUpload: {},
                    onClose: dismissSign
                )
            )
        }
        guard let field = session.currentField, hasSavedMark(for: field) else { return nil }
        let kind = signatureKind(for: field)
        let saved = store.savedSignatures.filter { $0.kind == kind }
        return AnyView(
            SignPopover(
                kind: kind, savedSignatures: saved, progress: session.progress, done: false,
                onPlace: { commitPlacement(pngData: $0.pngData, for: field, saveForReuse: false) },
                onDrawNew: { activeSheet = .signatureField(field, .draw) },
                onType: { activeSheet = .signatureField(field, .type) },
                onUpload: { activeSheet = .signatureField(field, .upload) },
                onClose: dismissSign
            )
        )
    }

    /// Tear down the sign session and return to Select. Used by the sign box's
    /// close button, its "Done" button, and the click-outside-when-done tap.
    private func dismissSign() {
        signSession = nil
        lastSignAnchorField = nil
        tool = .select
    }

    /// How long a freshly placed signature stays visible, with no "Sign
    /// here" box or popover over it, before the session hops to the next
    /// field — long enough to actually register what was just signed
    /// rather than an instant jump.
    private static let signPlacementPause: TimeInterval = 2.0

    private func commitPlacement(pngData: Data, for field: FormField, saveForReuse: Bool) {
        let placement = SignaturePlacement(page: field.page, x: field.x, y: field.y, width: field.width, height: field.height)
        store.applySignature(pngData: pngData, at: placement)
        if saveForReuse {
            store.saveSignature(pngData: pngData, kind: signatureKind(for: field))
        }
        activeSheet = nil
        // Hide this field's "Sign here" affordance immediately, but hold off
        // on advancing `signSession` (which is what moves the popover/anchor
        // to the next field) for a couple of seconds, so the user actually
        // sees their signature land before anything moves.
        justSignedFieldNames.insert(field.name)
        isPausingAfterPlacement = true
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.signPlacementPause) {
            isPausingAfterPlacement = false
            justSignedFieldNames.remove(field.name)
            guard var session = signSession else { return }
            session.completedNames.insert(field.name)
            signSession = session
            if !session.done { presentCurrentSignStep() }
        }
    }

    // MARK: - Sheets

    @ViewBuilder
    private func sheetView(for sheet: ActiveSheet) -> some View {
        switch sheet {
        case .signatureField(let field, let tab):
            SignatureSheet(
                kind: signatureKind(for: field), initialTab: tab,
                onPlace: { pngData, saveForReuse in
                    commitPlacement(pngData: pngData, for: field, saveForReuse: saveForReuse)
                },
                onCancel: { activeSheet = nil }
            )

        case .signaturePoint(let point):
            SignatureSheet(
                kind: .signature, initialTab: .draw,
                onPlace: { pngData, saveForReuse in
                    store.applySignature(
                        pngData: pngData,
                        at: SignaturePlacement(page: store.pageIndex, x: Float(point.x), y: Float(point.y), width: 150, height: 60)
                    )
                    if saveForReuse { store.saveSignature(pngData: pngData, kind: .signature) }
                    activeSheet = nil
                },
                onCancel: { activeSheet = nil }
            )

        case .signatureBox(let box):
            SignatureSheet(
                kind: .signature, initialTab: .draw,
                onPlace: { pngData, saveForReuse in
                    store.applySignature(
                        pngData: pngData,
                        at: SignaturePlacement(page: box.page, x: box.x, y: box.y, width: box.width, height: box.height)
                    )
                    if saveForReuse { store.saveSignature(pngData: pngData, kind: .signature) }
                    activeSheet = nil
                },
                onCancel: { activeSheet = nil }
            )

        case .note(let point):
            TextPromptSheet(title: "Add Note", placeholder: "Note text") { text in
                store.applyAnnotation(Annotation(
                    page: store.pageIndex, kind: .note,
                    x: Float(point.x), y: Float(point.y), width: 24, height: 24,
                    color: nil, note: text, points: []
                ))
                activeSheet = nil
            } onCancel: {
                activeSheet = nil
            }

        case .editText(let run):
            TextPromptSheet(
                title: "Replace Text", placeholder: "Replacement", initialText: run.text
            ) { text in
                store.applyTextReplace(page: run.page, find: run.text, replace: text)
                activeSheet = nil
            } onCancel: {
                activeSheet = nil
            }

        case .fillForm:
            FormsPanel(store: store) { activeSheet = nil }

        case .extractedText(let text, let viaOCR):
            ExtractedTextSheet(text: text, viaOCR: viaOCR) { activeSheet = nil }

        case .splitRanges:
            SplitSheet(pageCount: store.pageCount) { ranges in
                store.splitExport(ranges: ranges) { pieces in
                    if let pieces { savePieces(pieces) }
                }
                activeSheet = nil
            } onCancel: {
                activeSheet = nil
            }

        case .aiAssistant:
            if let data = store.data {
                AIPanel(pdfBytes: data, documentTitle: store.title) { activeSheet = nil }
            }

        case .exportPassword:
            PasswordExportSheet(
                onExport: { password in
                    activeSheet = nil
                    store.exportEncrypted(password: password) { encrypted in
                        if let encrypted { saveExportedData(encrypted) }
                    }
                },
                onCancel: { activeSheet = nil }
            )
        }
    }

    // MARK: - File operations

    private var errorBinding: Binding<Bool> {
        Binding(get: { store.errorMessage != nil }, set: { if !$0 { store.errorMessage = nil } })
    }

    private func openDocument() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url, let data = try? Data(contentsOf: url) {
            cancelInlineEdit()
            signSession = nil
            isPausingAfterPlacement = false
            justSignedFieldNames.removeAll()
            store.openReplacing(data: data, url: url)
            tool = .select
        }
    }

    private func exportDocument() {
        guard let data = store.data else { return }
        saveExportedData(data)
    }

    /// Extracts the document's real text layer, falling back to OCR on the
    /// current page when there isn't one (see
    /// `PDFDocumentStore.extractDocumentText`) — this is the only place OCR
    /// is reachable from the UI today.
    private func extractDocumentText() {
        store.extractDocumentText { result in
            switch result {
            case .documentText(let text):
                activeSheet = .extractedText(text, viaOCR: false)
            case .ocrCurrentPage(let text):
                activeSheet = .extractedText(text, viaOCR: true)
            case nil:
                break // store already set errorMessage
            }
        }
    }

    /// Shared save-panel flow for both the plain export and the
    /// password-protected export (which produces its own, separate
    /// encrypted `Data` rather than reusing `store.data` — see
    /// `PDFDocumentStore.exportEncrypted`).
    private func saveExportedData(_ data: Data) {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        panel.nameFieldStringValue = store.fileURL?.lastPathComponent ?? "Untitled.pdf"
        if panel.runModal() == .OK, let url = panel.url {
            do {
                try data.write(to: url)
            } catch {
                store.errorMessage = "\(error)"
            }
        }
    }

    /// PDFKit only, and only for the printed-out bytes: `PDFDocument` here is
    /// a throwaway view over `store.data` purely to reach
    /// `printOperation(for:scalingMode:autoRotate:)`, which hands back a real
    /// `NSPrintOperation` (paging, scaling, the native print panel) for
    /// free — SwiftUI has no print API of its own, and pdfree-core's own
    /// PDFium-backed rendering isn't what's on screen here. Every other part
    /// of the app (render, edit, sign, …) still goes through pdfree-core;
    /// nothing about the document's actual handling changes.
    private func printDocument() {
        guard let data = store.data, let pdfDocument = PDFDocument(data: data) else { return }
        guard let printOperation = pdfDocument.printOperation(
            for: NSPrintInfo.shared, scalingMode: .pageScaleToFit, autoRotate: true
        ) else { return }
        printOperation.run()
    }

    private func mergeDocument() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.pdf]
        if panel.runModal() == .OK, let url = panel.url, let data = try? Data(contentsOf: url) {
            store.mergeAppending(data)
        }
    }

    private func insertImage() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.png, .jpeg, .tiff]
        if panel.runModal() == .OK, let url = panel.url, let data = try? Data(contentsOf: url) {
            store.insertImagePage(data)
        }
    }

    private func savePieces(_ pieces: [Data]) {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.prompt = "Choose Folder"
        guard panel.runModal() == .OK, let dir = panel.url else { return }
        for (i, piece) in pieces.enumerated() {
            let url = dir.appendingPathComponent("split-\(i + 1).pdf")
            try? piece.write(to: url)
        }
    }
}

#Preview {
    ContentView()
}
