import SwiftUI

/// Text fields and checkboxes are the only kinds `pdfree-core` can write
/// (see docs/api.md's `FillValue` note) — dropdowns/list boxes/radios/
/// signature fields show read-only for now.
struct FormsPanel: View {
    @ObservedObject var store: PDFDocumentStore
    let onDone: () -> Void

    @State private var textValues: [String: String] = [:]
    @State private var checkboxValues: [String: Bool] = [:]

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Fill Form Fields").font(.headline).foregroundStyle(Theme.Color.textHigh)
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(store.formFieldsList, id: \.name) { field in
                        fieldRow(field)
                    }
                }
            }
            HStack {
                Spacer()
                Button("Cancel") { onDone() }
                Button("Apply") { apply() }.keyboardShortcut(.defaultAction)
            }
        }
        .padding()
        .frame(width: 420, height: 480)
        .background(Theme.Color.popoverBg)
        .onAppear(perform: seed)
    }

    private func seed() {
        for field in store.formFieldsList {
            switch field.kind {
            case .text: textValues[field.name] = field.value ?? ""
            case .checkbox: checkboxValues[field.name] = field.value == "true"
            default: break
            }
        }
    }

    @ViewBuilder
    private func fieldRow(_ field: FormField) -> some View {
        switch field.kind {
        case .text:
            VStack(alignment: .leading, spacing: 2) {
                Text(field.name).font(.caption).foregroundStyle(.secondary)
                TextField("", text: Binding(
                    get: { textValues[field.name] ?? "" },
                    set: { textValues[field.name] = $0 }
                ))
            }
        case .checkbox:
            Toggle(field.name, isOn: Binding(
                get: { checkboxValues[field.name] ?? false },
                set: { checkboxValues[field.name] = $0 }
            ))
        default:
            VStack(alignment: .leading, spacing: 2) {
                Text(field.name).font(.caption).foregroundStyle(.secondary)
                Text("\(field.value ?? "—")  ·  \(kindLabel(field.kind))")
                    .font(.callout)
                    .foregroundStyle(.tertiary)
            }
        }
    }

    private func kindLabel(_ kind: FieldKind) -> String {
        switch kind {
        case .dropdown: return "dropdown (read-only)"
        case .listBox: return "list box (read-only)"
        case .radioButton: return "radio (read-only)"
        case .signature: return "signature field (read-only)"
        case .pushButton: return "button"
        default: return "unsupported"
        }
    }

    private func apply() {
        var fills: [FieldFill] = []
        for (name, value) in textValues {
            fills.append(FieldFill(name: name, value: .text(value: value)))
        }
        for (name, checked) in checkboxValues {
            fills.append(FieldFill(name: name, value: .checkbox(checked: checked)))
        }
        store.applyFormFill(fills)
        onDone()
    }
}
