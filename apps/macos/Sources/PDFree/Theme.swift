import AppKit
import SwiftUI

/// Design tokens for the calm-dark, right-inspector redesign — see
/// `design_handoff_swiftui_redesign/README.md` for the reference values.
/// Every color/metric the UI uses should come from here rather than a
/// literal, so the look stays consistent as views are added.
enum Theme {
    // MARK: - Color

    enum Color {
        static let titlebarTop = SwiftUI.Color(hex: 0x2A2723)
        static let titlebarBottom = SwiftUI.Color(hex: 0x242118)
        static let panelBg = SwiftUI.Color(hex: 0x201D1A)
        static let railBg = SwiftUI.Color(hex: 0x1A1815)
        static let canvasTop = SwiftUI.Color(hex: 0x161311)
        static let canvasBottom = SwiftUI.Color(hex: 0x0F0D0B)
        static let popoverBg = SwiftUI.Color(hex: 0x252220)
        static let menuBg = SwiftUI.Color(hex: 0x2B2724)

        static let hairline = SwiftUI.Color.white.opacity(0.07)
        static let hairlineFaint = SwiftUI.Color.white.opacity(0.06)

        static let textHigh = SwiftUI.Color(hex: 0xF3EFE8)
        static let textMid = SwiftUI.Color(hex: 0xA49C90)
        static let textMid2 = SwiftUI.Color(hex: 0x8F887D)
        static let textLow = SwiftUI.Color(hex: 0x6F6860)
        static let textRow = SwiftUI.Color(hex: 0xD8D2C8)

        static let green = SwiftUI.Color(hex: 0x37C07A)
        static let greenDark = SwiftUI.Color(hex: 0x2EA36A)
        static let greenFaint = SwiftUI.Color(hex: 0x9ED9BC)
        static let greenBright = SwiftUI.Color(hex: 0x5FD699)
        static let greenBadgeText = SwiftUI.Color(hex: 0x7FE0AA)
        static let greenChipText = SwiftUI.Color(hex: 0x6FDCA2)
        static let greenInk = SwiftUI.Color(hex: 0x08130C)
        static let greenGradientStart = SwiftUI.Color(hex: 0x42D089)
        static let greenGradientEnd = SwiftUI.Color(hex: 0x279A60)

        static let amber = SwiftUI.Color(hex: 0xE8B45A)
        static let amberInk = SwiftUI.Color(hex: 0x4A3200)
        static let amberText = SwiftUI.Color(hex: 0x9A6C1E)

        static let trafficRed = SwiftUI.Color(hex: 0xFF5F57)
        static let trafficYellow = SwiftUI.Color(hex: 0xFEBC2E)
        static let trafficGreen = SwiftUI.Color(hex: 0x28C840)

        static let signatureInk = SwiftUI.Color(hex: 0x1A2B6B)

        static let fieldFillWash = green.opacity(0.10)
        static let fieldBorder = green.opacity(0.6)
        static let fieldFocusRing = green.opacity(0.18)

        static let greenTintPanelBg = green.opacity(0.15)
        static let greenBadgeBg = green.opacity(0.22)
        static let greenChipBg = green.opacity(0.14)
        static let greenChipBorder = green.opacity(0.4)
        static let greenToolActiveBg = green.opacity(0.13)

        static let amberFieldWash = amber.opacity(0.12)
        static let amberBadgeBg = amber.opacity(0.16)
    }

    // MARK: - Typography

    enum Font {
        static func wordmarkPD(_ size: CGFloat) -> SwiftUI.Font { .system(size: size, weight: .heavy) }
        static func wordmarkFree(_ size: CGFloat) -> SwiftUI.Font { .system(size: size, weight: .bold) }
        static let titlebarTitle = SwiftUI.Font.system(size: 13, weight: .semibold)
        static let sectionLabel = SwiftUI.Font.system(size: 10, weight: .semibold)
        static let inspectorRowIdle = SwiftUI.Font.system(size: 13.5, weight: .medium)
        static let inspectorRowActive = SwiftUI.Font.system(size: 13.5, weight: .semibold)
        static let primaryButton = SwiftUI.Font.system(size: 14, weight: .bold)
        static let overlayChip = SwiftUI.Font.system(size: 11, weight: .semibold)
        static let pageNav = SwiftUI.Font.system(size: 12, weight: .medium)

        /// The signature/initials cursive preview. Tries known cursive
        /// system fonts in order and falls back to an italic system font if
        /// none are installed, so this never silently renders upright.
        static func cursive(_ size: CGFloat) -> SwiftUI.Font {
            let candidates = ["SnellRoundhand", "Snell Roundhand", "BrushScriptMT", "Brush Script MT", "Zapfino"]
            for name in candidates where NSFont(name: name, size: size) != nil {
                return .custom(name, size: size)
            }
            return .system(size: size, weight: .regular).italic()
        }
    }

    // MARK: - Metrics

