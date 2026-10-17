// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import struct Gemstone.GemAcquireAsset
import GemstonePrimitives
import InfoSheet
import Primitives

public enum SwapSheetType: Identifiable, Sendable {
    case info(InfoSheetModel)
    case selectAsset(SelectAssetSwapType)
    case swapDetails
    case getAsset(AssetAddress, acquire: GemAcquireAsset)

    public var id: String {
        switch self {
        case let .info(sheet): "info-\(sheet.id)"
        case let .selectAsset(type): "selectAsset-\(type)"
        case .swapDetails: "details"
        case let .getAsset(assetAddress, _): "get-asset-\(assetAddress.asset.id.identifier)"
        }
    }
}
