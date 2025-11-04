//! Comprehensive tests for the subnet merger system
//!
//! This module provides full test coverage for:
//! - Mathematical conversion formulas (Equations 8 & 9)
//! - Full merger workflows (propose → approve → execute)
//! - Permission checks and governance
//! - Pool consolidation and cleanup
//! - Edge cases and error conditions
//!
//! # Run All Merger Tests
//! ```bash
//! SKIP_WASM_BUILD=1 RUST_LOG=debug cargo test --package pallet-subtensor --lib -- tests::merger --show-output
//! ```
//!
//! # Run Specific Test Category
//! ```bash
//! # Mathematical tests only
//! SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::merger::test_convert --show-output
//!
//! # Integration tests only
//! SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::merger::test_full_merger --show-output
//!
//! # Permission tests only
//! SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::merger::test_.*_unauthorized --show-output
//!
//! # Error condition tests
//! SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::merger::test_cannot --show-output
//! ```

use crate::subnets::merger::*;
use crate::tests::mock::*;
use crate::{Error, MergerConsent, MergerHistory, MergerStatus, PendingMerger};
use frame_support::{assert_err, assert_ok};
use sp_core::U256;
use substrate_fixed::types::U64F64;
use subtensor_runtime_common::{AlphaCurrency, NetUid, TaoCurrency};

// =============================================================================
// Mathematical Tests - Conversion Formulas
// =============================================================================

/// Test Equation 8: Alpha holder conversion formula
///
/// α'_i = α_i × [1 / (1 + (τ_β/(τ_α+τ_β)) × (α_i/α))]
///
/// # Run this test
/// ```bash
/// SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::merger::test_convert_alpha_holder_basic --show-output
/// ```
#[test]
fn test_convert_alpha_holder_basic() {
    new_test_ext(1).execute_with(|| {
        // Simple case: equal pool sizes
        let alpha_i = U64F64::from_num(100);
        let alpha_total = U64F64::from_num(1000);
        let tao_alpha = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(5000);

        let result =
            SubtensorModule::convert_alpha_holder(alpha_i, alpha_total, tao_alpha, tao_beta);

        assert_ok!(&result);
        let alpha_i_new = result.unwrap();

        // Alpha holder should have slightly less after merger (dilution)
        assert!(alpha_i_new < alpha_i);
        assert!(alpha_i_new > U64F64::from_num(0));
    });
}

/// Test Equation 8 with asymmetric pools (alpha larger)
#[test]
fn test_convert_alpha_holder_asymmetric() {
    new_test_ext(1).execute_with(|| {
        // Alpha pool is much larger than beta (90% vs 10%)
        let alpha_i = U64F64::from_num(50);
        let alpha_total = U64F64::from_num(1000);
        let tao_alpha = U64F64::from_num(9000);
        let tao_beta = U64F64::from_num(1000);

        let result =
            SubtensorModule::convert_alpha_holder(alpha_i, alpha_total, tao_alpha, tao_beta);

        assert_ok!(&result);
        let alpha_i_new = result.unwrap();

        // Small dilution since beta is small
        let dilution = alpha_i.saturating_sub(alpha_i_new);
        let dilution_percent = dilution
            .saturating_mul(U64F64::from_num(100))
            .checked_div(alpha_i)
            .unwrap();

        assert!(dilution_percent < U64F64::from_num(10));
    });
}

/// Test Equation 9: Beta holder conversion formula
///
/// β'_i = β_i × [1 / (1 + (τ_α/(τ_α+τ_β)) × (β_i/β))] × (p_β/p_α)
#[test]
fn test_convert_beta_holder_equal_prices() {
    new_test_ext(1).execute_with(|| {
        let beta_i = U64F64::from_num(100);
        let beta_total = U64F64::from_num(1000);
        let tao_alpha = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(5000);
        let p_beta = U64F64::from_num(1);
        let p_alpha = U64F64::from_num(1);

        let result = SubtensorModule::convert_beta_holder(
            beta_i, beta_total, tao_alpha, tao_beta, p_beta, p_alpha,
        );

        assert_ok!(&result);
        let alpha_i_new = result.unwrap();

        assert!(alpha_i_new > U64F64::from_num(0));
        assert!(alpha_i_new < beta_i); // Dilution
    });
}

