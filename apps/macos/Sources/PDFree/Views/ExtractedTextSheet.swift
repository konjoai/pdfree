import AppKit
import SwiftUI

struct ExtractedTextSheet: View {
    let text: String
    let onDone: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Extracted Text").font(.headline)
            ScrollView {
                Text(text)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
            }
            .frame(width: 480, height: 400)
            .border(Color.secondary.opacity(0.3))
            HStack {
                Button("Copy") {
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                }
                Spacer()
                Button("Done") { onDone() }.keyboardShortcut(.defaultAction)
            }
        }
        .padding()
    }
}
