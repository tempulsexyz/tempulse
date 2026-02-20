use alloy::sol;

// ─── TIP-20 Token Interface ─────────────────────────────────────────────────
sol! {
    #[allow(missing_docs)]
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract TIP20 {
        // === ERC-20 core events ===
        event Transfer(address indexed from, address indexed to, uint256 amount);
        event Approval(address indexed owner, address indexed spender, uint256 amount);

        // === TIP-20 extended events ===
        event Mint(address indexed to, uint256 amount);
        event Burn(address indexed from, uint256 amount);
        event BurnBlocked(address indexed from, uint256 amount);
        event TransferWithMemo(
            address indexed from,
            address indexed to,
            uint256 amount,
            bytes32 indexed memo
        );
        event PauseStateUpdate(address indexed updater, bool isPaused);
        event SupplyCapUpdate(address indexed updater, uint256 indexed newSupplyCap);
        event RewardDistributed(address indexed funder, uint256 amount);

        // === ERC-20 view functions ===
        function name() external view returns (string memory);
        function symbol() external view returns (string memory);
        function decimals() external pure returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);

        // === TIP-20 view functions ===
        function currency() external view returns (string memory);
        function paused() external view returns (bool);
        function supplyCap() external view returns (uint256);
    }
}

// ─── TIP-20 Factory ─────────────────────────────────────────────────────────
sol! {
    #[allow(missing_docs)]
    #[derive(Debug, PartialEq, Eq)]
    #[sol(rpc)]
    contract TIP20Factory {
        event TokenCreated(
            address indexed token,
            string name,
            string symbol,
            string currency,
            address quoteToken,
            address admin,
            bytes32 salt
        );

        function isTIP20(address token) external view returns (bool);
        function getTokenAddress(address sender, bytes32 salt) external pure returns (address token);
    }
}
