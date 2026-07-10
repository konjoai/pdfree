import XCTest

/// Phase 2 e2e: drive the real app UI. Deliberately minimal to start — a
/// launch smoke test and the empty-state affordance — because XCUITest that
/// opens `NSOpenPanel` or performs OS-level drag gestures is flaky in headless
/// CI (which is why the `macos-ui` job is `continue-on-error`). Grow this
/// toward the full fill → sign → export path as it proves stable locally.
final class PDFreeUITests: XCTestCase {
    override func setUpWithError() throws {
        continueAfterFailure = false
    }

    /// The app launches and lands on the empty state (never auto-loading a
    /// document), showing the drop affordance — Core UX Principle: "the drop
    /// surface IS the window."
    func testLaunchesToEmptyState() throws {
        let app = XCUIApplication()
        app.launch()

        XCTAssertTrue(
            app.staticTexts["Drop a PDF or image to start"].waitForExistence(timeout: 10),
            "Empty-state drop prompt should be visible on launch"
        )
    }

    /// A window is present and the app is responsive after launch.
    func testHasAWindow() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.windows.firstMatch.waitForExistence(timeout: 10))
    }
}
