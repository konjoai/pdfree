import AppKit
import SwiftUI

/// A minimal draw-your-signature pad: freehand strokes rasterized to a PNG,
/// which is what `signatures::place_signature` (the "basic e-sign" path —
/// see docs/api.md) stamps onto the page. No cryptography involved.
struct SignatureSheet: View {
    let onSave: (Data) -> Void
    let onCancel: () -> Void

    @State private var strokes: [[CGPoint]] = []
    @State private var currentStroke: [CGPoint] = []

    private let canvasSize = CGSize(width: 300, height: 120)

    var body: some View {
        VStack(spacing: 12) {
            Text("Draw Your Signature").font(.headline)
            signatureCanvas
                .frame(width: canvasSize.width, height: canvasSize.height)
                .background(Color.white)
                .border(Color.secondary)
                .gesture(drawGesture)
            HStack {
                Button("Clear") {
                    strokes = []
                    currentStroke = []
                }
                Spacer()
                Button("Cancel") { onCancel() }
                Button("Use Signature") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(strokes.isEmpty)
            }
        }
        .padding()
        .frame(width: 360)
    }

    private var signatureCanvas: some View {
        Canvas { context, _ in
            for stroke in strokes + [currentStroke] {
                guard stroke.count > 1 else { continue }
                var path = Path()
                path.addLines(stroke)
                context.stroke(path, with: .color(.black), lineWidth: 3)
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

    private func save() {
        let renderer = ImageRenderer(
            content: signatureCanvas
                .frame(width: canvasSize.width, height: canvasSize.height)
                .background(Color.white)
        )
        renderer.scale = 2
        guard let nsImage = renderer.nsImage,
              let tiff = nsImage.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiff),
              let png = bitmap.representation(using: .png, properties: [:])
        else {
            onCancel()
            return
        }
        onSave(png)
    }
}
