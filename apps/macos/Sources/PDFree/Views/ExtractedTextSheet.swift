import AppKit
import SwiftUI

struct ExtractedTextSheet: View {
    let text: String
    let onDone: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Extracted Text").font(.headline).foregroundStyle(Theme.Color.textHigh)
            ScrollView {
                Text(text)
                    .textSelection(.enabled)
                    .foregroundStyle(Theme.Color.textRow)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
            }
            .frame(width: 480, height: 400)
            .background(Color.white.opacity(0.04))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.white.opacity(0.08)))
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
        .background(Theme.Color.popoverBg)
    }
}
