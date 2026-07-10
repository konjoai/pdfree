import XCTest

// No `@testable import` — this target compiles PDFree's own sources
// directly (see project.yml's comment on `PDFreeTests`), so its types are
// already in this module, not a separate one to import.

/// `TextFit` is what guarantees WYSIWYG between the live inline editor and
/// the exported `overlay_text` stamp (see `TextFit.swift`'s doc comment for
/// why this matters) — worth pinning down with real assertions since a
/// silent regression here would be a WYSIWYG bug, not a crash.
final class TextFitTests: XCTestCase {
    func testShortTextUsesTheHeightBasedSize() {
        // Well within both bounds: governed by box height, clamped to 18pt.
        let size = TextFit.fontSize(for: "Hi", boxWidthPts: 200, boxHeightPts: 30)
        XCTAssertEqual(size, 18, accuracy: 0.01)
    }

    func testShortBoxClampsToTheHeightFloor() {
        let size = TextFit.fontSize(for: "Hi", boxWidthPts: 200, boxHeightPts: 4)
        XCTAssertEqual(size, 7, accuracy: 0.01, "must never go below the legibility floor")
    }

    func testEmptyTextUsesTheHeightBoundWithNoWidthMeasurement() {
        let size = TextFit.fontSize(for: "", boxWidthPts: 1, boxHeightPts: 30)
        XCTAssertEqual(size, 18, accuracy: 0.01, "empty text has no width to overflow")
    }

    func testLongTextShrinksToFitTheBoxWidth() {
        let narrow = TextFit.fontSize(
            for: "Wolfeschlegelsteinhausenbergerdorff", boxWidthPts: 100, boxHeightPts: 30
        )
        let wide = TextFit.fontSize(
            for: "Wolfeschlegelsteinhausenbergerdorff", boxWidthPts: 400, boxHeightPts: 30
        )
        XCTAssertLessThan(narrow, wide, "the same text must render smaller in a narrower box")
        XCTAssertLessThan(narrow, 18, "long text in a narrow box should shrink below the height-only bound")
    }

    func testNeverShrinksBelowTheLegibilityFloorEvenForExtremelyLongText() {
        let text = String(repeating: "M", count: 500)
        let size = TextFit.fontSize(for: text, boxWidthPts: 50, boxHeightPts: 30)
        XCTAssertEqual(size, 7, accuracy: 0.01)
    }

    func testIsDeterministic() {
        // The whole point: identical inputs must always produce identical
        // output, since the live editor and the export path both call this
        // independently and must agree.
        let a = TextFit.fontSize(for: "Nakamura", boxWidthPts: 120, boxHeightPts: 26)
        let b = TextFit.fontSize(for: "Nakamura", boxWidthPts: 120, boxHeightPts: 26)
        XCTAssertEqual(a, b)
    }
}
