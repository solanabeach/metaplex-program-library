#![cfg(feature = "test-bpf")]
mod utils;

use mpl_testing_utils::solana::airdrop;
use mpl_testing_utils::utils::Metadata;
use solana_program_test::*;
use solana_sdk::{signature::Keypair, signer::Signer};
use std::assert_eq;
use mpl_testing_utils::assert_error;
use solana_program::instruction::Instruction;
use solana_sdk::transaction::Transaction;
use spl_associated_token_account::get_associated_token_address;
use mpl_auction_house::pda::{find_escrow_payment_address, find_public_bid_trade_state_address};
use utils::setup_functions::*;
use anchor_lang::{InstructionData, ToAccountMetas};
use solana_program::sysvar;
use solana_sdk::{
    instruction::InstructionError, transaction::TransactionError, transport::TransportError,
};
#[tokio::test]
async fn buy_success() {
    let mut context = auction_house_program_test().start_with_context().await;
    // Payer Wallet
    let (ah, ahkey, _) = existing_auction_house_test_context(&mut context)
        .await
        .unwrap();
    let test_metadata = Metadata::new();

    airdrop(&mut context, &test_metadata.token.pubkey(), 1000000000)
        .await
        .unwrap();
    test_metadata
        .create(
            &mut context,
            "Test".to_string(),
            "TST".to_string(),
            "uri".to_string(),
            None,
            10,
            false,
        )
        .await
        .unwrap();
    let buyer = Keypair::new();
    airdrop(&mut context, &buyer.pubkey(), 10000000000)
        .await
        .unwrap();
    let (_, deposit_tx) = deposit(
        &mut context,
        &ahkey,
        &ah,
        &test_metadata,
        &buyer,
        1000000000,
    );
    context
        .banks_client
        .process_transaction(deposit_tx)
        .await
        .unwrap();
    let (acc, buy_tx) = buy(
        &mut context,
        &ahkey,
        &ah,
        &test_metadata,
        &test_metadata.token.pubkey(),
        &buyer,
        1000000000,
    );
    context
        .banks_client
        .process_transaction(buy_tx)
        .await
        .unwrap();
    let sts = context
        .banks_client
        .get_account(acc.buyer_trade_state)
        .await
        .expect("Error Getting Trade State")
        .expect("Trade State Empty");
    assert_eq!(sts.data.len(), 1);
}


#[tokio::test]
async fn buy_fail_wrong_bid_type() {
    let mut context = auction_house_program_test().start_with_context().await;
    // Payer Wallet
    let (ah, ahkey, _) = existing_auction_house_test_context(&mut context)
        .await
        .unwrap();
    let test_metadata = Metadata::new();

    airdrop(&mut context, &test_metadata.token.pubkey(), 1000000000)
        .await
        .unwrap();
    test_metadata
        .create(
            &mut context,
            "Test".to_string(),
            "TST".to_string(),
            "uri".to_string(),
            None,
            10,
            false,
        )
        .await
        .unwrap();
    let buyer = Keypair::new();
    let sale_price = 1000000000;
    airdrop(&mut context, &buyer.pubkey(), 10000000000)
        .await
        .unwrap();
    let (_, deposit_tx) = deposit(
        &mut context,
        &ahkey,
        &ah,
        &test_metadata,
        &buyer,
        sale_price,
    );
    context
        .banks_client
        .process_transaction(deposit_tx)
        .await
        .unwrap();

    // Since I need to fudge the trade state I dont use the buy helper
    let seller_token_account =
        get_associated_token_address(&context.payer.pubkey(), &test_metadata.mint.pubkey());
    let trade_state = find_public_bid_trade_state_address(
        &buyer.pubkey(),
        &ahkey,
        &ah.treasury_mint,
        &test_metadata.mint.pubkey(),
        sale_price,
        1,
    );
    let (escrow, escrow_bump) = find_escrow_payment_address(&ahkey, &buyer.pubkey());
    let (bts, bts_bump) = trade_state;
    let accounts = mpl_auction_house::accounts::Buy {
        wallet: buyer.pubkey(),
        token_account: seller_token_account,
        metadata: test_metadata.pubkey,
        authority: ah.authority,
        auction_house: ahkey,
        auction_house_fee_account: ah.auction_house_fee_account,
        buyer_trade_state: bts,
        token_program: spl_token::id(),
        treasury_mint: ah.treasury_mint,
        payment_account: buyer.pubkey(),
        transfer_authority: buyer.pubkey(),
        system_program: solana_program::system_program::id(),
        rent: sysvar::rent::id(),
        escrow_payment_account: escrow,
    };

    let account_metas = accounts.to_account_metas(None);

    let buy_ix = mpl_auction_house::instruction::Buy {
        trade_state_bump: bts_bump,
        escrow_payment_bump: escrow_bump,
        token_size: 1,
        buyer_price: sale_price,
    };
    let data = buy_ix.data();
    let instruction = Instruction {
        program_id: mpl_auction_house::id(),
        data,
        accounts: account_metas,
    };
    let buy_tx =Transaction::new_signed_with_payer(
        &[instruction],
        Some(&buyer.pubkey()),
        &[&buyer],
        context.last_blockhash,
    );

    let err = context
        .banks_client
        .process_transaction(buy_tx)
        .await
        .unwrap_err();
    assert_error!(err, 6013);
}
