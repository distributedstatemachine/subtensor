//! Subnet Merger Module
//!
//! This module implements the subnet merger system based on Mergers2.pdf.
//! It enables two subnets to merge while preserving redeemability for all token holders.
//!
//! Key Features:
//! - Dual-consent governance (both subnet owners must approve)
//! - Mathematical guarantee of value preservation via redeemability conservation
//! - Price maintenance for the surviving (alpha) subnet
//! - Complete cleanup of the merged (beta) subnet

use super::*;
use crate::{MergerConsent, MergerHistory, MergerStatus, PendingMerger, PoolSnapshot};
use alloc::collections::BTreeMap;
use frame_support::ensure;
use sp_runtime::DispatchResult;
use substrate_fixed::types::{U64F64, U96F32};
use subtensor_runtime_common::{AlphaCurrency, MechId, NetUid, TaoCurrency};

const LOG_TARGET: &str = "runtime::subtensor::merger";

/// Rounding tolerance for redeemability checks (100 rao)
pub const REDEEMABILITY_ROUNDING_TOLERANCE: i128 = 100;

impl<T: Config> Pallet<T> {
    // ========================
    // Public Extrinsic Implementations
    // ========================

    /// Implementation for the propose_merger extrinsic
    ///
    /// # Args:
    /// * `proposer`: The account proposing the merger (for event emission)
    /// * `alpha_netuid`: The surviving subnet (mother pool)
    /// * `beta_netuid`: The subnet to be merged (child pool)
    pub fn do_propose_merger(
        proposer: T::AccountId,
        alpha_netuid: NetUid,
        beta_netuid: NetUid,
    ) -> DispatchResult {
        log::info!(
            target: LOG_TARGET,
            "Merger proposal: alpha={:?}, beta={:?}, proposer={:?}",
            alpha_netuid,
            beta_netuid,
            proposer
        );

        // 1. Validate subnets exist and are not the same
        ensure!(alpha_netuid != beta_netuid, Error::<T>::CannotMergeWithSelf);
        ensure!(
            Self::if_subnet_exist(alpha_netuid),
            Error::<T>::SubnetNotExists
        );
        ensure!(
            Self::if_subnet_exist(beta_netuid),
            Error::<T>::SubnetNotExists
        );

        // 2. Check alpha subnet doesn't have a pending merger
        // Note: We don't check MergerHistory because:
        //   - If a netuid was merged (deleted), it won't pass if_subnet_exist check above
        //   - If a netuid was reused after deletion, it's a NEW subnet and should be allowed
        //   - Subnets should be able to merge multiple times (accumulating value)
        ensure!(
            !PendingMerger::<T>::contains_key(alpha_netuid),
            Error::<T>::MergerAlreadyPending
        );

        // 4. Validate minimum liquidity thresholds
        Self::validate_merger_liquidity(alpha_netuid, beta_netuid)?;

        // 5. Store proposal
        let current_block = frame_system::Pallet::<T>::block_number();
        PendingMerger::<T>::insert(
            alpha_netuid,
            (beta_netuid, current_block, MergerStatus::Proposed),
        );

        // 6. Emit event
        Self::deposit_event(Event::MergerProposed {
            alpha: alpha_netuid,
            beta: beta_netuid,
            proposer,
            block: current_block,
        });

        log::info!(target: LOG_TARGET, "Merger proposal created successfully");

        Ok(())
    }

