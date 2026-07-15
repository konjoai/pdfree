import SwiftUI

/// Returning-user sign flow: a compact card anchored at the clicked
/// signature/initials field, showing saved marks as tap-to-place chips.
/// After a placement, the caller re-anchors this view at the next pending
/// field (`PageCanvasView` animates the move with `Theme.Anim.hop`) and
/// updates `progress`; when nothing is left, `done` switches to a
/// confirmation with a reset.
struct SignPopover: View {
    let kind: SavedSignature.Kind
    let savedSignatures: [SavedSignature]
    let progress: (current: Int, total: Int)
    let done: Bool

    let onPlace: (SavedSignature) -> Void
    let onDrawNew: () -> Void
    let onType: () -> Void
    let onUpload: () -> Void
    /// Dismiss the whole sign box. Available mid-session too (the header X),
    /// so one party can place a few marks and close it to hand off to the
    /// next signer without having to fill every field first.
    let onClose: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            header
            if done {
                doneView
            } else {
                pendingView
            }
        }
        .padding(20)
        // Wide enough for 3 92pt chips (9pt gaps) plus real breathing room on
        // both sides — at the old 300pt width, minus the 20pt padding on
        // each side, there wasn't even enough room to fit the chips
        // themselves (3×92 + 2×9 = 294pt needed vs. 260pt available), let
        // alone any margin, so they rendered cramped and nearly clipped.
        .frame(width: 360)
        .background(Theme.Color.popoverBg)
        .clipShape(RoundedRectangle(cornerRadius: 16))
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Color.white.opacity(0.1)))
        .shadow(color: .black.opacity(0.6), radius: 34, y: 14)
    }

    private var header: some View {
        HStack(spacing: 8) {
            Text(kind == .initials ? "Add initials" : "Add signature")
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(Theme.Color.textHigh)
            Spacer()
            if !done {
                Text("\(progress.current) / \(progress.total)")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(Theme.Color.green)
                    .padding(.horizontal, 10).padding(.vertical, 3)
                    .background(Theme.Color.green.opacity(0.14), in: Capsule())
            }
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(Theme.Color.textMid)
                    .frame(width: 22, height: 22)
                    .background(Color.white.opacity(0.07), in: Circle())
            }
            .buttonStyle(.plain)
            .help("Close")
        }
        .padding(.bottom, 14)
    }

    private var pendingView: some View {
        VStack(alignment: .leading, spacing: 9) {
            Text("TAP TO PLACE YOUR SAVED MARK")
                .font(.system(size: 9.5, weight: .medium))
                .tracking(0.5)
                .foregroundStyle(Theme.Color.textLow)

            HStack(spacing: 9) {
                ForEach(savedSignatures.prefix(3)) { signature in
                    chip(signature)
                }
                if savedSignatures.isEmpty {
                    Text("None saved yet").font(.system(size: 11)).foregroundStyle(Theme.Color.textMid)
                }
            }

            HStack(spacing: 7) {
                secondaryButton("Draw new", systemImage: "signature", action: onDrawNew)
                secondaryButton("Type", action: onType)
                secondaryButton("Upload", systemImage: "square.and.arrow.up", action: onUpload)
            }
            .padding(.top, 2)
        }
    }

    private func chip(_ signature: SavedSignature) -> some View {
        Button {
            onPlace(signature)
        } label: {
            Group {
                if let image = NSImage(data: signature.pngData) {
                    Image(nsImage: image).resizable().scaledToFit().padding(9)
                } else {
                    Image(systemName: "signature")
                }
            }
            .frame(width: 92, height: 65)
            .background(Color.white, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(Theme.Color.green, lineWidth: signature == savedSignatures.first ? 2 : 0))
        }
        .buttonStyle(.plain)
    }

    private func secondaryButton(_ title: String, systemImage: String? = nil, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 5) {
                if let systemImage { Image(systemName: systemImage).font(.system(size: 10.5)) }
                Text(title).font(.system(size: 11.5, weight: .semibold))
            }
            .foregroundStyle(Theme.Color.textRow)
            .frame(maxWidth: .infinity)
            .padding(.vertical, 9)
            .background(Color.white.opacity(0.07), in: RoundedRectangle(cornerRadius: 9))
        }
        .buttonStyle(.plain)
    }

    private var doneView: some View {
        VStack(spacing: 9) {
            ZStack {
                Circle().fill(Theme.Color.green.opacity(0.16)).frame(width: 44, height: 44)
                Image(systemName: "checkmark").font(.system(size: 17, weight: .bold)).foregroundStyle(Theme.Color.green)
            }
            Text("Everything's signed").font(.system(size: 13, weight: .semibold)).foregroundStyle(Theme.Color.textHigh)
            Text("Saved locally · time & name recorded")
                .font(.system(size: 10, weight: .regular))
                .foregroundStyle(Theme.Color.textLow)
                .multilineTextAlignment(.center)
            Button("Done", action: onClose)
                .buttonStyle(PillButtonStyle(bg: Theme.Color.green, fg: Theme.Color.greenInk, weight: .semibold))
                .padding(.top, 2)
        }
        .padding(.vertical, 4)
        .frame(maxWidth: .infinity)
    }
}
