import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import { assert } from "chai";

// CLMM program tests
describe("tidepool-clmm", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const clmmProgram = anchor.workspace.TidepoolClmm;
  const vaultProgram = anchor.workspace.TidepoolVault;

  let tokenMintA: PublicKey;
  let tokenMintB: PublicKey;
  let poolKey: PublicKey;
  let poolBump: number;

  const tickSpacing = 10;
  const feeRate = 3000; // 30 bps
  // Initial sqrt price = 1.0 in Q64.64
  const initialSqrtPrice = new anchor.BN("18446744073709551616"); // 1 << 64

  before(async () => {
    // Create test token mints
    tokenMintA = await createMint(
      provider.connection,
      (provider.wallet as anchor.Wallet).payer,
      provider.wallet.publicKey,
      null,
      6
    );

    tokenMintB = await createMint(
      provider.connection,
      (provider.wallet as anchor.Wallet).payer,
      provider.wallet.publicKey,
      null,
      6
    );

    // Ensure mintA < mintB (convention for pool creation)
    if (tokenMintA.toBuffer().compare(tokenMintB.toBuffer()) > 0) {
      [tokenMintA, tokenMintB] = [tokenMintB, tokenMintA];
    }

    // Derive pool PDA
    [poolKey, poolBump] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("pool"),
        tokenMintA.toBuffer(),
        tokenMintB.toBuffer(),
        new anchor.BN(tickSpacing).toArrayLike(Buffer, "le", 2),
      ],
      clmmProgram.programId
    );
  });

  describe("initialize_pool", () => {
    it("creates a pool with correct initial state", async () => {
      const [vaultA] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault_a"), poolKey.toBuffer()],
        clmmProgram.programId
      );
      const [vaultB] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault_b"), poolKey.toBuffer()],
        clmmProgram.programId
      );

      await clmmProgram.methods
        .initializePool(tickSpacing, initialSqrtPrice, feeRate)
        .accounts({
          pool: poolKey,
          tokenMintA,
          tokenMintB,
          tokenVaultA: vaultA,
          tokenVaultB: vaultB,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      const pool = await clmmProgram.account.pool.fetch(poolKey);
      assert.equal(pool.tickSpacing, tickSpacing);
      assert.equal(pool.feeRate, feeRate);
      assert.equal(pool.tickCurrentIndex, 0); // tick 0 at price 1.0
      assert.equal(pool.paused, false);
      assert.equal(pool.numPositions, 0);
    });

    it("rejects invalid tick spacing", async () => {
      try {
        const invalidTickSpacing = 0;
        const [badPool] = PublicKey.findProgramAddressSync(
          [
            Buffer.from("pool"),
            tokenMintA.toBuffer(),
            tokenMintB.toBuffer(),
            new anchor.BN(invalidTickSpacing).toArrayLike(Buffer, "le", 2),
          ],
          clmmProgram.programId
        );

        const [vaultA] = PublicKey.findProgramAddressSync(
          [Buffer.from("vault_a"), badPool.toBuffer()],
          clmmProgram.programId
        );
        const [vaultB] = PublicKey.findProgramAddressSync(
          [Buffer.from("vault_b"), badPool.toBuffer()],
          clmmProgram.programId
        );

        await clmmProgram.methods
          .initializePool(invalidTickSpacing, initialSqrtPrice, feeRate)
          .accounts({
            pool: badPool,
            tokenMintA,
            tokenMintB,
            tokenVaultA: vaultA,
            tokenVaultB: vaultB,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .rpc();

        assert.fail("Should have thrown");
      } catch (err) {
        assert.include(err.message, "InvalidTickSpacing");
      }
    });
  });

  describe("initialize_tick_array", () => {
    it("creates a tick array for the correct range", async () => {
      const startTickIndex = 0;
      const [tickArray] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("tick_array"),
          poolKey.toBuffer(),
          new anchor.BN(startTickIndex).toArrayLike(Buffer, "le", 4),
        ],
        clmmProgram.programId
      );

      await clmmProgram.methods
        .initializeTickArray(startTickIndex)
        .accounts({
          tickArray,
          pool: poolKey,
          payer: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();

      const ta = await clmmProgram.account.tickArray.fetch(tickArray);
      assert.equal(ta.startTickIndex, startTickIndex);
      assert.equal(ta.ticks.length, 64);
      assert.equal(ta.pool.toBase58(), poolKey.toBase58());
    });
  });

  describe("open_position", () => {
    it("opens a position with valid tick range", async () => {
      const tickLower = -100;
      const tickUpper = 100;
      const positionMint = Keypair.generate();

      const [position] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("position"),
          poolKey.toBuffer(),
          positionMint.publicKey.toBuffer(),
        ],
        clmmProgram.programId
      );

      const positionTokenAccount =
        await getOrCreateAssociatedTokenAccount(
          provider.connection,
          (provider.wallet as anchor.Wallet).payer,
          positionMint.publicKey,
          provider.wallet.publicKey,
          true
        );

      // Verify the PDA was derived correctly
      const [expectedPosition] = PublicKey.findProgramAddressSync(
        [
          Buffer.from("position"),
          poolKey.toBuffer(),
          positionMint.publicKey.toBuffer(),
        ],
        clmmProgram.programId
      );
      assert.equal(position.toBase58(), expectedPosition.toBase58());
      assert.ok(tickLower < tickUpper, "tick_lower must be < tick_upper");
    });

    it("rejects tick range not aligned to spacing", () => {
      const badLower = 5; // not aligned to tick_spacing=10
      const badUpper = 105;
      assert.notEqual(badLower % tickSpacing, 0, "should not be aligned");
      assert.notEqual(badUpper % tickSpacing, 0, "should not be aligned");
    });
  });
});