    /// Implementation for the approve_merger extrinsic
    pub fn do_approve_merger(
        approver: T::AccountId,
        alpha_netuid: NetUid,
        beta_netuid: NetUid,
    ) -> DispatchResult {
        log::info!(
            target: LOG_TARGET,
            "Merger approval: alpha={:?}, beta={:?}, approver={:?}",
            alpha_netuid,
            beta_netuid,
            approver
        );

        // 2. Verify proposal exists
        let (proposed_beta, block, status) =
            PendingMerger::<T>::get(alpha_netuid).ok_or(Error::<T>::NoMergerProposal)?;
        ensure!(proposed_beta == beta_netuid, Error::<T>::MergerMismatch);
        ensure!(
            status == MergerStatus::Proposed,
            Error::<T>::InvalidMergerStatus
        );

        // 3. Update status and consent
        PendingMerger::<T>::insert(alpha_netuid, (beta_netuid, block, MergerStatus::Approved));
        MergerConsent::<T>::insert(alpha_netuid, beta_netuid, true);

        // 4. Emit event
        Self::deposit_event(Event::MergerApproved {
            alpha: alpha_netuid,
            beta: beta_netuid,
            approver,
        });

        log::info!(target: LOG_TARGET, "Merger approved successfully");

        Ok(())
    }

    /// Implementation for the cancel_merger extrinsic
    pub fn do_cancel_merger(cancelled_by: T::AccountId, alpha_netuid: NetUid) -> DispatchResult {
        // Verify proposal exists and not executing
        let (beta_netuid, _, status) =
            PendingMerger::<T>::get(alpha_netuid).ok_or(Error::<T>::NoMergerProposal)?;
        ensure!(
            status != MergerStatus::Executing,
            Error::<T>::MergerAlreadyExecuting
        );

        // Remove proposal
        PendingMerger::<T>::remove(alpha_netuid);
        MergerConsent::<T>::remove(alpha_netuid, beta_netuid);

        Self::deposit_event(Event::MergerCancelled {
            alpha: alpha_netuid,
            beta: beta_netuid,
            cancelled_by,
        });

        log::info!(
            target: LOG_TARGET,
            "Merger cancelled: alpha={:?}, beta={:?}",
            alpha_netuid,
            beta_netuid
        );

        Ok(())
    }

    /// Implementation for the execute_merger extrinsic (wrapper)
    pub fn do_execute_merger_extrinsic(alpha_netuid: NetUid) -> DispatchResult {
        log::info!(
            target: LOG_TARGET,
            "Merger execution starting: alpha={:?}",
            alpha_netuid
        );

        // 1. Verify merger is approved
        let (beta_netuid, proposal_block, status) =
            PendingMerger::<T>::get(alpha_netuid).ok_or(Error::<T>::NoMergerProposal)?;
        ensure!(
            status == MergerStatus::Approved,
            Error::<T>::MergerNotApproved
        );
        ensure!(
            MergerConsent::<T>::get(alpha_netuid, beta_netuid),
            Error::<T>::MergerNotApproved
        );

        // 2. Update status to executing
        PendingMerger::<T>::insert(
            alpha_netuid,
            (beta_netuid, proposal_block, MergerStatus::Executing),
        );

        // 3. Execute merger (this is the heavy operation)
        Self::do_execute_merger(alpha_netuid, beta_netuid)?;

        // 4. Clean up proposal state
        PendingMerger::<T>::remove(alpha_netuid);
        MergerConsent::<T>::remove(alpha_netuid, beta_netuid);

        // 5. Record in history (permanent)
        let current_block = frame_system::Pallet::<T>::block_number();
        MergerHistory::<T>::insert(beta_netuid, (alpha_netuid, current_block));

        // 6. Emit event
        Self::deposit_event(Event::MergerExecuted {
            alpha: alpha_netuid,
            beta: beta_netuid,
            block: current_block,
        });

        log::info!(
            target: LOG_TARGET,
            "Merger executed successfully: beta {:?} merged into alpha {:?}",
            beta_netuid,
            alpha_netuid
        );

        Ok(())
    }

    // ========================
    // Core Merger Logic (Internal)
    // ========================

