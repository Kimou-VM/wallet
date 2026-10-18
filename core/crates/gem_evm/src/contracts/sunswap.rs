use alloy_sol_types::sol;

sol! {
    interface ISunSwapReferralVault {
        event ReferralAllocated(address indexed token, address indexed rebateRecipient, uint256 rebateAmount, uint256 protocolAmount);
    }
}
