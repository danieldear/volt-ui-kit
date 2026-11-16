import XCTest

/// The keyboard slides over the page, as in UIKit lists: the field and the
/// page move together, and the field stays above the keyboard.
final class KeyboardTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func testTheFieldMovesWithItsPageAndStaysAboveTheKeyboard() throws {
        let app = XCUIApplication()
        app.launchEnvironment["VOLT_GALLERY_PAGE"] = "text"
        app.launch()
        XCTAssertTrue(app.navigationBars["Text input"].waitForExistence(timeout: 10))
        let header = app.staticTexts["Text field"]
        let name = app.descendants(matching: .any)["name"]
        XCTAssertTrue(name.waitForExistence(timeout: 5))
        let gap = name.frame.minY - header.frame.minY
        name.tap()
        let editor = app.textFields["editor-name"]
        XCTAssertTrue(editor.waitForExistence(timeout: 5))
        let keyboard = app.keyboards.element
        XCTAssertTrue(keyboard.waitForExistence(timeout: 5))
        sleep(1)
        XCTAssertEqual(editor.frame.minY - header.frame.minY, gap, accuracy: 6, "the field moved with its page")
        XCTAssertLessThan(editor.frame.maxY, keyboard.frame.minY, "the field is above the keyboard")
        editor.typeText("Zoë")

        app.descendants(matching: .any)["notes"].tap()
        let area = app.textViews["editor-notes"]
        XCTAssertTrue(area.waitForExistence(timeout: 5))
        area.typeText("Milk\nEggs")
        sleep(1)
        XCTAssertEqual(area.value as? String, "Milk\nEggs")
        XCTAssertLessThan(area.frame.minY, keyboard.frame.minY, "the text area is above the keyboard")

        app.descendants(matching: .any)["save"].tap()
        XCTAssertTrue(app.staticTexts["Saved Zoë."].waitForExistence(timeout: 5))
    }
}