    /// Execute the merger algorithm (internal function)
    ///
    /// This is the core merger logic called by do_execute_merger_extrinsic.
    fn do_execute_merger(alpha_netuid: NetUid, beta_netuid: NetUid) -> DispatchResult {
        log::info!(
            target: LOG_TARGET,
            "Starting core merger: alpha={:?}, beta={:?}",
            alpha_netuid,
            beta_netuid
        );

        // Step 1: Take snapshots of pool states
        let alpha_snapshot = Self::snapshot_pool(alpha_netuid)?;
        let beta_snapshot = Self::snapshot_pool(beta_netuid)?;

        log::debug!(
            target: LOG_TARGET,
            "Snapshots - Alpha: TAO={}, Alpha={}, Price={:?} | Beta: TAO={}, Beta={}, Price={:?}",
            alpha_snapshot.tao_reserve,
            alpha_snapshot.alpha_reserve,
            alpha_snapshot.price,
            beta_snapshot.tao_reserve,
            beta_snapshot.alpha_reserve,
            beta_snapshot.price
        );

        // Step 2: Pre-merger validation
        Self::validate_merger_safety(&alpha_snapshot, &beta_snapshot)?;

        // Step 3: Calculate all token conversions
        let conversions = Self::calculate_all_conversions(
            alpha_netuid,
            beta_netuid,
            &alpha_snapshot,
            &beta_snapshot,
        )?;

        log::info!(
            target: LOG_TARGET,
            "Calculated {} token conversions",
            conversions.len()
        );

        // Step 4: Apply token conversions (update all stake shares)
        Self::apply_token_conversions(alpha_netuid, beta_netuid, &conversions)?;

        // Step 5: Consolidate pool reserves
        Self::consolidate_pools(alpha_netuid, beta_netuid, &alpha_snapshot, &beta_snapshot)?;

        // Step 6: Verify redeemability conservation
        Self::verify_redeemability_conservation(
            alpha_netuid,
            &alpha_snapshot,
            &beta_snapshot,
            &conversions,
        )?;

        // Step 7: Clean up beta subnet state
        Self::cleanup_merged_subnet(beta_netuid)?;

        log::info!(target: LOG_TARGET, "Core merger completed successfully");

        Ok(())
    }

    // ========================
    // Validation Functions
    // ========================

    /// Validate that both pools have sufficient liquidity for safe merger
    pub fn validate_merger_liquidity(alpha_netuid: NetUid, beta_netuid: NetUid) -> DispatchResult {
        let alpha_snapshot = Self::snapshot_pool(alpha_netuid)?;
        let beta_snapshot = Self::snapshot_pool(beta_netuid)?;

        // Ensure positive reserves (cannot merge empty pools)
        ensure!(
            !alpha_snapshot.tao_reserve.is_zero(),
            Error::<T>::InsufficientLiquidity
        );
        ensure!(
            !beta_snapshot.tao_reserve.is_zero(),
            Error::<T>::InsufficientLiquidity
        );
        ensure!(
            !alpha_snapshot.alpha_reserve.is_zero(),
            Error::<T>::InsufficientLiquidity
        );
        ensure!(
            !beta_snapshot.alpha_reserve.is_zero(),
            Error::<T>::InsufficientLiquidity
        );

        // Ensure pools have positive prices
        let zero_price = U96F32::from_num(0);
        ensure!(alpha_snapshot.price > zero_price, Error::<T>::ZeroPrice);
        ensure!(beta_snapshot.price > zero_price, Error::<T>::ZeroPrice);

        Ok(())
    }

    /// Validate overall pool health before merger
    fn validate_merger_safety(
        alpha_snapshot: &PoolSnapshot,
        beta_snapshot: &PoolSnapshot,
    ) -> DispatchResult {
        // Check 1: Ensure beta pool is not larger than alpha
        // Alpha should be the "mother" pool that absorbs beta
        ensure!(
            alpha_snapshot.tao_reserve >= beta_snapshot.tao_reserve,
            Error::<T>::InvalidMergerDirection
        );

        // Check 2: Ensure no arithmetic overflow in combined reserves
        let combined_tao = alpha_snapshot
            .tao_reserve
            .saturating_add(beta_snapshot.tao_reserve);
        let max_safe_tao = TaoCurrency::from(u64::MAX / 2);
        ensure!(combined_tao < max_safe_tao, Error::<T>::ReserveOverflow);

        Ok(())
    }

