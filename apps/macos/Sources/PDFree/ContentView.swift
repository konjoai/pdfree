import AppKit
import SwiftUI
import UniformTypeIdentifiers

/// One modal task at a time — a sheet, keyed so SwiftUI can present it.
private enum ActiveSheet: Identifiable {
    case signature(CGPoint)
    case note(CGPoint)
    case overlay(CGPoint)
    case editText(TextRun)
    case fillForm
    case extractedText(String)
    case splitRanges

    var id: String {
        switch self {
        case .signature: return "signature"
        case .note: return "note"
        case .overlay: return "overlay"
        case .editText: return "editText"
        case .fillForm: return "fillForm"
        case .extractedText: return "extractedText"
        case .splitRanges: return "splitRanges"
        }
    }
}

struct ContentView: View {
    @StateObject private var store: PDFDocumentStore
    @State private var tool: Tool = .select
    @State private var activeSheet: ActiveSheet?
    @State private var inlineEditBox: DetectedBox?
    @State private var inlineEditText: String = ""

    /// Fallback box size (PDF points) when double-clicking finds no drawn
    /// rectangle or ruled-line cell to snap to.
    private static let defaultBoxSize = CGSize(width: 140, height: 18)

    init() {
        let url = Bundle.main.url(forResource: "sample", withExtension: "pdf")
        let data = url.flatMap { try? Data(contentsOf: $0) } ?? Data()
        _store = StateObject(wrappedValue: PDFDocumentStore(data: data, url: nil))
    }

