import XCTest

/// Drives the gallery through UIKit's real navigation bar.
final class GalleryTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func testEveryPageOpensAndTheBarGoesBack() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.navigationBars["Volt Gallery"].waitForExistence(timeout: 10))
        for (row, title) in [("to-navigation", "Navigation"), ("to-buttons", "Buttons"),
                             ("to-controls", "Controls"), ("to-lists", "Lists"),
                             ("to-typography", "Typography"), ("to-badges", "Badges and cards"),
                             ("to-text", "Text input")] {
            let element = app.descendants(matching: .any)[row]
            XCTAssertTrue(element.waitForExistence(timeout: 5), row)
            element.tap()
            XCTAssertTrue(app.navigationBars[title].waitForExistence(timeout: 5), title)
            app.navigationBars[title].buttons.element(boundBy: 0).tap()
            XCTAssertTrue(app.navigationBars["Volt Gallery"].waitForExistence(timeout: 5))
        }
    }

    func testTheBarButtonRunsRustAndDeepPagesUnwind() throws {
        let app = XCUIApplication()
        app.launch()
        app.descendants(matching: .any)["to-navigation"].tap()
        let bar = app.navigationBars["Navigation"]
        XCTAssertTrue(bar.waitForExistence(timeout: 5))
        bar.buttons["Edit"].tap()
        XCTAssertTrue(bar.buttons["Done"].waitForExistence(timeout: 5))
        for level in 2...5 {
            app.descendants(matching: .any)["to-level-\(level)"].tap()
            XCTAssertTrue(app.navigationBars["Level \(level)"].waitForExistence(timeout: 5))
        }
        app.descendants(matching: .any)["start-5"].tap()
        XCTAssertTrue(app.navigationBars["Volt Gallery"].waitForExistence(timeout: 5))
    }
}