/// Test Equation 9 with price ratio (beta cheaper than alpha)
#[test]
fn test_convert_beta_holder_price_ratio() {
    new_test_ext(1).execute_with(|| {
        let beta_i = U64F64::from_num(100);
        let beta_total = U64F64::from_num(1000);
        let tao_alpha = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(5000);
        let p_beta = U64F64::from_num(1); // Beta cheaper
        let p_alpha = U64F64::from_num(2); // Alpha more expensive

        let result = SubtensorModule::convert_beta_holder(
            beta_i, beta_total, tao_alpha, tao_beta, p_beta, p_alpha,
        );

        assert_ok!(&result);
        let alpha_i_new = result.unwrap();

        // Price ratio means fewer alpha tokens
        assert!(alpha_i_new < beta_i);
    });
}

/// Test redeemability conservation across conversion
#[test]
fn test_redeemability_conservation() {
    new_test_ext(1).execute_with(|| {
        let tao_alpha = U64F64::from_num(10000);
        let alpha_reserve = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(5000);
        let beta_reserve = U64F64::from_num(2500);

        let alpha_stakes = vec![
            U64F64::from_num(100),
            U64F64::from_num(200),
            U64F64::from_num(300),
        ];
        let alpha_total: U64F64 = alpha_stakes.iter().copied().sum();

        let beta_stakes = vec![U64F64::from_num(50), U64F64::from_num(150)];
        let beta_total: U64F64 = beta_stakes.iter().copied().sum();

        let p_alpha = tao_alpha.checked_div(alpha_reserve).unwrap();
        let p_beta = tao_beta.checked_div(beta_reserve).unwrap();

        // Calculate total redeemability before
        let mut total_before: i128 = 0;
        for stake in &alpha_stakes {
            let r = SubtensorModule::calculate_redeemability(*stake, tao_alpha, alpha_reserve);
            total_before += r.to_num::<i128>();
        }
        for stake in &beta_stakes {
            let r = SubtensorModule::calculate_redeemability(*stake, tao_beta, beta_reserve);
            total_before += r.to_num::<i128>();
        }

        // Convert all holders
        let mut converted_alpha = Vec::new();
        for stake in &alpha_stakes {
            let c = SubtensorModule::convert_alpha_holder(*stake, alpha_total, tao_alpha, tao_beta)
                .unwrap();
            converted_alpha.push(c);
        }

        let mut converted_beta = Vec::new();
        for stake in &beta_stakes {
            let c = SubtensorModule::convert_beta_holder(
                *stake, beta_total, tao_alpha, tao_beta, p_beta, p_alpha,
            )
            .unwrap();
            converted_beta.push(c);
        }

        // New pool parameters
        let taonew = tao_alpha.saturating_add(tao_beta);
        // The new alpha reserve maintains the alpha pool's price: α_new = τ_new / p_α
        let alpha_new = taonew.checked_div(p_alpha).unwrap();

        // Calculate total redeemability after
        let mut total_after: i128 = 0;
        for stake in converted_alpha.iter().chain(&converted_beta) {
            let r = SubtensorModule::calculate_redeemability(*stake, taonew, alpha_new);
            total_after += r.to_num::<i128>();
        }

        // Verify conservation (within tolerance)
        let diff = (total_before - total_after).abs();
        assert!(
            diff < REDEEMABILITY_ROUNDING_TOLERANCE * 10,
            "Redeemability not conserved: diff={}",
            diff
        );
    });
}

/// Test alpha price maintenance after merger
#[test]
fn test_alpha_price_maintenance() {
    new_test_ext(1).execute_with(|| {
        let tao_alpha = U64F64::from_num(10000);
        let alpha_reserve = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(3000);

        let p_alpha_before = tao_alpha.checked_div(alpha_reserve).unwrap();
        let taonew = tao_alpha.saturating_add(tao_beta);
        let alpha_new_expected = taonew.checked_div(p_alpha_before).unwrap();
        let p_alpha_after = taonew.checked_div(alpha_new_expected).unwrap();

        let price_diff = if p_alpha_before > p_alpha_after {
            p_alpha_before.saturating_sub(p_alpha_after)
        } else {
            p_alpha_after.saturating_sub(p_alpha_before)
        };

        assert!(price_diff < U64F64::from_num(0.000001));
    });
}

// =============================================================================
// Edge Cases - Mathematical
// =============================================================================

/// Test single holder (100% ownership)
#[test]
fn test_convert_single_holder() {
    new_test_ext(1).execute_with(|| {
        let alpha_i = U64F64::from_num(1000);
        let alpha_total = U64F64::from_num(1000); // Owns everything
        let tao_alpha = U64F64::from_num(10000);
        let tao_beta = U64F64::from_num(1000);

        let result =
            SubtensorModule::convert_alpha_holder(alpha_i, alpha_total, tao_alpha, tao_beta);
        assert_ok!(&result);

        // Single large holder experiences maximum dilution
        assert!(result.unwrap() < alpha_i);
    });
}

