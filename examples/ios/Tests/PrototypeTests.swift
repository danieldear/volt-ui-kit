import XCTest
final class PrototypeTests: XCTestCase {
    private func capture(_ app: XCUIApplication, _ name: String) {
        let attachment = XCTAttachment(screenshot:app.screenshot())
        attachment.name = name; attachment.lifetime = .keepAlways; add(attachment)
    }
    func testEmailKeyboardVisibilityAndCancel() throws {
        let app = XCUIApplication(); app.launch()
        XCTAssertTrue(app.buttons["profile"].waitForExistence(timeout:60))
        capture(app,"01-Welcome")
        app.buttons["profile"].tap()
        capture(app,"02-Profile")
        app.buttons["email"].tap()
        let field = app.textFields["editor-email"]
        XCTAssertTrue(field.waitForExistence(timeout:5))
        field.tap()
        XCTAssertTrue(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate(format:"hittable == true"), object:field)], timeout:5) == .completed)
        if app.keyboards.firstMatch.exists {
            XCTAssertLessThanOrEqual(field.frame.maxY, app.keyboards.firstMatch.frame.minY + 1)
        }
        field.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count:18) + "wrong")
        capture(app,"03-Keyboard")
        app.toolbars.buttons["Done"].tap()
        XCTAssertEqual(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate { _, _ in
            !app.keyboards.firstMatch.exists && app.buttons["save"].exists && app.buttons["save"].frame.height >= 50
        }, object:app)], timeout:5), .completed)
        app.buttons["save"].tap()
        XCTAssertTrue(app.staticTexts["validation"].label.contains("email"))
        app.buttons["cancel"].tap()
        app.buttons["profile"].tap()
        app.buttons["email"].tap()
        XCTAssertEqual(app.textFields["editor-email"].value as? String,"daniel@example.com")
        app.buttons["cancel"].tap()
        XCTAssertTrue(app.buttons["profile"].waitForExistence(timeout:5))
        XCTAssertFalse(app.staticTexts["host-error"].exists)
    }

    func testNavigationScrollAndEditableForm() throws {
        let app = XCUIApplication()
        app.launch()
        XCTAssertTrue(app.buttons["profile"].waitForExistence(timeout:60))
        XCTAssertFalse(app.staticTexts["host-error"].exists)
        let proof = app.descendants(matching:.any)["render-proof"]
        XCTAssertTrue(proof.exists)
        XCTAssertTrue((proof.value as? String)?.contains("Metal") == true)
        let idle = proof.value as? String
        Thread.sleep(forTimeInterval:1)
        XCTAssertEqual(proof.value as? String, idle, "Idle must not drive a perpetual frame loop")
        app.buttons["note-0"].tap()
        XCTAssertTrue(app.buttons["back"].waitForExistence(timeout:5))
        app.buttons["profile"].tap()
        app.buttons["name"].tap()
        let name = app.textFields["editor-name"]
        XCTAssertTrue(name.waitForExistence(timeout:5))
        name.tap()
        name.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 6))
        app.toolbars.buttons["Done"].tap()
        XCTAssertEqual(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate { _, _ in
            !app.keyboards.firstMatch.exists && app.buttons["save"].exists && app.buttons["save"].frame.height >= 50
        }, object:app)], timeout:5), .completed)
        app.buttons["save"].tap()
        XCTAssertTrue(app.staticTexts["validation"].label.contains("1–60"))
        app.buttons["name"].tap()
        app.textFields["editor-name"].typeText("Alex")
        app.toolbars.buttons["Done"].tap()
        XCTAssertEqual(XCTWaiter.wait(for:[XCTNSPredicateExpectation(predicate:NSPredicate { _, _ in
            !app.keyboards.firstMatch.exists && app.buttons["save"].exists && app.buttons["save"].frame.height >= 50
        }, object:app)], timeout:5), .completed)
        app.buttons["save"].tap()
        XCTAssertTrue(app.buttons["back"].waitForExistence(timeout:5))
        app.buttons["back"].tap()
        XCTAssertTrue(app.staticTexts.containing(NSPredicate(format:"label CONTAINS %@", "Alex")).firstMatch.exists)
        for _ in 0..<18 where !app.buttons["note-23"].isHittable { app.swipeUp() }
        XCTAssertTrue(app.buttons["note-23"].isHittable)
        capture(app,"04-Scrolled-list")
        app.buttons["note-23"].tap()
        XCTAssertTrue(app.buttons["back"].exists)
        XCUIDevice.shared.orientation = .landscapeLeft
        XCTAssertTrue(app.buttons["profile"].waitForExistence(timeout:5))
        capture(app,"05-Landscape")
        XCUIDevice.shared.orientation = .portrait
        XCUIDevice.shared.press(.home)
        app.activate()
        XCTAssertTrue(app.buttons["back"].waitForExistence(timeout:5))
        XCTAssertFalse(app.staticTexts["host-error"].exists)
        let shot = XCTAttachment(screenshot:app.screenshot()); shot.lifetime = .keepAlways; add(shot)
    }
}
