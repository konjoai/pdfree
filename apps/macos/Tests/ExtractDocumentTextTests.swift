import XCTest

/// Exercises `PDFDocumentStore.extractDocumentText` — real embedded-text
/// extraction, and the OCR-on-current-page fallback for a document with no
/// text layer at all (the "Extract Text on a scanned PDF used to silently
/// return nothing" gap this feature closes). Skips (rather than fails) when
/// `PDFium`/`tesseract` aren't available, matching this test target's
/// existing convention for optional external tools.
@MainActor
final class ExtractDocumentTextTests: XCTestCase {
    func testExtractsRealEmbeddedTextWhenPresent() throws {
        let store = try openOrSkip("search_sample")

        let expectation = expectation(description: "extraction completes")
        var result: PDFDocumentStore.TextExtractionResult?
        store.extractDocumentText { result = $0; expectation.fulfill() }
        wait(for: [expectation], timeout: 5)

        guard case .documentText(let text) = result else {
            XCTFail("expected real document text, got \(String(describing: result))")
            return
        }
        XCTAssertTrue(text.contains("fox"), "search_sample.pdf's known content")
    }

    func testFallsBackToOcrWhenTheDocumentHasNoTextLayer() throws {
        // form_sample.pdf has only AcroForm widget dictionaries and no real
        // page text content (confirmed via `strings` inspection when this
        // fixture was first used for search testing) — exactly the "scanned,
        // no text layer" case this fallback exists for.
        let store = try openOrSkip("form_sample")

        let expectation = expectation(description: "extraction completes")
        var result: PDFDocumentStore.TextExtractionResult?
        store.extractDocumentText { result = $0; expectation.fulfill() }
        wait(for: [expectation], timeout: 10)

        switch result {
        case .ocrCurrentPage:
            break // expected: no text layer, fell back to OCR
        case .documentText(let text):
            XCTFail("empty document text (\"\(text)\") should trigger the OCR fallback, not pass through")
        case nil:
            throw XCTSkip("tesseract not found on PATH — install it to enable this assertion")
        }
    }

    private func openOrSkip(_ name: String) throws -> PDFDocumentStore {
        let store = PDFDocumentStore()
        guard let url = Bundle(for: Self.self).url(forResource: name, withExtension: "pdf") else {
            throw XCTSkip("missing fixture \(name).pdf in the test bundle")
        }
        let data = try Data(contentsOf: url)
        guard (try? PdfDocument.fromBytes(data: data)) != nil else {
            throw XCTSkip("PDFium library not found — run scripts/fetch-pdfium.sh to enable")
        }
        store.openReplacing(data: data, url: nil)

        let deadline = Date().addingTimeInterval(5)
        while !store.hasDocument && Date() < deadline {
            RunLoop.main.run(until: Date().addingTimeInterval(0.02))
        }
        XCTAssertTrue(store.hasDocument)
        return store
    }
}
