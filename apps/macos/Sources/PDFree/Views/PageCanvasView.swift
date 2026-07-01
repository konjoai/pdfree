import AppKit
import SwiftUI

/// Renders the current page at its native pixel size (no `.resizable()`, so
/// gesture coordinates are already in image-pixel space — no scale factor to
/// compute) and translates taps/drags into PDF points (72/inch, origin at the
/// page's bottom-left), the convention every `pdfree-core` coordinate takes.
///
/// Also draws every pre-scanned `detectedBoxes` entry as a highlighted
/// outline (so fillable areas are visible up front, not discovered one
/// click at a time), and hosts the inline text editor: when `inlineEditBox`
/// is set, an editable field is overlaid exactly on top of that box, in the
/// same pixel space as the page image.
struct PageCanvasView: View {
    let image: NSImage
    let pagePointSize: CGSize
    let tool: Tool
    let detectedBoxes: [DetectedBox]
    let onTap: (CGPoint) -> Void
    let onDrag: (CGPoint, CGPoint) -> Void
    let onDoubleTap: (CGPoint) -> Void

    let inlineEditBox: DetectedBox?
    @Binding var inlineEditText: String
    let onCommitInlineEdit: () -> Void
    let onCancelInlineEdit: () -> Void

    @FocusState private var inlineEditFocused: Bool

    private var ptsPerPixel: CGFloat {
        image.size.width > 0 ? pagePointSize.width / image.size.width : 1
    }

    private func toPDFPoint(_ p: CGPoint) -> CGPoint {
        CGPoint(x: p.x * ptsPerPixel, y: pagePointSize.height - (p.y * ptsPerPixel))
    }

    /// Inverse of `toPDFPoint`: a box's PDF-points rect back to a pixel rect
    /// in this view's (top-left origin, y-down) coordinate space.
    private func toPixelRect(_ box: DetectedBox) -> CGRect {
        CGRect(
            x: CGFloat(box.x) / ptsPerPixel,
            y: (pagePointSize.height - CGFloat(box.y) - CGFloat(box.height)) / ptsPerPixel,
            width: CGFloat(box.width) / ptsPerPixel,
            height: CGFloat(box.height) / ptsPerPixel
        )
    }

    var body: some View {
        ZStack(alignment: .topLeading) {
            Image(nsImage: image)
                .contentShape(Rectangle())
                .gesture(dragGesture)
                .simultaneousGesture(combinedTapGesture)

            if tool == .select {
                ForEach(Array(detectedBoxes.enumerated()), id: \.offset) { _, box in
                    let rect = toPixelRect(box)
                    Rectangle()
                        .stroke(Color.accentColor.opacity(0.6), lineWidth: 1)
                        .background(Color.accentColor.opacity(0.06))
                        .frame(width: rect.width, height: rect.height)
                        .position(x: rect.midX, y: rect.midY)
                        .allowsHitTesting(false)
                }
            }

            if let box = inlineEditBox {
                let rect = toPixelRect(box)
                TextField("", text: $inlineEditText)
                    .textFieldStyle(.plain)
                    .font(.system(size: max(9, rect.height * 0.7)))
                    .foregroundColor(.black)
                    .tint(.black)
                    .padding(.horizontal, 2)
                    .frame(width: max(rect.width, 20), height: max(rect.height, 14))
                    .background(Color.yellow.opacity(0.35))
                    .overlay(Rectangle().stroke(Color.accentColor, lineWidth: 1.5))
                    .position(x: rect.midX, y: rect.midY)
                    .focused($inlineEditFocused)
                    .onSubmit { onCommitInlineEdit() }
                    .onExitCommand { onCancelInlineEdit() }
                    .onAppear { inlineEditFocused = true }
            }
        }
    }

    private var combinedTapGesture: some Gesture {
        SpatialTapGesture(count: 2)
            .onEnded { value in onDoubleTap(toPDFPoint(value.location)) }
            .exclusively(
                before: SpatialTapGesture(count: 1)
                    .onEnded { value in
                        guard !tool.isDragBased else { return }
                        onTap(toPDFPoint(value.location))
                    }
            )
    }

    private var dragGesture: some Gesture {
        DragGesture(minimumDistance: 4).onEnded { value in
            guard tool.isDragBased else { return }
            onDrag(toPDFPoint(value.startLocation), toPDFPoint(value.location))
        }
    }
}
