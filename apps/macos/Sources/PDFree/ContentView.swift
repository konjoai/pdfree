import AppKit
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
    case note(CGPoint)
    case editText(TextRun)
    case fillForm
    case extractedText(String)
    case splitRanges
    case aiAssistant

    var id: String {
        switch self {
        case .signatureField(let field, let tab): return "signatureField-\(field.name)-\(tab.rawValue)"
        case .signaturePoint: return "signaturePoint"
        case .note: return "note"
        case .editText: return "editText"
        case .fillForm: return "fillForm"
        case .extractedText: return "extractedText"
        case .splitRanges: return "splitRanges"
        case .aiAssistant: return "aiAssistant"
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
                        onSelectSign: { beginSigning(from: nil) },
                        onAskAI: { activeSheet = .aiAssistant }
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

                if let image = store.pageImage {
                    ScrollView([.horizontal, .vertical]) {
                        PageCanvasView(
                            image: image,
                            pagePointSize: store.pagePointSize,
                            tool: tool,
                            fieldOverlays: visibleFieldOverlays,
                            onTap: handleTap,
                            onDrag: handleDrag,
                            onDoubleTap: handleDoubleTap,
                            inlineEditBox: inlineEditBox,
                            inlineEditText: $inlineEditText,
                            onCommitInlineEdit: commitInlineEdit,
                            onCancelInlineEdit: cancelInlineEdit,
                            signAnchorBox: signAnchorBox,
                            signOverlay: signOverlayView,
                            onSignBackgroundTap: (signSession?.done == true) ? dismissSign : nil
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
                    pageNavBar
                } else {
                    ProgressView("Loading…").tint(.white)
                }
            }
            .overlay {
                // Scroll/swipe over the canvas turns pages (pass-through, so
                // clicks and drags still reach the page).
                if store.hasDocument {
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
            .animation(.easeInOut(duration: 0.16), value: tool.isAnnotation)
            .onAppear { updateViewport(geo.size) }
            .onChange(of: geo.size) { updateViewport($0) }
        }
    }

    private var fieldCountChip: some View {
        VStack {
            HStack {
                if !store.formFieldsList.isEmpty {
                    HStack(spacing: 7) {
                        Circle().fill(Theme.Color.green).frame(width: 6, height: 6)
                        Text("\(store.formFieldsList.count) fillable fields detected")
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

    private var pageNavBar: some View {
        VStack {
            Spacer()
            HStack(spacing: 2) {
                Button { store.goToPage(store.pageIndex &- 1) } label: { Image(systemName: "chevron.left") }
                    .disabled(store.pageIndex == 0)
                Text("\(store.pageIndex + 1) / \(store.pageCount)")
                    .font(Theme.Font.pageNav).monospacedDigit()
                    .foregroundStyle(Theme.Color.textHigh)
                    .padding(.horizontal, 6)
                Button { store.goToPage(store.pageIndex &+ 1) } label: { Image(systemName: "chevron.right") }
                    .disabled(store.pageIndex + 1 >= store.pageCount)
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
                if overlay.isSignature, let field = matchingField(overlay) {
                    beginSigning(from: field)
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
               let field = matchingField(overlay) {
                beginSigning(from: field)
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
            if let run = store.textRun(atPage: store.pageIndex, x: Float(point.x), y: Float(point.y)) {
                activeSheet = .editText(run)
            } else if store.errorMessage == nil {
                store.errorMessage = "No text found at that point."
            }
        case .highlight, .underline, .strikeout:
            break // handled by handleDrag
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
        default: return
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
            color: nil, note: nil
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

        case .note(let point):
            TextPromptSheet(title: "Add Note", placeholder: "Note text") { text in
                store.applyAnnotation(Annotation(
                    page: store.pageIndex, kind: .note,
                    x: Float(point.x), y: Float(point.y), width: 24, height: 24,
                    color: nil, note: text
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

        case .extractedText(let text):
            ExtractedTextSheet(text: text) { activeSheet = nil }

        case .splitRanges:
            SplitSheet(pageCount: store.pageCount) { ranges in
                if let pieces = store.splitExport(ranges: ranges) {
                    savePieces(pieces)
                }
                activeSheet = nil
            } onCancel: {
                activeSheet = nil
            }

        case .aiAssistant:
            if let data = store.data {
                AIPanel(pdfBytes: data, documentTitle: store.title) { activeSheet = nil }
            }
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
