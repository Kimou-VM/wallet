// Copyright (c). Gem Wallet. All rights reserved.

import enum Gemstone.GemPerpetualSubscription
import Primitives

public protocol PerpetualObservable: Actor {
    var chartService: ChartObserverService { get }
    func setup(for wallet: Wallet) async
    func disconnect() async
    nonisolated func subscribe(_ subscription: GemPerpetualSubscription)
    nonisolated func unsubscribe(_ subscription: GemPerpetualSubscription)
}
