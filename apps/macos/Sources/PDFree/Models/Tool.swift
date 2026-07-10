import Foundation

/// The active canvas interaction mode. Drag-based tools draw a rectangle;
/// everything else is a single click at a point.
enum Tool: String, CaseIterable, Identifiable {
    /// The default, non-editing view mode — no field affordances drawn, so a
    /// non-fillable PDF just reads cleanly until the user picks a tool.
    case select
    /// Field-fill mode: entered by the "Fill fields" button, this is what
    /// reveals the detected fillable-field overlays and click-to-fill.
    case fill
    case highlight
    case underline
    case strikeout
    case note
    case sign
    case editText
    case overlayText

    var id: String { rawValue }

    var label: String {
        switch self {
        case .select: return "Select"
        case .fill: return "Fill fields"
        case .highlight: return "Highlight"
        case .underline: return "Underline"
        case .strikeout: return "Strikeout"
        case .note: return "Note"
        case .sign: return "Sign"
        case .editText: return "Edit Text"
        case .overlayText: return "Add Text"
        }
    }

    var systemImage: String {
        switch self {
        case .select: return "cursorarrow"
        case .fill: return "rectangle.and.pencil.and.ellipsis"
        case .highlight: return "highlighter"
        case .underline: return "underline"
        case .strikeout: return "strikethrough"
        case .note: return "note.text"
        case .sign: return "signature"
        case .editText: return "pencil"
        case .overlayText: return "textformat"
        }
    }

    /// Drag-based tools draw a bounding rectangle; everything else is a tap.
    var isDragBased: Bool {
        self == .highlight || self == .underline || self == .strikeout
    }

    /// The annotation family, surfaced together by the canvas's floating
    /// annotate toolbar.
    var isAnnotation: Bool {
        self == .highlight || self == .underline || self == .strikeout || self == .note
    }
}
