package com.gemwallet.android.features.transfer.viewmodels.confirm.models

import androidx.compose.runtime.Immutable
import com.gemwallet.android.ui.components.InfoSheetEntity

@Immutable
data class ConfirmErrorUIModel(val text: String, val info: InfoSheetEntity?)
