package com.gemwallet.android.data.services.store.database.di

import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase

object Migration_102_103 : Migration(102, 103) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL("ALTER TABLE `perpetuals` ADD COLUMN `isEnabled` INTEGER NOT NULL DEFAULT 1")
    }
}
