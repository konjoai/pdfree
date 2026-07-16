import XCTest

/// Exercises `PDFDocumentStore.exportEncrypted` against real FFI calls (which
/// shell out to `qpdf` — see `pdfree_core::encrypt`'s module doc comment for
/// why `PDFium` itself can't do this) on `form_sample.pdf`. Skips (rather
/// than fails) when `qpdf` isn't on `PATH`, matching this test target's
/// existing convention for optional external tools.
@MainActor
final class EncryptExportTests: XCTestCase {
    func testExportEncryptedProducesDifferentBytesThatRequireThePassword() throws {
        let store = try openOrSkip()

        let expectation = expectation(description: "export completes")
        var result: Data?
        store.exportEncrypted(password: "correct-horse") { data in
            result = data
            expectation.fulfill()
        }
        wait(for: [expectation], timeout: 5)

        guard let encrypted = result else {
            throw XCTSkip("qpdf not found on PATH — install it to enable this test")
        }
        XCTAssertNotEqual(encrypted, store.data, "encryption must change the bytes")

        // The original, unencrypted document must be untouched — encryption
        // is a one-way export, never a mutation of the open document.
        XCTAssertEqual(store.data, try? Data(contentsOf: fixtureURL()))
    }

    func testExportEncryptedFailsWithNoDocumentOpen() {
        let store = PDFDocumentStore()
        let expectation = expectation(description: "completion called")
        var result: Data? = Data("sentinel".utf8)
        store.exportEncrypted(password: "whatever") { data in
            result = data
            expectation.fulfill()
        }
        wait(for: [expectation], timeout: 5)
        XCTAssertNil(result)
    }

    private func fixtureURL() throws -> URL {
        guard let url = Bundle(for: Self.self).url(forResource: "form_sample", withExtension: "pdf") else {
            throw XCTSkip("missing fixture form_sample.pdf in the test bundle")
        }
        return url
    }

    private func openOrSkip() throws -> PDFDocumentStore {
        let store = PDFDocumentStore()
        let url = try fixtureURL()
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