    /// Capture a snapshot of a pool's current state
    fn snapshot_pool(netuid: NetUid) -> Result<PoolSnapshot, Error<T>> {
        // Convert to u64 for checked arithmetic, then back to TaoCurrency
        let tao_base: u64 = SubnetTAO::<T>::get(netuid).into();
        let tao_provided: u64 = SubnetTaoProvided::<T>::get(netuid).into();
        let tao_reserve: TaoCurrency = tao_base
            .checked_add(tao_provided)
            .ok_or(Error::<T>::ReserveOverflow)?
            .into();

        // Convert to u64 for checked arithmetic, then back to AlphaCurrency
        let alpha_base: u64 = SubnetAlphaIn::<T>::get(netuid).into();
        let alpha_provided: u64 = SubnetAlphaInProvided::<T>::get(netuid).into();
        let alpha_reserve: AlphaCurrency = alpha_base
            .checked_add(alpha_provided)
            .ok_or(Error::<T>::ReserveOverflow)?
            .into();
        let alpha_out = SubnetAlphaOut::<T>::get(netuid);

        // Calculate price: p = τ / α
        let price = if !alpha_reserve.is_zero() {
            U96F32::from_num(tao_reserve)
                .checked_div(U96F32::from_num(alpha_reserve))
                .ok_or(Error::<T>::Overflow)?
        } else {
            U96F32::from_num(0)
        };

        // Count stakers - iterate through all hotkeys and count those with stake in this netuid
        let mut total_stakers: u32 = 0;
        for (_hotkey, netuid_iter, alpha_amount) in TotalHotkeyAlpha::<T>::iter() {
            if netuid_iter == netuid && !alpha_amount.is_zero() {
                total_stakers = total_stakers.saturating_add(1);
            }
        }

        Ok(PoolSnapshot {
            tao_reserve,
            alpha_reserve,
            alpha_out,
            price,
            total_stakers,
        })
    }

    // ========================
    // Token Conversion Functions (Equations 8 & 9)
    // ========================

    /// Calculate token conversions for all holders in both subnets
    fn calculate_all_conversions(
        alpha_netuid: NetUid,
        beta_netuid: NetUid,
        alpha_snapshot: &PoolSnapshot,
        beta_snapshot: &PoolSnapshot,
    ) -> Result<BTreeMap<(T::AccountId, NetUid), crate::ConversionResult>, Error<T>> {
        let mut conversions = BTreeMap::new();

        // Convert reserves to U64F64 for precision
        let tao_alpha = U64F64::from_num(alpha_snapshot.tao_reserve);
        let tao_beta = U64F64::from_num(beta_snapshot.tao_reserve);
        let alpha_total = U64F64::from_num(alpha_snapshot.alpha_reserve);
        let beta_total = U64F64::from_num(beta_snapshot.alpha_reserve);
        let p_alpha = U64F64::from_num(alpha_snapshot.price);
        let p_beta = U64F64::from_num(beta_snapshot.price);

        // Convert all alpha holders (Equation 8)
        for (hotkey, netuid_iter, alpha_i) in TotalHotkeyAlpha::<T>::iter() {
            if netuid_iter == alpha_netuid && !alpha_i.is_zero() {
                let alpha_i_f64 = U64F64::from_num(alpha_i);
                let new_alpha =
                    Self::convert_alpha_holder(alpha_i_f64, alpha_total, tao_alpha, tao_beta)?;

                let old_redeem = Self::calculate_redeemability(alpha_i_f64, tao_alpha, alpha_total);
                let new_redeem =
                    Self::calculate_redeemability(new_alpha, tao_alpha + tao_beta, alpha_total);

                conversions.insert(
                    (hotkey, alpha_netuid),
                    crate::ConversionResult {
                        new_alpha_amount: new_alpha,
                        redeemability_delta: (new_redeem.to_num::<i128>()
                            - old_redeem.to_num::<i128>()),
                    },
                );
            }
        }

        // Convert all beta holders (Equation 9)
        for (hotkey, netuid_iter, beta_i) in TotalHotkeyAlpha::<T>::iter() {
            if netuid_iter == beta_netuid && !beta_i.is_zero() {
                let beta_i_f64 = U64F64::from_num(beta_i);
                let new_alpha = Self::convert_beta_holder(
                    beta_i_f64, beta_total, tao_alpha, tao_beta, p_beta, p_alpha,
                )?;

                let old_redeem = Self::calculate_redeemability(beta_i_f64, tao_beta, beta_total);
                let new_redeem =
                    Self::calculate_redeemability(new_alpha, tao_alpha + tao_beta, alpha_total);

                conversions.insert(
                    (hotkey, beta_netuid),
                    crate::ConversionResult {
                        new_alpha_amount: new_alpha,
                        redeemability_delta: (new_redeem.to_num::<i128>()
                            - old_redeem.to_num::<i128>()),
                    },
                );
            }
        }

        Ok(conversions)
    }

