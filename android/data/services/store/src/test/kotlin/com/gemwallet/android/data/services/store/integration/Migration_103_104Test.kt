package com.gemwallet.android.data.services.store.integration

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.gemwallet.android.data.services.store.database.di.Migration_103_104
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class Migration_103_104Test {
    private val testDb = "migration-103-104-test"

    @get:Rule
    val helper = gemDatabaseMigrationTestHelper()

    @Before
    fun setUp() {
        InstrumentationRegistry.getInstrumentation().targetContext.deleteDatabase(testDb)
    }

    @Test
    fun aPriceWriteFindsItsMarketThroughTheNameIndex() {
        helper.createDatabase(testDb, 103).close()

        helper.runMigrationsAndValidate(testDb, 104, true, Migration_103_104).use { database ->
            database.query("EXPLAIN QUERY PLAN UPDATE perpetuals SET price = 1.0 WHERE name = 'BTC'").use { cursor ->
                val plan = buildList { while (cursor.moveToNext()) add(cursor.getString(cursor.getColumnIndexOrThrow("detail"))) }
                assertTrue(plan.toString(), plan.any { it.contains("index_perpetuals_name") })
            }
        }
    }
}