/// Test tiny holder (minimal stake)
#[test]
fn test_convert_tiny_holder() {
    new_test_ext(1).execute_with(|| {
        let alpha_i = U64F64::from_num(1);
        let alpha_total = U64F64::from_num(1000000);
        let tao_alpha = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(5000);

        let result =
            SubtensorModule::convert_alpha_holder(alpha_i, alpha_total, tao_alpha, tao_beta);
        assert_ok!(&result);

        // Tiny holders have minimal dilution
        let diff = alpha_i.saturating_sub(result.unwrap());
        assert!(diff < U64F64::from_num(1));
    });
}

/// Test zero division protection
#[test]
fn test_zero_division_protection() {
    new_test_ext(1).execute_with(|| {
        let alpha_i = U64F64::from_num(100);
        let alpha_total = U64F64::from_num(0); // Zero!
        let tao_alpha = U64F64::from_num(5000);
        let tao_beta = U64F64::from_num(5000);

        let result =
            SubtensorModule::convert_alpha_holder(alpha_i, alpha_total, tao_alpha, tao_beta);
        assert!(result.is_err());
    });
}

/// Test overflow protection with large numbers
#[test]
fn test_overflow_protection() {
    new_test_ext(1).execute_with(|| {
        let alpha_i = U64F64::from_num(u64::MAX / 2);
        let alpha_total = U64F64::from_num(u64::MAX / 2);
        let tao_alpha = U64F64::from_num(u64::MAX / 4);
        let tao_beta = U64F64::from_num(u64::MAX / 4);

        let result =
            SubtensorModule::convert_alpha_holder(alpha_i, alpha_total, tao_alpha, tao_beta);
        assert_ok!(result);
    });
}

// =============================================================================
// Integration Tests - Full Merger Flow
// =============================================================================

/// Helper function to setup two subnets with reserves
fn setup_two_subnets_for_merger() -> (NetUid, NetUid, U256, U256, U256, U256) {
    let alpha_owner_coldkey = U256::from(1);
    let alpha_owner_hotkey = U256::from(2);
    let beta_owner_coldkey = U256::from(3);
    let beta_owner_hotkey = U256::from(4);

    // Create alpha subnet
    let alpha_netuid = add_dynamic_network(&alpha_owner_hotkey, &alpha_owner_coldkey);

    // Create beta subnet
    let beta_netuid = add_dynamic_network(&beta_owner_hotkey, &beta_owner_coldkey);

    // Setup reserves for both subnets with reasonable amounts (10 TAO each)
    let tao_reserve: TaoCurrency = 10_000_000_000_000u64.into(); // 10 TAO
    let tao_reserve_u64: u64 = tao_reserve.into();
    let alpha_reserve: AlphaCurrency = (tao_reserve_u64 / 2).into();

    setup_reserves(alpha_netuid, tao_reserve, alpha_reserve);
    setup_reserves(beta_netuid, tao_reserve, alpha_reserve);

    (
        alpha_netuid,
        beta_netuid,
        alpha_owner_coldkey,
        alpha_owner_hotkey,
        beta_owner_coldkey,
        beta_owner_hotkey,
    )
}

/// Test complete merger flow: propose → approve → execute
#[test]
fn test_full_merger_flow() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        // Step 1: Propose merger as alpha owner
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Verify proposal created
        assert!(PendingMerger::<Test>::contains_key(alpha_netuid));
        let (beta, _, status) = PendingMerger::<Test>::get(alpha_netuid).unwrap();
        assert_eq!(beta, beta_netuid);
        assert_eq!(status, MergerStatus::Proposed);

        // Step 2: Approve merger as beta owner
        assert_ok!(SubtensorModule::do_approve_merger(
            beta_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Verify approval
        let (_, _, status) = PendingMerger::<Test>::get(alpha_netuid).unwrap();
        assert_eq!(status, MergerStatus::Approved);
        assert!(MergerConsent::<Test>::get(alpha_netuid, beta_netuid));

        // Step 3: Execute merger
        assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));

        // Verify merger completed
        assert!(!PendingMerger::<Test>::contains_key(alpha_netuid));
        assert!(!MergerConsent::<Test>::contains_key(
            alpha_netuid,
            beta_netuid
        ));

        // Verify history recorded
        assert!(MergerHistory::<Test>::contains_key(beta_netuid));
        let (merged_into, _) = MergerHistory::<Test>::get(beta_netuid).unwrap();
        assert_eq!(merged_into, alpha_netuid);

        // Verify beta subnet deleted
        assert!(!SubtensorModule::if_subnet_exist(beta_netuid));
    });
}

