import SwiftUI
import UniformTypeIdentifiers

/// First launch / no document open. The drop surface *is* the window — no
/// bundled sample auto-loads (Core UX Principles / design handoff: "the
/// drop surface IS the window"). Accepts drag-drop of a PDF/PNG/JPEG and
/// click-to-browse; both hand off to `store.openReplacing`.
struct EmptyStateView: View {
    @ObservedObject var store: PDFDocumentStore
    let onOpen: () -> Void

    @State private var isTargeted = false

    var body: some View {
        ZStack {
            RadialGradient(
                colors: [Theme.Color.canvasTop, Theme.Color.canvasBottom],
                center: .init(x: 0.5, y: 0.15), startRadius: 1, endRadius: 700
            )

            VStack(spacing: 24) {
                // Green paper mark above the pd·free wordmark, centered in the
                // whole window as the branding hero.
                VStack(spacing: 16) {
                    AppMark(style: .document, size: 52)
                        .shadow(color: Theme.Color.greenDark.opacity(0.5), radius: 14, y: 8)
                    Wordmark(size: .large)
                }

                VStack(spacing: 14) {
                    Text("Drop a PDF or image to start")
                        .font(.system(size: 19, weight: .bold))
                        .foregroundStyle(Theme.Color.textHigh)

                    (
                        Text("or ")
                            .foregroundStyle(Theme.Color.textMid2)
                            + Text("browse your Mac").foregroundStyle(Theme.Color.green).fontWeight(.semibold)
                            + Text(" — everything stays on your device").foregroundStyle(Theme.Color.textMid2)
                    )
                    .font(.system(size: 13))
                }
                .padding(.vertical, 40)
                .padding(.horizontal, 30)
                .frame(maxWidth: .infinity)
                .background(Theme.Color.green.opacity(0.05))
                .overlay(
                    RoundedRectangle(cornerRadius: 18)
                        .stroke(Theme.Color.green.opacity(isTargeted ? 0.9 : 0.5), style: StrokeStyle(lineWidth: 2, dash: [7, 5]))
                )
                .clipShape(RoundedRectangle(cornerRadius: 18))
                .contentShape(RoundedRectangle(cornerRadius: 18))
                .onTapGesture(perform: onOpen)
                .onDrop(of: [.fileURL], isTargeted: $isTargeted, perform: handleDrop)

                if !store.recentFiles.isEmpty {
                    recentRow
                }
            }
            .frame(width: 520)
        }
    }

    private var recentRow: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("RECENT")
                .font(.system(size: 10, weight: .semibold))
                .tracking(1)
                .foregroundStyle(Theme.Color.textLow)
            HStack(spacing: 10) {
                ForEach(store.recentFiles.prefix(2), id: \.self) { url in
                    recentChip(url)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func recentChip(_ url: URL) -> some View {
        Button {
            if let data = try? Data(contentsOf: url) {
                store.openReplacing(data: data, url: url)
            }
        } label: {
            HStack(spacing: 10) {
                RoundedRectangle(cornerRadius: 3).fill(.white).frame(width: 22, height: 28)
                Text(url.lastPathComponent)
                    .font(.system(size: 12.5, weight: .medium))
                    .foregroundStyle(Theme.Color.textRow)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            .padding(.horizontal, 12).padding(.vertical, 10)
            .frame(maxWidth: .infinity, alignment: .leading)
            .background(Theme.Color.panelBg, in: RoundedRectangle(cornerRadius: 10))
            .overlay(RoundedRectangle(cornerRadius: 10).stroke(Color.white.opacity(0.07)))
        }
        .buttonStyle(.plain)
    }

    private func handleDrop(_ providers: [NSItemProvider]) -> Bool {
        guard let provider = providers.first else { return false }
        _ = provider.loadObject(ofClass: URL.self) { url, _ in
            guard let url, let data = try? Data(contentsOf: url) else { return }
            DispatchQueue.main.async {
                store.openReplacing(data: data, url: url)
            }
        }
        return true
    }
}
