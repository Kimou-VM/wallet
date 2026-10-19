// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

extension XCUIApplication {
    var isOnboarding: Bool {
        let create = buttons["create"].firstMatch
        let mainTab = element("mainTab")
        let launched = XCTNSPredicateExpectation(predicate: NSPredicate { _, _ in create.exists || mainTab.exists }, object: nil)
        _ = XCTWaiter().wait(for: [launched], timeout: 30)
        return create.exists
    }

    func element(_ identifier: String) -> XCUIElement {
        descendants(matching: .any)[identifier].firstMatch
    }

    func tap(_ identifier: String) {
        let element = element(identifier)
        XCTAssertTrue(element.waitForExistence(timeout: 10), "\(identifier) not found")
        element.tap()
    }

    func acceptTerms() {
        guard element("term_1").waitForExistence(timeout: 2) else {
            return
        }
        (1 ... 3).forEach { tap("term_\($0)") }
        tapMainAction()
    }

    func tapMainAction() {
        tap("main_action")
    }

    func skipEnableAuthentication() {
        let skip = buttons["skip"].firstMatch
        if skip.waitForExistence(timeout: 2) {
            skip.tap()
        }
    }

    func tapBack() {
        navigationBars.buttons.element(boundBy: 0).tap()
    }

    func getWords() -> [String] {
        XCTAssertTrue(staticTexts["word_11"].waitForExistence(timeout: 10), "word_11 not found")
        return (0 ..< 12).map { staticTexts["word_\($0)"].label }
    }

    func tapWalletTab() {
        tap("mainTab")
    }

    func tapWalletBar() {
        tap("walletBar")
    }

    func tapWalletSettings() {
        tap("wallet_edit")
    }

    func logout() {
        if isOnboarding == false {
            tapWalletTab()
            tapWalletBar()
            while element("wallet_edit").waitForExistence(timeout: 2) {
                tapWalletSettings()
                buttons["Delete"].firstMatch.tap()
                alerts.buttons["Delete"].tap()
            }
        }
    }

    func tapCreateWallet() {
        buttons["Create a New Wallet"].firstMatch.tap()
    }

    func tapImportWallet() {
        let button = buttons["Import an Existing Wallet"].firstMatch
        XCTAssertTrue(button.waitForExistence(timeout: 2), "button not found")
        button.tap()
    }
}
