// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import func Gemstone.feeAssetId
import Primitives

public struct SwapPairSelectorViewModel: Equatable {
    public var fromAssetId: AssetId?
    public var toAssetId: AssetId?

    var feeAssetId: AssetId? {
        fromAssetId.map { Gemstone.feeAssetId(assetId: $0) }
    }

    public init(
        fromAssetId: AssetId?,
        toAssetId: AssetId?,
    ) {
        self.fromAssetId = fromAssetId
        self.toAssetId = toAssetId
    }
}
