import AppKit
import SwiftUI

/// Renders the current page at its native pixel size (no `.resizable()`, so
/// gesture coordinates are already in image-pixel space — no scale factor to
/// compute) and translates taps/drags into PDF points (72/inch, origin at the
/// page's bottom-left), the convention every `pdfree-core` coordinate takes.
///
/// Draws every pre-scanned `fieldOverlays` entry up front (Core UX Principles:
/// no "turn on field detection" mode) — a quiet green wash for an ordinary
/// fill box, or an amber "Sign here" affordance for a signature/initials
/// field, which never opens a text caret. Also hosts the inline text editor
/// and, during a sign session, the hop-to-next-field popover, both anchored
/// in the same pixel space as the page image.
struct PageCanvasView: View {
    let image: NSImage
    let pagePointSize: CGSize
    let tool: Tool
    let fieldOverlays: [FieldOverlayBox]
    let onTap: (CGPoint) -> Void
    let onDrag: (CGPoint, CGPoint) -> Void
    let onDoubleTap: (CGPoint) -> Void

    let inlineEditBox: DetectedBox?
    @Binding var inlineEditText: String
    let onCommitInlineEdit: () -> Void
    let onCancelInlineEdit: () -> Void

    /// The field a sign session is currently anchored on, if one is active.
    var signAnchorBox: DetectedBox?
    /// The popover content itself (`SignPopover`), type-erased so this view
    /// doesn't need to be generic — `nil` when no sign session is active.
    var signOverlay: AnyView?
    /// Non-nil only once every field is signed: a tap anywhere off the sign
    /// box then dismisses it. Before completion this stays `nil`, so a stray
    /// click can't close the box mid-signing.
    var onSignBackgroundTap: (() -> Void)?

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

            if tool == .fill || tool == .sign {
                ForEach(fieldOverlays) { overlay in
                    fieldOverlayView(overlay)
                }
            }

            if let box = inlineEditBox {
                inlineEditor(for: box, at: toPixelRect(box))
            }