    var body: some View {
        HSplitView {
            PagesSidebarView(store: store)
                .frame(minWidth: 160, idealWidth: 200, maxWidth: 280)

            VStack(spacing: 0) {
                toolbar
                Divider()
                canvasArea
            }
            .frame(minWidth: 520)
        }
        .frame(minWidth: 780, minHeight: 640)
        .onChange(of: store.pageIndex) { _ in cancelInlineEdit() }
        .sheet(item: $activeSheet) { sheet in sheetView(for: sheet) }
        .alert("Error", isPresented: errorBinding) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(store.errorMessage ?? "")
        }
        .overlay(alignment: .top) {
            if store.isBusy {
                ProgressView().padding(6).background(.regularMaterial, in: Capsule())
                    .padding(.top, 4)
            }
        }
    }

    // MARK: - Toolbar

    private var toolbar: some View {
        HStack(spacing: 10) {
            Button("Open…", action: openDocument)
            Menu("Save") {
                Button("Save") { saveInPlace() }.disabled(store.fileURL == nil)
                Button("Save As…") { saveAs() }
            }
            Divider().frame(height: 20)
            Picker("Tool", selection: $tool) {
                ForEach(Tool.allCases) { t in
                    Label(t.label, systemImage: t.systemImage).tag(t)
                }
            }
            .labelsHidden()
            .frame(width: 170)
            Divider().frame(height: 20)
            Button("Merge…", action: mergeDocument)
            Button("Insert Image…", action: insertImage)
            Button("Split…") { activeSheet = .splitRanges }
            Button("Extract Text…") {
                if let text = store.extractText() { activeSheet = .extractedText(text) }
            }
            Button("Fill Form (\(store.formFieldsList.count))") { activeSheet = .fillForm }
                .disabled(store.formFieldsList.isEmpty)
            Spacer()
            pageNav
        }
        .padding(8)
    }

    private var pageNav: some View {
        HStack {
            Button("◀") { store.goToPage(store.pageIndex &- 1) }
                .disabled(store.pageIndex == 0)
            Text("\(store.pageIndex + 1) / \(store.pageCount)").monospacedDigit()
            Button("▶") { store.goToPage(store.pageIndex &+ 1) }
                .disabled(store.pageIndex + 1 >= store.pageCount)
        }
    }

    // MARK: - Canvas

    /// Space reserved for `.padding()` around the page on each axis, so the
    /// viewport size fed to fit-to-page math accounts for it — otherwise the
    /// rendered page would be very slightly too large and require scrolling.
    private static let canvasPadding: CGFloat = 16

    @ViewBuilder
    private var canvasArea: some View {
        GeometryReader { geo in
            Group {
                if let image = store.pageImage {
                    ScrollView([.horizontal, .vertical]) {
                        PageCanvasView(
                            image: image,
                            pagePointSize: store.pagePointSize,
                            tool: tool,
                            detectedBoxes: store.detectedBoxes,
                            onTap: handleTap,
                            onDrag: handleDrag,
                            onDoubleTap: handleDoubleTap,
                            inlineEditBox: inlineEditBox,
                            inlineEditText: $inlineEditText,
                            onCommitInlineEdit: commitInlineEdit,
                            onCancelInlineEdit: cancelInlineEdit
                        )
                        .padding(Self.canvasPadding)
                    }
                } else {
                    VStack {
                        Spacer()
                        ProgressView("Loading…")
                        Spacer()
                    }
                }
            }
            .onAppear { updateViewport(geo.size) }
            .onChange(of: geo.size) { updateViewport($0) }
        }
    }

    private func updateViewport(_ size: CGSize) {
        let usable = CGSize(
            width: max(size.width - Self.canvasPadding * 2, 0),
            height: max(size.height - Self.canvasPadding * 2, 0)
        )
        store.updateViewport(usable)
    }

    private func handleTap(_ point: CGPoint) {
        switch tool {
        case .select:
            // Every fillable box is already highlighted (scanned on page
            // load) — clicking directly on one starts typing immediately,
            // no double-click needed.
            if let box = store.boxContaining(x: Float(point.x), y: Float(point.y)) {
                inlineEditText = ""
                inlineEditBox = box
            }
        case .note:
            activeSheet = .note(point)
        case .sign:
            activeSheet = .signature(point)
        case .overlayText:
            activeSheet = .overlay(point)
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
        inlineEditText = ""
        inlineEditBox = box
    }

    private func commitInlineEdit() {
        guard let box = inlineEditBox else { return }
        inlineEditBox = nil
        let text = inlineEditText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        let fontSize = max(9, min(box.height * 0.7, 18))
        store.applyOverlay(TextOverlay(page: box.page, x: box.x, y: box.y, text: text, fontSize: fontSize))
    }

    private func cancelInlineEdit() {
        inlineEditBox = nil
        inlineEditText = ""
    }

    // MARK: - Sheets

    @ViewBuilder
    private func sheetView(for sheet: ActiveSheet) -> some View {
        switch sheet {
        case .signature(let point):
            SignatureSheet { pngData in
                store.applySignature(
                    pngData: pngData,
                    at: SignaturePlacement(
                        page: store.pageIndex, x: Float(point.x), y: Float(point.y),
                        width: 150, height: 60
                    )
                )
                activeSheet = nil
            } onCancel: {
                activeSheet = nil
            }

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

        case .overlay(let point):
            TextPromptSheet(title: "Add Text", placeholder: "Text to stamp") { text in
                store.applyOverlay(TextOverlay(
                    page: store.pageIndex, x: Float(point.x), y: Float(point.y),
                    text: text, fontSize: 12
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
            store.openReplacing(data: data, url: url)
            tool = .select
        }
    }

    private func saveAs() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.pdf]
        panel.nameFieldStringValue = store.fileURL?.lastPathComponent ?? "Untitled.pdf"
        if panel.runModal() == .OK, let url = panel.url {
            do {
                try store.data.write(to: url)
                store.fileURL = url
            } catch {
                store.errorMessage = "\(error)"
            }
        }
    }

    private func saveInPlace() {
        guard let url = store.fileURL else {
            saveAs()
            return
        }
        do {
            try store.data.write(to: url)
        } catch {
            store.errorMessage = "\(error)"
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
