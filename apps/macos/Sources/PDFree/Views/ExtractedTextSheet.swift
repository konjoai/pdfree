import AppKit
import SwiftUI

struct ExtractedTextSheet: View {
    let text: String
    /// Set when this text came from OCR on the current page rather than the
    /// document's own embedded text layer — surfaced so the user knows why
    /// results might be page-scoped or imperfect (OCR, not a real text
    /// layer), not silently presented as if it were exact extracted text.
    var viaOCR = false
    let onDone: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Extracted Text").font(.headline).foregroundStyle(Theme.Color.textHigh)
            if viaOCR {
                Text("This document has no embedded text layer — recognized via OCR on the current page only.")
                    .font(.system(size: 11.5))
                    .foregroundStyle(Theme.Color.textMid)
            }
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
