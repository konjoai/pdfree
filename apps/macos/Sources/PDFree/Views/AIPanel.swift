import AppKit
import SwiftUI

/// Summarize the current document, or ask it a question, via `pdfree-ai`.
///
/// Per CLAUDE.md's local-first/cloud-optional AI principle, the provider
/// choice defaults to on-device (Ollama) and every result is labeled with
/// where it actually ran — there's no silent default to a cloud call.
/// FFI calls into `pdfree-ai` are blocking (`reqwest::blocking`), so they're
/// dispatched off the main thread here rather than via Swift concurrency —
/// matching the rest of this app, which has no other `async`/`Task` use.
struct AIPanel: View {
    let pdfBytes: Data
    let documentTitle: String
    let onDone: () -> Void

    private enum ProviderKind: String, CaseIterable, Identifiable {
        case ollama = "On-device"
        case anthropic = "Cloud"
        var id: String { rawValue }
    }

    private enum Mode: String, CaseIterable, Identifiable {
        case summarize = "Summarize"
        case ask = "Ask a question"
        var id: String { rawValue }
    }

    @State private var mode: Mode = .summarize
    @State private var providerKind: ProviderKind
    @State private var ollamaModel: String
    @State private var anthropicApiKey: String
    @State private var question = ""
    @State private var resultText = ""
    @State private var lastRunProvider: ProviderKind?
    @State private var isLoading = false
    @State private var errorMessage: String?

    private static let ollamaModelKey = "PDFree.ai.ollamaModel"
    private static let anthropicApiKeyKey = "PDFree.ai.anthropicApiKey"
    private static let defaultOllamaModel = "qwen3:8b"

