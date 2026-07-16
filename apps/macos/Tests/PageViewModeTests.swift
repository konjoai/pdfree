import XCTest

/// Exercises `PDFDocumentStore`'s continuous-scroll-mode support: the
/// `pageViewMode` toggle, `pageJumpToken`'s explicit-jump-vs-scroll
/// distinction, and `continuousPageImage`'s lazy/cached rendering — against
/// real FFI calls on `form_sample.pdf`, not mocks.
@MainActor
final class PageViewModeTests: XCTestCase {
    func testDefaultsToSinglePageMode() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        XCTAssertEqual(store.pageViewMode, .single)
    }

    func testGoToPageBumpsTheJumpTokenButDirectAssignmentDoesNot() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("search_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let tokenBeforeJump = store.pageJumpToken
        store.goToPage(1)
        XCTAssertEqual(store.pageJumpToken, tokenBeforeJump + 1, "explicit navigation bumps the jump token")

        let tokenBeforeScroll = store.pageJumpToken
        store.pageIndex = 0
        XCTAssertEqual(
            store.pageJumpToken, tokenBeforeScroll,
            "scroll-driven pageIndex updates must NOT bump the jump token, or continuous scroll would snap back mid-drag"
        )
    }

    func testContinuousPageImageIsNilUntilTheBackgroundRenderCompletes() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        XCTAssertNil(store.continuousPageImage(at: 0, viewportWidth: 600))
        waitUntil(timeout: 5) { store.continuousPageImage(at: 0, viewportWidth: 600) != nil }
        XCTAssertNotNil(store.continuousPageImage(at: 0, viewportWidth: 600))
    }

    func testContinuousPageImagePopulatesTheCachedPageSize() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        _ = store.continuousPageImage(at: 0, viewportWidth: 600)
        waitUntil(timeout: 5) { store.cachedPageSize(at: 0) != nil }

        let size = try XCTUnwrap(store.cachedPageSize(at: 0))
        XCTAssertGreaterThan(size.width, 0)
        XCTAssertGreaterThan(size.height, 0)
    }

    func testContinuousPageImageCacheIsInvalidatedByAMutation() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        _ = store.continuousPageImage(at: 0, viewportWidth: 600)
        waitUntil(timeout: 5) { store.continuousPageImage(at: 0, viewportWidth: 600) != nil }

        store.rotate(page: 0, rotation: .clockwise90)
        waitUntil(timeout: 5) { store.canUndo }
        // Right after a mutation the continuous-image cache was cleared, so
        // the very next call must go back to `nil` (kick a fresh render)
        // rather than silently return the pre-rotation image.
        XCTAssertNil(store.continuousPageImage(at: 0, viewportWidth: 600))
    }

    func testContinuousPageImageReturnsNilForZeroViewportWidth() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        XCTAssertNil(store.continuousPageImage(at: 0, viewportWidth: 0))
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
