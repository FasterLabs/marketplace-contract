
use anchor_lang::prelude::*;
use anchor_spl::token::{self, TokenAccount, Token, Mint};
use mpl_token_metadata::state::{TokenMetadataAccount, Metadata};
use anchor_lang::system_program;
use anchor_spl::associated_token::{get_associated_token_address};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

// PDA seeds
pub const NFT_INFO_SEED: &str = "faster_nft_info";
pub const MIDDLE_MAN_SEED: &str = "faster_middle_man";
pub const PROGRAM_NFT_AUTHORITY_SEED: &str = "faster_program_nft_authority";
pub const METADATA_SEED: &str = "metadata";

pub const METAPLEX_PROGRAM_ID: &'static str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

// Numbers
pub const LAMPORTS_PER_DROPLET: u16 = 100000000;
pub const DROPLETS_PER_NFT: u16 = 100;

// Collection info, required to verify if an NFT belongs to a collection
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum CollectionInfo {
    // Symbol and verified creators of the collection, for metadata accounts created by CreateMetadataAccount
    V1 {
        symbol: String,
        verified_creators: Vec<Pubkey>,
    },
    // The token mint of the collection NFT, for metadata accounts created by CreateMetadataAccountV2
    V2 {
        collection_mint: Pubkey,
    },
}

impl CollectionInfo {
    // 1 + largest variant: 1 String of 8 chars, 1 Vev<Pubkey>, 1 hash of 32 bytes
    pub const LEN: usize = 1 + (4 + 32) + (4 + (32 * 5));
}

// Verify in the NFT belongs to the collection
pub fn verify_collection(
    metadata: &AccountInfo,
    collection_mint: &Pubkey,
) -> bool {
    let metadata: Metadata = Metadata::from_account_info(metadata).unwrap();

    return match metadata.collection {
        // Check that the collection field exists
        None => false,
        Some(collection) => {
            // Check that the collection mint matches, and verified is true
            collection.key == *collection_mint && collection.verified
        }
    };
}

// pub fn pay_fee_droplet<'info>(
//     amount: u64, 
//     buyer_droplet_token_account: [],
//     creator_droplet_token_account: &Pubkey, 
//     buyer_signer: &Pubkey,
//     token_program: &AccountInfo
// ) -> Result<()>
// {

//     let transfer_droplet_ctx = CpiContext::new(
//         token_program.clone(),
//         token::Transfer{
//             from: buyer_droplet_token_account.clone(),
//             to: creator_droplet_token_account.clone(),
//             authority: buyer_signer.to_account_info(),
//         }
//     );
//     token::transfer(transfer_droplet_ctx, amount)?;

//     Ok(())
// }


#[program]
pub mod marketplace_contract {

    use super::*;

    pub fn create_nft_list(
        ctx: Context<CreateNFTList>,
        _program_nft_authority_bump: u8, 
        tip_creators_sol_fee: u8,
    ) -> Result<()> {
            
        require!(verify_collection(&ctx.accounts.nft_metadata, &ctx.accounts.collection_mint.key()), FasterError::CollectionVerificationFailed);
        
        require!(
            get_associated_token_address(
                &ctx.accounts.signer.key(), 
                &ctx.accounts.nft_mint.key()
            ) == ctx.accounts.nft.key(),
            FasterError::WrongNFTOwner
        );

        let metaplex_pubkey = METAPLEX_PROGRAM_ID
            .parse::<Pubkey>()
            .expect("Failed to parse Metaplex Program Id");
        
        let mint = ctx.accounts.nft_mint.key();
        let seeds = &[
            "metadata".as_bytes(),
            metaplex_pubkey.as_ref(),
            mint.as_ref(),
        ];

        let (metadata_pda, _) = Pubkey::find_program_address(seeds, &metaplex_pubkey);
        if metadata_pda != *ctx.accounts.nft_metadata.key {
            return Err(FasterError::NoMatchMetadata.into());
        }

        let nft_info = &mut ctx.accounts.nft_info;
        nft_info.owner = *ctx.accounts.signer.key;
        nft_info.nft_token_account = ctx.accounts.nft.key();
        nft_info.nft_mint = ctx.accounts.nft_mint.key();
        nft_info.metadata = ctx.accounts.nft_metadata.key();
        nft_info.collection_mint = ctx.accounts.collection_mint.key();
        nft_info.droplet_mint = ctx.accounts.droplet_mint.key();
        nft_info.droplet_token_account = get_associated_token_address(
            &ctx.accounts.signer.key(), 
            &ctx.accounts.droplet_mint.key()
        );
        nft_info.middle_man = ctx.accounts.middle_man.key();
        nft_info.middle_man_bump = *ctx.bumps.get("middle_man").unwrap();
        nft_info.nft_info_bump = *ctx.bumps.get("nft_info").unwrap();
        nft_info.program_nft_authority_bump = *ctx.bumps.get("program_nft_authority").unwrap();
        nft_info.tip_creators_sol_fee = tip_creators_sol_fee;

        Ok(())
    }

