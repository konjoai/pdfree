import XCTest

/// Exercises `PDFDocumentStore.undo`/`redo` against real mutations (rotate,
/// delete page) run through the actual FFI on `form_sample.pdf` — not a mock,
/// since the whole point is verifying the byte-snapshot stack round-trips
/// through a real `pdfree-core` mutation and reload.
@MainActor
final class UndoRedoTests: XCTestCase {
    func testFreshlyOpenedDocumentHasNothingToUndo() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        XCTAssertFalse(store.canUndo)
        XCTAssertFalse(store.canRedo)
    }

    func testUndoRestoresThePreMutationBytes() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        let originalPageCount = store.pageCount

        store.rotate(page: 0, rotation: .clockwise90)
        waitUntil(timeout: 5) { store.data != originalData }
        XCTAssertTrue(store.canUndo)

        store.undo()
        waitUntil(timeout: 5) { store.data == originalData }

        XCTAssertEqual(store.data, originalData)
        XCTAssertEqual(store.pageCount, originalPageCount)
        XCTAssertFalse(store.canUndo, "one mutation, one undo — the stack should be empty again")
        XCTAssertTrue(store.canRedo)
    }

    func testRedoReappliesTheUndoneMutation() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.rotate(page: 0, rotation: .clockwise90)
        waitUntil(timeout: 5) { store.data != originalData }
        let rotatedData = store.data

        store.undo()
        waitUntil(timeout: 5) { store.data == originalData }

        store.redo()
        waitUntil(timeout: 5) { store.data == rotatedData }

        XCTAssertEqual(store.data, rotatedData)
        XCTAssertTrue(store.canUndo)
        XCTAssertFalse(store.canRedo)
    }

    func testANewMutationClearsRedoHistory() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.rotate(page: 0, rotation: .clockwise90)
        waitUntil(timeout: 5) { store.data != originalData }

        store.undo()
        waitUntil(timeout: 5) { store.data == originalData }
        XCTAssertTrue(store.canRedo)

        store.rotate(page: 0, rotation: .clockwise270)
        waitUntil(timeout: 5) { store.data != originalData }

        XCTAssertFalse(store.canRedo, "a fresh edit forks away from the old redo history")
    }

    func testUndoRedoAreNoOpsWithNothingOnTheStack() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.undo()
        store.redo()

        XCTAssertEqual(store.data, originalData)
        XCTAssertFalse(store.canUndo)
        XCTAssertFalse(store.canRedo)
    }

    func testUndoHistoryResetsOnDocumentClose() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.rotate(page: 0, rotation: .clockwise90)
        waitUntil(timeout: 5) { store.data != originalData }
        XCTAssertTrue(store.canUndo)

        store.closeDocument()
        XCTAssertFalse(store.canUndo)
        XCTAssertFalse(store.canRedo)
    }

    func testUndoHistoryResetsOnOpeningAnotherDocument() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.rotate(page: 0, rotation: .clockwise90)
        waitUntil(timeout: 5) { store.data != originalData }
        XCTAssertTrue(store.canUndo)

        let otherData = try openOrSkip("search_sample")
        store.openReplacing(data: otherData, url: nil)
        waitUntil { store.data == otherData }

        XCTAssertFalse(store.canUndo)
        XCTAssertFalse(store.canRedo)
    }

    /// Mirrors the other test files' own private helpers — kept local rather
    /// than shared, per the existing convention in this test target.
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
