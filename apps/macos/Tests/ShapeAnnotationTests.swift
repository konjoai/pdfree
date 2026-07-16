import XCTest

/// Exercises `PDFDocumentStore.applyAnnotation` for the shape/freehand kinds
/// (Rectangle/Circle/Line/Arrow/Ink) against real FFI calls on
/// `form_sample.pdf` — confirms they actually mutate the document and that
/// `annotationsList` reflects them afterward (Shape for the four Stamp-backed
/// kinds, Ink for its own real annotation type — see
/// `pdfree_core::annotations`' module doc comment for why they read back
/// differently).
@MainActor
final class ShapeAnnotationTests: XCTestCase {
    func testRectangleAnnotationMutatesTheDocumentAndListsAsShape() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.applyAnnotation(Annotation(
            page: 0, kind: .rectangle,
            x: 72, y: 600, width: 100, height: 50,
            color: nil, note: nil, points: []
        ))
        waitUntil(timeout: 5) { store.data != originalData }

        waitUntil(timeout: 5) { store.annotationsList.contains { $0.kind == .shape } }
        XCTAssertTrue(store.annotationsList.contains { $0.kind == .shape })
    }

    func testCircleAnnotationMutatesTheDocument() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.applyAnnotation(Annotation(
            page: 0, kind: .circle,
            x: 72, y: 600, width: 100, height: 100,
            color: nil, note: nil, points: []
        ))
        waitUntil(timeout: 5) { store.data != originalData }
    }

    func testLineAnnotationMutatesTheDocumentAndListsAsShape() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.applyAnnotation(Annotation(
            page: 0, kind: .line,
            x: 0, y: 0, width: 0, height: 0,
            color: nil, note: nil,
            points: [AnnotationPoint(x: 72, y: 600), AnnotationPoint(x: 200, y: 650)]
        ))
        waitUntil(timeout: 5) { store.data != originalData }

        waitUntil(timeout: 5) { store.annotationsList.contains { $0.kind == .shape } }
        XCTAssertTrue(store.annotationsList.contains { $0.kind == .shape })
    }

    func testArrowAnnotationMutatesTheDocument() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.applyAnnotation(Annotation(
            page: 0, kind: .arrow,
            x: 0, y: 0, width: 0, height: 0,
            color: nil, note: nil,
            points: [AnnotationPoint(x: 72, y: 600), AnnotationPoint(x: 200, y: 650)]
        ))
        waitUntil(timeout: 5) { store.data != originalData }
    }

    func testInkFreehandStrokeMutatesTheDocumentAndListsAsInk() throws {
        let store = PDFDocumentStore()
        let data = try openOrSkip("form_sample")
        store.openReplacing(data: data, url: nil)
        waitUntil { store.hasDocument }

        let originalData = store.data
        store.applyAnnotation(Annotation(
            page: 0, kind: .ink,
            x: 0, y: 0, width: 0, height: 0,
            color: nil, note: nil,
            points: [
                AnnotationPoint(x: 72, y: 600),
                AnnotationPoint(x: 90, y: 620),
                AnnotationPoint(x: 110, y: 590),
            ]
        ))
        waitUntil(timeout: 5) { store.data != originalData }

        waitUntil(timeout: 5) { store.annotationsList.contains { $0.kind == .ink } }
        XCTAssertTrue(
            store.annotationsList.contains { $0.kind == .ink },
            "ink is a real, distinct annotation type — it round-trips as itself, unlike the Stamp-backed shapes"
        )
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
