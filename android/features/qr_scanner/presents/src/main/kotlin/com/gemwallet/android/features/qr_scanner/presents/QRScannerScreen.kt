package com.gemwallet.android.features.qr_scanner.presents

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.provider.Settings
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts.RequestPermission
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LifecycleEventEffect
import com.gemwallet.android.ui.R
import com.gemwallet.android.ui.components.empty.EmptyAction
import com.gemwallet.android.ui.findActivity
import com.wallet.core.primitives.QRScanType

@Composable
fun QRScannerScreen(scanType: QRScanType, onCancel: () -> Unit, titleContent: @Composable () -> Unit = { QRScannerTitle() }, onResult: (String) -> Unit) {
    val context = LocalContext.current
    val activity = checkNotNull(context.findActivity())
    var isCameraGranted by remember { mutableStateOf(context.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
    var shouldShowRationale by remember { mutableStateOf(activity.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)) }
    var isPermissionRequested by remember { mutableStateOf(false) }
    val cameraPermission = rememberLauncherForActivityResult(RequestPermission()) { granted ->
        isCameraGranted = granted
        shouldShowRationale = activity.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)
        isPermissionRequested = true
    }
    val isPermanentlyDenied = isPermissionRequested && !shouldShowRationale
    LifecycleEventEffect(Lifecycle.Event.ON_RESUME) {
        isCameraGranted = context.checkSelfPermission(Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED
        shouldShowRationale = activity.shouldShowRequestPermissionRationale(Manifest.permission.CAMERA)
    }
    LaunchedEffect(Unit) {
        if (!isCameraGranted) {
            cameraPermission.launch(Manifest.permission.CAMERA)
        }
    }
    BackHandler(true) {
        onCancel()
    }
    QRScannerScene(
        scanType = scanType,
        isCameraGranted = isCameraGranted,
        permissionAction = if (isPermanentlyDenied) {
            EmptyAction(title = stringResource(R.string.common_open_settings), onClick = context::openAppSettings)
        } else {
            EmptyAction(title = stringResource(R.string.common_grant_permission), onClick = { cameraPermission.launch(Manifest.permission.CAMERA) })
        },
        onCancel = onCancel,
        titleContent = titleContent,
        onResult = onResult,
    )
}

private fun Context.openAppSettings() {
    startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS, Uri.fromParts("package", packageName, null)))
}
