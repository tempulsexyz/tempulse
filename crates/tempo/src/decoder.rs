use alloy::primitives::Address;
use alloy::rpc::types::Log;

use crate::abi::{TIP20, TIP20Factory};

/// Zero address constant for mint/burn detection.
pub const ZERO_ADDRESS: Address = Address::ZERO;

/// Decoded TIP20Factory TokenCreated event.
#[derive(Debug, Clone)]
pub struct TokenCreatedEvent {
    pub token_address: Address,
    pub name: String,
    pub symbol: String,
    pub currency: String,
    pub quote_token: Address,
    pub admin: Address,
    pub salt: [u8; 32],
    pub block_number: u64,
    pub transaction_hash: String,
}

/// Classified TIP-20 event.
#[derive(Debug, Clone)]
pub enum Tip20Event {
    Transfer {
        token_address: Address,
        from: Address,
        to: Address,
        amount: alloy::primitives::U256,
        block_number: u64,
        transaction_hash: String,
        log_index: u32,
    },
    Mint {
        token_address: Address,
        to: Address,
        amount: alloy::primitives::U256,
        block_number: u64,
        transaction_hash: String,
        log_index: u32,
    },
    Burn {
        token_address: Address,
        from: Address,
        amount: alloy::primitives::U256,
        block_number: u64,
        transaction_hash: String,
        log_index: u32,
    },
    TransferWithMemo {
        token_address: Address,
        from: Address,
        to: Address,
        amount: alloy::primitives::U256,
        memo: [u8; 32],
        block_number: u64,
        transaction_hash: String,
        log_index: u32,
    },
}

/// Attempt to decode a log as a TIP20Factory `TokenCreated` event.
pub fn decode_factory_log(log: &Log) -> Option<TokenCreatedEvent> {
    let block_number = log.block_number?;
    let tx_hash = log
        .transaction_hash
        .map(|h| format!("{h:#x}"))
        .unwrap_or_default();

    let decoded = log.log_decode::<TIP20Factory::TokenCreated>().ok()?;
    let inner = decoded.inner.data;

    Some(TokenCreatedEvent {
        token_address: inner.token,
        name: inner.name,
        symbol: inner.symbol,
        currency: inner.currency,
        quote_token: inner.quoteToken,
        admin: inner.admin,
        salt: inner.salt.into(),
        block_number,
        transaction_hash: tx_hash,
    })
}

/// Attempt to decode a log as a TIP-20 Transfer/Mint/Burn event.
///
/// Transfer events with `from == 0x0` are classified as Mint;
/// Transfer events with `to == 0x0` are classified as Burn.
pub fn decode_tip20_log(log: &Log) -> Option<Tip20Event> {
    let block_number = log.block_number?;
    let tx_hash = log
        .transaction_hash
        .map(|h| format!("{h:#x}"))
        .unwrap_or_default();
    let log_index = log.log_index? as u32;
    let token_address = log.address();

    // Try Transfer event first (covers most cases)
    if let Ok(decoded) = log.log_decode::<TIP20::Transfer>() {
        let d = decoded.inner.data;
        let event = if d.from == ZERO_ADDRESS {
            Tip20Event::Mint {
                token_address,
                to: d.to,
                amount: d.amount,
                block_number,
                transaction_hash: tx_hash,
                log_index,
            }
        } else if d.to == ZERO_ADDRESS {
            Tip20Event::Burn {
                token_address,
                from: d.from,
                amount: d.amount,
                block_number,
                transaction_hash: tx_hash,
                log_index,
            }
        } else {
            Tip20Event::Transfer {
                token_address,
                from: d.from,
                to: d.to,
                amount: d.amount,
                block_number,
                transaction_hash: tx_hash,
                log_index,
            }
        };
        return Some(event);
    }

    // Try TransferWithMemo
    if let Ok(decoded) = log.log_decode::<TIP20::TransferWithMemo>() {
        let d = decoded.inner.data;
        return Some(Tip20Event::TransferWithMemo {
            token_address,
            from: d.from,
            to: d.to,
            amount: d.amount,
            memo: d.memo.into(),
            block_number,
            transaction_hash: tx_hash,
            log_index,
        });
    }

    None
}
