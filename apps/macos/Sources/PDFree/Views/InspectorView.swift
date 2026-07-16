import SwiftUI

/// The right-hand command surface: the persistent "+" add/merge action, the
/// TOOLS group (fill / sign / annotate), the PAGES group, and a pinned
/// Export button. Tools/pages/export are dimmed and non-interactive until a
/// document is open (`store.hasDocument`).
struct InspectorView: View {
    @ObservedObject var store: PDFDocumentStore
    @Binding var tool: Tool

    let onOpen: () -> Void
    let onMerge: () -> Void
    let onInsertBlankPage: () -> Void
    let onInsertImagePage: () -> Void
    let onSplit: () -> Void
    let onRotate: () -> Void
    let onDelete: () -> Void
    let onExport: () -> Void
    let onExportPasswordProtected: () -> Void
    let onPrint: () -> Void
    let onUndo: () -> Void
    let onRedo: () -> Void
    let onSelectSign: () -> Void
    let onAskAI: () -> Void
    let onExtractText: () -> Void

    @State private var showAddMenu = false
    @State private var showDeleteConfirm = false

    var body: some View {
        VStack(alignment: .leading, spacing: 15) {
            addButton

            VStack(alignment: .leading, spacing: 15) {
                toolsGroup
                pagesGroup
                aiGroup
            }
            .opacity(store.hasDocument ? 1 : 0.45)
            .allowsHitTesting(store.hasDocument)

            if !store.hasDocument {
                Spacer()
                Text("Tools wake up\nonce a document is open.")
                    .font(.system(size: 11))
                    .foregroundStyle(Theme.Color.textLow)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
            } else {
                Spacer()
            }

            exportFooter
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 18)
        .frame(width: Theme.Metric.inspectorWidth)
        .frame(maxHeight: .infinity)
        .background(Theme.Color.panelBg)
        .overlay(Rectangle().fill(Theme.Color.hairlineFaint).frame(width: 1), alignment: .leading)
    }

    // MARK: - Add / merge

    private var addButton: some View {
        Button {
            showAddMenu = true
        } label: {
            HStack(spacing: 8) {
                Image(systemName: "plus").font(.system(size: 13, weight: .bold))
                Text("Add or merge").font(.system(size: 13.5, weight: .semibold))
            }
            .foregroundStyle(store.hasDocument ? Theme.Color.greenBadgeText : Theme.Color.greenInk)
            .frame(maxWidth: .infinity)
            .frame(height: 40)
            .background(
                store.hasDocument ? Theme.Color.greenTintPanelBg : Theme.Color.green,
                in: RoundedRectangle(cornerRadius: Theme.Metric.buttonRadius)
            )
            .overlay(
                RoundedRectangle(cornerRadius: Theme.Metric.buttonRadius)
                    .stroke(store.hasDocument ? Theme.Color.green.opacity(0.55) : .clear, lineWidth: 1)
            )
        }
        .buttonStyle(.plain)
        .popover(isPresented: $showAddMenu, arrowEdge: .top) {
            AddMenuPopover(
                onOpen: { showAddMenu = false; onOpen() },
                onMerge: { showAddMenu = false; onMerge() },
                onInsertBlankPage: { showAddMenu = false; onInsertBlankPage() },
                onInsertImagePage: { showAddMenu = false; onInsertImagePage() },
                onSplit: { showAddMenu = false; onSplit() }
            )
        }
    }

    // MARK: - Tools

    private var toolsGroup: some View {
        VStack(alignment: .leading, spacing: 2) {
            sectionLabel("TOOLS")
            toolRow(
                title: "Fill fields", systemImage: "rectangle.and.pencil.and.ellipsis",
                isActive: tool == .fill, badge: store.formFieldsList.isEmpty ? nil : "\(store.formFieldsList.count)",
                badgeTint: .green
            ) { tool = .fill }
            toolRow(
                title: "Sign", systemImage: "signature",
                isActive: tool == .sign, badge: store.signatureFields.isEmpty ? nil : "\(store.signatureFields.count)",
                badgeTint: .amber
            ) { tool = .sign; onSelectSign() }
            toolRow(
                title: "Add text", systemImage: "textformat",
                isActive: tool == .overlayText
            ) { tool = .overlayText }
            // Annotate enters annotate mode (defaulting to highlight); the
            // floating toolbar on the canvas is where the user switches
            // between highlight/underline/strikeout/note without a popover.
            toolRow(
                title: "Annotate", systemImage: "highlighter",
                isActive: tool.isAnnotation
            ) { if !tool.isAnnotation { tool = .highlight } }
        }
    }

