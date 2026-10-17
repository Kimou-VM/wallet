// Copyright (c). Gem Wallet. All rights reserved.

import Components
import SwiftUI

public struct NetworkFeeSheet: View {
    private let model: NetworkFeeSceneViewModel

    @State private var contentHeight: CGFloat?

    public init(model: NetworkFeeSceneViewModel) {
        self.model = model
    }

    public var body: some View {
        NavigationStack {
            NetworkFeeScene(model: model)
                .onScrollContentHeightChange { contentHeight = $0 }
        }
        .sheetPresentation(presentationDetent)
    }

    private var presentationDetent: Set<PresentationDetent> {
        if model.showFeeRates || model.showFeeAssets {
            return Set([.medium, .large])
        }
        if let contentHeight {
            return Set([.height(contentHeight)])
        }
        return model.feeItems.isEmpty ? Set([.height(180)]) : Set([.medium])
    }
}