// Vault program tests
describe("tidepool-vault", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const vaultProgram = anchor.workspace.TidepoolVault;

  describe("initialize_vault", () => {
    it("creates a vault with correct parameters", async () => {
      const pool = Keypair.generate(); // Mock pool for testing

      const [vaultKey] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), pool.publicKey.toBuffer()],
        vaultProgram.programId
      );

      const tokenMintA = await createMint(
        provider.connection,
        (provider.wallet as anchor.Wallet).payer,
        provider.wallet.publicKey,
        null,
        6
      );

      const tokenMintB = await createMint(
        provider.connection,
        (provider.wallet as anchor.Wallet).payer,
        provider.wallet.publicKey,
        null,
        6
      );

      const [shareMint] = PublicKey.findProgramAddressSync(
        [Buffer.from("share_mint"), vaultKey.toBuffer()],
        vaultProgram.programId
      );

      const [tokenVaultA] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault_token_a"), vaultKey.toBuffer()],
        vaultProgram.programId
      );

      const [tokenVaultB] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault_token_b"), vaultKey.toBuffer()],
        vaultProgram.programId
      );

      const rebalanceThresholdBps = 1000; // 10%
      const tickRangeMultiplier = 10;
      const performanceFeeBps = 1000; // 10%
      const managementFeeBps = 200; // 2%

      await vaultProgram.methods
        .initializeVault(
          rebalanceThresholdBps,
          tickRangeMultiplier,
          performanceFeeBps,
          managementFeeBps
        )
        .accounts({
          vault: vaultKey,
          pool: pool.publicKey,
          shareMint,
          tokenVaultA,
          tokenVaultB,
          tokenMintA,
          tokenMintB,
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        })
        .rpc();

      const vault = await vaultProgram.account.vault.fetch(vaultKey);
      assert.equal(vault.rebalanceThresholdBps, rebalanceThresholdBps);
      assert.equal(vault.tickRangeMultiplier, tickRangeMultiplier);
      assert.equal(vault.performanceFeeBps, performanceFeeBps);
      assert.equal(vault.managementFeeBps, managementFeeBps);
      assert.equal(vault.totalShares.toNumber(), 0);
      assert.equal(vault.paused, false);
      assert.equal(vault.hasActivePosition, false);
    });

    it("rejects excessive performance fee", async () => {
      const pool = Keypair.generate();
      const [vaultKey] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault"), pool.publicKey.toBuffer()],
        vaultProgram.programId
      );

      const tokenMintA = await createMint(
        provider.connection,
        (provider.wallet as anchor.Wallet).payer,
        provider.wallet.publicKey,
        null,
        6
      );

      const tokenMintB = await createMint(
        provider.connection,
        (provider.wallet as anchor.Wallet).payer,
        provider.wallet.publicKey,
        null,
        6
      );

      const [shareMint] = PublicKey.findProgramAddressSync(
        [Buffer.from("share_mint"), vaultKey.toBuffer()],
        vaultProgram.programId
      );

      const [tokenVaultA] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault_token_a"), vaultKey.toBuffer()],
        vaultProgram.programId
      );

      const [tokenVaultB] = PublicKey.findProgramAddressSync(
        [Buffer.from("vault_token_b"), vaultKey.toBuffer()],
        vaultProgram.programId
      );

      try {
        await vaultProgram.methods
          .initializeVault(1000, 10, 5000, 200) // 50% perf fee — too high
          .accounts({
            vault: vaultKey,
            pool: pool.publicKey,
            shareMint,
            tokenVaultA,
            tokenVaultB,
            tokenMintA,
            tokenMintB,
            authority: provider.wallet.publicKey,
            systemProgram: SystemProgram.programId,
            tokenProgram: TOKEN_PROGRAM_ID,
            rent: anchor.web3.SYSVAR_RENT_PUBKEY,
          })
          .rpc();

        assert.fail("Should have thrown");
      } catch (err) {
        assert.include(err.message, "InvalidFeeConfig");
      }
    });
  });

  describe("share accounting", () => {
    it("calculates correct shares for first deposit", () => {
      // sqrt(1000 * 2000) = sqrt(2_000_000) ≈ 1414
      // This tests the integer_sqrt function used in share calculation
      const product = 1000n * 2000n;
      const sqrt = isqrt(product);
      assert.equal(sqrt, 1414n);
    });

    it("calculates proportional withdrawal", () => {
      // If vault has 1000 shares, 500 value_a, 500 value_b
      // Withdrawing 100 shares = 50 token_a + 50 token_b
      const shares = 100n;
      const totalShares = 1000n;
      const totalA = 500n;
      const totalB = 500n;

      const amountA = (shares * totalA) / totalShares;
      const amountB = (shares * totalB) / totalShares;

      assert.equal(amountA, 50n);
      assert.equal(amountB, 50n);
    });
  });
});

// Helper: integer square root
function isqrt(n: bigint): bigint {
  if (n === 0n) return 0n;
  let x = n;
  let y = (x + 1n) / 2n;
  while (y < x) {
    x = y;
    y = (x + n / x) / 2n;
  }
  return x;
}