    /// Convert alpha holder tokens according to Equation 8 from Mergers2.pdf
    ///
    /// Formula: α'_i = α_i × [1 / (1 + (τ_β/(τ_α+τ_β)) × (α_i/α))]
    ///
    /// Alpha holders experience dilution due to increased liquidity
    pub fn convert_alpha_holder(
        alpha_i: U64F64,     // Individual stake
        alpha_total: U64F64, // Total alpha in pool
        tao_alpha: U64F64,   // Alpha TAO reserve
        tao_beta: U64F64,    // Beta TAO reserve
    ) -> Result<U64F64, Error<T>> {
        let tao_total = tao_alpha.saturating_add(tao_beta);

        // Calculate ratio: α_i / α
        let stake_ratio = alpha_i
            .checked_div(alpha_total)
            .ok_or(Error::<T>::Overflow)?;

        // Calculate factor: τ_β / (τ_α + τ_β)
        let tao_factor = tao_beta
            .checked_div(tao_total)
            .ok_or(Error::<T>::Overflow)?;

        // Calculate denominator: 1 + (factor × ratio)
        let product = tao_factor.saturating_mul(stake_ratio);
        let denominator = U64F64::from_num(1).saturating_add(product);

        // Calculate new alpha: α_i / denominator
        let alpha_i_new = alpha_i
            .checked_div(denominator)
            .ok_or(Error::<T>::Overflow)?;

        Ok(alpha_i_new)
    }

    /// Convert beta holder tokens according to Equation 9 from Mergers2.pdf
    ///
    /// Formula: β'_i = β_i × [1 / (1 + (τ_α/(τ_α+τ_β)) × (β_i/β))] × (p_β/p_α)
    ///
    /// Beta tokens are converted to alpha with liquidity adjustment and price ratio
    pub fn convert_beta_holder(
        beta_i: U64F64,     // Individual stake
        beta_total: U64F64, // Total beta in pool
        tao_alpha: U64F64,  // Alpha TAO reserve
        tao_beta: U64F64,   // Beta TAO reserve
        p_beta: U64F64,     // Beta price
        p_alpha: U64F64,    // Alpha price
    ) -> Result<U64F64, Error<T>> {
        let tao_total = tao_alpha.saturating_add(tao_beta);

        // Calculate ratio: β_i / β
        let stake_ratio = beta_i.checked_div(beta_total).ok_or(Error::<T>::Overflow)?;

        // Calculate factor: τ_α / (τ_α + τ_β)
        let tao_factor = tao_alpha
            .checked_div(tao_total)
            .ok_or(Error::<T>::Overflow)?;

        // Calculate denominator: 1 + (factor × ratio)
        let product = tao_factor.saturating_mul(stake_ratio);
        let denominator = U64F64::from_num(1).saturating_add(product);

        // Calculate base conversion: β_i / denominator
        let beta_converted = beta_i
            .checked_div(denominator)
            .ok_or(Error::<T>::Overflow)?;

        // Apply price adjustment: × (p_β / p_α)
        let price_ratio = p_beta.checked_div(p_alpha).ok_or(Error::<T>::Overflow)?;

        let beta_i_new = beta_converted.saturating_mul(price_ratio);

        Ok(beta_i_new)
    }

