import XCTest

/// Pure unit tests for `ContinuousScrollView.nearestPageToTop` — no view
/// rendering or PDFium involved, so these never need to skip.
final class ContinuousScrollViewTests: XCTestCase {
    func testPicksThePageWhoseTopHasJustScrolledPastTheThreshold() {
        // Page 2's top is just below the threshold (40 <= 80); page 3's top
        // is still further down; page 1 has already scrolled past (negative).
        let offsets: [UInt16: CGFloat] = [1: -400, 2: 40, 3: 820]
        XCTAssertEqual(ContinuousScrollView.nearestPageToTop(offsets: offsets), 2)
    }

    func testPicksTheLargestOffsetAtOrBelowTheThresholdNotJustAnyMatch() {
        // Both page 0 and page 1 are above the threshold; page 1's top is
        // closer to (but still at-or-below) the threshold, so it's current.
        let offsets: [UInt16: CGFloat] = [0: -900, 1: 10, 2: 900]
        XCTAssertEqual(ContinuousScrollView.nearestPageToTop(offsets: offsets), 1)
    }

    func testFallsBackToClosestToZeroWhenNothingHasScrolledPastTheThresholdYet() {
        // Right after opening: every page's top is still below the
        // threshold (nothing has been scrolled up to the top edge yet).
        let offsets: [UInt16: CGFloat] = [0: 120, 1: 950]
        XCTAssertEqual(ContinuousScrollView.nearestPageToTop(offsets: offsets), 0)
    }

    func testSinglePageDocumentReturnsThatPage() {
        let offsets: [UInt16: CGFloat] = [0: 16]
        XCTAssertEqual(ContinuousScrollView.nearestPageToTop(offsets: offsets), 0)
    }

    func testEmptyOffsetsReturnsNil() {
        XCTAssertNil(ContinuousScrollView.nearestPageToTop(offsets: [:]))
    }

    func testExactlyAtThresholdCounts() {
        let offsets: [UInt16: CGFloat] = [0: -50, 1: 80, 2: 500]
        XCTAssertEqual(ContinuousScrollView.nearestPageToTop(offsets: offsets, threshold: 80), 1)
    }
}
