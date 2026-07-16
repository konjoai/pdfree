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
    case rectangle
    case circle
    case line
    case arrow
    case ink
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
        case .rectangle: return "Rectangle"
        case .circle: return "Circle"
        case .line: return "Line"
        case .arrow: return "Arrow"
        case .ink: return "Draw"
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
        case .rectangle: return "rectangle"
        case .circle: return "circle"
        case .line: return "line.diagonal"
        case .arrow: return "arrow.up.right"
        case .ink: return "pencil.tip"
        case .sign: return "signature"
        case .editText: return "pencil"
        case .overlayText: return "textformat"
        }
    }

    /// Drag-based tools draw a bounding rectangle or a two-point line by
    /// dragging from start to end; everything else (including `.ink`, which
    /// tracks the whole freehand path, not just two points) is handled
    /// separately.
    var isDragBased: Bool {
        self == .highlight || self == .underline || self == .strikeout
            || self == .rectangle || self == .circle || self == .line || self == .arrow
    }

    /// `Line`/`Arrow` need the drag's actual start/end points (direction
    /// matters), not a reordered bounding box the way the box-shaped kinds
    /// use.
    var isLineBased: Bool {
        self == .line || self == .arrow
    }

    /// The annotation family, surfaced together by the canvas's floating
    /// annotate toolbar.
    var isAnnotation: Bool {
        self == .highlight || self == .underline || self == .strikeout || self == .note
            || self == .rectangle || self == .circle || self == .line || self == .arrow
            || self == .ink
    }
}
