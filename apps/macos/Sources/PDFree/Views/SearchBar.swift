import SwiftUI

/// Floating "⌘F" find bar: a text field, live match count, and prev/next
/// navigation. Search itself runs off the main thread
/// (`PDFDocumentStore.search`), so updating the query on every keystroke —
/// the same live-highlight-while-typing convention Preview and every
/// browser use — doesn't stutter the UI.
struct SearchBar: View {
    @ObservedObject var store: PDFDocumentStore
    @Binding var query: String
    var isFocused: FocusState<Bool>.Binding
    let onClose: () -> Void

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: "magnifyingglass")
                .font(.system(size: 12))
                .foregroundStyle(Theme.Color.textMid)

            TextField("Find in document", text: $query)
                .textFieldStyle(.plain)
                .font(.system(size: 13))
                .foregroundStyle(Theme.Color.textHigh)
                .focused(isFocused)
                .frame(width: 180)
                .onSubmit { store.goToNextSearchMatch() }
                .onExitCommand(perform: onClose)

            if !query.isEmpty {
                Text(countLabel)
                    .font(.system(size: 11.5, weight: .medium))
                    .foregroundStyle(Theme.Color.textLow)
                    .monospacedDigit()
                    .fixedSize()

                HStack(spacing: 2) {
                    Button { store.goToPreviousSearchMatch() } label: {
                        Image(systemName: "chevron.up")
                    }
                    .help("Previous match")
                    Button { store.goToNextSearchMatch() } label: {
                        Image(systemName: "chevron.down")
                    }
                    .help("Next match")
                }
                .buttonStyle(.plain)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(Theme.Color.textMid)
                .disabled(store.searchMatches.isEmpty)
                .opacity(store.searchMatches.isEmpty ? 0.4 : 1)
            }

            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(Theme.Color.textMid)
                    .frame(width: 20, height: 20)
                    .background(Color.white.opacity(0.07), in: Circle())
            }
            .buttonStyle(.plain)
            .help("Close (Esc)")
        }
        .padding(.horizontal, 12).padding(.vertical, 9)
        .background(Theme.Color.popoverBg, in: RoundedRectangle(cornerRadius: 10))
        .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.1)))
        .shadow(color: .black.opacity(0.4), radius: 16, y: 6)
    }

    private var countLabel: String {
        guard !store.searchMatches.isEmpty else { return "No results" }
        let current = (store.currentSearchMatchIndex ?? 0) + 1
        return "\(current)/\(store.searchMatches.count)"
    }
}
