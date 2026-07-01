import Foundation

/// The active canvas interaction mode. Drag-based tools draw a rectangle;
/// everything else is a single click at a point.
enum Tool: String, CaseIterable, Identifiable {
    case select
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
}
