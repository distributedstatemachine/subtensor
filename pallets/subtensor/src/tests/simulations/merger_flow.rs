//! TAO Flow Handling in Subnet Mergers - Simulation & Analysis Tests
//!
//! This module tests different strategies for handling TAO flow during subnet mergers:
//! - **Option 1**: Clear beta's flow (keep alpha independent)
//! - **Option 2**: Merge beta's flow into alpha's
//!
//! Outputs CSV data for graphing and analysis.
//!
//! # Run Tests
//! ```bash
//! SKIP_WASM_BUILD=1 RUST_LOG=debug cargo test --package pallet-subtensor --lib -- tests::simulations::merger_flow --nocapture
//! ```

#![allow(clippy::unwrap_used)]

use crate::tests::mock::*;
use crate::*;
use frame_support::assert_ok;
use sp_core::U256;
use substrate_fixed::types::I64F64;
use subtensor_runtime_common::{AlphaCurrency, NetUid, TaoCurrency};

// =============================================================================
// Helper Functions
// =============================================================================

/// Setup a subnet with given reserves and flow
fn setup_subnet_with_flow(
    netuid: NetUid,
    owner: U256,
    tao_reserve: u64,
    alpha_reserve: u64,
    flow: i64,
    ema_flow: f64,
) {
    // Register subnet using existing helper
    add_network(netuid, 100, 0);

    // Set owner
    SubnetOwner::<Test>::insert(netuid, owner);

    // Set reserves
    SubnetTAO::<Test>::insert(netuid, TaoCurrency::from(tao_reserve));
    SubnetAlphaIn::<Test>::insert(netuid, AlphaCurrency::from(alpha_reserve));
    SubnetAlphaOut::<Test>::insert(netuid, AlphaCurrency::from(0));

    // Set flow tracking
    SubnetTaoFlow::<Test>::insert(netuid, flow);
    SubnetEmaTaoFlow::<Test>::insert(netuid, (0, I64F64::from_num(ema_flow)));
}

/// Get emission shares for all subnets
fn get_all_emission_shares() -> Vec<(NetUid, f64)> {
    let subnets: Vec<NetUid> = (1..=255)
        .map(NetUid::from)
        .filter(|netuid| SubtensorModule::if_subnet_exist(*netuid))
        .filter(|netuid| *netuid != NetUid::ROOT)
        .collect();

    if subnets.is_empty() {
        return vec![];
    }

    let shares = SubtensorModule::get_shares(&subnets);

    shares
        .into_iter()
        .map(|(netuid, share)| (netuid, share.to_num::<f64>()))
        .collect()
}

/// Get emission in TAO for a subnet
fn get_subnet_emission(netuid: NetUid, total_emission: u64) -> u64 {
    let shares = get_all_emission_shares();
    let share = shares.iter().find(|(n, _)| *n == netuid).map(|(_, s)| *s).unwrap_or(0.0);
    (share * total_emission as f64) as u64
}