    init(pdfBytes: Data, documentTitle: String, onDone: @escaping () -> Void) {
        self.pdfBytes = pdfBytes
        self.documentTitle = documentTitle
        self.onDone = onDone
        let savedModel = UserDefaults.standard.string(forKey: Self.ollamaModelKey)
        _ollamaModel = State(initialValue: savedModel?.isEmpty == false ? savedModel! : Self.defaultOllamaModel)
        _anthropicApiKey = State(initialValue: UserDefaults.standard.string(forKey: Self.anthropicApiKeyKey) ?? "")
        _providerKind = State(initialValue: .ollama)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            header
            providerPicker
            modePicker

            if mode == .ask {
                TextField("Ask about this document…", text: $question, axis: .vertical)
                    .lineLimit(2...4)
                    .textFieldStyle(.roundedBorder)
            }

            resultArea

            if let errorMessage {
                Text(errorMessage)
                    .font(.system(size: 11.5))
                    .foregroundStyle(.red)
            }

            footer
        }
        .padding(18)
        .frame(width: 460)
        .background(Theme.Color.popoverBg)
        .tint(Theme.Color.green)
        .preferredColorScheme(.dark)
        .onChange(of: ollamaModel) { UserDefaults.standard.set($0, forKey: Self.ollamaModelKey) }
        .onChange(of: anthropicApiKey) { UserDefaults.standard.set($0, forKey: Self.anthropicApiKeyKey) }
    }

    /// The app's green-pill segmented control — the stock `.segmented`
    /// `Picker` renders with the system blue accent, which clashes with the
    /// calm-dark green theme everywhere else.
    private func segmented<T: Hashable>(_ options: [(T, String)], selection: Binding<T>) -> some View {
        HStack(spacing: 4) {
            ForEach(options, id: \.0) { value, title in
                let isSelected = selection.wrappedValue == value
                Text(title)
                    .font(.system(size: 12.5, weight: isSelected ? .semibold : .medium))
                    .foregroundStyle(isSelected ? Theme.Color.greenInk : Theme.Color.textMid)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 7)
                    .background(isSelected ? Theme.Color.green : Color.clear, in: RoundedRectangle(cornerRadius: 8))
                    .contentShape(Rectangle())
                    .onTapGesture { selection.wrappedValue = value }
            }
        }
        .padding(3)
        .background(Color.white.opacity(0.05), in: RoundedRectangle(cornerRadius: 11))
    }

    // MARK: - Sections

    private var header: some View {
        HStack {
            Image(systemName: "sparkles")
                .font(.system(size: 15))
                .foregroundStyle(Theme.Color.green)
            Text("Ask AI")
                .font(.headline)
                .foregroundStyle(Theme.Color.textHigh)
            Spacer()
        }
    }

    private var providerPicker: some View {
        VStack(alignment: .leading, spacing: 6) {
            segmented(ProviderKind.allCases.map { ($0, $0.rawValue) }, selection: $providerKind)

            switch providerKind {
            case .ollama:
                HStack(spacing: 6) {
                    Text("Model").font(.system(size: 11.5)).foregroundStyle(Theme.Color.textMid2)
                    TextField(Self.defaultOllamaModel, text: $ollamaModel)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 11.5))
                }
                Text("Processes entirely on this Mac via a local Ollama instance. Nothing leaves your device.")
                    .font(.system(size: 10.5))
                    .foregroundStyle(Theme.Color.textLow)

            case .anthropic:
                HStack(spacing: 6) {
                    Text("API key").font(.system(size: 11.5)).foregroundStyle(Theme.Color.textMid2)
                    SecureField("sk-ant-…", text: $anthropicApiKey)
                        .textFieldStyle(.roundedBorder)
                        .font(.system(size: 11.5))
                }
                Text("Sends the document's text (or an excerpt) to Anthropic's API. Only used when you choose this option.")
                    .font(.system(size: 10.5))
                    .foregroundStyle(Theme.Color.amberText)
            }
        }
    }

    private var modePicker: some View {
        segmented(Mode.allCases.map { ($0, $0.rawValue) }, selection: $mode)
    }

    private var resultArea: some View {
        VStack(alignment: .leading, spacing: 6) {
            if let lastRunProvider {
                Text(lastRunProvider == .ollama ? "Ran on-device" : "Ran via Anthropic")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(Theme.Color.textLow)
            }
            ScrollView {
                if isLoading {
                    HStack {
                        ProgressView().controlSize(.small)
                        Text("Thinking…").font(.system(size: 12)).foregroundStyle(Theme.Color.textMid2)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                } else {
                    Text(resultText.isEmpty ? "No result yet." : resultText)
                        .textSelection(.enabled)
                        .foregroundStyle(resultText.isEmpty ? Theme.Color.textLow : Theme.Color.textRow)
                        .font(.system(size: 12.5))
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(8)
                }
            }
            .frame(height: 220)
            .background(Color.white.opacity(0.04))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.white.opacity(0.08)))
        }
    }

    private var footer: some View {
        HStack(spacing: 10) {
            Button("Copy") {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(resultText, forType: .string)
            }
            .buttonStyle(.plain)
            .font(.system(size: 12.5, weight: .medium))
            .foregroundStyle(resultText.isEmpty ? Theme.Color.textLow : Theme.Color.textMid)
            .disabled(resultText.isEmpty)
            Spacer()
            Button("Close", action: onDone)
                .buttonStyle(PillButtonStyle(bg: Color.white.opacity(0.08), fg: Theme.Color.textRow))
            Button(mode == .summarize ? "Summarize" : "Ask") { run() }
                .buttonStyle(PillButtonStyle(bg: Theme.Color.green, fg: Theme.Color.greenInk, weight: .bold))
                .keyboardShortcut(.defaultAction)
                .disabled(isLoading || !canRun)
        }
    }

    private var canRun: Bool {
        switch providerKind {
        case .ollama:
            guard !ollamaModel.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        case .anthropic:
            guard !anthropicApiKey.trimmingCharacters(in: .whitespaces).isEmpty else { return false }
        }
        if mode == .ask {
            return !question.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
        }
        return true
    }

    // MARK: - Running

    private func providerConfig() -> AiProviderConfig {
        switch providerKind {
        case .ollama:
            return .ollama(model: ollamaModel, baseUrl: nil)
        case .anthropic:
            return .anthropic(apiKey: anthropicApiKey, model: nil)
        }
    }

    private func run() {
        isLoading = true
        errorMessage = nil
        resultText = ""
        let bytes = pdfBytes
        let config = providerConfig()
        let runningProvider = providerKind
        let currentMode = mode
        let askedQuestion = question

        DispatchQueue.global(qos: .userInitiated).async {
            do {
                let text: String
                switch currentMode {
                case .summarize:
                    text = try aiSummarize(pdfBytes: bytes, provider: config)
                case .ask:
                    text = try aiRagAnswer(pdfBytes: bytes, question: askedQuestion, provider: config)
                }
                DispatchQueue.main.async {
                    resultText = text
                    lastRunProvider = runningProvider
                    isLoading = false
                }
            } catch {
                DispatchQueue.main.async {
                    errorMessage = (error as? LocalizedError)?.errorDescription ?? "\(error)"
                    isLoading = false
                }
            }
        }
    }
}
