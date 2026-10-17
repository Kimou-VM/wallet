// Copyright (c). Gem Wallet. All rights reserved.

import Assets
import Components
import FiatConnect
import struct Gemstone.GemAcquireAsset
import enum Gemstone.GemAcquireOption
import GemstonePrimitives
import InfoSheet
import Primitives
import PrimitivesComponents
import Swap
import SwiftUI
import Transfer

struct ConfirmTransferNavigationView: View {
    @Environment(\.viewModelFactory) private var viewModelFactory

    @State var model: ConfirmTransferSceneViewModel

    var body: some View {
        ConfirmTransferScene(model: model)
            .sheet(item: $model.isPresentingSheet) {
                switch $0 {
                case let .info(sheet):
                    InfoSheetScene(sheet: sheet, onAction: model.onInfoAction)
                case .networkFeeSelector:
                    NetworkFeeSheet(model: model.feeModel)
                case let .paymentAsset(type):
                    SelectAssetNavigationStack(
                        model: viewModelFactory.selectAssetScene(
                            wallet: model.assetAcquisitionWallet,
                            selectType: type,
                            selectAssetAction: model.selectPaymentAsset,
                        ),
                    )
                case let .paymentVerification(url):
                    PaymentVerificationScene(model: PaymentVerificationSceneViewModel(url: url, onComplete: model.onPaymentVerified, onError: model.onPaymentVerificationFailed))
                case .payloadDetails:
                    NavigationStack {
                        SimulationPayloadDetailsScene(
                            primaryRows: model.primaryPayloadFields,
                            secondaryRows: model.secondaryPayloadFields,
                            onSelectAddress: model.onSelectPayloadAddress,
                        )
                    }
                    .sheetPresentation([.large])
                case let .fiatConnect(assetAddress, wallet, amount):
                    NavigationStack {
                        FiatConnectNavigationView(
                            model: viewModelFactory.fiatScene(assetAddress: assetAddress, wallet: wallet, amount: amount),
                        )
                        .navigationBarTitleDisplayMode(.inline)
                        .toolbarDismissItem(type: .close, placement: .topBarLeading)
                    }
                case let .getAsset(asset, acquire):
                    GetAssetNavigationStack(
                        assetAddress: model.assetAddress(asset),
                        wallet: model.assetAcquisitionWallet,
                        acquire: acquire,
                        viewModelFactory: viewModelFactory,
                        onComplete: { model.isPresentingSheet = nil },
                    )
                case let .selectedAsset(input, wallet):
                    SelectedAssetNavigationStack(
                        input: input,
                        wallet: wallet,
                        onComplete: { model.isPresentingSheet = nil },
                    )
                case .swapDetails:
                    if case let .swapDetails(details) = model.detailsItemModel {
                        NavigationStack {
                            SwapDetailsView(details: details)
                        }
                        .sheetPresentation(.forCurrentDeviceSize(expandable: true))
                    }
                case let .perpetualDetails(model):
                    NavigationStack {
                        PerpetualDetailsView(model: model)
                    }
                    .sheetPresentation(.forCurrentDeviceSize(expandable: true))
                case let .addressDetails(chainAddress):
                    AddressDetailsDestination(chainAddress: chainAddress)
                }
            }
    }
}

struct GetAssetNavigationStack: View {
    private static let optionsDetent = PresentationDetent.height(360)

    let assetAddress: AssetAddress
    let wallet: Wallet
    let acquire: GemAcquireAsset
    let viewModelFactory: ViewModelFactory
    let onComplete: () -> Void

    @State private var selectedAction: GemAcquireOption?
    @State private var actionNavigationPath = NavigationPath()

    var body: some View {
        NavigationStack {
            GetAssetScene(
                asset: assetAddress.asset,
                options: acquire.options,
                onSelect: {
                    actionNavigationPath = NavigationPath()
                    selectedAction = $0
                },
            )
            .toolbarDismissItem(type: .close, placement: .topBarLeading)
            .sheet(item: $selectedAction) { action in
                NavigationStack(path: $actionNavigationPath) {
                    destination(for: action)
                        .toolbarDismissItem(type: .close, placement: .topBarLeading)
                        .navigationBarTitleDisplayMode(.inline)
                        .navigationDestination(for: ConfirmTransferInput.self) { input in
                            ConfirmTransferNavigationView(
                                model: viewModelFactory.confirmTransferScene(
                                    wallet: wallet,
                                    data: input.data,
                                    onComplete: onComplete,
                                ),
                            )
                        }
                }
                .sheetPresentation([.large])
            }
        }
        .sheetPresentation([Self.optionsDetent])
    }

    @ViewBuilder
    private func destination(for type: GemAcquireOption) -> some View {
        switch type {
        case .buy:
            FiatConnectNavigationView(
                model: viewModelFactory.fiatScene(
                    assetAddress: assetAddress,
                    wallet: wallet,
                    amount: acquire.buyAmount.map(Int.init),
                ),
            )
        case .swap:
            SwapNavigationView(
                model: viewModelFactory.swapScene(
                    input: SwapInput(
                        wallet: wallet,
                        pairSelector: acquire.swapPair.map(),
                    ),
                    onSwap: { actionNavigationPath.append(ConfirmTransferInput(data: $0)) },
                ),
            )
        case .receive:
            ReceiveScene(model: viewModelFactory.receiveScene(assetAddress: assetAddress, wallet: wallet))
        }
    }
}