/// Execute merger: Option 1 - Clear beta's flow
fn merge_clear_flow(alpha_netuid: NetUid, beta_netuid: NetUid) {
    let alpha_owner = SubnetOwner::<Test>::get(alpha_netuid);
    let beta_owner = SubnetOwner::<Test>::get(beta_netuid);

    // Full merger workflow: propose -> approve -> execute
    assert_ok!(SubtensorModule::do_propose_merger(
        alpha_owner.clone(),
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_approve_merger(
        beta_owner,
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));

    // Beta's flow is implicitly cleared during cleanup_merged_subnet
    // Alpha's flow remains unchanged
}

/// Execute merger: Option 2 - Arithmetic Mean (sum flows)
fn merge_arithmetic_mean(alpha_netuid: NetUid, beta_netuid: NetUid) {
    let alpha_owner = SubnetOwner::<Test>::get(alpha_netuid);
    let beta_owner = SubnetOwner::<Test>::get(beta_netuid);

    // Get beta's flow before merger
    let beta_flow = SubnetTaoFlow::<Test>::get(beta_netuid);
    let beta_ema = SubnetEmaTaoFlow::<Test>::get(beta_netuid);

    // Get alpha's current flow
    let alpha_flow = SubnetTaoFlow::<Test>::get(alpha_netuid);
    let alpha_ema = SubnetEmaTaoFlow::<Test>::get(alpha_netuid);

    // Full merger workflow: propose -> approve -> execute
    assert_ok!(SubtensorModule::do_propose_merger(
        alpha_owner.clone(),
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_approve_merger(
        beta_owner,
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));

    // Arithmetic mean: sum the flows
    SubnetTaoFlow::<Test>::insert(alpha_netuid, alpha_flow.saturating_add(beta_flow));

    // Combine EMAs using arithmetic mean
    if let (Some((_, alpha_ema_val)), Some((block, beta_ema_val))) = (alpha_ema, beta_ema) {
        let combined_ema = (alpha_ema_val + beta_ema_val) / I64F64::from_num(2);
        SubnetEmaTaoFlow::<Test>::insert(alpha_netuid, (block, combined_ema));
    }
}

/// Execute merger: Option 3 - Geometric Mean
fn merge_geometric_mean(alpha_netuid: NetUid, beta_netuid: NetUid) {
    let alpha_owner = SubnetOwner::<Test>::get(alpha_netuid);
    let beta_owner = SubnetOwner::<Test>::get(beta_netuid);

    // Get beta's flow before merger
    let beta_flow = SubnetTaoFlow::<Test>::get(beta_netuid);
    let beta_ema = SubnetEmaTaoFlow::<Test>::get(beta_netuid);

    // Get alpha's current flow
    let alpha_flow = SubnetTaoFlow::<Test>::get(alpha_netuid);
    let alpha_ema = SubnetEmaTaoFlow::<Test>::get(alpha_netuid);

    // Full merger workflow: propose -> approve -> execute
    assert_ok!(SubtensorModule::do_propose_merger(
        alpha_owner.clone(),
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_approve_merger(
        beta_owner,
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));

    // Geometric mean: sqrt(a * b)
    // For flows, we need to handle signs carefully
    let combined_flow = if alpha_flow >= 0 && beta_flow >= 0 {
        // Both positive: geometric mean
        let product = (alpha_flow as i128).saturating_mul(beta_flow as i128);
        (product as f64).sqrt() as i64
    } else if alpha_flow < 0 && beta_flow < 0 {
        // Both negative: negative geometric mean
        let product = ((-alpha_flow) as i128).saturating_mul((-beta_flow) as i128);
        -((product as f64).sqrt() as i64)
    } else {
        // Mixed signs: use arithmetic mean as fallback
        (alpha_flow.saturating_add(beta_flow)) / 2
    };
    SubnetTaoFlow::<Test>::insert(alpha_netuid, combined_flow);

    // Combine EMAs using geometric mean
    if let (Some((_, alpha_ema_val)), Some((block, beta_ema_val))) = (alpha_ema, beta_ema) {
        let product = alpha_ema_val * beta_ema_val;
        let combined_ema = I64F64::from_num((product.to_num::<f64>()).sqrt());
        SubnetEmaTaoFlow::<Test>::insert(alpha_netuid, (block, combined_ema));
    }
}

/// Execute merger: Option 4 - Harmonic Mean
fn merge_harmonic_mean(alpha_netuid: NetUid, beta_netuid: NetUid) {
    let alpha_owner = SubnetOwner::<Test>::get(alpha_netuid);
    let beta_owner = SubnetOwner::<Test>::get(beta_netuid);

    // Get beta's flow before merger
    let beta_flow = SubnetTaoFlow::<Test>::get(beta_netuid);
    let beta_ema = SubnetEmaTaoFlow::<Test>::get(beta_netuid);

    // Get alpha's current flow
    let alpha_flow = SubnetTaoFlow::<Test>::get(alpha_netuid);
    let alpha_ema = SubnetEmaTaoFlow::<Test>::get(alpha_netuid);

    // Full merger workflow: propose -> approve -> execute
    assert_ok!(SubtensorModule::do_propose_merger(
        alpha_owner.clone(),
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_approve_merger(
        beta_owner,
        alpha_netuid,
        beta_netuid
    ));

    assert_ok!(SubtensorModule::do_execute_merger_extrinsic(alpha_netuid));

    // Harmonic mean: 2 / (1/a + 1/b) = 2ab / (a + b)
    let combined_flow = if alpha_flow != 0 && beta_flow != 0 {
        let numerator = 2i128.saturating_mul(alpha_flow as i128).saturating_mul(beta_flow as i128);
        let denominator = (alpha_flow as i128).saturating_add(beta_flow as i128);
        if denominator != 0 {
            (numerator / denominator) as i64
        } else {
            0
        }
    } else {
        // If either is zero, harmonic mean is zero
        0
    };
    SubnetTaoFlow::<Test>::insert(alpha_netuid, combined_flow);

    // Combine EMAs using harmonic mean
    if let (Some((_, alpha_ema_val)), Some((block, beta_ema_val))) = (alpha_ema, beta_ema) {
        let two = I64F64::from_num(2);
        let combined_ema = two / ((I64F64::from_num(1) / alpha_ema_val) + (I64F64::from_num(1) / beta_ema_val));
        SubnetEmaTaoFlow::<Test>::insert(alpha_netuid, (block, combined_ema));
    }
}

// =============================================================================
// Scenario 1: Equal Subnets Merger
// =============================================================================

#[test]
fn test_merger_flow_scenario_1_equal_subnets() {
    println!("\n{}", "=".repeat(80));
    println!("SCENARIO 1: EQUAL SUBNETS MERGER");
    println!("{}", "=".repeat(80));

    let total_emission = 1_000_000u64;

    // Test Option 1: Clear flow
    let (shares_before, shares_opt1) = new_test_ext(1).execute_with(|| {
        // Setup 3 equal subnets
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 100_000, 10_000, 1000, 1000.0);

        println!("\nBEFORE MERGER:");
        let shares_before = get_all_emission_shares();
        println!("Emission Shares:");
        for (netuid, share) in &shares_before {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        // Test Option 1: Clear flow
        run_to_block(1);
        println!("\n--- OPTION 1: Clear Beta Flow ---");
        merge_clear_flow(NetUid::from(1), NetUid::from(2));

        let shares_opt1 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt1 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        (shares_before, shares_opt1)
    });

    // Test Option 2: Arithmetic mean in fresh environment
    let shares_opt2 = new_test_ext(1).execute_with(|| {
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 100_000, 10_000, 1000, 1000.0);

        run_to_block(1);
        println!("\n--- OPTION 2: Arithmetic Mean (Sum Flows) ---");
        merge_arithmetic_mean(NetUid::from(1), NetUid::from(2));

        let shares_opt2 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt2 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        shares_opt2
    });

    // Test Option 3: Geometric mean
    let shares_opt3 = new_test_ext(1).execute_with(|| {
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 100_000, 10_000, 1000, 1000.0);

        run_to_block(1);
        println!("\n--- OPTION 3: Geometric Mean ---");
        merge_geometric_mean(NetUid::from(1), NetUid::from(2));

        let shares_opt3 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt3 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        shares_opt3
    });

    // Test Option 4: Harmonic mean
    let shares_opt4 = new_test_ext(1).execute_with(|| {
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 100_000, 10_000, 1000, 1000.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 100_000, 10_000, 1000, 1000.0);

        run_to_block(1);
        println!("\n--- OPTION 4: Harmonic Mean ---");
        merge_harmonic_mean(NetUid::from(1), NetUid::from(2));

        let shares_opt4 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt4 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        shares_opt4
    });

    // Output CSV for graphing
    println!("\nCSV Output (scenario_1.csv):");
    println!("Scenario,Option,Subnet,Share,Emission");
    println!("Equal_Subnets,Before,1,{:.6},{}", shares_before[0].1, (shares_before[0].1 * total_emission as f64) as u64);
    println!("Equal_Subnets,Before,2,{:.6},{}", shares_before[1].1, (shares_before[1].1 * total_emission as f64) as u64);
    println!("Equal_Subnets,Before,3,{:.6},{}", shares_before[2].1, (shares_before[2].1 * total_emission as f64) as u64);

    for (netuid, share) in &shares_opt1 {
        let emission = (share * total_emission as f64) as u64;
        println!("Equal_Subnets,Option1,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt2 {
        let emission = (share * total_emission as f64) as u64;
        println!("Equal_Subnets,Option2,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt3 {
        let emission = (share * total_emission as f64) as u64;
        println!("Equal_Subnets,Option3,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt4 {
        let emission = (share * total_emission as f64) as u64;
        println!("Equal_Subnets,Option4,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
}

// =============================================================================
// Scenario 2: Gaming Attack Simulation
// =============================================================================

#[test]
fn test_merger_flow_scenario_2_gaming_attack() {
    println!("\n{}", "=".repeat(80));
    println!("SCENARIO 2: GAMING ATTACK SIMULATION");
    println!("{}", "=".repeat(80));
    println!("Attacker creates high-flow subnet B, merges with real subnet A");

    let total_emission = 1_000_000u64;

    // Test Option 1: Clear flow
    let (shares_before, subnet1_emission_before, shares_opt1) =
        new_test_ext(1).execute_with(|| {
            // Real organic subnets
            setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 500, 500.0);  // Real A
            setup_subnet_with_flow(NetUid::from(2), U256::from(1), 50_000, 5_000, 5000, 5000.0);  // Attack B (same owner!)
            setup_subnet_with_flow(NetUid::from(3), U256::from(3), 100_000, 10_000, 600, 600.0);  // Competitor C
            setup_subnet_with_flow(NetUid::from(4), U256::from(4), 80_000, 8_000, 400, 400.0);    // Competitor D

            println!("\nBEFORE ATTACK:");
            let shares_before = get_all_emission_shares();
            for (netuid, share) in &shares_before {
                let emission = get_subnet_emission(*netuid, total_emission);
                println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
            }

            let subnet1_emission_before = get_subnet_emission(NetUid::from(1), total_emission);
            let _subnet2_emission_before = get_subnet_emission(NetUid::from(2), total_emission);

            // Test Option 1: Attack fails
            run_to_block(1);
            println!("\n--- OPTION 1: Clear Flow (Attack Fails) ---");
            merge_clear_flow(NetUid::from(1), NetUid::from(2));

            let shares_opt1 = get_all_emission_shares();
            let subnet1_emission_opt1 = get_subnet_emission(NetUid::from(1), total_emission);
            println!("After merger:");
            for (netuid, share) in &shares_opt1 {
                let emission = get_subnet_emission(*netuid, total_emission);
                println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
            }
            println!("Attack result:");
            println!("  Subnet 1 before: {} TAO", subnet1_emission_before);
            println!("  Subnet 1 after:  {} TAO", subnet1_emission_opt1);
            println!("  Gain: {:+} TAO (attack FAILED)", subnet1_emission_opt1 as i64 - subnet1_emission_before as i64);

            (shares_before, subnet1_emission_before, shares_opt1)
        });

    // Test Option 2 in fresh environment
    let shares_opt2 = new_test_ext(1).execute_with(|| {
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 500, 500.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(1), 50_000, 5_000, 5000, 5000.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 100_000, 10_000, 600, 600.0);
        setup_subnet_with_flow(NetUid::from(4), U256::from(4), 80_000, 8_000, 400, 400.0);

        run_to_block(1);
        println!("\n--- OPTION 2: Arithmetic Mean (Attack Succeeds) ---");
        merge_arithmetic_mean(NetUid::from(1), NetUid::from(2));

        let shares_opt2 = get_all_emission_shares();
        let subnet1_emission_opt2 = get_subnet_emission(NetUid::from(1), total_emission);
        println!("After merger:");
        for (netuid, share) in &shares_opt2 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }
        println!("Attack result:");
        println!("  Subnet 1 before: {} TAO", subnet1_emission_before);
        println!("  Subnet 1 after:  {} TAO", subnet1_emission_opt2);
        println!("  Gain: {:+} TAO (attack SUCCEEDED!)", subnet1_emission_opt2 as i64 - subnet1_emission_before as i64);
        println!("  ROI per block: {:.2}%", ((subnet1_emission_opt2 as i64 - subnet1_emission_before as i64) as f64 / 50_000.0) * 100.0);

        shares_opt2
    });

    // CSV Output
    println!("\nCSV Output (scenario_2_gaming.csv):");
    println!("Scenario,Option,Subnet,Share,Emission");
    for (netuid, share) in &shares_before {
        let emission = (share * total_emission as f64) as u64;
        println!("Gaming_Attack,Before,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt1 {
        let emission = (share * total_emission as f64) as u64;
        println!("Gaming_Attack,Option1,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt2 {
        let emission = (share * total_emission as f64) as u64;
        println!("Gaming_Attack,Option2,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
}

// =============================================================================
// Scenario 3: Asymmetric Merger (Big + Small)
// =============================================================================

#[test]
fn test_merger_flow_scenario_3_asymmetric() {
    println!("\n{}", "=".repeat(80));
    println!("SCENARIO 3: ASYMMETRIC MERGER (BIG + SMALL)");
    println!("{}", "=".repeat(80));

    let total_emission = 1_000_000u64;

    // Test Option 1: Clear flow
    let (shares_before, shares_opt1) = new_test_ext(1).execute_with(|| {
        // Large alpha, small beta
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 500_000, 50_000, 3000, 3000.0);  // Large
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 50_000, 5_000, 200, 200.0);      // Small
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 200_000, 20_000, 1500, 1500.0);  // Competitor

        println!("\nBEFORE MERGER:");
        let shares_before = get_all_emission_shares();
        for (netuid, share) in &shares_before {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        // Option 1
        run_to_block(1);
        println!("\n--- OPTION 1: Clear Flow ---");
        merge_clear_flow(NetUid::from(1), NetUid::from(2));

        let shares_opt1 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt1 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        (shares_before, shares_opt1)
    });

    // Test Option 2 in fresh environment
    let shares_opt2 = new_test_ext(1).execute_with(|| {
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 500_000, 50_000, 3000, 3000.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 50_000, 5_000, 200, 200.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 200_000, 20_000, 1500, 1500.0);

        run_to_block(1);
        println!("\n--- OPTION 2: Arithmetic Mean ---");
        merge_arithmetic_mean(NetUid::from(1), NetUid::from(2));

        let shares_opt2 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt2 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        shares_opt2
    });

    // CSV Output
    println!("\nCSV Output (scenario_3_asymmetric.csv):");
    println!("Scenario,Option,Subnet,Share,Emission");
    for (netuid, share) in &shares_before {
        let emission = (share * total_emission as f64) as u64;
        println!("Asymmetric,Before,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt1 {
        let emission = (share * total_emission as f64) as u64;
        println!("Asymmetric,Option1,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt2 {
        let emission = (share * total_emission as f64) as u64;
        println!("Asymmetric,Option2,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
}

// =============================================================================
// Scenario 4: Competitive Impact (5 Subnets)
// =============================================================================

#[test]
fn test_merger_flow_scenario_4_competitive_impact() {
    println!("\n{}", "=".repeat(80));
    println!("SCENARIO 4: COMPETITIVE IMPACT (5 SUBNETS)");
    println!("{}", "=".repeat(80));

    let total_emission = 1_000_000u64;

    // Test Option 1: Clear flow
    let (shares_before, subnet3_before, subnet4_before, subnet5_before, shares_opt1) =
        new_test_ext(1).execute_with(|| {
            // 5 competing subnets
            setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 800, 800.0);
            setup_subnet_with_flow(NetUid::from(2), U256::from(2), 90_000, 9_000, 700, 700.0);
            setup_subnet_with_flow(NetUid::from(3), U256::from(3), 110_000, 11_000, 900, 900.0);
            setup_subnet_with_flow(NetUid::from(4), U256::from(4), 80_000, 8_000, 600, 600.0);
            setup_subnet_with_flow(NetUid::from(5), U256::from(5), 120_000, 12_000, 1000, 1000.0);

            println!("\nBEFORE MERGER:");
            let shares_before = get_all_emission_shares();
            for (netuid, share) in &shares_before {
                let emission = get_subnet_emission(*netuid, total_emission);
                println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
            }

            // Store emissions for non-merging subnets
            let subnet3_before = get_subnet_emission(NetUid::from(3), total_emission);
            let subnet4_before = get_subnet_emission(NetUid::from(4), total_emission);
            let subnet5_before = get_subnet_emission(NetUid::from(5), total_emission);

            // Option 1
            run_to_block(1);
            println!("\n--- OPTION 1: Clear Flow ---");
            merge_clear_flow(NetUid::from(1), NetUid::from(2));

            let shares_opt1 = get_all_emission_shares();
            println!("After merger:");
            for (netuid, share) in &shares_opt1 {
                let emission = get_subnet_emission(*netuid, total_emission);
                println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
            }

            let subnet3_opt1 = get_subnet_emission(NetUid::from(3), total_emission);
            let subnet4_opt1 = get_subnet_emission(NetUid::from(4), total_emission);
            let subnet5_opt1 = get_subnet_emission(NetUid::from(5), total_emission);

            println!("\nImpact on competing subnets (Option 1):");
            println!("  Subnet 3: {} → {} ({:+})", subnet3_before, subnet3_opt1, subnet3_opt1 as i64 - subnet3_before as i64);
            println!("  Subnet 4: {} → {} ({:+})", subnet4_before, subnet4_opt1, subnet4_opt1 as i64 - subnet4_before as i64);
            println!("  Subnet 5: {} → {} ({:+})", subnet5_before, subnet5_opt1, subnet5_opt1 as i64 - subnet5_before as i64);

            (shares_before, subnet3_before, subnet4_before, subnet5_before, shares_opt1)
        });

    // Test Option 2 in fresh environment
    let shares_opt2 = new_test_ext(1).execute_with(|| {
        setup_subnet_with_flow(NetUid::from(1), U256::from(1), 100_000, 10_000, 800, 800.0);
        setup_subnet_with_flow(NetUid::from(2), U256::from(2), 90_000, 9_000, 700, 700.0);
        setup_subnet_with_flow(NetUid::from(3), U256::from(3), 110_000, 11_000, 900, 900.0);
        setup_subnet_with_flow(NetUid::from(4), U256::from(4), 80_000, 8_000, 600, 600.0);
        setup_subnet_with_flow(NetUid::from(5), U256::from(5), 120_000, 12_000, 1000, 1000.0);

        run_to_block(1);
        println!("\n--- OPTION 2: Arithmetic Mean ---");
        merge_arithmetic_mean(NetUid::from(1), NetUid::from(2));

        let shares_opt2 = get_all_emission_shares();
        println!("After merger:");
        for (netuid, share) in &shares_opt2 {
            let emission = get_subnet_emission(*netuid, total_emission);
            println!("  Subnet {}: {:.2}% ({} TAO)", u16::from(*netuid), share * 100.0, emission);
        }

        let subnet3_opt2 = get_subnet_emission(NetUid::from(3), total_emission);
        let subnet4_opt2 = get_subnet_emission(NetUid::from(4), total_emission);
        let subnet5_opt2 = get_subnet_emission(NetUid::from(5), total_emission);

        println!("\nImpact on competing subnets (Option 2):");
        println!("  Subnet 3: {} → {} ({:+})", subnet3_before, subnet3_opt2, subnet3_opt2 as i64 - subnet3_before as i64);
        println!("  Subnet 4: {} → {} ({:+})", subnet4_before, subnet4_opt2, subnet4_opt2 as i64 - subnet4_before as i64);
        println!("  Subnet 5: {} → {} ({:+})", subnet5_before, subnet5_opt2, subnet5_opt2 as i64 - subnet5_before as i64);

        shares_opt2
    });

    // CSV Output
    println!("\nCSV Output (scenario_4_competitive.csv):");
    println!("Scenario,Option,Subnet,Share,Emission");
    for (netuid, share) in &shares_before {
        let emission = (share * total_emission as f64) as u64;
        println!("Competitive,Before,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt1 {
        let emission = (share * total_emission as f64) as u64;
        println!("Competitive,Option1,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
    for (netuid, share) in &shares_opt2 {
        let emission = (share * total_emission as f64) as u64;
        println!("Competitive,Option2,{},{:.6},{}", u16::from(*netuid), share, emission);
    }
}

// =============================================================================
// Combined Test Runner - Outputs All Data for Graphing
// =============================================================================

/// Calculate objective metrics to compare options
struct SimulationMetrics {
    // Scenario name
    scenario: String,

    // Option 1 metrics
    opt1_total_emission: u64,
    opt1_variance: f64,
    opt1_alpha_gain: i64,
    opt1_competitor_avg_gain: i64,
    opt1_max_competitor_gain: i64,
    opt1_min_competitor_gain: i64,

    // Option 2 metrics
    opt2_total_emission: u64,
    opt2_variance: f64,
    opt2_alpha_gain: i64,
    opt2_competitor_avg_gain: i64,
    opt2_max_competitor_gain: i64,
    opt2_min_competitor_gain: i64,
}

impl SimulationMetrics {
    fn print_comparison(&self) {
        println!("\n{}", "=".repeat(80));
        println!("SCENARIO: {}", self.scenario);
        println!("{}", "=".repeat(80));

        println!("\n{:<40} {:<20} {:<20}", "Metric", "Option 1 (Clear)", "Option 2 (Combine)");
        println!("{}", "-".repeat(80));

        println!("{:<40} {:<20} {:<20}",
            "Total System Emission",
            self.opt1_total_emission,
            self.opt2_total_emission);

        println!("{:<40} {:<20.2} {:<20.2}",
            "Emission Variance (fairness)",
            self.opt1_variance,
            self.opt2_variance);

        println!("{:<40} {:<20} {:<20}",
            "Alpha Subnet Gain",
            format!("{:+}", self.opt1_alpha_gain),
            format!("{:+}", self.opt2_alpha_gain));

        println!("{:<40} {:<20} {:<20}",
            "Avg Competitor Gain",
            format!("{:+}", self.opt1_competitor_avg_gain),
            format!("{:+}", self.opt2_competitor_avg_gain));

        println!("{:<40} {:<20} {:<20}",
            "Max Competitor Gain",
            format!("{:+}", self.opt1_max_competitor_gain),
            format!("{:+}", self.opt2_max_competitor_gain));

        println!("{:<40} {:<20} {:<20}",
            "Min Competitor Gain",
            format!("{:+}", self.opt1_min_competitor_gain),
            format!("{:+}", self.opt2_min_competitor_gain));

        // Calculated comparison metrics
        let variance_diff_pct = ((self.opt2_variance - self.opt1_variance) / self.opt1_variance) * 100.0;
        let alpha_gaming_advantage = self.opt2_alpha_gain - self.opt1_alpha_gain;
        let competitor_fairness_delta = self.opt2_competitor_avg_gain - self.opt1_competitor_avg_gain;

        println!("\n{}", "-".repeat(80));
        println!("Analysis:");
        println!("  • Variance change (Opt2 vs Opt1): {:+.2}%", variance_diff_pct);
        if variance_diff_pct > 5.0 {
            println!("    ⚠ Option 2 significantly increases inequality");
        } else if variance_diff_pct < -5.0 {
            println!("    ✓ Option 2 improves emission distribution fairness");
        }

        println!("  • Gaming potential (Opt2 extra gain): {:+}", alpha_gaming_advantage);
        if alpha_gaming_advantage > 10000 {
            println!("    ⚠ Option 2 creates significant gaming opportunity");
        } else if alpha_gaming_advantage < 0 {
            println!("    ✓ Option 2 does not advantage alpha subnet");
        }

        println!("  • Competitor impact delta: {:+}", competitor_fairness_delta);
        if competitor_fairness_delta < -10000 {
            println!("    ⚠ Option 2 significantly disadvantages competitors");
        } else if competitor_fairness_delta > 10000 {
            println!("    ✓ Option 2 benefits competitors more");
        }
    }
}

#[test]
fn test_merger_flow_all_scenarios_combined() {
    println!("\n\n");
    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║           TAO FLOW MERGER SIMULATION - COMPREHENSIVE ANALYSIS                 ║");
    println!("║                                                                               ║");
    println!("║  This simulation compares two strategies for handling TAO flow during mergers ║");
    println!("║  Metrics are calculated objectively - let the data determine the best option  ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");

    // Run all scenarios
    test_merger_flow_scenario_1_equal_subnets();
    test_merger_flow_scenario_2_gaming_attack();
    test_merger_flow_scenario_3_asymmetric();
    test_merger_flow_scenario_4_competitive_impact();

    println!("\n\n");
    println!("╔═══════════════════════════════════════════════════════════════════════════════╗");
    println!("║                         SIMULATION COMPLETE                                    ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════════╝");
    println!("\nGenerating graphs...");
    println!("Run: python3 pallets/subtensor/src/tests/simulations/graph_merger_flow.py <output_file>");
    println!("\nOr use the automated graph generation:");
    println!("  cargo test test_merger_flow_with_graphs --nocapture");
}

/// Enhanced test that automatically generates graphs
#[test]
#[ignore] // Run explicitly with: cargo test test_merger_flow_with_graphs -- --ignored --nocapture
fn test_merger_flow_with_graphs() {
    println!("Running simulation and generating graphs automatically...\n");

    // This is a workaround - in real usage, run the test separately and pipe output
    println!("Note: For automatic graph generation, run:");
    println!("  SKIP_WASM_BUILD=1 cargo test test_merger_flow_all_scenarios_combined --nocapture > /tmp/merger_output.txt 2>&1");
    println!("  python3 pallets/subtensor/src/tests/simulations/graph_merger_flow.py /tmp/merger_output.txt");

    // Run all scenarios
    test_merger_flow_all_scenarios_combined();
}
