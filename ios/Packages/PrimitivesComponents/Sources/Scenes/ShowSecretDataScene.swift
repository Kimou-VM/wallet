// Copyright (c). Gem Wallet. All rights reserved.

import Components
import struct Gemstone.GemCopy
import struct Gemstone.GemPrivateKeyDetails
import struct Gemstone.GemSecretScreen
import enum Gemstone.GemWalletSecret
import func Gemstone.privateKeyCopy
import func Gemstone.secretPhraseCopy
import func Gemstone.secretScreen
import GemstonePrimitives
import Localization
import Primitives
import Style
import SwiftUI

public struct ShowSecretDataScene: View {
    private let secret: GemWalletSecret
    private let continueAction: VoidAction
    @State private var isPresentingCopyToast = false

    public init(secret: GemWalletSecret, continueAction: VoidAction = nil) {
        self.secret = secret
        self.continueAction = continueAction
    }

    public var body: some View {
        let screen = screen
        List {
            Section {
                CalloutView(style: screen.warning.calloutViewStyle)
            }
            .cleanListRow()

            Section {
                SecretDataTypeView(secret: secret)
            }
            .cleanListRow()

            if let scope {
                Text(scope)
                    .textStyle(.calloutSecondary)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: .infinity)
                    .cleanListRow()
            }

            ListButton(
                title: Localized.Common.copy,
                image: Images.System.copy,
                action: copy,
            )
            .frame(maxWidth: .infinity, alignment: .center)
            .cleanListRow()

            if let details {
                Section {
                    ListItemView(model: ListItemModel(title: Localized.Transfer.network, subtitle: details.row.title))
                    ListItemView(model: ListItemModel(title: Localized.Common.address, titleExtra: details.address.preventingHyphenation, titleStyleExtra: .calloutSecondary))
                    ListItemView(model: ListItemModel(title: Localized.PrivateKey.addressType, subtitle: details.addressType.title))
                    ListItemView(model: ListItemModel(title: Localized.PrivateKey.derivationPath, subtitle: details.derivationPath))
                } header: {
                    Text(Localized.Common.details)
                } footer: {
                    Text(Localized.PrivateKey.addressTypeHint(details.addressType.title))
                }
            }
        }
        .safeAreaButton(isVisible: continueAction != nil) {
            StateButton(
                text: Localized.Common.continue,
                action: onContinue,
            )
        }
        .contentMargins([.top], .extraSmall, for: .scrollContent)
        .listSectionSpacing(.custom(.medium))
        .toolbarInfoButton(url: docsUrl)
        .navigationTitle(screen.title.text)
        .copyToast(
            copy: secretCopy,
            isPresenting: $isPresentingCopyToast,
        )
        .detectScreenshots(kind: secret.kind)
        .protectFromScreenRecording()
    }

    private var screen: GemSecretScreen {
        secretScreen(kind: secret.kind, isNew: continueAction != nil)
    }

    private var scope: String? {
        switch secret {
        case .words: nil
        case let .privateKey(_, scope, _): scope.description
        }
    }

    private var details: GemPrivateKeyDetails? {
        switch secret {
        case .words: nil
        case let .privateKey(_, _, details): details
        }
    }

    private var secretCopy: GemCopy {
        switch secret {
        case let .words(words): secretPhraseCopy(words: words)
        case let .privateKey(key, _, _): privateKeyCopy(key: key)
        }
    }

    private var docsUrl: URL {
        secret.kind.securityDocsUrl
    }

    private func copy() {
        isPresentingCopyToast = true
    }

    private func onContinue() {
        continueAction?()
    }
}
