# Merger Flow Simulations

This directory contains comprehensive simulations for analyzing TAO flow handling strategies during subnet mergers.

## Purpose

The simulations test multiple strategies for handling TAO flow when merging subnets:

### Main Approaches (Scenarios 2-4)
- **Option 1 (Clear)**: Clear beta's flow (keep alpha independent)
- **Option 2 (Arithmetic Mean)**: Merge beta's flow into alpha's

### Mean Strategy Comparison (Scenario 1)
- **Option 1 (Clear)**: Set beta flow to 0
- **Option 2 (Arithmetic Mean)**: Simple average of flows
- **Option 3 (Geometric Mean)**: Multiplicative average
- **Option 4 (Harmonic Mean)**: Reciprocal average

**Key Principle**: The simulations are **data-driven** - metrics are calculated objectively and the data determines which strategy is best, not hardcoded assumptions.

## Quick Start

### Automated (Recommended)

```bash
# From project root
cd pallets/subtensor/src/tests/simulations
chmod +x run_simulation.sh
./run_simulation.sh
```

This will:
1. Run all simulation scenarios
2. Generate graphs automatically
3. Save results to `merger_flow_results/`

### Manual

```bash
# 1. Run simulation
SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib \
  -- tests::simulations::merger_flow::test_merger_flow_all_scenarios_combined \
  --nocapture > /tmp/merger_output.txt 2>&1

# 2. Generate graphs
python3 pallets/subtensor/src/tests/simulations/graph_merger_flow.py \
  /tmp/merger_output.txt \
  ./merger_flow_results
```

## Files

### Rust Implementation

- **`merger_flow.rs`** - Main simulation code
  - Uses actual pallet storage and functions
  - 4 comprehensive scenarios
  - Objective metrics calculation
  - CSV output for graphing

### Python Visualization

- **`graph_merger_flow.py`** - Graph generator
  - Parses CSV from Rust output
  - Creates comparison charts
  - Multi-panel visualizations
  - Summary metrics

### Automation

- **`run_simulation.sh`** - Automated runner
  - Checks dependencies
  - Runs tests
  - Generates graphs
  - Opens results

## Simulation Scenarios

### Scenario 1: Equal Subnets (Mean Strategy Comparison)
- 3 subnets with equal reserves and flow
- Merge subnets 1 & 2 using **4 different strategies**:
  - **Clear**: Set beta flow to 0 (gaming prevention)
  - **Arithmetic Mean**: Simple average of flows
  - **Geometric Mean**: Multiplicative average
  - **Harmonic Mean**: Reciprocal average
- Measures fairness and impact of each strategy
- Includes dedicated mean strategy comparison analysis

### Scenario 2: Gaming Attack (2 Options)
- Attacker creates high-flow subnet
- Attempts to boost emissions via merger
- Tests gaming resistance
- Compares: Clear (Option 1) vs Arithmetic Mean (Option 2)

### Scenario 3: Asymmetric Merger (2 Options)
- Large subnet merges with small subnet
- Tests size-dependent effects
- Compares: Clear (Option 1) vs Arithmetic Mean (Option 2)

### Scenario 4: Competitive Impact (2 Options)
- 5 competing subnets
- Measures system-wide effects
- Fairness to non-participants
- Compares: Clear (Option 1) vs Arithmetic Mean (Option 2)

## Output

### Console Output

The simulation prints:

1. **Setup**: Initial state for each scenario
2. **Results**: Emissions before and after merger
3. **CSV Data**: Structured data for graphing
4. **Objective Metrics**:
   - Total emission (should be constant)
   - Variance (fairness metric - lower is better)
   - Alpha gain (gaming potential)
   - Competitor gains (fairness to others)

### Generated Graphs

Saved to `merger_flow_results/` (or specified directory):

1. `scenario_1_equal_subnets.png` - Equal merger analysis (4 strategies)
2. **`mean_strategy_comparison.png`** - Detailed comparison of all 4 mean strategies:
   - Absolute emissions comparison
   - Delta from baseline
   - Fairness metrics (variance & std dev)
   - Impact on merged subnet
   - Impact on competitors
   - Strategy recommendations table
3. `scenario_2_gaming_attack.png` - Gaming vulnerability test
4. `scenario_3_asymmetric.png` - Size effects
5. `scenario_4_competitive.png` - System-wide impact
6. `summary_comparison.png` - Cross-scenario overview

## Metrics Explained

### Total System Emission
- Sum of all subnet emissions
- Should remain constant (conservation check)
- **Expected**: Identical for both options

### Emission Variance
- Statistical variance of emission distribution
- **Lower = More Fair** distribution
- Calculated as: σ² = Σ(xᵢ - μ)² / n

### Alpha Subnet Gain
- Change in alpha's emission after merger
- **Option 1**: Small or zero change
- **Option 2**: May gain from beta's flow

### Gaming Potential
- Delta between Option 2 and Option 1 alpha gains
- **Higher = More Exploitable**
- Quantifies advantage from flow manipulation

### Competitor Impact
- Average change in non-merging subnets' emissions
- **Option 1**: Should gain (redistribution)
- **Option 2**: May lose (alpha takes share)

## Analysis Framework

The simulation calculates thresholds for objective interpretation:

```rust
// Variance change
if variance_diff > 5.0% {
    "Option 2 increases inequality"
}

// Gaming potential
if gaming_advantage > 10000 TAO {
    "Option 2 creates gaming opportunity"
}

// Competitor fairness
if competitor_delta < -10000 TAO {
    "Option 2 disadvantages competitors"
}
```

**These are data-driven observations, not prescriptions.**

## Extending

### Adding Scenarios

1. Create new test function in `merger_flow.rs`:

```rust
#[test]
fn test_merger_flow_scenario_5_my_scenario() {
    new_test_ext(1).execute_with(|| {
        // Setup
        setup_subnet_with_flow(NetUid::from(1), owner, tao, alpha, flow, ema);

        // Test both options
        let opt1_result = test_option_1();
        let opt2_result = test_option_2();

        // Print CSV
        println!("CSV Output (scenario_5.csv):");
        // ...
    });
}
```

2. Add graphing function in `graph_merger_flow.py`

3. Update `run_simulation.sh` if needed

### Customizing Parameters

Modify in test setup:
- `total_emission` - TAO emitted per block
- Subnet reserves - Pool liquidity
- Flow values - Activity levels
- EMA values - Historical flow

## Mean Strategy Analysis (Scenario 1)

Scenario 1 provides a comprehensive comparison of 4 flow merging strategies:

### The Four Strategies

1. **Clear (Option 1)**
   - Sets beta's flow to 0
   - **Best for:** Gaming prevention, security-first approach
   - **Trade-off:** No recognition of beta's historical activity

2. **Arithmetic Mean (Option 2)**
   - Simple average: `(flow_α + flow_β) / 2`
   - **Best for:** Collaborative mergers, equal weighting
   - **Trade-off:** Vulnerable to wash trading attacks

3. **Geometric Mean (Option 3)**
   - Multiplicative average: `√(flow_α × flow_β)`
   - **Best for:** Conservative middle ground
   - **Trade-off:** Dampens both high and low flows

4. **Harmonic Mean (Option 4)**
   - Reciprocal average: `2 / (1/flow_α + 1/flow_β)`
   - **Best for:** Risk-averse approach, penalizes low flows
   - **Trade-off:** Minimal incentive for mergers

### Strategy Comparison Metrics

The `mean_strategy_comparison.png` graph provides:

- **Fairness Ranking**: Variance and standard deviation across all strategies
- **Merged Subnet Impact**: How much each strategy benefits/penalizes alpha
- **Competitor Impact**: How non-merging subnets are affected
- **Recommendations Table**: Objective data on when to use each strategy

### Interpreting Results

- **Lower variance** = More equitable distribution
- **Positive merged gain** = Incentivizes mergers
- **Positive competitor impact** = Fair to ecosystem
- **Gaming potential** = Difference from Clear strategy

## Dependencies

### Rust
- Standard test framework
- Pallet storage access
- No external crates needed

### Python
```bash
pip3 install matplotlib pandas numpy
```

### System
- Bash (for automation script)
- Python 3.6+

## Interpretation Guide

### What to Look For

**Clear (Option 1) Advantages:**
- Lower variance = fairer
- Zero gaming potential
- Competitors benefit
- Simple and secure

**Arithmetic Mean (Option 2) Advantages:**
- Preserves flow "momentum"
- Rewards consolidation
- May incentivize mergers
- Recognizes combined activity

**Geometric/Harmonic Means:**
- Conservative alternatives
- Dampens extreme values
- Reduces gaming potential vs arithmetic
- May discourage some mergers

**Red Flags:**
- High gaming potential (>50k TAO)
- Large variance increase (>20%)
- Significant competitor disadvantage
- Unfair advantage to merged subnet

### Making a Decision

1. Run all scenarios (including mean strategy comparison)
2. Review objective metrics from all graphs
3. Check gaming attack results (Scenario 2)
4. Analyze mean strategy comparison (Scenario 1)
5. Consider network goals:
   - **Security First** → Clear (Option 1) - zero gaming potential
   - **Fairness Focus** → Strategy with lowest variance
   - **Growth & Collaboration** → Strategy that incentivizes productive mergers
   - **Conservative** → Geometric or Harmonic mean
6. Weigh trade-offs between strategies

**Let the data guide the decision, not assumptions.**

## Troubleshooting

### Simulation Fails

```bash
# Clean build
cargo clean
SKIP_WASM_BUILD=1 cargo build --package pallet-subtensor

# Run with more info
RUST_LOG=debug cargo test simulations::merger_flow -- --nocapture
```

### No Graphs Generated

Check:
1. Python dependencies installed
2. CSV data in output file
3. Output directory writable
4. Correct file path passed to script

### Results Look Wrong

Verify:
1. Using correct storage items (`SubnetTaoFlow`, etc.)
2. Merger workflow completes (propose → approve → execute)
3. Test environment properly reset between runs

## Related Documentation

- `MERGER_EMISSIONS_ANALYSIS.md` - Emissions handling overview
- `MERGER_TAO_FLOW_ANALYSIS.md` - Deep theoretical analysis
- `Mergers2.pdf` - Mathematical specification
- `../../merger.rs` - Actual merger implementation

## Contributing

When adding new scenarios:
1. Use real pallet functions
2. Calculate metrics objectively
3. Output CSV for graphing
4. Document assumptions
5. Let data speak for itself

**Principle**: Simulations reveal truth, they don't prescribe it.