    // MARK: - Pages

    private var pagesGroup: some View {
        VStack(alignment: .leading, spacing: 2) {
            sectionLabel("PAGES")
            actionRow(title: "Rotate", systemImage: "arrow.clockwise", action: onRotate)
            deleteRow
        }
    }

    /// Delete never fires directly — it opens a small confirm popover first,
    /// so a page can't vanish on a single misclick.
    private var deleteRow: some View {
        actionRow(title: "Delete page", systemImage: "trash") { showDeleteConfirm = true }
            .popover(isPresented: $showDeleteConfirm, arrowEdge: .leading) {
                VStack(alignment: .leading, spacing: 12) {
                    Text("Delete page \(store.pageIndex + 1)?")
                        .font(.system(size: 13, weight: .semibold))
                        .foregroundStyle(Theme.Color.textHigh)
                    Text("This removes it from the document. You can undo by re-importing the original.")
                        .font(.system(size: 11))
                        .foregroundStyle(Theme.Color.textMid)
                        .fixedSize(horizontal: false, vertical: true)
                    HStack(spacing: 8) {
                        Spacer()
                        Button("Cancel") { showDeleteConfirm = false }
                            .buttonStyle(PillButtonStyle(bg: Color.white.opacity(0.08), fg: Theme.Color.textRow))
                        Button("Delete") { showDeleteConfirm = false; onDelete() }
                            .buttonStyle(PillButtonStyle(bg: Theme.Color.trafficRed, fg: .white, weight: .semibold))
                    }
                }
                .padding(16)
                .frame(width: 240)
                .background(Theme.Color.popoverBg)
            }
    }

    // MARK: - AI

    private var aiGroup: some View {
        VStack(alignment: .leading, spacing: 2) {
            sectionLabel("AI")
            actionRow(title: "Ask AI", systemImage: "sparkles", action: onAskAI)
            actionRow(title: "Extract Text", systemImage: "doc.text.magnifyingglass", action: onExtractText)
        }
    }

    // MARK: - Rows

    private func sectionLabel(_ text: String) -> some View {
        Text(text)
            .font(Theme.Font.sectionLabel)
            .tracking(1.2)
            .foregroundStyle(Theme.Color.textLow)
            .padding(.horizontal, 4)
            .padding(.bottom, 8)
            .padding(.top, 2)
    }

