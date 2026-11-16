import XCTest

/// UIKit owns scrolling: a flick glides, and the large title shrinks into
/// the bar as the content passes under it.
final class ScrollTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func testAFlickGlidesAndTheLargeTitleCollapses() throws {
        let app = XCUIApplication()
        app.launch()
        let bar = app.navigationBars["Volt Gallery"]
        XCTAssertTrue(bar.waitForExistence(timeout: 10))
        let tall = bar.frame.height
        app.descendants(matching: .any)["to-lists"].tap()
        XCTAssertTrue(app.navigationBars["Lists"].waitForExistence(timeout: 5))
        let row = app.descendants(matching: .any)["Row 1"]
        let start = row.frame.minY
        app.swipeUp(velocity: .fast)
        // Momentum carries the list well past one swipe's distance.
        sleep(2)
        XCTAssertFalse(row.isHittable && row.frame.minY >= start - 50, "the list moved")
        app.navigationBars["Lists"].buttons.element(boundBy: 0).tap()
        XCTAssertTrue(bar.waitForExistence(timeout: 5))
        app.swipeUp(velocity: .slow)
        XCTAssertLessThan(bar.frame.height, tall, "the large title collapsed")
    }
}