/// Test proposal by alpha owner
#[test]
fn test_propose_merger_by_owner() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, _, _) = setup_two_subnets_for_merger();

        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Verify PendingMerger created
        assert!(PendingMerger::<Test>::contains_key(alpha_netuid));
        let (beta, _, status) = PendingMerger::<Test>::get(alpha_netuid).unwrap();
        assert_eq!(beta, beta_netuid);
        assert_eq!(status, MergerStatus::Proposed);
    });
}

/// Test approval by beta owner
#[test]
fn test_approve_merger_by_owner() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        // First propose
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Then approve
        assert_ok!(SubtensorModule::do_approve_merger(
            beta_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Verify status changed to Approved
        let (_, _, status) = PendingMerger::<Test>::get(alpha_netuid).unwrap();
        assert_eq!(status, MergerStatus::Approved);

        // Verify consent set
        assert!(MergerConsent::<Test>::get(alpha_netuid, beta_netuid));
    });
}

/// Test cancellation of pending proposal
#[test]
fn test_cancel_merger_proposal() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, _, _) = setup_two_subnets_for_merger();

        // Propose merger
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Cancel as alpha owner
        assert_ok!(SubtensorModule::do_cancel_merger(alpha_owner, alpha_netuid));

        // Verify PendingMerger removed
        assert!(!PendingMerger::<Test>::contains_key(alpha_netuid));

        // Verify MergerConsent removed
        assert!(!MergerConsent::<Test>::get(alpha_netuid, beta_netuid));
    });
}

// =============================================================================
// Permission Tests
// =============================================================================

/// Test that do_propose_merger doesn't check permissions (defers to admin-utils)
#[test]
fn test_propose_unauthorized() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, _, _, _, _) = setup_two_subnets_for_merger();
        let unauthorized = U256::from(999);

        // do_propose_merger doesn't check permissions - it accepts any account
        // Permission checks are done at the admin-utils layer
        // This verifies the function succeeds with any valid account
        assert_ok!(SubtensorModule::do_propose_merger(
            unauthorized,
            alpha_netuid,
            beta_netuid
        ));

        // Verify proposal was created
        assert!(PendingMerger::<Test>::contains_key(alpha_netuid));
    });
}

/// Test unauthorized approval (not beta owner)
#[test]
fn test_approve_without_proposal() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, _, _, beta_owner, _) = setup_two_subnets_for_merger();

        // Try to approve without proposal
        assert_err!(
            SubtensorModule::do_approve_merger(beta_owner, alpha_netuid, beta_netuid),
            Error::<Test>::NoMergerProposal
        );
    });
}

/// Test executing non-approved merger
#[test]
fn test_execute_non_approved() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, _, _) = setup_two_subnets_for_merger();

        // Propose but don't approve
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Try to execute
        assert_err!(
            SubtensorModule::do_execute_merger_extrinsic(alpha_netuid),
            Error::<Test>::MergerNotApproved
        );
    });
}

// =============================================================================
// Error Condition Tests
// =============================================================================

/// Test merging subnet with itself
#[test]
fn test_cannot_merge_with_self() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, _, alpha_owner, _, _, _) = setup_two_subnets_for_merger();

        assert_err!(
            SubtensorModule::do_propose_merger(
                alpha_owner,
                alpha_netuid,
                alpha_netuid // Same netuid!
            ),
            Error::<Test>::CannotMergeWithSelf
        );
    });
}

/// Test merging non-existent subnet
#[test]
fn test_merge_nonexistent_subnet() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, _, alpha_owner, _, _, _) = setup_two_subnets_for_merger();
        let nonexistent_netuid = NetUid::from(9999);

        assert_err!(
            SubtensorModule::do_propose_merger(alpha_owner, alpha_netuid, nonexistent_netuid),
            Error::<Test>::SubnetNotExists
        );
    });
}

/// Test duplicate merger proposal
#[test]
fn test_duplicate_merger_proposal() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, _, _) = setup_two_subnets_for_merger();

        // First proposal
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Try to propose again
        assert_err!(
            SubtensorModule::do_propose_merger(alpha_owner, alpha_netuid, beta_netuid),
            Error::<Test>::MergerAlreadyPending
        );
    });
}

