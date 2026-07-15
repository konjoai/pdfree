import AppKit
import SwiftUI

/// The full sign surface: draw / type / upload a signature or initials.
/// Shown the first time (nothing saved yet) or when a returning user
/// explicitly asks to draw/type/upload a new mark from `SignPopover`.
/// Whatever tab produces the PNG, `signatures::place_signature` (the "basic
/// e-sign" path — see docs/api.md) is what stamps it onto the page. No
/// cryptography involved.
struct SignatureSheet: View {
    enum Tab: String, CaseIterable, Identifiable {
        case draw = "Draw", type = "Type", upload = "Upload"
        var id: String { rawValue }
    }

    let kind: SavedSignature.Kind
    var initialTab: Tab = .draw
    let onPlace: (Data, Bool) -> Void
    let onCancel: () -> Void

    @State private var tab: Tab = .draw
    @State private var strokes: [[CGPoint]] = []
    @State private var currentStroke: [CGPoint] = []
    @State private var typedName: String = ""
    @State private var uploadedImage: NSImage?
    @State private var saveForReuse = true
    /// The draw pad's actual on-screen size, measured live via
    /// `GeometryReader` — strokes are recorded in this view's own coordinate
    /// space, so the exported render must use these exact dimensions (not a
    /// guessed fallback) or anything drawn near the true edges gets clipped
    /// out of a too-small export canvas. Falls back to a sane default before
    /// the first layout pass reports a real size.
    @State private var drawCanvasSize = CGSize(width: 300, height: 110)

    private var title: String {
        kind == .initials ? "Add your initials" : "Add your signature"
    }

