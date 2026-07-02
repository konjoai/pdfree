import SwiftUI

/// Generic single-text-field sheet, reused for sticky notes, text overlays,
/// and in-place text replacement.
struct TextPromptSheet: View {
    let title: String
    let placeholder: String
    var initialText: String = ""
    let onSubmit: (String) -> Void
    let onCancel: () -> Void

    @State private var text: String = ""

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            TextField(placeholder, text: $text, axis: .vertical)
                .lineLimit(3...6)
                .textFieldStyle(.roundedBorder)
            HStack {
                Spacer()
                Button("Cancel") { onCancel() }
                Button("OK") { onSubmit(text) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding()
        .frame(width: 360)
        .onAppear { text = initialText }
    }
}