    enum Metric {
        static let windowRadius: CGFloat = 13
        static let cardRadius: CGFloat = 15
        static let buttonRadius: CGFloat = 10.5
        static let inspectorRowRadius: CGFloat = 9
        static let pillRadius: CGFloat = 999
        static let fieldRadius: CGFloat = 5
        static let railWidth: CGFloat = 132
        static let inspectorWidth: CGFloat = 274
        static let thumbnailSize = CGSize(width: 88, height: 114)
        static let canvasPagePadding: CGFloat = 30
        static let titlebarHeight: CGFloat = 44
    }

    // MARK: - Animation

    enum Anim {
        static let hop = SwiftUI.Animation.spring(response: 0.6, dampingFraction: 0.72)
        static let focusRing = SwiftUI.Animation.easeInOut(duration: 0.3)
        static let rowHighlight = SwiftUI.Animation.easeInOut(duration: 0.15)
    }
}

extension SwiftUI.Color {
    init(hex: UInt32, opacity: Double = 1) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: opacity
        )
    }
}

/// The `pd·free` wordmark: "pd" in high-contrast off-white immediately
/// followed by a green pill containing "free". Used small in the titlebar
/// and large in the empty-state hero.
struct Wordmark: View {
    enum Size {
        case small
        case large

        var pd: CGFloat { self == .small ? 13 : 34 }
        var free: CGFloat { self == .small ? 11 : 25 }
        var gap: CGFloat { self == .small ? 2 : 3 }
        var pillPadding: (v: CGFloat, h: CGFloat) { self == .small ? (2, 7) : (5, 14) }
    }

    var size: Size = .large

    var body: some View {
        HStack(spacing: size.gap) {
            Text("pd")
                .font(.system(size: size.pd, weight: .heavy))
                .tracking(-0.6)
                .foregroundStyle(size == .small ? Theme.Color.textRow : Theme.Color.textHigh)
            Text("free")
                .font(.system(size: size.free, weight: .bold))
                .tracking(-0.3)
                .foregroundStyle(Theme.Color.greenInk)
                .padding(.vertical, size.pillPadding.v)
                .padding(.horizontal, size.pillPadding.h)
                .background(Theme.Color.green, in: Capsule())
        }
    }
}

/// The document-silhouette app mark: a folded-corner page with three
/// horizontal bars, in a tile (green background, white page+bars) or a
/// standalone document (green page, white bars) fill.
struct AppMark: View {
    enum Style { case tile, document }
    var style: Style = .tile
    var size: CGFloat = 52

    private var corner: CGFloat { size * 0.17 }

    var body: some View {
        Group {
            switch style {
            case .tile:
                RoundedRectangle(cornerRadius: size * 0.24)
                    .fill(LinearGradient(
                        colors: [Theme.Color.greenGradientStart, Theme.Color.greenGradientEnd],
                        startPoint: .topLeading, endPoint: .bottomTrailing
                    ))
                    .overlay(pageShape.fill(.white).padding(size * 0.18))
            case .document:
                pageShape.fill(LinearGradient(
                    colors: [Theme.Color.greenGradientStart, Theme.Color.greenGradientEnd],
                    startPoint: .topLeading, endPoint: .bottomTrailing
                ))
            }
        }
        .frame(width: size, height: size * 1.23)
        .overlay(bars, alignment: .center)
    }

    private var pageShape: some Shape {
        FoldedCornerPage(cornerFraction: 0.28)
    }

    private var bars: some View {
        let barColor = style == .tile ? Theme.Color.greenDark : SwiftUI.Color.white
        return VStack(alignment: .leading, spacing: size * 0.09) {
            Capsule().fill(barColor).frame(height: size * 0.07)
            Capsule().fill(barColor).frame(width: size * 0.5, height: size * 0.07)
            Capsule().fill(style == .tile ? Theme.Color.greenFaint : .white.opacity(0.55))
                .frame(width: size * 0.36, height: size * 0.07)
        }
        .padding(.horizontal, size * (style == .tile ? 0.3 : 0.16))
    }
}

/// A page silhouette with the top-right corner folded — approximates the
/// design's `clip-path: polygon(0 0,70% 0,100% 22%,100% 100%,0 100%)`.
private struct FoldedCornerPage: Shape {
    var cornerFraction: CGFloat

    func path(in rect: CGRect) -> Path {
        var path = Path()
        let foldX = rect.minX + rect.width * 0.7
        let foldY = rect.minY + rect.height * cornerFraction * 0.78
        path.move(to: CGPoint(x: rect.minX, y: rect.minY))
        path.addLine(to: CGPoint(x: foldX, y: rect.minY))
        path.addLine(to: CGPoint(x: rect.maxX, y: foldY))
        path.addLine(to: CGPoint(x: rect.maxX, y: rect.maxY))
        path.addLine(to: CGPoint(x: rect.minX, y: rect.maxY))
        path.closeSubpath()
        return path
    }
}
