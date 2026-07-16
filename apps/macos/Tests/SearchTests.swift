import XCTest

/// Exercises `PDFDocumentStore.search`/`goToNextSearchMatch`/
/// `goToPreviousSearchMatch` against `search_sample.pdf` — a 2-page fixture
/// with real (uncompressed text-object) content: 3 occurrences of "fox"
/// total, spread across both pages (confirmed by generating and inspecting
/// the fixture directly against `pdfree_core::search::find_text` before
/// committing it — see git history for the one-off generator).
@MainActor
final class SearchTests: XCTestCase {
    func testSearchFindsEveryMatchAcrossBothPages() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("search_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        store.search(query: "fox")
        waitUntil { !store.searchMatches.isEmpty }

        XCTAssertEqual(store.searchMatches.count, 3)
        XCTAssertEqual(Set(store.searchMatches.map(\.page)), Set([0, 1]), "matches should span both pages")
        XCTAssertEqual(store.currentSearchMatchIndex, 0, "a fresh search selects the first match")
    }

    func testSearchJumpsToTheFirstMatchsPage() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("search_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        store.search(query: "fox")
        waitUntil { !store.searchMatches.isEmpty }

        XCTAssertEqual(store.pageIndex, store.searchMatches[0].page)
    }

    func testNextAndPreviousMatchWrapAround() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("search_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        store.search(query: "fox")
        waitUntil { !store.searchMatches.isEmpty }

        let count = store.searchMatches.count
        for _ in 0..<count {
            store.goToNextSearchMatch()
        }
        // Advancing exactly `count` times from index 0 should land back on 0.
        XCTAssertEqual(store.currentSearchMatchIndex, 0)

        store.goToPreviousSearchMatch()
        XCTAssertEqual(store.currentSearchMatchIndex, count - 1, "stepping back from 0 wraps to the last match")
    }

    func testEmptyQueryClearsResults() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("search_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        store.search(query: "fox")
        waitUntil { !store.searchMatches.isEmpty }

        store.search(query: "")
        XCTAssertTrue(store.searchMatches.isEmpty)
        XCTAssertNil(store.currentSearchMatchIndex)
    }

    func testNoMatchesLeavesCurrentIndexNil() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("search_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        store.search(query: "nonexistentword")
        // Give the background search a moment to actually run and report
        // back — there's no truthy condition to wait on for "found nothing",
        // so wait for the (already fast) round trip via a short deadline.
        let deadline = Date().addingTimeInterval(2)
        while Date() < deadline { RunLoop.main.run(until: Date().addingTimeInterval(0.02)) }

        XCTAssertTrue(store.searchMatches.isEmpty)
        XCTAssertNil(store.currentSearchMatchIndex)
    }

    /// Mirrors `PDFDocumentStoreTests`' own private helpers — kept local
    /// rather than shared, since making them internal purely for one other
    /// test file isn't worth widening their visibility.
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
