import AppKit

/// Deterministic shrink-to-fit font sizing for the inline field editor —
/// computed once, in PDF points, and used identically for the live
/// `TextField` and the exported `overlay_text` stamp, so what's on screen
/// while editing is exactly what exports (Core UX Principles: "WYSIWYG text
/// sizing, always"). `pdfree-core`'s `overlay_text` draws literally at
/// whatever `font_size` it's given — it does no wrapping or clipping of its
/// own — so the shell owning this calculation is what makes the guarantee
/// hold, not the engine.
enum TextFit {
    /// Font never renders larger than this, or smaller than this — matches
    /// the box-height-based clamp already in use before width was
    /// considered, so single-character/short fills keep their prior sizing.
    private static let maxFontSize: CGFloat = 18
    private static let minFontSize: CGFloat = 7
    /// Horizontal breathing room subtracted from the box width before
    /// measuring, so text doesn't render flush against the box edge.
    private static let horizontalInset: CGFloat = 4

    /// The largest font size (in PDF points) that fits `text` inside a box
    /// `boxWidthPts` × `boxHeightPts`, without exceeding `maxFontSize`.
    static func fontSize(for text: String, boxWidthPts: CGFloat, boxHeightPts: CGFloat) -> CGFloat {
        let heightBound = max(minFontSize, min(boxHeightPts * 0.7, maxFontSize))
        guard !text.isEmpty else { return heightBound }

        let available = boxWidthPts - horizontalInset
        guard available > 0 else { return heightBound }

        let font = NSFont(name: "Helvetica", size: heightBound) ?? .systemFont(ofSize: heightBound)
        let measuredWidth = (text as NSString).size(withAttributes: [.font: font]).width
        guard measuredWidth > available else { return heightBound }

        let scale = available / measuredWidth
        return max(minFontSize, heightBound * scale)
    }
}
