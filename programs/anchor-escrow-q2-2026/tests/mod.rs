#[cfg(test)]
mod tests {

    use {
        anchor_lang::{
            prelude::msg, 
            solana_program::program_pack::Pack, 
            AccountDeserialize, 
            InstructionData, 
            ToAccountMetas
        }, anchor_spl::{
            associated_token::{
                self, 
                spl_associated_token_account
            }, 
            token::spl_token
        }, 
        litesvm::LiteSVM, 
        litesvm_token::{
            spl_token::ID as TOKEN_PROGRAM_ID, 
            CreateAssociatedTokenAccount, 
            CreateMint, MintTo
        }, 
        solana_rpc_client::rpc_client::RpcClient,
        solana_account::Account,
        solana_instruction::Instruction, 
        solana_keypair::Keypair, 
        solana_message::Message, 
        solana_native_token::LAMPORTS_PER_SOL, 
        solana_pubkey::Pubkey, 
        solana_sdk_ids::system_program::ID as SYSTEM_PROGRAM_ID, 
        solana_signer::Signer, 
        solana_transaction::Transaction, 
        solana_address::Address, 
        std::{
            path::PathBuf, 
            str::FromStr
        }
    };

    static PROGRAM_ID: Pubkey = anchor_escrow_q2_2026::ID;

    fn setup() -> (LiteSVM, Keypair) {
        let mut program = LiteSVM::new();
        let payer = Keypair::new();
    
        program
            .airdrop(&payer.pubkey(), 10 * LAMPORTS_PER_SOL)
            .expect("Failed to airdrop SOL to payer");
    
        let so_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/deploy/anchor_escrow_q2_2026.so");
    
        let program_data = std::fs::read(so_path).expect("Failed to read program SO file");
    
        program.add_program(PROGRAM_ID, &program_data);

        let rpc_client = RpcClient::new("https://api.devnet.solana.com");
        let account_address = Address::from_str("DRYvf71cbF2s5wgaJQvAGkghMkRcp5arvsK2w97vXhi2").unwrap();
        let fetched_account = rpc_client
            .get_account(&account_address)
            .expect("Failed to fetch account from devnet");

        program.set_account(payer.pubkey(), Account { 
            lamports: fetched_account.lamports, 
            data: fetched_account.data, 
            owner: Pubkey::from(fetched_account.owner.to_bytes()), 
            executable: fetched_account.executable, 
            rent_epoch: fetched_account.rent_epoch 
        }).unwrap();

        msg!("Lamports of fetched account: {}", fetched_account.lamports);
    
        (program, payer)
    }

    #[allow(dead_code)]
    struct EscrowSetup {
        svm: LiteSVM,
        maker: Keypair,
        taker: Keypair,
        mint_a: Pubkey,
        mint_b: Pubkey,
        maker_ata_a: Pubkey,
        taker_ata_b: Pubkey,
        escrow: Pubkey,
        vault: Pubkey,
        seed: u64,
        deposit_amount: u64,
        receive_amount: u64,
    }

    fn setup_escrow(seed: u64, deposit_amount: u64, receive_amount: u64) -> EscrowSetup {
        let (mut svm, maker) = setup();

        let taker = Keypair::new();
        svm.airdrop(&taker.pubkey(), 10 * LAMPORTS_PER_SOL).unwrap();

        let mint_a = CreateMint::new(&mut svm, &maker)
            .decimals(6).authority(&maker.pubkey()).send().unwrap();
        let mint_b = CreateMint::new(&mut svm, &maker)
            .decimals(6).authority(&maker.pubkey()).send().unwrap();

        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut svm, &maker, &mint_a)
            .owner(&maker.pubkey()).send().unwrap();

