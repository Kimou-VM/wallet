package com.gemwallet.android.data.services.store.database.entities

import com.gemwallet.android.testkit.mockTransaction
import com.wallet.core.primitives.WalletId
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Test

class DbTransactionTest {

    private val wallet = WalletId("wallet")

    @Test
    fun toDTO_keepsATransactionCoreCanRead() {
        assertNotNull(mockTransaction(fee = "21000", value = "100", metadata = "{\"pnl\":1.5}").toRecord(wallet).toDTO())
    }

    @Test
    fun toDTO_dropsATransactionCoreCannotRead() {
        assertNull("a negative value", mockTransaction(value = "-1").toRecord(wallet).toDTO())
        assertNull("a negative fee", mockTransaction(fee = "-1").toRecord(wallet).toDTO())
        assertNull("metadata that is not JSON", mockTransaction(metadata = "{broken").toRecord(wallet).toDTO())
    }
}
