import SwiftUI

/// Prompts for a password before exporting an encrypted copy of the
/// document. Requires the password to be entered twice (a plain-text field
/// with no confirmation is an easy way to lock yourself out of your own
/// export with a typo) and disables the action until both match and are
/// non-empty.
struct PasswordExportSheet: View {
    let onExport: (String) -> Void
    let onCancel: () -> Void

    @State private var password = ""
    @State private var confirmPassword = ""
    @FocusState private var focused: Bool

    private var isValid: Bool {
        !password.isEmpty && password == confirmPassword
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("Export Password-Protected PDF")
                .font(.system(size: 15, weight: .semibold))
                .foregroundStyle(Theme.Color.textHigh)

            Text("Anyone opening this file will need the password below. PDFree can't recover a lost password — no watermark, no limits, but no back door either.")
                .font(.system(size: 11.5))
                .foregroundStyle(Theme.Color.textMid)
                .fixedSize(horizontal: false, vertical: true)

            VStack(alignment: .leading, spacing: 8) {
                SecureField("Password", text: $password)
                    .focused($focused)
                SecureField("Confirm password", text: $confirmPassword)
                    .onSubmit { if isValid { onExport(password) } }
            }
            .textFieldStyle(.roundedBorder)

            if !confirmPassword.isEmpty, password != confirmPassword {
                Text("Passwords don't match.")
                    .font(.system(size: 11))
                    .foregroundStyle(Theme.Color.trafficRed)
            }

            HStack(spacing: 8) {
                Spacer()
                Button("Cancel", action: onCancel)
                    .buttonStyle(PillButtonStyle(bg: Color.white.opacity(0.08), fg: Theme.Color.textRow))
                Button("Export") { onExport(password) }
                    .buttonStyle(PillButtonStyle(bg: Theme.Color.green, fg: Theme.Color.greenInk, weight: .semibold))
                    .disabled(!isValid)
            }
        }
        .padding(20)
        .frame(width: 320)
        .background(Theme.Color.popoverBg)
        .onAppear { focused = true }
    }
}
