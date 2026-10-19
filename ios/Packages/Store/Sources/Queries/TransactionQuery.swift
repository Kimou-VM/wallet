// Copyright (c). Gem Wallet. All rights reserved.

import Foundation
import GRDB
import Primitives

public struct TransactionQuery: DatabaseQueryable {
    public enum Key: Equatable, Sendable {
        case record(UInt64)
        case transaction(TransactionId)
    }

    private let walletId: WalletId
    private let key: Key

    public init(walletId: WalletId, key: Key) {
        self.walletId = walletId
        self.key = key
    }

    public func fetch(_ db: Database) throws -> TransactionExtended? {
        let wallet = TransactionRecord.filter(TransactionRecord.Columns.walletId == walletId.id)
        switch key {
        case let .record(recordId):
            guard let transaction = try TransactionsQuery.extendedTransactions(db, request: wallet.filter(TransactionRecord.Columns.id == recordId)).first else {
                throw RecordError.recordNotFound(databaseTableName: TransactionRecord.databaseTableName, key: [:])
            }
            return transaction
        case let .transaction(transactionId):
            return try TransactionsQuery.extendedTransactions(db, request: wallet.filter(TransactionRecord.Columns.transactionId == transactionId.identifier)).first
        }
    }
}

extension TransactionQuery: Equatable {}
