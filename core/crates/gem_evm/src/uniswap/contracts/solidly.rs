use alloy_sol_types::sol;

sol! {
    interface ISolidlyPoolFactory {
        function getPool(address tokenA, address tokenB, bool stable) external view returns (address pool);
        function getFee(address pool, bool stable) external view returns (uint256 fee);
    }

    interface ISolidlyPool {
        function getReserves() external view returns (uint256 reserve0, uint256 reserve1, uint256 blockTimestampLast);
    }

    #[derive(Debug, PartialEq)]
    interface ISolidlyRouter {
        struct Route {
            address from;
            address to;
            bool stable;
            address factory;
        }

        function getAmountsOut(uint256 amountIn, Route[] memory routes) external view returns (uint256[] memory amounts);
    }
}