    pub fn list_nft(ctx: Context<ListNFT>) -> Result<()>
    {
        // Transfer NFT to middleman
        let transfer_nft_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            token::Transfer {
                from: ctx.accounts.nft.to_account_info(),
                to: ctx.accounts.middle_man.to_account_info(),
                authority: ctx.accounts.signer.to_account_info(),
            }
        );
        token::transfer(transfer_nft_ctx, 1)?;

        Ok(())
    }

    pub fn buy_nft(ctx: Context<BuyNFT>, sol_amount: u8) -> Result<()>
    {
        let metadata: Metadata = Metadata::from_account_info(&ctx.accounts.nft_metadata).unwrap();
        // Fee for Creators
        let buyer_droplet_token_account = get_associated_token_address(
            &ctx.accounts.buyer.key(), 
            &ctx.accounts.nft_mint.key(),
        );
        let creator_fees = metadata.data.seller_fee_basis_points;
        
        if metadata.data.creators.is_some()
        {
            if let Some(creators) = metadata.data.creators{
                 // Pay Royalties (TODO - Divide by 100 if above 100)           
                let creator_droplet_fee = creator_fees
                        .checked_div(creators.len() as u16)
                        .unwrap();
                let creator_sol_fee = ctx
                        .accounts
                        .nft_info
                        .tip_creators_sol_fee
                        .checked_div(creators.len() as u8)
                        .unwrap();
                for creator in creators
                {
                    let creator_droplet_token_account = get_associated_token_address(
                        &creator.address,
                        &ctx.accounts.nft_mint.key()
                    );
                    // pay_fee_droplet(
                    //     creator_droplet_fee
                    //            .checked_mul(DROPLETS_PER_NFT)
                    //            .unwrap()
                    //            .checked_mul(LAMPORTS_PER_DROPLET)
                    //            .unwrap() as u64, 
                    //     AccountMeta::new(buyer_droplet_token_account, false),
                    //     &creator_droplet_token_account,
                    //     &ctx.accounts.buyer.key(),
                    //     &ctx.accounts.token_program.to_account_info(),
                    // );

                    if ctx.accounts.nft_info.tip_creators_sol_fee != 0
                    {
                        system_program::transfer(
                            CpiContext::new(
                                ctx.accounts.system_program.to_account_info(),
                                system_program::Transfer {
                                    from: ctx.accounts.buyer.to_account_info(),
                                    to: creator.address,
                                },
                            ),
                            creator_sol_fee,
                        )?;
                    }
                }
            
            };
        }
        // Fee for protocol

        // check if nft info has collection and matches with metadata

        let is_collection_present = metadata.collection.is_some();
        if !is_collection_present {
            return Err(FasterError::MetadataNotInCollection.into());
        }
        if let Some(collection) = metadata.collection{
            ctx.accounts.nft_info.collection_mint == collection.key
        }else{
            false // throw
        };

        let metaplex_pubkey = METAPLEX_PROGRAM_ID
        .parse::<Pubkey>()
        .expect("Failed to parse Metaplex Program Id");
    
        let mint = ctx.accounts.nft_mint.key();
        let seeds = &[
            "metadata".as_bytes(),
            metaplex_pubkey.as_ref(),
            mint.as_ref(),
        ];

        let (metadata_pda, _) = Pubkey::find_program_address(seeds, &metaplex_pubkey);
        if metadata_pda != *ctx.accounts.nft_metadata.key {
            return Err(FasterError::NoMatchMetadata.into());
        }

        
        // Transfer NFT to Buyer and Close Seller Token Amount

        Ok(())
    }

}

