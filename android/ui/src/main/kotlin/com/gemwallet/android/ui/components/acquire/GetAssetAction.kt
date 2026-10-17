package com.gemwallet.android.ui.components.acquire

import com.wallet.core.primitives.AssetId

sealed interface GetAssetAction {
    data class Buy(val amount: Int? = null) : GetAssetAction
    data class Swap(val payAssetId: AssetId? = null) : GetAssetAction
    data object Receive : GetAssetAction
}
