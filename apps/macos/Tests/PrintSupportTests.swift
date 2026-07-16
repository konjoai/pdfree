import PDFKit
import XCTest

/// `printDocument()` in `ContentView` builds a throwaway `PDFKit.PDFDocument`
/// over the store's raw bytes purely to reach
/// `printOperation(for:scalingMode:autoRotate:)` — this doesn't need
/// `PDFium` at all, so unlike most of this test target it never skips.
/// Deliberately never calls `.run()`/`.runModal(for:)` on the resulting
/// `NSPrintOperation` — that would show a real, blocking system print panel,
/// which has no place in an automated test run. Constructing the operation
/// (without running it) is enough to confirm the integration is wired
/// correctly: a real document parses and produces a real operation.
final class PrintSupportTests: XCTestCase {
    func testPDFKitBuildsAPrintOperationFromRealDocumentBytes() throws {
        guard let url = Bundle(for: Self.self).url(forResource: "form_sample", withExtension: "pdf") else {
            XCTFail("missing fixture form_sample.pdf in the test bundle")
            return
        }
        let data = try Data(contentsOf: url)

        let pdfDocument = try XCTUnwrap(PDFDocument(data: data), "PDFKit should parse a real PDF's bytes")
        XCTAssertGreaterThan(pdfDocument.pageCount, 0)

        let operation = pdfDocument.printOperation(
            for: .shared, scalingMode: .pageScaleToFit, autoRotate: true
        )
        XCTAssertNotNil(operation, "printOperation(for:scalingMode:autoRotate:) should succeed for a real document")
    }

    func testPDFKitReturnsNilForUnparseableBytes() {
        let garbage = Data("not a pdf".utf8)
        XCTAssertNil(PDFDocument(data: garbage), "printDocument() relies on this being nil to silently no-op")
    }
}