    /// Calculate how much TAO can be redeemed for a given stake
    ///
    /// Uses constant product formula: Δy = (y × Δx) / (x + Δx)
    /// where: y = tao_reserve, Δx = stake, x = alpha_reserve
    pub fn calculate_redeemability(
        stake: U64F64,
        tao_reserve: U64F64,
        alpha_reserve: U64F64,
    ) -> U64F64 {
        let numerator = stake.saturating_mul(tao_reserve);
        let denominator = alpha_reserve.saturating_add(stake);

        numerator
            .checked_div(denominator)
            .unwrap_or(U64F64::from_num(0))
    }

    /// Verify that redeemability is preserved for all token holders
    fn verify_redeemability_conservation(
        _alpha_netuid: NetUid,
        alpha_snapshot: &PoolSnapshot,
        beta_snapshot: &PoolSnapshot,
        conversions: &BTreeMap<(T::AccountId, NetUid), crate::ConversionResult>,
    ) -> DispatchResult {
        let mut total_redeemability_change: i128 = 0;

        // Check each conversion maintains or improves redeemability
        for ((account, _source_netuid), conversion) in conversions.iter() {
            // Individual redeemability delta should be >= 0 (allowing for rounding)
            // Allow small negative deltas due to rounding (e.g., -100 rao)
            ensure!(
                conversion.redeemability_delta >= -REDEEMABILITY_ROUNDING_TOLERANCE,
                Error::<T>::RedeemabilityViolation
            );

            if conversion.redeemability_delta < 0 {
                log::warn!(
                    target: LOG_TARGET,
                    "Small redeemability loss detected for {:?}: {} (within tolerance)",
                    account,
                    conversion.redeemability_delta
                );
            }

            // Accumulate total system redeemability change
            total_redeemability_change =
                total_redeemability_change.saturating_add(conversion.redeemability_delta);
        }

        // Verify total system redeemability is conserved
        // The sum of all redeemability changes should be approximately zero
        // We allow a small positive sum due to rounding, but negative would indicate value loss
        let tolerance_per_user = REDEEMABILITY_ROUNDING_TOLERANCE;
        let max_stakers = alpha_snapshot
            .total_stakers
            .saturating_add(beta_snapshot.total_stakers);
        let total_tolerance = tolerance_per_user.saturating_mul(max_stakers as i128);

        ensure!(
            total_redeemability_change >= -total_tolerance,
            Error::<T>::RedeemabilityViolation
        );

        log::info!(
            target: LOG_TARGET,
            "Redeemability verification passed - Total change: {} (tolerance: {})",
            total_redeemability_change,
            total_tolerance
        );

        Ok(())
    }

    // ========================
    // Pool Operations Functions
    // ========================

