use alloy_sol_types::sol;

sol! {
    interface IVault {
        function xSwapNative(uint32 dstChain, bytes dstAddress, uint32 dstToken, bytes cfParameters) external payable;
        function xSwapToken(uint32 dstChain, bytes dstAddress, uint32 dstToken, address srcToken, uint256 amount, bytes cfParameters) external;
    }
}
