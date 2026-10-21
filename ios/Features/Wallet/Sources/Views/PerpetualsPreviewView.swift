// Copyright (c). Gem Wallet. All rights reserved.

import Components
import Perpetuals
import Primitives
import Store
import Style
import SwiftUI

struct PerpetualsPreviewView: View {
    @State private var viewModel: PerpetualsPreviewViewModel
    @Binding private var showBalancePrivacy: Bool

    private let balance: PerpetualBalance?

    init(
        walletId: WalletId,
        balance: PerpetualBalance?,
        showBalancePrivacy: Binding<Bool>,
    ) {
        self.balance = balance
        _showBalancePrivacy = showBalancePrivacy
        _viewModel = State(initialValue: PerpetualsPreviewViewModel(walletId: walletId))
    }

    var body: some View {
        Group {
            switch viewModel.preview(balance: balance) {
            case let .trade(balance):
                NavigationLink(value: Scenes.Perpetuals()) {
                    tradePerpetualsItem(balance: balance.text())
                }
                .accessibilityIdentifier("trade_perpetuals")
            case .positions:
                PerpetualPositionsList(
                    positions: viewModel.positions,
                    showBalancePrivacy: $showBalancePrivacy,
                )
            }
        }
        .bindQuery(viewModel.positionsQuery)
    }

    private func tradePerpetualsItem(balance: String) -> some View {
        HStack {
            Text(viewModel.tradePerpetualsTitle)
                .textStyle(ListItemModel.StyleDefaults.titleStyle)
                .lineLimit(1)
                .truncationMode(.tail)

            Spacer(minLength: .extraSmall)

            PrivacyText(balance, isEnabled: $showBalancePrivacy)
                .textStyle(ListItemModel.StyleDefaults.subtitleStyle)
                .multilineTextAlignment(.trailing)
                .lineLimit(1)
                .truncationMode(.middle)
        }
    }
}