#[account]
pub struct NFTInfo
{
    pub owner: Pubkey,
    pub nft_token_account: Pubkey,
    pub nft_mint: Pubkey,
    pub metadata: Pubkey,
    pub collection_mint: Pubkey,
    pub droplet_mint: Pubkey,
    pub droplet_token_account: Pubkey,
    pub middle_man: Pubkey,
    pub middle_man_bump: u8,
    pub nft_info_bump: u8,
    pub program_nft_authority_bump: u8,
    pub tip_creators_sol_fee: u8,
}

impl NFTInfo 
{
    // Discrimiator, 8 Pubkeys, 4 u8
    pub const LEN: usize = 8 + (32 * 8) + (4 * 1);
}

#[derive(Accounts)]
#[instruction(program_nft_authority_bump: u8)]
pub struct CreateNFTList<'info>
{
    #[account(mut)]
    pub signer: Signer<'info>,
    
    #[account(mut, constraint = nft.mint == nft_mint.key())]
    pub nft: Account<'info, TokenAccount>,

    #[account(mut)]
    pub nft_mint: Account<'info, Mint>,

    #[account(mut)]
    pub collection_mint: Account<'info, Mint>,

    #[account(
        address = mpl_token_metadata::pda::find_metadata_account(&nft_mint.key()).0 @ FasterError::InValidMetadataAccount,
        constraint = mpl_token_metadata::check_id(nft_metadata.owner),
    )]
    /// CHECK: Safe because there are already enough constraints
    // PDA also derived in instruction
    pub nft_metadata: UncheckedAccount<'info>,

    #[account(
        init,
        seeds = [
            NFT_INFO_SEED.as_bytes(),
            signer.key.as_ref(),
            nft_mint.key().as_ref(),
        ],
        payer = signer,
        space = NFTInfo::LEN,
        bump,
    )]
    pub nft_info: Box<Account<'info, NFTInfo>>,

    #[account(
        mint::decimals = 8, 
        mint::authority = solvent_program
    )]
    pub droplet_mint: Box<Account<'info, Mint>>,

    #[account(
        init,
        payer = signer,
        seeds = [
            MIDDLE_MAN_SEED.as_bytes(), 
            signer.key().as_ref(), 
            nft_mint.key().as_ref()
        ],
        bump,
        token::mint = nft_mint,
        token::authority = program_nft_authority,
    )]
    pub middle_man: Account<'info, TokenAccount>,

    #[account(
        seeds = [
            PROGRAM_NFT_AUTHORITY_SEED.as_bytes(),
            solvent_program.key.as_ref(),
        ],
        bump = program_nft_authority_bump,
    )]
    pub program_nft_authority: UncheckedAccount<'info>,

    #[account(executable)]
    pub solvent_program: UncheckedAccount<'info>, 

    // sysvars
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct BuyNFT<'info>
{
    #[account(mut)]
    pub buyer: Signer<'info>,

    #[account(
        constraint = nft_metadata.key() == nft_info.metadata,
    )]
    pub nft_metadata: UncheckedAccount<'info>,

    #[account(mut, constraint = nft.mint == nft_mint.key())]
    pub nft: Account<'info, TokenAccount>,

    #[account(mut)]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        seeds = [
            NFT_INFO_SEED.as_bytes(),
            nft_info.owner.as_ref(),
            nft_mint.key().as_ref(),
        ],
        bump = nft_info.nft_info_bump,
        has_one = nft_mint @ FasterError::InvalidNFTMint,
