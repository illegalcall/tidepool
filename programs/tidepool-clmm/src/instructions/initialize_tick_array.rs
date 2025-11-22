use anchor_lang::prelude::*;

use crate::errors::TidePoolError;
use crate::state::tick::{check_tick_bounds, Tick, TickArray, TICK_ARRAY_SIZE};
use crate::state::Pool;

#[derive(Accounts)]
#[instruction(start_tick_index: i32)]
pub struct InitializeTickArray<'info> {
    #[account(
        init,
        payer = payer,
        space = 8 + TickArray::INIT_SPACE,
        seeds = [
            b"tick_array",
            pool.key().as_ref(),
            &start_tick_index.to_le_bytes(),
        ],
        bump,
    )]
    pub tick_array: Account<'info, TickArray>,

    pub pool: Account<'info, Pool>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<InitializeTickArray>, start_tick_index: i32) -> Result<()> {
    let pool = &ctx.accounts.pool;

    // Validate start tick index alignment
    let ticks_in_array = TICK_ARRAY_SIZE as i32 * pool.tick_spacing as i32;
    require!(
        start_tick_index % ticks_in_array == 0,
        TidePoolError::TickNotAligned
    );
    require!(
        check_tick_bounds(start_tick_index),
        TidePoolError::TickOutOfRange
    );

    let tick_array = &mut ctx.accounts.tick_array;
    tick_array.pool = pool.key();
    tick_array.start_tick_index = start_tick_index;
    tick_array.ticks = vec![Tick::default(); TICK_ARRAY_SIZE];
    tick_array.bump = ctx.bumps.tick_array;

    msg!(
        "TickArray initialized: start={}, pool={}",
        start_tick_index,
        pool.key()
    );

    Ok(())
}