        MintTo::new(&mut svm, &maker, &mint_a, &maker_ata_a, 1_000_000_000).send().unwrap();

        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()], &PROGRAM_ID
        ).0;
        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);

        let make_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: anchor_escrow_q2_2026::accounts::Make {
                maker: maker.pubkey(), mint_a, mint_b, maker_ata_a, escrow, vault,
                associated_token_program: spl_associated_token_account::program::ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Make { seed, deposit: deposit_amount, receive: receive_amount }.data(),
        };
        let message = Message::new(&[make_ix], Some(&maker.pubkey()));
        let tx = Transaction::new(&[&maker], message, svm.latest_blockhash());
        svm.send_transaction(tx).unwrap();

        let taker_ata_b = CreateAssociatedTokenAccount::new(&mut svm, &taker, &mint_b)
            .owner(&taker.pubkey()).send().unwrap();
        MintTo::new(&mut svm, &maker, &mint_b, &taker_ata_b, 1_000_000_000).send().unwrap();

        EscrowSetup {
            svm, maker, taker, mint_a, mint_b, maker_ata_a, taker_ata_b,
            escrow, vault, seed, deposit_amount, receive_amount,
        }
    }

    #[test]
    fn test_make() {
        let (mut program, payer) = setup();
        let maker = payer.pubkey();
        
        let mint_a = CreateMint::new(&mut program, &payer)
            .decimals(6).authority(&maker).send().unwrap();
        msg!("Mint A: {}\n", mint_a);

        let mint_b = CreateMint::new(&mut program, &payer)
            .decimals(6).authority(&maker).send().unwrap();
        msg!("Mint B: {}\n", mint_b);

        let maker_ata_a = CreateAssociatedTokenAccount::new(&mut program, &payer, &mint_a)
            .owner(&maker).send().unwrap();
        msg!("Maker ATA A: {}\n", maker_ata_a);

        let escrow = Pubkey::find_program_address(
            &[b"escrow", maker.as_ref(), &123u64.to_le_bytes()], &PROGRAM_ID
        ).0;
        msg!("Escrow PDA: {}\n", escrow);

        let vault = associated_token::get_associated_token_address(&escrow, &mint_a);
        msg!("Vault PDA: {}\n", vault);

        let asspciated_token_program = spl_associated_token_account::program::ID;
        let token_program = TOKEN_PROGRAM_ID;
        let system_program = SYSTEM_PROGRAM_ID;

        MintTo::new(&mut program, &payer, &mint_a, &maker_ata_a, 1000000000)
            .send().unwrap();

        let make_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: anchor_escrow_q2_2026::accounts::Make {
                maker, mint_a, mint_b, maker_ata_a, escrow, vault,
                associated_token_program: asspciated_token_program,
                token_program, system_program,
            }.to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Make { deposit: 10, seed: 123u64, receive: 10 }.data(),
        };

        let message = Message::new(&[make_ix], Some(&payer.pubkey()));
        let recent_blockhash = program.latest_blockhash();
        let transaction = Transaction::new(&[&payer], message, recent_blockhash);
        let tx = program.send_transaction(transaction).unwrap();

        msg!("\n\nMake transaction sucessfull");
        msg!("CUs Consumed: {}", tx.compute_units_consumed);
        msg!("Tx Signature: {}", tx.signature);

        let vault_account = program.get_account(&vault).unwrap();
        let vault_data = spl_token::state::Account::unpack(&vault_account.data).unwrap();
        assert_eq!(vault_data.amount, 10);
        assert_eq!(vault_data.owner, escrow);
        assert_eq!(vault_data.mint, mint_a);

        let escrow_account = program.get_account(&escrow).unwrap();
        let escrow_data = anchor_escrow_q2_2026::state::Escrow::try_deserialize(&mut escrow_account.data.as_ref()).unwrap();
        assert_eq!(escrow_data.seed, 123u64);
        assert_eq!(escrow_data.maker, maker);
        assert_eq!(escrow_data.mint_a, mint_a);
        assert_eq!(escrow_data.mint_b, mint_b);
        assert_eq!(escrow_data.receive, 10);
    }

    #[test]
    fn test_take() {
        let deposit_amount: u64 = 500;
        let receive_amount: u64 = 300;
        let mut env = setup_escrow(42, deposit_amount, receive_amount);

        let taker_ata_b_before = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.taker_ata_b).unwrap().data
        ).unwrap().amount;
        let vault_before = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.vault).unwrap().data
        ).unwrap().amount;
        let maker_lamports_before = env.svm.get_account(&env.maker.pubkey()).unwrap().lamports;

        assert_eq!(vault_before, deposit_amount, "Vault should hold the deposited amount");

        let taker_ata_a = associated_token::get_associated_token_address(&env.taker.pubkey(), &env.mint_a);
        let maker_ata_b = associated_token::get_associated_token_address(&env.maker.pubkey(), &env.mint_b);

        let take_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: anchor_escrow_q2_2026::accounts::Take {
                taker: env.taker.pubkey(),
                maker: env.maker.pubkey(),
                mint_a: env.mint_a, mint_b: env.mint_b,
                taker_ata_a, taker_ata_b: env.taker_ata_b, maker_ata_b,
                escrow: env.escrow, vault: env.vault,
                associated_token_program: spl_associated_token_account::program::ID,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Take {}.data(),
        };

        let message = Message::new(&[take_ix], Some(&env.taker.pubkey()));
        let tx = Transaction::new(&[&env.taker], message, env.svm.latest_blockhash());
        let result = env.svm.send_transaction(tx).unwrap();
        msg!("Take CUs: {}, Sig: {}", result.compute_units_consumed, result.signature);

        let taker_a = spl_token::state::Account::unpack(
            &env.svm.get_account(&taker_ata_a).unwrap().data
        ).unwrap();
        assert_eq!(taker_a.amount, deposit_amount, "Taker should receive all vault tokens");
        assert_eq!(taker_a.owner, env.taker.pubkey());
        assert_eq!(taker_a.mint, env.mint_a);

        let taker_b = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.taker_ata_b).unwrap().data
        ).unwrap();
        assert_eq!(taker_b.amount, taker_ata_b_before - receive_amount);

        let maker_b = spl_token::state::Account::unpack(
            &env.svm.get_account(&maker_ata_b).unwrap().data
        ).unwrap();
        assert_eq!(maker_b.amount, receive_amount, "Maker should receive requested mint_b");
        assert_eq!(maker_b.owner, env.maker.pubkey());
        assert_eq!(maker_b.mint, env.mint_b);

        assert!(env.svm.get_account(&env.vault).is_none(), "Vault should be closed");
        assert!(env.svm.get_account(&env.escrow).is_none(), "Escrow should be closed");

        let maker_lamports_after = env.svm.get_account(&env.maker.pubkey()).unwrap().lamports;
        assert!(maker_lamports_after > maker_lamports_before, "Maker should get rent back");

        msg!("── All Take assertions passed ──");
    }

    #[test]
    fn test_refund() {
        let deposit_amount: u64 = 750;
        let receive_amount: u64 = 200;
        let mut env = setup_escrow(99, deposit_amount, receive_amount);

        let maker_ata_a_before = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.maker_ata_a).unwrap().data
        ).unwrap().amount;
        let vault_before = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.vault).unwrap().data
        ).unwrap().amount;
        let maker_lamports_before = env.svm.get_account(&env.maker.pubkey()).unwrap().lamports;

        assert_eq!(vault_before, deposit_amount, "Vault should hold deposited amount");

        let refund_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: anchor_escrow_q2_2026::accounts::Refund {
                maker: env.maker.pubkey(),
                mint_a: env.mint_a,
                maker_ata_a: env.maker_ata_a,
                escrow: env.escrow, vault: env.vault,
                token_program: TOKEN_PROGRAM_ID,
                system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Refund {}.data(),
        };

        let message = Message::new(&[refund_ix], Some(&env.maker.pubkey()));
        let tx = Transaction::new(&[&env.maker], message, env.svm.latest_blockhash());
        let result = env.svm.send_transaction(tx).unwrap();
        msg!("Refund CUs: {}, Sig: {}", result.compute_units_consumed, result.signature);

        let maker_a = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.maker_ata_a).unwrap().data
        ).unwrap();
        assert_eq!(maker_a.amount, maker_ata_a_before + deposit_amount, "Maker should get tokens back");
        assert_eq!(maker_a.owner, env.maker.pubkey());
        assert_eq!(maker_a.mint, env.mint_a);

        assert!(env.svm.get_account(&env.vault).is_none(), "Vault should be closed");
        assert!(env.svm.get_account(&env.escrow).is_none(), "Escrow should be closed");

        let maker_lamports_after = env.svm.get_account(&env.maker.pubkey()).unwrap().lamports;
        assert!(maker_lamports_after > maker_lamports_before, "Maker should get rent back");

        msg!("── All Refund assertions passed ──");
    }

    #[test]
    fn test_take_insufficient_funds() {
        let mut env = setup_escrow(77, 100, 500);

        let drain_target = Keypair::new();
        env.svm.airdrop(&drain_target.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let drain_ata = CreateAssociatedTokenAccount::new(&mut env.svm, &drain_target, &env.mint_b)
            .owner(&drain_target.pubkey()).send().unwrap();

        let balance = spl_token::state::Account::unpack(
            &env.svm.get_account(&env.taker_ata_b).unwrap().data
        ).unwrap().amount;

        let drain_ix = spl_token::instruction::transfer_checked(
            &TOKEN_PROGRAM_ID, &env.taker_ata_b, &env.mint_b, &drain_ata,
            &env.taker.pubkey(), &[], balance, 6,
        ).unwrap();
        let msg = Message::new(&[drain_ix], Some(&env.taker.pubkey()));
        let tx = Transaction::new(&[&env.taker], msg, env.svm.latest_blockhash());
        env.svm.send_transaction(tx).unwrap();

        let taker_ata_a = associated_token::get_associated_token_address(&env.taker.pubkey(), &env.mint_a);
        let maker_ata_b = associated_token::get_associated_token_address(&env.maker.pubkey(), &env.mint_b);

        let take_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: anchor_escrow_q2_2026::accounts::Take {
                taker: env.taker.pubkey(), maker: env.maker.pubkey(),
                mint_a: env.mint_a, mint_b: env.mint_b,
                taker_ata_a, taker_ata_b: env.taker_ata_b, maker_ata_b,
                escrow: env.escrow, vault: env.vault,
                associated_token_program: spl_associated_token_account::program::ID,
                token_program: TOKEN_PROGRAM_ID, system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Take {}.data(),
        };
        let message = Message::new(&[take_ix], Some(&env.taker.pubkey()));
        let tx = Transaction::new(&[&env.taker], message, env.svm.latest_blockhash());
        assert!(env.svm.send_transaction(tx).is_err(), "Take should fail with no funds");

        assert!(env.svm.get_account(&env.escrow).is_some(), "Escrow should still exist");
        assert!(env.svm.get_account(&env.vault).is_some(), "Vault should still exist");
        msg!("── Take insufficient funds test passed ──");
    }

    #[test]
    fn test_refund_wrong_maker() {
        let mut env = setup_escrow(55, 100, 50);

        let imposter = Keypair::new();
        env.svm.airdrop(&imposter.pubkey(), LAMPORTS_PER_SOL).unwrap();
        let imposter_ata_a = CreateAssociatedTokenAccount::new(&mut env.svm, &imposter, &env.mint_a)
            .owner(&imposter.pubkey()).send().unwrap();

        let refund_ix = Instruction {
            program_id: PROGRAM_ID,
            accounts: anchor_escrow_q2_2026::accounts::Refund {
                maker: imposter.pubkey(),
                mint_a: env.mint_a, maker_ata_a: imposter_ata_a,
                escrow: env.escrow, vault: env.vault,
                token_program: TOKEN_PROGRAM_ID, system_program: SYSTEM_PROGRAM_ID,
            }.to_account_metas(None),
            data: anchor_escrow_q2_2026::instruction::Refund {}.data(),
        };
        let message = Message::new(&[refund_ix], Some(&imposter.pubkey()));
        let tx = Transaction::new(&[&imposter], message, env.svm.latest_blockhash());
        assert!(env.svm.send_transaction(tx).is_err(), "Refund should fail for non-maker");

        assert!(env.svm.get_account(&env.escrow).is_some(), "Escrow should still exist");
        assert!(env.svm.get_account(&env.vault).is_some(), "Vault should still exist");
        msg!("── Refund wrong maker test passed ──");
    }
}
