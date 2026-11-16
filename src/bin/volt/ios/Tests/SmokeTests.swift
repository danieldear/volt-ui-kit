import XCTest
final class SmokeTests:XCTestCase {
    func testRustApplicationAndNativeEditing() throws {
        let app=XCUIApplication();app.launch()
        XCTAssertTrue(app.buttons["increment"].waitForExistence(timeout:60))
        app.buttons["increment"].tap()
        XCTAssertEqual(app.buttons["increment"].label,"Count: 1")
        // The Settings bar button is UIKit's; Rust runs it.
        app.navigationBars.buttons["Settings"].tap()
        XCTAssertTrue(app.navigationBars["Settings"].waitForExistence(timeout:5))
        app.buttons["name"].tap()
        let field=app.textFields["editor-name"]
        XCTAssertTrue(field.waitForExistence(timeout:5))
        field.typeText(String(repeating:XCUIKeyboardKey.delete.rawValue,count:9)+"Rust app")
        app.toolbars.buttons["Done"].tap()
        XCTAssertEqual(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate {_,_ in !app.keyboards.firstMatch.exists},object:app)],timeout:5),.completed)
        XCTAssertTrue(app.switches["reminders"].exists)
        app.buttons["save"].tap()
        XCTAssertTrue(app.staticTexts["greeting"].waitForExistence(timeout:5))
        XCTAssertEqual(app.staticTexts["greeting"].label,"Hello, Rust app")
        for _ in 0..<12 where !app.staticTexts["item-30"].exists {app.swipeUp()}
        XCTAssertTrue(app.staticTexts["item-30"].exists)
        XCTAssertFalse(app.staticTexts["host-error"].exists)
    }
}
