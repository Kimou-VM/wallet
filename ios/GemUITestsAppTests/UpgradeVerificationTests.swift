// Copyright (c). Gem Wallet. All rights reserved.

import XCTest

@MainActor
final class UpgradeVerificationTests: XCTestCase {
    override func setUpWithError() throws {
        try super.setUpWithError()
        continueAfterFailure = false
    }

    func testWalletSurvivedUpgrade() throws {
        try XCTSkipUnless(ProcessInfo.processInfo.environment["GEM_UPGRADE_TEST"] == "1", "Runs from just test-upgrade, after an older build imported the wallet")

        let app = XCUIApplication()
        setupPermissionHandler()
        app.launch()

        XCTAssertFalse(app.isOnboarding, "App should not show onboarding after upgrade — wallet data was lost")

        app.tapWalletTab()
        app.tapWalletBar()
        app.tapWalletSettings()

        let showPhraseButton = app.buttons["Show Secret Phrase"].firstMatch
        XCTAssertTrue(showPhraseButton.waitForExistence(timeout: 10), "Show Secret Phrase not found")
        showPhraseButton.tap()

        app.tapMainAction()

        let expectedWords = UITestKitConstants.words.components(separatedBy: " ")
        let displayedWords = app.getWords()
        XCTAssertEqual(displayedWords, expectedWords, "Secret phrase mismatch after upgrade — keys were corrupted")
    }
}
