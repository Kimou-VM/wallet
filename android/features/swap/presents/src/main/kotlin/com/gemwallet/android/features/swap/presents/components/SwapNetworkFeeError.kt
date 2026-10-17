package com.gemwallet.android.features.swap.presents.components

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.gemwallet.android.ui.R
import com.gemwallet.android.ui.components.list_item.WarningItem
import com.gemwallet.android.ui.localization.text
import com.gemwallet.android.ui.models.ListPosition
import uniffi.gemstone.GemSwapNetworkFeeError

@Composable
internal fun SwapNetworkFeeError(error: GemSwapNetworkFeeError, onClick: () -> Unit) {
    WarningItem(
        title = stringResource(R.string.errors_error_occurred),
        message = error.display.text(LocalContext.current),
        color = MaterialTheme.colorScheme.error,
        position = ListPosition.Single,
        onClick = onClick,
    )
}