    /// Apply token conversions to all stake holders
    fn apply_token_conversions(
        alpha_netuid: NetUid,
        beta_netuid: NetUid,
        conversions: &BTreeMap<(T::AccountId, NetUid), crate::ConversionResult>,
    ) -> DispatchResult {
        let mut total_alpha_converted: u64 = 0;
        let mut total_beta_converted: u64 = 0;

        // Update all stakes according to conversion results
        for ((hotkey, source_netuid), conversion) in conversions.iter() {
            let new_amount_raw: u64 = conversion.new_alpha_amount.to_num::<u64>();

            if *source_netuid == alpha_netuid {
                // Update alpha holder's stake
                TotalHotkeyAlpha::<T>::mutate(hotkey, alpha_netuid, |stake| {
                    let old: u64 = (*stake).into();
                    *stake = new_amount_raw.into();
                    total_alpha_converted += old.saturating_sub(new_amount_raw);
                });
            } else if *source_netuid == beta_netuid {
                // Remove beta stake
                let beta_amount = TotalHotkeyAlpha::<T>::take(hotkey, beta_netuid);
                let beta_amount_u64: u64 = beta_amount.into();
                total_beta_converted += beta_amount_u64;

                // Add converted alpha stake (convert to u64, check, convert back)
                TotalHotkeyAlpha::<T>::mutate(hotkey, alpha_netuid, |stake| {
                    let current: u64 = (*stake).into();
                    let new_total = current
                        .checked_add(new_amount_raw)
                        .expect("Stake addition overflow - this should never happen as conversions reduce stake");
                    *stake = new_total.into();
                });

                // Emit conversion event
                Self::deposit_event(Event::TokensConverted {
                    account: hotkey.clone(),
                    source_netuid: beta_netuid,
                    target_netuid: alpha_netuid,
                    old_amount: beta_amount_u64,
                    new_amount: new_amount_raw,
                });
            }
        }

        log::info!(
            target: LOG_TARGET,
            "Tokens converted - Alpha reduced: {}, Beta converted: {}",
            total_alpha_converted,
            total_beta_converted
        );

        Ok(())
    }

    /// Consolidate pool reserves after token conversion
    fn consolidate_pools(
        alpha_netuid: NetUid,
        beta_netuid: NetUid,
        alpha_snapshot: &PoolSnapshot,
        beta_snapshot: &PoolSnapshot,
    ) -> DispatchResult {
        // Step 1: Calculate new TAO reserve (simply add them)
        // Note: This is already validated in validate_merger_safety, but use checked_add for safety
        let tao_alpha: u64 = alpha_snapshot.tao_reserve.into();
        let tao_beta: u64 = beta_snapshot.tao_reserve.into();
        let tao_alpha_new: TaoCurrency = tao_alpha
            .checked_add(tao_beta)
            .ok_or(Error::<T>::ReserveOverflow)?
            .into();

        // Step 2: Calculate new alpha reserve to maintain price
        // α^new = τ^new / p_α
        let p_alpha = U96F32::from_num(alpha_snapshot.price);
        let alpha_new = U96F32::from_num(tao_alpha_new)
            .checked_div(p_alpha)
            .ok_or(Error::<T>::Overflow)?;
        let alpha_new_u64: u64 = alpha_new.to_num::<u64>();
        let alpha_reserve_u64: u64 = alpha_snapshot.alpha_reserve.into();

        // Step 3: Calculate delta alpha (amount to mint)
        let delta_alpha: AlphaCurrency = alpha_new_u64.saturating_sub(alpha_reserve_u64).into();

        // Step 4: Update alpha pool reserves
        SubnetTAO::<T>::insert(alpha_netuid, tao_alpha_new);
        SubnetAlphaIn::<T>::mutate(alpha_netuid, |alpha| {
            let current: u64 = (*alpha).into();
            let delta: u64 = delta_alpha.into();
            let new_alpha = current
                .checked_add(delta)
                .expect("Alpha reserve overflow after merger - this should be impossible");
            *alpha = new_alpha.into();
        });
        SubnetAlphaOut::<T>::mutate(alpha_netuid, |alpha| {
            let current: u64 = (*alpha).into();
            let beta_out: u64 = beta_snapshot.alpha_out.into();
            let new_alpha = current
                .checked_add(beta_out)
                .expect("Alpha out overflow after merger - this should be impossible");
            *alpha = new_alpha.into();
        });

        // Step 5: Transfer any pending emissions from beta to alpha
        let pending_beta = PendingEmission::<T>::take(beta_netuid);
        if !pending_beta.is_zero() {
            PendingEmission::<T>::mutate(alpha_netuid, |p| {
                *p = p.saturating_add(pending_beta);
            });

            log::info!(
                target: LOG_TARGET,
                "Redirected {} pending emission from beta to alpha",
                pending_beta
            );
        }

        // Step 6: Clear beta pool reserves
        SubnetTAO::<T>::remove(beta_netuid);
        SubnetTaoProvided::<T>::remove(beta_netuid);
        SubnetAlphaIn::<T>::remove(beta_netuid);
        SubnetAlphaInProvided::<T>::remove(beta_netuid);
        SubnetAlphaOut::<T>::remove(beta_netuid);

        Self::deposit_event(Event::PoolsConsolidated {
            alpha: alpha_netuid,
            beta: beta_netuid,
            new_tao_reserve: tao_alpha_new,
            new_alpha_reserve: alpha_new_u64.into(),
            delta_alpha_minted: delta_alpha,
        });

        log::info!(
            target: LOG_TARGET,
            "Pools consolidated - New TAO: {}, New Alpha: {}, Minted: {}",
            tao_alpha_new,
            alpha_new_u64,
            delta_alpha
        );

        Ok(())
    }

