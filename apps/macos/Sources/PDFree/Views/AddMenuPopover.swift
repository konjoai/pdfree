import SwiftUI

/// The "+" quick-action menu — import / merge / insert / split, never a
/// File-menu trip (Core UX Principles). Opened from the inspector's
/// "Add or merge" button.
struct AddMenuPopover: View {
    let onOpen: () -> Void
    let onMerge: () -> Void
    let onInsertBlankPage: () -> Void
    let onInsertImagePage: () -> Void
    let onSplit: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            row(icon: "folder", title: "Open a PDF…", subtitle: "Replace what's open", accent: true, action: onOpen)
            row(icon: "arrow.triangle.merge", title: "Merge another PDF…", subtitle: "Append to the end", action: onMerge)
            row(icon: "doc.badge.plus", title: "Insert blank page", action: onInsertBlankPage)
            row(icon: "photo", title: "Image as a page…", action: onInsertImagePage)
            Divider().padding(.vertical, 5).padding(.horizontal, 10)
            row(icon: "arrow.triangle.branch", title: "Split or extract pages…", subtitle: "Pick a range → new file", action: onSplit)
        }
        .padding(6)
        .frame(width: 260)
        .background(Theme.Color.menuBg)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color.white.opacity(0.12)))
    }

    private func row(
        icon: String, title: String, subtitle: String? = nil, accent: Bool = false, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 10) {
                Image(systemName: icon)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(accent ? Theme.Color.green : Theme.Color.textRow)
                    .frame(width: 20)
                VStack(alignment: .leading, spacing: 1) {
                    Text(title)
                        .font(.system(size: 12.5, weight: accent ? .semibold : .medium))
                        .foregroundStyle(Theme.Color.textHigh)
                    if let subtitle {
                        Text(subtitle).font(.system(size: 10.5)).foregroundStyle(Theme.Color.textMid2)
                    }
                }
                Spacer()
            }
            .padding(.horizontal, 10).padding(.vertical, 9)
            .background(accent ? Color.white.opacity(0.05) : .clear, in: RoundedRectangle(cornerRadius: 8))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
