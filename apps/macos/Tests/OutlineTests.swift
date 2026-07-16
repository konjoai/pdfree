import XCTest

/// Exercises `PDFDocumentStore.documentOutline` against `outline_sample.pdf` —
/// a 3-page fixture (generated via `pypdf`, shared with `pdfree-core`'s own
/// `tests/bookmarks.rs`) with a known two-level outline: "Chapter 1" (page 0)
/// -> "Section 1.1" (page 1) as its only child, then "Chapter 2" (page 2) as
/// a second top-level sibling with no children.
@MainActor
final class OutlineTests: XCTestCase {
    func testDocumentWithNoOutlineReportsEmptyList() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        XCTAssertTrue(store.documentOutline.isEmpty)
    }

    func testReadsTopLevelTitlesInDocumentOrder() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("outline_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { !store.documentOutline.isEmpty }

        XCTAssertEqual(store.documentOutline.count, 2)
        XCTAssertEqual(store.documentOutline[0].title, "Chapter 1")
        XCTAssertEqual(store.documentOutline[1].title, "Chapter 2")
    }

    func testResolvesEachBookmarkToItsDestinationPage() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("outline_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { !store.documentOutline.isEmpty }

        XCTAssertEqual(store.documentOutline[0].page, 0)
        XCTAssertEqual(store.documentOutline[1].page, 2)
    }

    func testNestsChildBookmarksUnderTheirParent() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("outline_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { !store.documentOutline.isEmpty }

        let chapter1 = store.documentOutline[0]
        XCTAssertEqual(chapter1.children.count, 1)
        XCTAssertEqual(chapter1.children[0].title, "Section 1.1")
        XCTAssertEqual(chapter1.children[0].page, 1)

        let chapter2 = store.documentOutline[1]
        XCTAssertTrue(chapter2.children.isEmpty)
    }

    func testTappingABookmarkJumpsToItsPage() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("outline_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { !store.documentOutline.isEmpty }

        store.goToPage(store.documentOutline[0].children[0].page!)
        XCTAssertEqual(store.pageIndex, 1)
    }

    func testOutlineResetsOnDocumentClose() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("outline_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { !store.documentOutline.isEmpty }

        store.closeDocument()
        XCTAssertTrue(store.documentOutline.isEmpty)
    }

    /// Mirrors `SearchTests`'/`PDFDocumentStoreTests`' own private helpers —
    /// kept local rather than shared, per the existing convention in this
    /// test target.
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
