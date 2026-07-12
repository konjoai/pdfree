import XCTest

// No `@testable import` — see TextFitTests.swift's note.

/// Exercises `PDFDocumentStore` against two small fixtures compiled into this
/// bundle (`Tests/Fixtures/`): `form_sample.pdf` (a plain text field +
/// checkbox, no signature fields — mirrors `pdfree-core`'s own
/// `tests/fixtures/form_sample.pdf`) and `signature_fields.pdf` (a
/// synthetically-built AcroForm with `signature_1`/`initials_1`/`full_name`
/// text fields and *no* vector graphics at all — built the same way as the
/// fixture used to manually verify the macOS redesign, since neither of
/// `pdfree-core`'s real-world fixtures happens to contain a true
/// signature-kind or sign-named field).
///
/// Deliberately doesn't exercise `saveSignature`/saved-signature persistence
/// — that writes real files under `~/Library/Application Support/PDFree/`,
/// and a test suite shouldn't leave side effects in the user's actual app
/// data directory with no way to sandbox it.
@MainActor
final class PDFDocumentStoreTests: XCTestCase {
    func testStartsWithNoDocument() {
        let store = PDFDocumentStore()
        XCTAssertFalse(store.hasDocument)
        XCTAssertEqual(store.pageCount, 0)
        XCTAssertEqual(store.title, "Untitled")
    }

    func testOpenReplacingLoadsFormFields() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")

        store.openReplacing(data: data, url: nil)
        // `openReplacing` parses off the main thread now, so wait for the
        // whole-document field list to land before asserting on it.
        waitUntil { store.formFieldsList.count == 2 }

        XCTAssertTrue(store.hasDocument)
        XCTAssertNil(store.errorMessage)
        XCTAssertEqual(store.pageCount, 1)
        XCTAssertEqual(store.formFieldsList.count, 2)
        XCTAssertTrue(store.formFieldsList.contains { $0.name == "FullName" })
        XCTAssertTrue(store.signatureFields.isEmpty, "form_sample has no signature/initials fields")
    }

    func testOpenReplacingClassifiesSignatureAndInitialsFields() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("signature_fields")

        store.openReplacing(data: data, url: nil)
        waitUntil { !store.formFieldsList.isEmpty }

        let fields = store.signatureFields
        XCTAssertEqual(fields.count, 2, "signature_1 + initials_1, not full_name")
        XCTAssertTrue(fields.contains { $0.name == "signature_1" && $0.signatureKind == .signature })
        XCTAssertTrue(fields.contains { $0.name == "initials_1" && $0.signatureKind == .initials })
        XCTAssertFalse(fields.contains { $0.name == "full_name" })
    }

    func testFieldOverlaysIncludeEveryAcroFormFieldEvenWithNoDetectedBox() throws {
        let store = PDFDocumentStore()
        // This fixture has raw AcroForm widgets and zero vector graphics.
        // `pdfree_core::fields::fillable_fields` always reports real AcroForm
        // widgets (that's the "never miss a fillable field" guarantee), so
        // every field — including ones no drawn box surrounds — gets an
        // overlay, with signature/initials fields marked distinctly.
        let data = try openOrSkip("signature_fields")

        store.openReplacing(data: data, url: nil)
        waitUntil { !store.fieldOverlays.isEmpty }

        let overlayNames = Set(store.fieldOverlays.compactMap(\.fieldName))
        XCTAssertTrue(overlayNames.contains("signature_1"))
        XCTAssertTrue(overlayNames.contains("initials_1"))
        XCTAssertTrue(overlayNames.contains("full_name"))
        XCTAssertTrue(
            store.fieldOverlays.allSatisfy { $0.fieldName != "signature_1" || $0.isSignature },
            "signature_1's overlay must be marked as a signature field, not a normal one"
        )
        XCTAssertTrue(
            store.fieldOverlays.allSatisfy { $0.fieldName != "full_name" || !$0.isSignature },
            "full_name is an ordinary field, not a signature one"
        )
    }

    func testCloseDocumentResetsToEmptyState() throws {
        let store = PDFDocumentStore()
        store.openReplacing(data: try openOrSkip("form_sample"), url: nil)
        waitUntil { store.hasDocument }
        XCTAssertTrue(store.hasDocument)

        store.closeDocument()

        XCTAssertFalse(store.hasDocument)
        XCTAssertEqual(store.pageCount, 0)
        XCTAssertTrue(store.formFieldsList.isEmpty)
        XCTAssertTrue(store.fieldOverlays.isEmpty)
        XCTAssertNil(store.pageImage)
    }

    func testBoxContainingIsNilWithNoDocument() {
        let store = PDFDocumentStore()
        // `boxContaining` hit-tests `fieldOverlays`, which is empty with no
        // document open, so any point resolves to nothing.
        XCTAssertNil(store.boxContaining(x: 0, y: 0), "no document loaded, nothing to contain the point")
    }

    // MARK: - Helpers

    /// Pump the main run loop until `condition` holds or `timeout` elapses.
    /// The store's open/render/scan work runs on a background queue and
    /// republishes on the main queue, so a test has to let the run loop turn
    /// for those results to arrive.
    private func waitUntil(
        timeout: TimeInterval = 5,
        _ condition: () -> Bool,
        file: StaticString = #filePath,
        line: UInt = #line
    ) {
        let deadline = Date().addingTimeInterval(timeout)
        while !condition() && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.02))
        }
        XCTAssertTrue(condition(), "condition not met within \(timeout)s", file: file, line: line)
    }

    /// Loads a bundled fixture, skipping (not failing) the test if the
    /// PDFium dylib isn't available in this environment — mirrors
    /// `pdfree-core`'s own tests, which do the same for exactly the same
    /// reason (see `scripts/fetch-pdfium.sh`).
    private func openOrSkip(_ name: String) throws -> Data {
        guard let url = Bundle(for: Self.self).url(forResource: name, withExtension: "pdf") else {
            XCTFail("missing fixture \(name).pdf in the test bundle")
            throw XCTSkip("fixture not found")
        }
        let data = try Data(contentsOf: url)
        guard (try? PdfDocument.fromBytes(data: data)) != nil else {
            throw XCTSkip("PDFium library not found — run scripts/fetch-pdfium.sh to enable")
        }
        return data
    }
}
