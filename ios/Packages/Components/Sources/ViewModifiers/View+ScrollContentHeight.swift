// Copyright (c). Gem Wallet. All rights reserved.

import SwiftUI

public extension View {
    @ViewBuilder
    func onScrollContentHeightChange(_ action: @escaping (CGFloat) -> Void) -> some View {
        if #available(iOS 18.0, *) {
            onScrollGeometryChange(for: CGFloat.self) { geometry in
                geometry.contentSize.height + geometry.contentInsets.top
            } action: { _, height in
                action(height)
            }
        } else {
            self
        }
    }
}
