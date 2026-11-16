import XCTest

/// Going back shows a page where it was left: no jump, no creeping.
final class ReturnTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    private func roundTrip(_ app: XCUIApplication) {
        let any = app.descendants(matching: .any)
        any["to-buttons"].tap()
        XCTAssertTrue(app.navigationBars["Buttons"].waitForExistence(timeout: 5))
        app.navigationBars["Buttons"].buttons.element(boundBy: 0).tap()
        XCTAssertTrue(app.navigationBars["Volt Gallery"].waitForExistence(timeout: 5))
        sleep(1)
    }

    func testHomeStaysPutAcrossRoundTrips() throws {
        let app = XCUIApplication()
        app.launch()
        let row = app.descendants(matching: .any)["to-buttons"]
        XCTAssertTrue(row.waitForExistence(timeout: 10))
        sleep(1)
        let top = row.frame.minY
        for _ in 0..<5 { roundTrip(app) }
        XCTAssertEqual(row.frame.minY, top, accuracy: 1, "at the top: no drift after five returns")

        // Scrolled a little: the same place after returning, every time.
        let start = app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.7))
        start.press(forDuration: 0.05, thenDragTo: app.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.62)))
        sleep(2)
        let scrolled = row.frame.minY
        for _ in 0..<5 { roundTrip(app) }
        XCTAssertEqual(row.frame.minY, scrolled, accuracy: 1, "scrolled: no drift after five returns")
    }
}
