import XCTest

/// The Pantry sample shop by touch: grid, carousel, product, cart, checkout.
final class ShopTests: XCTestCase {
    override func setUp() { continueAfterFailure = false }

    func testAShopperOrdersByTouch() throws {
        let app = XCUIApplication()
        app.launch()
        let any = app.descendants(matching: .any)
        any["to-shop"].tap()
        let shop = app.navigationBars["Pantry"]
        XCTAssertTrue(shop.waitForExistence(timeout: 5))
        XCTAssertTrue(any["deal-1-add"].waitForExistence(timeout: 5))
        any["deal-1-add"].tap()
        XCTAssertTrue(shop.buttons["Cart · 1"].waitForExistence(timeout: 5))
        any["deal-4"].tap()
        XCTAssertTrue(app.navigationBars["Sourdough loaf"].waitForExistence(timeout: 5))
        any["qty-plus"].tap()
        any["add-to-cart"].tap()
        XCTAssertTrue(shop.buttons["Cart · 3"].waitForExistence(timeout: 5))
        shop.buttons["Cart · 3"].tap()
        XCTAssertTrue(app.navigationBars["Cart"].waitForExistence(timeout: 5))
        any["checkout"].tap()
        XCTAssertTrue(app.navigationBars["Checkout"].waitForExistence(timeout: 5))
        any["place-order"].tap()
        XCTAssertTrue(app.navigationBars["Order placed"].waitForExistence(timeout: 5))
        any["back-to-shop"].tap()
        XCTAssertTrue(shop.buttons["Cart · 0"].waitForExistence(timeout: 5))
        // A sideways swipe on the deals carousel scrolls it; it never goes back.
        any["deal-1"].swipeLeft()
        sleep(1)
        XCTAssertTrue(shop.exists, "still in the shop after swiping the carousel")
    }
}
