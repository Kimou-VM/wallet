package com.gemwallet.android.ui.components.acquire

import androidx.compose.runtime.Immutable
import com.gemwallet.android.ext.toAssetId
import com.wallet.core.primitives.Asset
import com.wallet.core.primitives.AssetId
import uniffi.gemstone.GemAcquireAsset
import uniffi.gemstone.GemAcquireAssetFlow
import uniffi.gemstone.GemAcquireOption

@Immutable
data class AcquireAssetRequest(val asset: Asset, val acquire: GemAcquireAsset) {
    val offersOptions: Boolean get() = acquire.flow == GemAcquireAssetFlow.OPTIONS
    val buyAmount: Int? get() = acquire.buyAmount
    val swapPayAssetId: AssetId? get() = acquire.swapPair.payAssetId?.toAssetId()

    fun action(option: GemAcquireOption): GetAssetAction = when (option) {
        GemAcquireOption.BUY -> GetAssetAction.Buy(buyAmount)
        GemAcquireOption.SWAP -> GetAssetAction.Swap(swapPayAssetId)
        GemAcquireOption.RECEIVE -> GetAssetAction.Receive
    }
}