    private var hasContent: Bool {
        switch tab {
        case .draw: return !strokes.isEmpty
        case .type: return !typedName.trimmingCharacters(in: .whitespaces).isEmpty
        case .upload: return uploadedImage != nil
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            header
            tabBar
            content
                .padding(.horizontal, 22)
                .padding(.top, 16)
            reuseToggle
            footer
        }
        .frame(width: 452)
        .background(Theme.Color.popoverBg)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Color.white.opacity(0.09)))
        .shadow(color: .black.opacity(0.5), radius: 40, y: 20)
        .onAppear { tab = initialTab }
    }

    private var header: some View {
        HStack {
            Text(title).font(.system(size: 16, weight: .bold)).foregroundStyle(Theme.Color.textHigh)
            Spacer()
            Button(action: onCancel) {
                Image(systemName: "xmark")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Theme.Color.textMid)
                    .frame(width: 26, height: 26)
                    .background(Color.white.opacity(0.07), in: Circle())
            }
            .buttonStyle(.plain)
        }
        .padding(.horizontal, 22)
        .padding(.top, 20)
    }

    private var tabBar: some View {
        HStack(spacing: 4) {
            ForEach(Tab.allCases) { t in
                Text(t.rawValue)
                    .font(.system(size: 12.5, weight: tab == t ? .semibold : .medium))
                    .foregroundStyle(tab == t ? Theme.Color.greenInk : Theme.Color.textMid)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                    .background(tab == t ? Theme.Color.green : .clear, in: RoundedRectangle(cornerRadius: 9))
                    .contentShape(Rectangle())
                    .onTapGesture { tab = t }
            }
        }
        .padding(.horizontal, 22)
        .padding(.top, 16)
    }

    @ViewBuilder
    private var content: some View {
        switch tab {
        case .draw: drawPad
        case .type: typePad
        case .upload: uploadPad
        }
    }

    private var drawPad: some View {
        ZStack {
            Color.white
            signatureCanvas
            VStack {
                Spacer()
                Rectangle().fill(Color(nsColor: .separatorColor)).frame(height: 1.5)
                    .padding(.horizontal, 20).padding(.bottom, 22)
            }
            if strokes.isEmpty && currentStroke.isEmpty {
                VStack {
                    Spacer()
                    HStack {
                        Text("Draw with trackpad or mouse")
                            .font(.system(size: 10))
                            .foregroundStyle(Color.black.opacity(0.35))
                        Spacer()
                    }
                    .padding(.horizontal, 16).padding(.bottom, 8)
                }
            }
        }
        .frame(height: 150)
        .clipShape(RoundedRectangle(cornerRadius: 11))
        .gesture(drawGesture)
        .background(
            GeometryReader { geo in
                Color.clear
                    .onAppear { drawCanvasSize = geo.size }
                    .onChange(of: geo.size) { drawCanvasSize = $0 }
            }
        )
    }

    private var signatureCanvas: some View {
        Canvas { context, _ in
            for stroke in strokes + [currentStroke] {
                guard stroke.count > 1 else { continue }
                var path = Path()
                path.addLines(stroke)
                context.stroke(path, with: .color(Theme.Color.signatureInk), lineWidth: 3)
            }
        }
    }

    private var drawGesture: some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { value in currentStroke.append(value.location) }
            .onEnded { _ in
                strokes.append(currentStroke)
                currentStroke = []
            }
    }

    private var typePad: some View {
        VStack(spacing: 10) {
            ZStack {
                Color.white
                if typedName.isEmpty {
                    Text(kind == .initials ? "A.N." : "Your Name")
                        .font(Theme.Font.cursive(36))
                        .foregroundStyle(Theme.Color.signatureInk.opacity(0.25))
                } else {
                    Text(typedName)
                        .font(Theme.Font.cursive(36))
                        .foregroundStyle(Theme.Color.signatureInk)
                }
            }
            .frame(height: 110)
            .clipShape(RoundedRectangle(cornerRadius: 11))

            TextField(kind == .initials ? "Initials" : "Full name", text: $typedName)
                .textFieldStyle(.roundedBorder)
        }
    }

    private var uploadPad: some View {
        VStack(spacing: 10) {
            ZStack {
                Color.white
                if let uploadedImage {
                    Image(nsImage: uploadedImage).resizable().scaledToFit().padding(12)
                } else {
                    Text("No image chosen").font(.system(size: 12)).foregroundStyle(.secondary)
                }
            }
            .frame(height: 110)
            .clipShape(RoundedRectangle(cornerRadius: 11))

            Button("Choose Image…", action: chooseImage)
        }
    }

    private var reuseToggle: some View {
        Toggle(isOn: $saveForReuse) {
            Text(kind == .initials ? "Save these initials for reuse" : "Save this signature for reuse")
                .font(.system(size: 12.5, weight: .medium))
                .foregroundStyle(Theme.Color.textRow)
        }
        .toggleStyle(.switch)
        .tint(Theme.Color.green)
        .padding(.horizontal, 22)
        .padding(.top, 14)
    }

    private var footer: some View {
        HStack(spacing: 10) {
            if tab == .draw {
                Button("Clear") {
                    strokes = []
                    currentStroke = []
                }
                .buttonStyle(.plain)
                .font(.system(size: 13, weight: .medium))
                .foregroundStyle(Theme.Color.textMid)
            }
            Spacer()
            Button("Cancel", action: onCancel)
                .buttonStyle(PillButtonStyle(bg: Color.white.opacity(0.08), fg: Theme.Color.textRow))
            Button("Place \(kind == .initials ? "initials" : "signature")", action: place)
                .buttonStyle(PillButtonStyle(bg: Theme.Color.green, fg: Theme.Color.greenInk, weight: .bold))
                .disabled(!hasContent)
                .keyboardShortcut(.defaultAction)
        }
        .padding(18)
        .padding(.bottom, 4)
    }

    private func chooseImage() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.png, .jpeg]
        panel.allowsMultipleSelection = false
        if panel.runModal() == .OK, let url = panel.url {
            uploadedImage = NSImage(contentsOf: url)
        }
    }

    private func place() {
        guard let png = renderPNG() else {
            onCancel()
            return
        }
        onPlace(png, saveForReuse)
    }

    private func renderPNG() -> Data? {
        switch tab {
        case .draw:
            // Matches `drawPad`'s actual measured size exactly — using a
            // smaller, guessed frame here would silently clip any stroke
            // drawn near the real canvas's edges. No `.background()`: the
            // signature is stamped over the document, so it must render
            // with a transparent backdrop, not an opaque white rectangle
            // that would cover whatever text sits underneath it.
            let renderer = ImageRenderer(
                content: signatureCanvas.frame(width: drawCanvasSize.width, height: drawCanvasSize.height)
            )
            renderer.scale = 2
            return renderer.nsImage.flatMap(pngData)
        case .type:
            let renderer = ImageRenderer(
                content: Text(typedName)
                    .font(Theme.Font.cursive(48))
                    .foregroundStyle(Theme.Color.signatureInk)
                    .padding(16)
            )
            renderer.scale = 2
            return renderer.nsImage.flatMap(pngData)
        case .upload:
            return uploadedImage.flatMap(pngData)
        }
    }

    private func pngData(_ image: NSImage) -> Data? {
        guard let tiff = image.tiffRepresentation, let bitmap = NSBitmapImageRep(data: tiff) else { return nil }
        return bitmap.representation(using: .png, properties: [:])
    }
}

/// Green-primary / translucent-secondary pill button, matching the
/// design's footer buttons (Cancel / Place, Draw new / Type / Upload, …).
struct PillButtonStyle: ButtonStyle {
    var bg: Color
    var fg: Color
    var weight: Font.Weight = .medium

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: weight))
            .foregroundStyle(fg)
            .padding(.horizontal, 16)
            .padding(.vertical, 10)
            .background(bg.opacity(configuration.isPressed ? 0.8 : 1), in: RoundedRectangle(cornerRadius: 10))
            .opacity(configuration.isPressed ? 0.9 : 1)
    }
}