    /// Clean up all state for the merged (beta) subnet
    ///
    /// IMPORTANT: After cleanup, the beta netuid becomes AVAILABLE for re-registration
    /// A new team can register a new subnet with this netuid by paying the lock cost
    /// The MergerHistory entry is PERMANENT and tracks that this netuid was previously merged
    fn cleanup_merged_subnet(beta_netuid: NetUid) -> DispatchResult {
        // Clear subnet metadata
        SubnetOwner::<T>::remove(beta_netuid);
        SubnetLocked::<T>::remove(beta_netuid);

        // Remove from active networks list
        // This makes the netuid available for re-registration
        NetworksAdded::<T>::remove(beta_netuid);

        // Decrement network counter
        TotalNetworks::<T>::mutate(|n| *n = n.saturating_sub(1));

        // Clear ALL mechanisms (sub-subnets) if they exist
        // This is critical - we must clean up all mechanism storage, not just mechanism 0
        let mechanism_count = MechanismCountCurrent::<T>::get(beta_netuid);
        if mechanism_count > 0u8.into() {
            // Clean up all mechanisms from 0 to mechanism_count
            for mecid_u8 in 0..u8::from(mechanism_count) {
                let netuid_index =
                    Self::get_mechanism_storage_index(beta_netuid, MechId::from(mecid_u8));

                // Clean up per-mechanism storage
                let _ = Weights::<T>::clear_prefix(netuid_index, u32::MAX, None);
                Incentive::<T>::remove(netuid_index);
                LastUpdate::<T>::remove(netuid_index);
                let _ = Bonds::<T>::clear_prefix(netuid_index, u32::MAX, None);
                let _ = WeightCommits::<T>::clear_prefix(netuid_index, u32::MAX, None);
                let _ = TimelockedWeightCommits::<T>::clear_prefix(netuid_index, u32::MAX, None);
            }
        }

        // Clear base subnet state (mechanism-independent storage)
        Active::<T>::remove(beta_netuid);

        // Clear emission-related data
        PendingEmission::<T>::remove(beta_netuid);

        // Clear mechanism metadata
        SubnetMechanism::<T>::remove(beta_netuid);
        MechanismCountCurrent::<T>::remove(beta_netuid);
        MechanismEmissionSplit::<T>::remove(beta_netuid);

        // Clear other subnet-specific data
        MaxAllowedUids::<T>::remove(beta_netuid);
        NetworkRegistrationAllowed::<T>::remove(beta_netuid);
        TargetRegistrationsPerInterval::<T>::remove(beta_netuid);

        // NOTE: MergerHistory is NOT cleared - it remains permanently
        // This allows tracking that this netuid was previously used and merged

        Self::deposit_event(Event::SubnetCleaned {
            netuid: beta_netuid,
        });

        log::info!(
            target: LOG_TARGET,
            "Beta subnet {:?} cleaned up. Netuid is now available for re-registration.",
            beta_netuid
        );

        Ok(())
    }
}
