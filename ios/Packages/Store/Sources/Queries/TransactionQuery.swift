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
        let request = switch key {
        case let .record(recordId): wallet.filter(TransactionRecord.Columns.id == recordId)
        case let .transaction(transactionId): wallet.filter(TransactionRecord.Columns.transactionId == transactionId.identifier)
        }
        return try TransactionsQuery.extendedTransactions(db, request: request).first
    }
}

extension TransactionQuery: Equatable {}