    private func toolRow(
        title: String, systemImage: String, isActive: Bool, badge: String? = nil,
        badgeTint: BadgeTint = .green, action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 11) {
                Image(systemName: systemImage)
                    .font(.system(size: 14))
                    .foregroundStyle(isActive ? (badgeTint == .amber ? Theme.Color.amber : Theme.Color.green) : Theme.Color.textMid2)
                    .frame(width: 18)
                Text(title)
                    .font(isActive ? Theme.Font.inspectorRowActive : Theme.Font.inspectorRowIdle)
                    .foregroundStyle(isActive ? Theme.Color.textHigh : Theme.Color.textRow)
                Spacer()
                if let badge {
                    Text(badge)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(badgeTint == .amber ? Theme.Color.amber : Theme.Color.greenBadgeText)
                        .padding(.horizontal, 8).padding(.vertical, 2)
                        .background(
                            (badgeTint == .amber ? Theme.Color.amber : Theme.Color.green).opacity(0.22),
                            in: Capsule()
                        )
                }
            }
            .padding(.horizontal, 11).padding(.vertical, 10)
            .background(isActive ? Theme.Color.greenToolActiveBg : .clear, in: RoundedRectangle(cornerRadius: Theme.Metric.inspectorRowRadius))
            .contentShape(Rectangle())
        }
        .buttonStyle(RowHoverButtonStyle())
        .animation(Theme.Anim.rowHighlight, value: isActive)
    }

    private func actionRow(title: String, systemImage: String, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            HStack(spacing: 11) {
                Image(systemName: systemImage).font(.system(size: 14)).foregroundStyle(Theme.Color.textMid2).frame(width: 18)
                Text(title).font(Theme.Font.inspectorRowIdle).foregroundStyle(Theme.Color.textRow)
                Spacer()
            }
            .padding(.horizontal, 11).padding(.vertical, 10)
            .contentShape(Rectangle())
        }
        .buttonStyle(RowHoverButtonStyle())
    }

    enum BadgeTint { case green, amber }

    // MARK: - Export

    private var exportFooter: some View {
        VStack(spacing: 8) {
            HStack(spacing: 6) {
                HStack(spacing: 2) {
                    Button(action: onUndo) {
                        Image(systemName: "arrow.uturn.backward")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(Theme.Color.textRow)
                            .frame(width: 28, height: 44)
                    }
                    .buttonStyle(.plain)
                    .disabled(!store.canUndo)
                    .opacity(store.canUndo ? 1 : 0.35)
                    .help("Undo (⌘Z)")
                    .keyboardShortcut("z", modifiers: .command)
                    .accessibilityLabel("Undo")

                    Button(action: onRedo) {
                        Image(systemName: "arrow.uturn.forward")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(Theme.Color.textRow)
                            .frame(width: 28, height: 44)
                    }
                    .buttonStyle(.plain)
                    .disabled(!store.canRedo)
                    .opacity(store.canRedo ? 1 : 0.35)
                    .help("Redo (⇧⌘Z)")
                    .keyboardShortcut("z", modifiers: [.command, .shift])
                    .accessibilityLabel("Redo")
                }
                .background(Color.white.opacity(0.07), in: RoundedRectangle(cornerRadius: 11))

                Button(action: onPrint) {
                    Image(systemName: "printer")
                        .font(.system(size: 15, weight: .semibold))
                        .foregroundStyle(Theme.Color.textRow)
                        .frame(width: 44, height: 44)
                        .background(Color.white.opacity(0.07), in: RoundedRectangle(cornerRadius: 11))
                }
                .buttonStyle(.plain)
                .disabled(!store.hasDocument)
                .opacity(store.hasDocument ? 1 : 0.45)
                .help("Print (⌘P)")
                .keyboardShortcut("p", modifiers: .command)
                .accessibilityLabel("Print")

                Button(action: onExport) {
                    HStack(spacing: 8) {
                        Image(systemName: "square.and.arrow.up").font(.system(size: 15, weight: .semibold))
                        Text("Export").font(Theme.Font.primaryButton)
                    }
                    .foregroundStyle(Theme.Color.greenInk)
                    .frame(maxWidth: .infinity)
                    .frame(height: 44)
                    .background(Theme.Color.green, in: RoundedRectangle(cornerRadius: 11))
                    .shadow(color: Theme.Color.green.opacity(0.7), radius: 5, y: 2)
                }
                .buttonStyle(.plain)
                .disabled(!store.hasDocument)
                .opacity(store.hasDocument ? 1 : 0.45)
                // Password protection is a secondary/advanced path (Core UX
                // Principle #6: nothing beyond the common path competes for
                // attention on the default surface) — right-click for it
                // rather than a second permanent button crowding this row.
                .contextMenu {
                    Button {
                        onExportPasswordProtected()
                    } label: {
                        Label("Export Password-Protected PDF…", systemImage: "lock.doc")
                    }
                }
            }

            Text("No watermark · no limits · saved locally")
                .font(.system(size: 10.5))
                .foregroundStyle(Theme.Color.textLow)
        }
    }
}

/// Subtle hover highlight for idle inspector rows, matching the design's
/// `hover: rgba(255,255,255,.05)`.
private struct RowHoverButtonStyle: ButtonStyle {
    @State private var hovering = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(hovering ? Color.white.opacity(0.05) : .clear, in: RoundedRectangle(cornerRadius: Theme.Metric.inspectorRowRadius))
            .onHover { hovering = $0 }
    }
}