//        constraint = nft_info.owner == signer.key() @ FasterError::WrongNFTOwner,
        constraint = nft_info.nft_token_account == nft.key() @ FasterError::WrongNFTPassed,
        constraint = nft_info.metadata == nft_metadata.key() @ FasterError::InValidMetadataAccount,
    )]
    pub nft_info: Account<'info, NFTInfo>,

   pub token_program: Program<'info, Token>,
   pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ListNFT<'info>
{
    #[account(mut)]
    pub signer: Signer<'info>,

    #[account(mut, constraint = nft.mint == nft_mint.key())]
    pub nft: Account<'info, TokenAccount>,

    #[account(mut)]
    pub nft_mint: Account<'info, Mint>,

    #[account(
        seeds = [
            NFT_INFO_SEED.as_bytes(),
            signer.key.as_ref(),
            nft_mint.key().as_ref(),
        ],
        bump = nft_info.nft_info_bump,
        has_one = nft_mint @ FasterError::InvalidNFTMint,
        constraint = nft_info.owner == signer.key() @ FasterError::WrongNFTOwner,
        constraint = nft_info.nft_token_account == nft.key() @ FasterError::WrongNFTPassed,
        constraint = nft_info.metadata == nft_metadata.key() @ FasterError::InValidMetadataAccount,
    )]
    pub nft_info: Account<'info, NFTInfo>,

    #[account(
        mut,
        seeds = [
            MIDDLE_MAN_SEED.as_bytes(), 
            signer.key().as_ref(), 
            nft_mint.key().as_ref()
        ],
        bump,
        token::mint = nft_mint,
        token::authority = program_nft_authority,
        constraint = nft_info.middle_man == middle_man.key(),
    )]
    pub middle_man: Account<'info, TokenAccount>,

    #[account(
        seeds = [
            PROGRAM_NFT_AUTHORITY_SEED.as_bytes(),
            solvent_program.key.as_ref(),
        ],
        bump = nft_info.program_nft_authority_bump,
    )]
    pub program_nft_authority: UncheckedAccount<'info>,

    #[account(executable)]
    pub solvent_program: UncheckedAccount<'info>, 


    /// CHECK: Safe because only reading is done
    pub nft_metadata: UncheckedAccount<'info>,

    // sysvars
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}


#[error_code]
pub enum FasterError 
{
    #[msg("NFT does not match with the specified Collection")]
    CollectionVerificationFailed,

    #[msg("The metadata is invalid, check that correct mint is passed")]
    InValidMetadataAccount,

    #[msg("The NFT mint is Invalid")]
    InvalidNFTMint,

    #[msg("This Account does not own the given NFT")]
    WrongNFTOwner,

    #[msg("This NFT is Invalid")]
    WrongNFTPassed,

    #[msg("This metadata does not have a corresponding collection")]
    MetadataNotInCollection,

    #[msg("invalid metadata information")]
    NoMatchMetadata,
}
// TODO 
// SOLVENT PROGRAM should be static str
// Protocol fee shoould also be static str


/*
Danger

EXTREMELY IMPORTANT 🚨

Explorers, Wallets and Marketplaces, MUST CHECK that Verified is true. Verified can only be set true if the Authority on the Collection NFT has run the VerifyCollection instruction over the NFT.

This is the same pattern as the Creators field where Verified must be true to validate the NFT.

In Order to check if a collection is valid on an NFT you MUST:

    Check that the Collection struct is set.
    Check that the Key in the Collection struct is owned on chain by the SPL Token program.
    Check that Verified is true.

If those 3 steps are not followed you could be exposing fraudulent NFTs on real collections.


*/