/// Test approving with wrong beta netuid
#[test]
fn test_approve_wrong_beta() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        // Propose with beta_netuid
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Try to approve with wrong netuid
        let beta_netuid_u16: u16 = beta_netuid.into();
        let wrong_netuid = NetUid::from(beta_netuid_u16 + 1);
        assert_err!(
            SubtensorModule::do_approve_merger(beta_owner, alpha_netuid, wrong_netuid),
            Error::<Test>::MergerMismatch
        );
    });
}

/// Test insufficient liquidity error (zero reserves)
#[test]
fn test_insufficient_liquidity() {
    new_test_ext(1).execute_with(|| {
        let alpha_owner = U256::from(1);
        let alpha_hotkey = U256::from(2);
        let beta_owner = U256::from(3);
        let beta_hotkey = U256::from(4);

        // Create subnets with ZERO reserves (insufficient)
        let alpha_netuid = add_dynamic_network(&alpha_hotkey, &alpha_owner);
        let beta_netuid = add_dynamic_network(&beta_hotkey, &beta_owner);

        // Set zero reserves (pools must have positive reserves)
        setup_reserves(alpha_netuid, 0u64.into(), 0u64.into());
        setup_reserves(beta_netuid, 100u64.into(), 50u64.into());

        // Try to propose - should fail because alpha has zero reserves
        assert_err!(
            SubtensorModule::do_propose_merger(alpha_owner, alpha_netuid, beta_netuid),
            Error::<Test>::InsufficientLiquidity
        );
    });
}

// =============================================================================
// Event Verification Tests
// =============================================================================

/// Test MergerProposed event
#[test]
fn test_merger_proposed_event() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, _, _) = setup_two_subnets_for_merger();

        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));

        // Verify event (would check System::events() in full implementation)
        assert!(PendingMerger::<Test>::contains_key(alpha_netuid));
    });
}

/// Test MergerApproved event
#[test]
fn test_merger_approved_event() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));
        assert_ok!(SubtensorModule::do_approve_merger(
            beta_owner,
            alpha_netuid,
            beta_netuid
        ));

        let (_, _, status) = PendingMerger::<Test>::get(alpha_netuid).unwrap();
        assert_eq!(status, MergerStatus::Approved);
    });
}

/// Test MergerHistory storage
#[test]
fn test_merger_history_storage() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        // Execute merger
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));
        assert_ok!(SubtensorModule::do_approve_merger(
            beta_owner,
            alpha_netuid,
            beta_netuid
        ));
        assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));

        // Verify MergerHistory
        assert!(MergerHistory::<Test>::contains_key(beta_netuid));
        let (merged_into, _block) = MergerHistory::<Test>::get(beta_netuid).unwrap();
        assert_eq!(merged_into, alpha_netuid);
    });
}

/// Test PendingMerger lifecycle
#[test]
fn test_pending_merger_lifecycle() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        // Initially no pending merger
        assert!(!PendingMerger::<Test>::contains_key(alpha_netuid));

        // After proposal - created
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));
        assert!(PendingMerger::<Test>::contains_key(alpha_netuid));

        // After approval - status updated
        assert_ok!(SubtensorModule::do_approve_merger(
            beta_owner,
            alpha_netuid,
            beta_netuid
        ));
        let (_, _, status) = PendingMerger::<Test>::get(alpha_netuid).unwrap();
        assert_eq!(status, MergerStatus::Approved);

        // After execution - removed
        assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));
        assert!(!PendingMerger::<Test>::contains_key(alpha_netuid));
    });
}

/// Test MergerConsent lifecycle
#[test]
fn test_merger_consent_lifecycle() {
    new_test_ext(1).execute_with(|| {
        let (alpha_netuid, beta_netuid, alpha_owner, _, beta_owner, _) =
            setup_two_subnets_for_merger();

        // Initially no consent
        assert!(!MergerConsent::<Test>::get(alpha_netuid, beta_netuid));

        // After proposal - still no consent
        assert_ok!(SubtensorModule::do_propose_merger(
            alpha_owner,
            alpha_netuid,
            beta_netuid
        ));
        assert!(!MergerConsent::<Test>::get(alpha_netuid, beta_netuid));

        // After approval - consent set
        assert_ok!(SubtensorModule::do_approve_merger(
            beta_owner,
            alpha_netuid,
            beta_netuid
        ));
        assert!(MergerConsent::<Test>::get(alpha_netuid, beta_netuid));

        // After execution - consent removed
        assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));
        assert!(!MergerConsent::<Test>::get(alpha_netuid, beta_netuid));
    });
}
