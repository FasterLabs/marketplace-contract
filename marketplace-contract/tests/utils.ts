import * as anchor from "@project-serum/anchor";
import {
  createAssociatedTokenAccount,
  createMint,
  getAccount,
  mintToChecked,
} from "@solana/spl-token-latest";
import {
  PROGRAM_ID as METADATA_PROGRAM_ID,
  createCreateMetadataAccountV2Instruction,
  createCreateMetadataAccountInstruction,
  createCreateMasterEditionV3Instruction,
  createVerifyCollectionInstruction,
} from "@metaplex-foundation/mpl-token-metadata";

export const provider = anchor.getProvider() as anchor.AnchorProvider;
export const BUCKET_SEED = Buffer.from("bucket-seed-v3");
export const createKeypair = async (provider: anchor.Provider) => {
    const keypair = new anchor.web3.Keypair();
    const txn = await provider.connection.requestAirdrop(
      keypair.publicKey,
      10 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(txn);
    return keypair;
  };

  export const mintNft = async (
    provider: anchor.Provider,
    symbol: string,
    creator: anchor.web3.Keypair,
    destination: anchor.web3.PublicKey,
    collectionMint?: anchor.web3.PublicKey,
    v1: boolean = false
  ) => {
    const mint = await createMint(
      provider.connection,
      creator,
      creator.publicKey,
      null,
      0
    );
  
    const tokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      creator,
      mint,
      destination
    );
  
    await mintToChecked(
      provider.connection,
      creator,
      mint,
      tokenAccount,
      creator.publicKey,
      1,
      0
    );
  
    const transaction = new anchor.web3.Transaction();
  
    // Set Metadata
    const metadata = await getTokenMetadata(mint);
    v1
      ? transaction.add(
          createCreateMetadataAccountInstruction(
            {
              metadata,
              mint,
              mintAuthority: creator.publicKey,
              updateAuthority: creator.publicKey,
              payer: creator.publicKey,
            },
            {
              createMetadataAccountArgs: {
                isMutable: false,
                data: {
                  name: "Pretty Cool NFT",
                  symbol: nftSymbol,
                  sellerFeeBasisPoints: 10,
                  uri: "https://pretty-cool-nft.xyz/metadata",
                  creators: [
                    {
                      address: creator.publicKey,
                      share: 100,
                      verified: true,
                    },
                  ],
                },
              },
            }
          )
        )
      : transaction.add(
          createCreateMetadataAccountV2Instruction(
            {
              metadata,
              mint,
              mintAuthority: creator.publicKey,
              updateAuthority: creator.publicKey,
              payer: creator.publicKey,
            },
            {
              createMetadataAccountArgsV2: {
                isMutable: false,
                data: {
                  name: "Pretty Cool NFT",
                  symbol: "PCN",
                  sellerFeeBasisPoints: 10,
                  uri: "https://pretty-cool-nft.xyz/metadata",
                  creators: [
                    {
                      address: creator.publicKey,
                      share: 100,
                      verified: true,
                    },
                  ],
                  collection: collectionMint
                    ? { key: collectionMint, verified: false }
                    : null,
                  uses: null,
                },
              },
            }
          )
        );
  
    // Create master edition
    const edition = await getTokenEdition(mint);
    transaction.add(
      createCreateMasterEditionV3Instruction(
        {
          edition,
          mint,
          updateAuthority: creator.publicKey,
          mintAuthority: creator.publicKey,
          payer: creator.publicKey,
          metadata,
        },
        { createMasterEditionArgs: { maxSupply: 0 } }
      )
    );
  
    // @ts-ignore
    await provider.sendAndConfirm(transaction, [creator]);
  
    return mint;
  };
  

  const getTokenMetadata = async (tokenMint: anchor.web3.PublicKey) => {
    const [tokenMetadataAddress, bump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [
          Buffer.from("metadata"),
          METADATA_PROGRAM_ID.toBuffer(),
          tokenMint.toBuffer(),
        ],
        METADATA_PROGRAM_ID
      );
    return tokenMetadataAddress;
  };

  const getTokenEdition = async (tokenMint: anchor.web3.PublicKey) => {
    const [tokenMetadataAddress, bump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [
          Buffer.from("metadata"),
          METADATA_PROGRAM_ID.toBuffer(),
          tokenMint.toBuffer(),
          Buffer.from("edition"),
        ],
        METADATA_PROGRAM_ID
      );
    return tokenMetadataAddress;
  };

  export const verifyCollection = async (
    provider: anchor.AnchorProvider,
    nftMint: anchor.web3.PublicKey,
    collectionMint: anchor.web3.PublicKey,
    collectionAuthority: anchor.web3.Keypair
  ) => {
    // Setup: Verify collection of the NFT
    const transaction = new anchor.web3.Transaction();
    transaction.add(
      createVerifyCollectionInstruction({
        metadata: await getTokenMetadata(nftMint),
        collectionAuthority: collectionAuthority.publicKey,
        payer: provider.wallet.publicKey,
        collectionMint: collectionMint,
        collection: await getTokenMetadata(collectionMint),
        collectionMasterEditionAccount: await getTokenEdition(collectionMint),
      })
    );
    return provider.sendAndConfirm(transaction, [collectionAuthority]);
  };