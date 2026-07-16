import SwiftUI

/// A floating, single-column toolbar that hovers over the right edge of the
/// document while annotating — replaces the old inspector popover so switching
/// between highlight / underline / strikeout / note stays one click away while
/// marking up the page. A cursor button at the bottom returns to Select and
/// dismisses the toolbar.
struct AnnotationToolbar: View {
    @Binding var tool: Tool

    private struct Item: Identifiable {
        let tool: Tool
        let icon: String
        let help: String
        var id: String { tool.rawValue }
    }

    private let items: [Item] = [
        Item(tool: .highlight, icon: "highlighter", help: "Highlight"),
        Item(tool: .underline, icon: "underline", help: "Underline"),
        Item(tool: .strikeout, icon: "strikethrough", help: "Strikeout"),
        Item(tool: .note, icon: "note.text", help: "Sticky note"),
        Item(tool: .rectangle, icon: "rectangle", help: "Rectangle"),
        Item(tool: .circle, icon: "circle", help: "Circle"),
        Item(tool: .line, icon: "line.diagonal", help: "Line"),
        Item(tool: .arrow, icon: "arrow.up.right", help: "Arrow"),
        Item(tool: .ink, icon: "pencil.tip", help: "Freehand draw"),
    ]

    var body: some View {
        VStack(spacing: 6) {
            ForEach(items) { item in
                button(icon: item.icon, help: item.help, active: tool == item.tool) {
                    tool = item.tool
                }
            }
            Rectangle().fill(Theme.Color.hairline).frame(height: 1).padding(.horizontal, 4)
            button(icon: "cursorarrow", help: "Done annotating", active: false) {
                tool = .select
            }
        }
        .padding(6)
        .background(Theme.Color.popoverBg, in: RoundedRectangle(cornerRadius: 13))
        .overlay(RoundedRectangle(cornerRadius: 13).stroke(Color.white.opacity(0.1)))
        .shadow(color: .black.opacity(0.5), radius: 18, y: 8)
    }

    private func button(icon: String, help: String, active: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(active ? Theme.Color.greenInk : Theme.Color.textRow)
                .frame(width: 34, height: 34)
                .background(active ? Theme.Color.green : Color.white.opacity(0.05), in: RoundedRectangle(cornerRadius: 9))
        }
        .buttonStyle(.plain)
        .help(help)
    }
}