            if let signAnchorBox, let signOverlay {
                // Once complete, a transparent full-page catcher behind the
                // box turns any off-box tap into a dismissal.
                if let onSignBackgroundTap {
                    Color.black.opacity(0.001)
                        .frame(width: image.size.width, height: image.size.height)
                        .contentShape(Rectangle())
                        .onTapGesture { onSignBackgroundTap() }
                }
                let rect = toPixelRect(signAnchorBox)
                signOverlay
                    .position(x: rect.midX, y: min(rect.maxY + 130, pixelHeight - 90))
                    .animation(Theme.Anim.hop, value: signAnchorBox.y)
                    .animation(Theme.Anim.hop, value: signAnchorBox.x)
            }
        }
    }

    private var pixelHeight: CGFloat { image.size.height }

    // MARK: - Field overlays

    @ViewBuilder
    private func fieldOverlayView(_ overlay: FieldOverlayBox) -> some View {
        let rect = toPixelRect(overlay.box)
        let isFocused = inlineEditBox.map { $0.x == overlay.box.x && $0.y == overlay.box.y } ?? false

        if overlay.isSignature {
            signatureFieldView(rect: rect)
        } else {
            normalFieldView(rect: rect, focused: isFocused)
        }
    }

    private func normalFieldView(rect: CGRect, focused: Bool) -> some View {
        ZStack(alignment: .topTrailing) {
            RoundedRectangle(cornerRadius: Theme.Metric.fieldRadius)
                .fill(Theme.Color.green.opacity(focused ? 0.10 : 0.09))
                .overlay(
                    RoundedRectangle(cornerRadius: Theme.Metric.fieldRadius)
                        .stroke(Theme.Color.green.opacity(focused ? 1 : 0.6), lineWidth: focused ? 2 : 1.4)
                )
                .shadow(color: focused ? Theme.Color.fieldFocusRing : .clear, radius: focused ? 4 : 0)

            if focused, rect.width > 40 {
                Text("auto-fit ✓")
                    .font(.system(size: 8, weight: .semibold))
                    .foregroundStyle(Theme.Color.green)
                    .padding(.horizontal, 4).padding(.vertical, 1)
                    .background(Theme.Color.green.opacity(0.12), in: RoundedRectangle(cornerRadius: 3))
                    .offset(x: -3, y: -12)
            }
        }
        .frame(width: max(rect.width, 4), height: max(rect.height, 4))
        .position(x: rect.midX, y: rect.midY)
        .allowsHitTesting(false)
        .animation(Theme.Anim.focusRing, value: focused)
    }

    /// Amber, dashed, never a text caret — clicking launches the sign flow.
    private func signatureFieldView(rect: CGRect) -> some View {
        let showsLabel = rect.width > 68

        return ZStack {
            RoundedRectangle(cornerRadius: 6)
                .fill(Theme.Color.amberFieldWash)
                .overlay(
                    RoundedRectangle(cornerRadius: 6)
                        .stroke(Theme.Color.amber, style: StrokeStyle(lineWidth: 2, dash: [5, 3]))
                )

            if showsLabel {
                HStack(spacing: 7) {
                    ZStack {
                        RoundedRectangle(cornerRadius: 7)
                            .fill(Theme.Color.amber)
                            .frame(width: min(22, rect.height - 6), height: min(22, rect.height - 6))
                        Image(systemName: "signature")
                            .font(.system(size: 10, weight: .bold))
                            .foregroundStyle(Theme.Color.amberInk)
                    }
                    if rect.width > 120 {
                        Text("Sign here")
                            .font(.system(size: 11, weight: .semibold))
                            .foregroundStyle(Theme.Color.amberText)
                            .lineLimit(1)
                    }
                }
                .padding(.horizontal, 8)
            }
        }
        .frame(width: max(rect.width, 22), height: max(rect.height, 18))
        .position(x: rect.midX, y: rect.midY)
        .allowsHitTesting(false)
    }

    // MARK: - Inline text editor

    private func inlineEditor(for box: DetectedBox, at rect: CGRect) -> some View {
        // TextFit works in PDF points (the same unit `overlay_text`'s
        // `font_size` exports in) so the two stay exactly in sync — but this
        // view renders in image-pixel space (`toPixelRect`'s space, per the
        // type doc above), so the point size has to convert to pixels for
        // the on-screen `.font()` or it'll visibly mismatch whenever the
        // render DPI isn't ~72 (i.e. almost always, since fit-to-page picks
        // whatever DPI fills the viewport).
        let fontSizePts = TextFit.fontSize(
            for: inlineEditText, boxWidthPts: CGFloat(box.width), boxHeightPts: CGFloat(box.height)
        )
        let fontSizePx = ptsPerPixel > 0 ? fontSizePts / ptsPerPixel : fontSizePts
        // Manual "Add text" is seamless: transparent, so the user sees the
        // page underneath and exactly where the text lands, with only a thin
        // dashed guide + baseline — never a raised opaque box. A detected
        // form field keeps its solid white fill (the field is white anyway).
        let seamless = tool == .overlayText
        return TextField("", text: $inlineEditText)
            .textFieldStyle(.plain)
            .font(.system(size: fontSizePx))
            .foregroundColor(.black)
            .tint(Theme.Color.green)
            .padding(.horizontal, seamless ? 1 : 4)
            .frame(width: max(rect.width, 24), height: max(rect.height, 16), alignment: .leading)
            .background(seamless ? Color.clear : Color.white, in: RoundedRectangle(cornerRadius: Theme.Metric.fieldRadius))
            .overlay(alignment: .bottom) {
                if seamless {
                    Rectangle().fill(Theme.Color.green.opacity(0.65)).frame(height: 1)
                }
            }
            .overlay {
                if seamless {
                    RoundedRectangle(cornerRadius: 3)
                        .stroke(Theme.Color.green.opacity(0.5), style: StrokeStyle(lineWidth: 1, dash: [3, 2]))
                } else {
                    RoundedRectangle(cornerRadius: Theme.Metric.fieldRadius)
                        .stroke(Theme.Color.green, lineWidth: 2)
                        .shadow(color: Theme.Color.fieldFocusRing, radius: 4)
                }
            }
            .position(x: rect.midX, y: rect.midY)
            .focused($inlineEditFocused)
            .onSubmit { onCommitInlineEdit() }
            .onExitCommand { onCancelInlineEdit() }
            .onAppear { inlineEditFocused = true }
    }

    // MARK: - Gestures

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
