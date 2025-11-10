#!/usr/bin/env python3
"""
Graph Generator for TAO Flow Merger Simulation

This script parses the CSV output from the Rust merger flow tests and generates
comparison graphs showing the impact of different flow handling strategies.

Usage:
    1. Run the test with output capture:
       SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::simulations::merger_flow --nocapture > output.txt

    2. Generate graphs:
       python3 graph_merger_flow.py output.txt

Requirements:
    pip install matplotlib pandas numpy
"""

import sys
import re
import numpy as np
import pandas as pd
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
from pathlib import Path


def parse_csv_from_output(output_file):
    """Extract CSV data from test output"""
    scenarios = {}

    with open(output_file, 'r') as f:
        content = f.read()

    # Find all CSV sections
    csv_pattern = r'CSV Output \((.*?).csv\):\n(Scenario,Option,Subnet,Share,Emission\n(?:[^\n]+\n)+)'

    for match in re.finditer(csv_pattern, content):
        scenario_name = match.group(1)
        csv_data = match.group(2)

        # Parse CSV
        lines = csv_data.strip().split('\n')
        data = []
        for line in lines[1:]:  # Skip header
            parts = line.split(',')
            if len(parts) >= 5:
                data.append({
                    'Scenario': parts[0],
                    'Option': parts[1],
                    'Subnet': int(parts[2]),
                    'Share': float(parts[3]),
                    'Emission': int(parts[4])
                })

        scenarios[scenario_name] = pd.DataFrame(data)

    return scenarios


def plot_scenario_1_equal_subnets(df, output_dir):
    """Plot equal subnets merger comparison with 4 options"""
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 6))

    # Group data
    before = df[df['Option'] == 'Before']
    opt1 = df[df['Option'] == 'Option1']
    opt2 = df[df['Option'] == 'Option2']
    opt3 = df[df['Option'] == 'Option3']
    opt4 = df[df['Option'] == 'Option4']

    # Get all unique subnet IDs and create position mapping
    all_subnets = sorted(set(before['Subnet'].tolist()))
    x_positions = list(range(len(all_subnets)))
    width = 0.15  # Narrower bars for 5 options

    # Align data to positions
    def align_to_subnets(data, subnets, column):
        return [data[data['Subnet'] == s][column].iloc[0] if s in data['Subnet'].values else 0
                for s in subnets]

    before_shares = align_to_subnets(before, all_subnets, 'Share')
    opt1_shares = align_to_subnets(opt1, all_subnets, 'Share')
    opt2_shares = align_to_subnets(opt2, all_subnets, 'Share')
    opt3_shares = align_to_subnets(opt3, all_subnets, 'Share')
    opt4_shares = align_to_subnets(opt4, all_subnets, 'Share')

    before_emissions = align_to_subnets(before, all_subnets, 'Emission')
    opt1_emissions = align_to_subnets(opt1, all_subnets, 'Emission')
    opt2_emissions = align_to_subnets(opt2, all_subnets, 'Emission')
    opt3_emissions = align_to_subnets(opt3, all_subnets, 'Emission')
    opt4_emissions = align_to_subnets(opt4, all_subnets, 'Emission')

    # Plot 1: Emission shares comparison
    ax1.bar([i - 2*width for i in x_positions], [s * 100 for s in before_shares], width, label='Before', alpha=0.8, color='gray')
    ax1.bar([i - width for i in x_positions], [s * 100 for s in opt1_shares], width, label='Opt1: Clear', alpha=0.8, color='blue')
    ax1.bar(x_positions, [s * 100 for s in opt2_shares], width, label='Opt2: Arith', alpha=0.8, color='green')
    ax1.bar([i + width for i in x_positions], [s * 100 for s in opt3_shares], width, label='Opt3: Geom', alpha=0.8, color='orange')
    ax1.bar([i + 2*width for i in x_positions], [s * 100 for s in opt4_shares], width, label='Opt4: Harm', alpha=0.8, color='red')

    ax1.set_xlabel('Subnet')
    ax1.set_ylabel('Emission Share (%)')
    ax1.set_title('Scenario 1: Equal Subnets - Emission Share Comparison')
    ax1.set_xticks(x_positions)
    ax1.set_xticklabels([f'SN{s}' for s in all_subnets])
    ax1.legend(fontsize=9)
    ax1.grid(axis='y', alpha=0.3)

    # Plot 2: Absolute emissions
    ax2.bar([i - 2*width for i in x_positions], before_emissions, width, label='Before', alpha=0.8, color='gray')
    ax2.bar([i - width for i in x_positions], opt1_emissions, width, label='Opt1: Clear', alpha=0.8, color='blue')
    ax2.bar(x_positions, opt2_emissions, width, label='Opt2: Arith', alpha=0.8, color='green')
    ax2.bar([i + width for i in x_positions], opt3_emissions, width, label='Opt3: Geom', alpha=0.8, color='orange')
    ax2.bar([i + 2*width for i in x_positions], opt4_emissions, width, label='Opt4: Harm', alpha=0.8, color='red')

    ax2.set_xlabel('Subnet')
    ax2.set_ylabel('Block Emission (TAO)')
    ax2.set_title('Scenario 1: Equal Subnets - Absolute Emission')
    ax2.set_xticks(x_positions)
    ax2.set_xticklabels([f'SN{s}' for s in all_subnets])
    ax2.legend(fontsize=9)
    ax2.grid(axis='y', alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_dir / 'scenario_1_equal_subnets.png', dpi=150, bbox_inches='tight')
    plt.close()


def plot_scenario_2_gaming_attack(df, output_dir):
    """Plot gaming attack simulation"""
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 6))

    before = df[df['Option'] == 'Before']
    opt1 = df[df['Option'] == 'Option1']
    opt2 = df[df['Option'] == 'Option2']

    # Plot 1: Show attack impact on Subnet 1
    subnet1_before = before[before['Subnet'] == 1]['Emission'].iloc[0]
    subnet1_opt1 = opt1[opt1['Subnet'] == 1]['Emission'].iloc[0] if 1 in opt1['Subnet'].values else 0
    subnet1_opt2 = opt2[opt2['Subnet'] == 1]['Emission'].iloc[0] if 1 in opt2['Subnet'].values else 0

    labels = ['Before', 'Option 1\n(Attack Fails)', 'Option 2\n(Attack Succeeds)']
    values = [subnet1_before, subnet1_opt1, subnet1_opt2]
    colors = ['gray', 'green', 'red']

    ax1.bar(labels, values, color=colors, alpha=0.7)
    ax1.set_ylabel('Subnet 1 Emission (TAO)')
    ax1.set_title('Gaming Attack: Impact on Attacker Subnet')
    ax1.axhline(y=subnet1_before, color='gray', linestyle='--', alpha=0.5, label='Pre-attack baseline')
    ax1.legend()
    ax1.grid(axis='y', alpha=0.3)

    # Add value labels on bars
    for i, (label, value) in enumerate(zip(labels, values)):
        delta = value - subnet1_before
        ax1.text(i, value + 5000, f'{value:,}\n({delta:+,})', ha='center', va='bottom', fontsize=9)

    # Plot 2: Impact on all subnets
    all_subnets = sorted(set(before['Subnet'].tolist()))
    x_positions = list(range(len(all_subnets)))
    width = 0.3

    # Align data to positions
    def align_to_subnets(data, subnets, column):
        return [data[data['Subnet'] == s][column].iloc[0] if s in data['Subnet'].values else 0
                for s in subnets]

    before_emissions = align_to_subnets(before, all_subnets, 'Emission')
    opt1_emissions = align_to_subnets(opt1, all_subnets, 'Emission')
    opt2_emissions = align_to_subnets(opt2, all_subnets, 'Emission')

    ax2.bar([i - width for i in x_positions], before_emissions, width, label='Before', alpha=0.8, color='gray')
    ax2.bar(x_positions, opt1_emissions, width, label='Option 1', alpha=0.8, color='blue')
    ax2.bar([i + width for i in x_positions], opt2_emissions, width, label='Option 2', alpha=0.8, color='red')

    ax2.set_xlabel('Subnet')
    ax2.set_ylabel('Block Emission (TAO)')
    ax2.set_title('Gaming Attack: System-wide Impact')
    ax2.set_xticks(x_positions)
    ax2.set_xticklabels([f'SN{s}' for s in all_subnets])
    ax2.legend()
    ax2.grid(axis='y', alpha=0.3)

    # Highlight the attacker subnet
    ax2.axvline(x=0, color='orange', linestyle=':', alpha=0.5)
    ax2.text(0, ax2.get_ylim()[1] * 0.95, 'Attacker', ha='center', fontsize=8, color='orange')

    plt.tight_layout()
    plt.savefig(output_dir / 'scenario_2_gaming_attack.png', dpi=150, bbox_inches='tight')
    plt.close()


def plot_scenario_3_asymmetric(df, output_dir):
    """Plot asymmetric merger (big + small)"""
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(14, 6))

    before = df[df['Option'] == 'Before']
    opt1 = df[df['Option'] == 'Option1']
    opt2 = df[df['Option'] == 'Option2']

    # Get all unique subnet IDs and create position mapping
    all_subnets = sorted(set(before['Subnet'].tolist()))
    x_positions = list(range(len(all_subnets)))
    width = 0.25

    # Align data to positions
    def align_to_subnets(data, subnets, column):
        return [data[data['Subnet'] == s][column].iloc[0] if s in data['Subnet'].values else 0
                for s in subnets]

    before_shares = align_to_subnets(before, all_subnets, 'Share')
    opt1_shares = align_to_subnets(opt1, all_subnets, 'Share')
    opt2_shares = align_to_subnets(opt2, all_subnets, 'Share')

    before_emissions = align_to_subnets(before, all_subnets, 'Emission')
    opt1_emissions = align_to_subnets(opt1, all_subnets, 'Emission')
    opt2_emissions = align_to_subnets(opt2, all_subnets, 'Emission')

    # Plot 1: Shares
    ax1.bar([i - width for i in x_positions], [s * 100 for s in before_shares], width, label='Before', alpha=0.8, color='gray')
    ax1.bar(x_positions, [s * 100 for s in opt1_shares], width, label='Option 1: Clear', alpha=0.8, color='blue')
    ax1.bar([i + width for i in x_positions], [s * 100 for s in opt2_shares], width, label='Option 2: Combine', alpha=0.8, color='red')

    ax1.set_xlabel('Subnet')
    ax1.set_ylabel('Emission Share (%)')
    ax1.set_title('Scenario 3: Asymmetric Merger - Share Distribution')
    ax1.set_xticks(x_positions)
    ax1.set_xticklabels([f'SN{s}\n{"(Large)" if s==1 else "(Small)" if s==2 else "(Comp)"}' for s in all_subnets])
    ax1.legend()
    ax1.grid(axis='y', alpha=0.3)

    # Plot 2: Emissions
    ax2.bar([i - width for i in x_positions], before_emissions, width, label='Before', alpha=0.8, color='gray')
    ax2.bar(x_positions, opt1_emissions, width, label='Option 1', alpha=0.8, color='blue')
    ax2.bar([i + width for i in x_positions], opt2_emissions, width, label='Option 2', alpha=0.8, color='red')

    ax2.set_xlabel('Subnet')
    ax2.set_ylabel('Block Emission (TAO)')
    ax2.set_title('Scenario 3: Asymmetric Merger - Absolute Emissions')
    ax2.set_xticks(x_positions)
    ax2.set_xticklabels([f'SN{s}' for s in all_subnets])
    ax2.legend()
    ax2.grid(axis='y', alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_dir / 'scenario_3_asymmetric.png', dpi=150, bbox_inches='tight')
    plt.close()


def plot_scenario_4_competitive(df, output_dir):
    """Plot competitive impact with 5 subnets"""
    fig, axes = plt.subplots(2, 2, figsize=(16, 12))

    before = df[df['Option'] == 'Before']
    opt1 = df[df['Option'] == 'Option1']
    opt2 = df[df['Option'] == 'Option2']

    # Plot 1: Overall comparison
    ax = axes[0, 0]

    # Get all unique subnet IDs and create position mapping
    all_subnets = sorted(set(before['Subnet'].tolist()))
    x_positions = list(range(len(all_subnets)))
    width = 0.25

    # Align data to positions
    def align_to_subnets(data, subnets, column):
        return [data[data['Subnet'] == s][column].iloc[0] if s in data['Subnet'].values else 0
                for s in subnets]

    before_emissions = align_to_subnets(before, all_subnets, 'Emission')
    opt1_emissions = align_to_subnets(opt1, all_subnets, 'Emission')
    opt2_emissions = align_to_subnets(opt2, all_subnets, 'Emission')

    ax.bar([i - width for i in x_positions], before_emissions, width, label='Before', alpha=0.8, color='gray')
    ax.bar(x_positions, opt1_emissions, width, label='Option 1', alpha=0.8, color='blue')
    ax.bar([i + width for i in x_positions], opt2_emissions, width, label='Option 2', alpha=0.8, color='red')

    ax.set_xlabel('Subnet')
    ax.set_ylabel('Block Emission (TAO)')
    ax.set_title('All Subnets: Emission Comparison')
    ax.set_xticks(x_positions)
    ax.set_xticklabels([f'SN{s}' for s in all_subnets])
    ax.legend()
    ax.grid(axis='y', alpha=0.3)

    # Highlight merged subnets
    ax.axvspan(-0.5, 1.5, alpha=0.1, color='orange', label='Merged')

    # Plot 2: Delta from baseline (Option 1)
    ax = axes[0, 1]
    deltas_opt1 = []
    for subnet in before['Subnet']:
        before_em = before[before['Subnet'] == subnet]['Emission'].iloc[0]
        opt1_em = opt1[opt1['Subnet'] == subnet]['Emission'].iloc[0] if subnet in opt1['Subnet'].values else 0
        deltas_opt1.append(opt1_em - before_em)

    colors_opt1 = ['green' if d >= 0 else 'red' for d in deltas_opt1]
    ax.bar(range(len(deltas_opt1)), deltas_opt1, color=colors_opt1, alpha=0.7)
    ax.set_xlabel('Subnet')
    ax.set_ylabel('Emission Change (TAO)')
    ax.set_title('Option 1: Change from Baseline')
    ax.set_xticks(range(len(before)))
    ax.set_xticklabels([f'SN{s}' for s in before['Subnet']])
    ax.axhline(y=0, color='black', linestyle='-', linewidth=0.5)
    ax.grid(axis='y', alpha=0.3)

    # Plot 3: Delta from baseline (Option 2)
    ax = axes[1, 0]
    deltas_opt2 = []
    for subnet in before['Subnet']:
        before_em = before[before['Subnet'] == subnet]['Emission'].iloc[0]
        opt2_em = opt2[opt2['Subnet'] == subnet]['Emission'].iloc[0] if subnet in opt2['Subnet'].values else 0
        deltas_opt2.append(opt2_em - before_em)

    colors_opt2 = ['green' if d >= 0 else 'red' for d in deltas_opt2]
    ax.bar(range(len(deltas_opt2)), deltas_opt2, color=colors_opt2, alpha=0.7)
    ax.set_xlabel('Subnet')
    ax.set_ylabel('Emission Change (TAO)')
    ax.set_title('Option 2: Change from Baseline')
    ax.set_xticks(range(len(before)))
    ax.set_xticklabels([f'SN{s}' for s in before['Subnet']])
    ax.axhline(y=0, color='black', linestyle='-', linewidth=0.5)
    ax.grid(axis='y', alpha=0.3)

    # Plot 4: Competitor impact comparison
    ax = axes[1, 1]
    competitors = [3, 4, 5]  # Non-merged subnets
    comp_labels = [f'SN{s}' for s in competitors]

    opt1_gains = [deltas_opt1[s-1] for s in competitors]
    opt2_gains = [deltas_opt2[s-1] for s in competitors]

    x_comp = range(len(competitors))
    width = 0.35

    ax.bar([i - width/2 for i in x_comp], opt1_gains, width, label='Option 1', alpha=0.8, color='blue')
    ax.bar([i + width/2 for i in x_comp], opt2_gains, width, label='Option 2', alpha=0.8, color='red')

    ax.set_xlabel('Competitor Subnet')
    ax.set_ylabel('Emission Change (TAO)')
    ax.set_title('Impact on Non-Merging Competitors')
    ax.set_xticks(x_comp)
    ax.set_xticklabels(comp_labels)
    ax.axhline(y=0, color='black', linestyle='-', linewidth=0.5)
    ax.legend()
    ax.grid(axis='y', alpha=0.3)

    # Add value labels
    for i, (v1, v2) in enumerate(zip(opt1_gains, opt2_gains)):
        ax.text(i - width/2, v1, f'{v1:+,.0f}', ha='center', va='bottom' if v1 > 0 else 'top', fontsize=8)
        ax.text(i + width/2, v2, f'{v2:+,.0f}', ha='center', va='bottom' if v2 > 0 else 'top', fontsize=8)

    plt.tight_layout()
    plt.savefig(output_dir / 'scenario_4_competitive.png', dpi=150, bbox_inches='tight')
    plt.close()


def plot_mean_strategy_comparison(df, output_dir):
    """
    Dedicated analysis comparing the 4 flow merging strategies:
    - Option 1: Clear (set beta flow to 0)
    - Option 2: Arithmetic Mean (simple average)
    - Option 3: Geometric Mean (multiplicative average)
    - Option 4: Harmonic Mean (reciprocal average)
    """
    fig = plt.figure(figsize=(20, 14))
    gs = fig.add_gridspec(3, 3, hspace=0.3, wspace=0.3)

    fig.suptitle('Flow Merging Strategy Comparison: Clear vs Arithmetic vs Geometric vs Harmonic Mean',
                 fontsize=16, fontweight='bold', y=0.995)

    # Extract data for all options
    before = df[df['Option'] == 'Before']
    opt1 = df[df['Option'] == 'Option1']  # Clear
    opt2 = df[df['Option'] == 'Option2']  # Arithmetic
    opt3 = df[df['Option'] == 'Option3']  # Geometric
    opt4 = df[df['Option'] == 'Option4']  # Harmonic

    all_subnets = sorted(set(before['Subnet'].tolist()))

    # Helper to align data
    def align_to_subnets(data, subnets, column):
        return [data[data['Subnet'] == s][column].iloc[0] if s in data['Subnet'].values else 0
                for s in subnets]

    before_emissions = align_to_subnets(before, all_subnets, 'Emission')
    opt1_emissions = align_to_subnets(opt1, all_subnets, 'Emission')
    opt2_emissions = align_to_subnets(opt2, all_subnets, 'Emission')
    opt3_emissions = align_to_subnets(opt3, all_subnets, 'Emission')
    opt4_emissions = align_to_subnets(opt4, all_subnets, 'Emission')

    # ============================================================================
    # Plot 1: Overall Emission Comparison (top row, full width)
    # ============================================================================
    ax1 = fig.add_subplot(gs[0, :])
    x_positions = list(range(len(all_subnets)))
    width = 0.15

    ax1.bar([i - 2*width for i in x_positions], before_emissions, width,
            label='Before', alpha=0.8, color='gray')
    ax1.bar([i - width for i in x_positions], opt1_emissions, width,
            label='Clear (Opt1)', alpha=0.8, color='#2E86AB')
    ax1.bar(x_positions, opt2_emissions, width,
            label='Arithmetic (Opt2)', alpha=0.8, color='#A23B72')
    ax1.bar([i + width for i in x_positions], opt3_emissions, width,
            label='Geometric (Opt3)', alpha=0.8, color='#F18F01')
    ax1.bar([i + 2*width for i in x_positions], opt4_emissions, width,
            label='Harmonic (Opt4)', alpha=0.8, color='#C73E1D')

    ax1.set_xlabel('Subnet', fontsize=12)
    ax1.set_ylabel('Block Emission (TAO)', fontsize=12)
    ax1.set_title('Absolute Emissions: All Strategies Compared', fontsize=13, fontweight='bold')
    ax1.set_xticks(x_positions)
    ax1.set_xticklabels([f'SN{s}\n{"(Merged)" if s in [1,2] else "(Comp)"}' for s in all_subnets])
    ax1.legend(loc='upper right', fontsize=10)
    ax1.grid(axis='y', alpha=0.3)

    # Highlight merged region
    ax1.axvspan(-0.5, 0.5, alpha=0.1, color='orange', label='Merged Subnet')

    # ============================================================================
    # Plot 2: Delta from Baseline (Change in Emissions)
    # ============================================================================
    ax2 = fig.add_subplot(gs[1, 0])

    deltas_opt1 = [o - b for o, b in zip(opt1_emissions, before_emissions)]
    deltas_opt2 = [o - b for o, b in zip(opt2_emissions, before_emissions)]
    deltas_opt3 = [o - b for o, b in zip(opt3_emissions, before_emissions)]
    deltas_opt4 = [o - b for o, b in zip(opt4_emissions, before_emissions)]

    x = range(len(all_subnets))
    width = 0.2

    ax2.bar([i - 1.5*width for i in x], deltas_opt1, width, label='Clear', alpha=0.8, color='#2E86AB')
    ax2.bar([i - 0.5*width for i in x], deltas_opt2, width, label='Arith', alpha=0.8, color='#A23B72')
    ax2.bar([i + 0.5*width for i in x], deltas_opt3, width, label='Geom', alpha=0.8, color='#F18F01')
    ax2.bar([i + 1.5*width for i in x], deltas_opt4, width, label='Harm', alpha=0.8, color='#C73E1D')

    ax2.axhline(y=0, color='black', linestyle='-', linewidth=1)
    ax2.set_xlabel('Subnet', fontsize=11)
    ax2.set_ylabel('Emission Change (TAO)', fontsize=11)
    ax2.set_title('Change from Baseline', fontsize=12, fontweight='bold')
    ax2.set_xticks(x)
    ax2.set_xticklabels([f'SN{s}' for s in all_subnets])
    ax2.legend(fontsize=9)
    ax2.grid(axis='y', alpha=0.3)

    # ============================================================================
    # Plot 3: Fairness Metrics (Variance & Std Dev)
    # ============================================================================
    ax3 = fig.add_subplot(gs[1, 1])

    strategies = ['Before', 'Clear', 'Arith', 'Geom', 'Harm']
    emission_sets = [before_emissions, opt1_emissions, opt2_emissions, opt3_emissions, opt4_emissions]

    variances = [np.var(emis) for emis in emission_sets]
    std_devs = [np.std(emis) for emis in emission_sets]

    x_fair = range(len(strategies))
    width = 0.35

    ax3.bar([i - width/2 for i in x_fair], [v/1e9 for v in variances], width,
            label='Variance (×10⁹)', alpha=0.8, color='#E63946')
    ax3.bar([i + width/2 for i in x_fair], [s/1e3 for s in std_devs], width,
            label='Std Dev (×10³)', alpha=0.8, color='#457B9D')

    ax3.set_xlabel('Strategy', fontsize=11)
    ax3.set_ylabel('Metric Value (scaled)', fontsize=11)
    ax3.set_title('Fairness: Lower = More Equal', fontsize=12, fontweight='bold')
    ax3.set_xticks(x_fair)
    ax3.set_xticklabels(strategies, rotation=45, ha='right')
    ax3.legend(fontsize=9)
    ax3.grid(axis='y', alpha=0.3)

    # Add text annotations for exact values
    for i, (v, s) in enumerate(zip(variances, std_devs)):
        ax3.text(i - width/2, v/1e9 + max(variances)/1e9*0.02,
                f'{v/1e9:.2f}', ha='center', va='bottom', fontsize=8)
        ax3.text(i + width/2, s/1e3 + max(std_devs)/1e3*0.02,
                f'{s/1e3:.2f}', ha='center', va='bottom', fontsize=8)

    # ============================================================================
    # Plot 4: Impact on Merged Subnet (SN1)
    # ============================================================================
    ax4 = fig.add_subplot(gs[1, 2])

    merged_subnet = 1  # SN1 is the merged result
    merged_idx = all_subnets.index(merged_subnet)

    merged_before = before_emissions[merged_idx]
    merged_values = [
        merged_before,
        opt1_emissions[merged_idx],
        opt2_emissions[merged_idx],
        opt3_emissions[merged_idx],
        opt4_emissions[merged_idx]
    ]

    colors_merged = ['gray', '#2E86AB', '#A23B72', '#F18F01', '#C73E1D']
    bars = ax4.bar(strategies, merged_values, color=colors_merged, alpha=0.7)

    ax4.set_ylabel('Merged Subnet Emission (TAO)', fontsize=11)
    ax4.set_title('Impact on Merged Subnet (SN1)', fontsize=12, fontweight='bold')
    ax4.set_xticks(range(len(strategies)))
    ax4.set_xticklabels(strategies, rotation=45, ha='right')
    ax4.axhline(y=merged_before, color='gray', linestyle='--', alpha=0.5, linewidth=1)
    ax4.grid(axis='y', alpha=0.3)

    # Add value labels and deltas
    for i, (val, label) in enumerate(zip(merged_values, strategies)):
        delta = val - merged_before
        color = 'green' if delta >= 0 else 'red'
        ax4.text(i, val + max(merged_values)*0.01,
                f'{val:,}\n({delta:+,})',
                ha='center', va='bottom', fontsize=8, color=color if i > 0 else 'black')

    # ============================================================================
    # Plot 5: Impact on Competitor Subnet (SN3)
    # ============================================================================
    ax5 = fig.add_subplot(gs[2, 0])

    competitor_subnet = 3
    comp_idx = all_subnets.index(competitor_subnet) if competitor_subnet in all_subnets else None

    if comp_idx is not None:
        comp_before = before_emissions[comp_idx]
        comp_values = [
            comp_before,
            opt1_emissions[comp_idx],
            opt2_emissions[comp_idx],
            opt3_emissions[comp_idx],
            opt4_emissions[comp_idx]
        ]

        colors_comp = ['gray', '#2E86AB', '#A23B72', '#F18F01', '#C73E1D']
        bars = ax5.bar(strategies, comp_values, color=colors_comp, alpha=0.7)

        ax5.set_ylabel('Competitor Emission (TAO)', fontsize=11)
        ax5.set_title('Impact on Competitor (SN3)', fontsize=12, fontweight='bold')
        ax5.set_xticks(range(len(strategies)))
        ax5.set_xticklabels(strategies, rotation=45, ha='right')
        ax5.axhline(y=comp_before, color='gray', linestyle='--', alpha=0.5, linewidth=1)
        ax5.grid(axis='y', alpha=0.3)

        # Add value labels and deltas
        for i, (val, label) in enumerate(zip(comp_values, strategies)):
            delta = val - comp_before
            color = 'green' if delta >= 0 else 'red'
            ax5.text(i, val + max(comp_values)*0.01,
                    f'{val:,}\n({delta:+,})',
                    ha='center', va='bottom', fontsize=8, color=color if i > 0 else 'black')

    # ============================================================================
    # Plot 6: Strategy Recommendations (Text Summary)
    # ============================================================================
    ax6 = fig.add_subplot(gs[2, 1:])
    ax6.axis('off')

    # Calculate key metrics for recommendations
    total_emissions = [sum(emis) for emis in emission_sets]
    fairness_scores = [1/v if v > 0 else 0 for v in variances[1:]]  # Higher = more fair

    # Normalized scores (0-100)
    fairness_normalized = [score/max(fairness_scores)*100 if max(fairness_scores) > 0 else 0
                          for score in fairness_scores]

    merged_gains = [merged_values[i] - merged_before for i in range(1, 5)]
    comp_gains = [comp_values[i] - comp_before for i in range(1, 5)] if comp_idx else [0]*4

    # Create recommendation text
    summary_text = """
STRATEGY ANALYSIS & RECOMMENDATIONS

┌─────────────┬──────────────┬──────────────┬─────────────────┬─────────────────┐
│  Strategy   │   Fairness   │ Merged Gain  │  Competitor Δ   │  Best Use Case  │
│             │   (0-100)    │    (TAO)     │      (TAO)      │                 │
├─────────────┼──────────────┼──────────────┼─────────────────┼─────────────────┤
│  Clear      │   {f1:>6.1f}     │   {m1:>+8,}   │     {c1:>+8,}    │ Gaming defense  │
│  Arithmetic │   {f2:>6.1f}     │   {m2:>+8,}   │     {c2:>+8,}    │ Collaborative   │
│  Geometric  │   {f3:>6.1f}     │   {m3:>+8,}   │     {c3:>+8,}    │ Conservative    │
│  Harmonic   │   {f4:>6.1f}     │   {m4:>+8,}   │     {c4:>+8,}    │ Risk averse     │
└─────────────┴──────────────┴──────────────┴─────────────────┴─────────────────┘

KEY INSIGHTS:
• Clear Flow: Prevents gaming by eliminating beta's flow. Maximizes ecosystem protection.
• Arithmetic Mean: Simple average - rewards collaboration, but vulnerable to gaming attacks.
• Geometric Mean: Conservative middle ground - dampens extreme flows, moderate rewards.
• Harmonic Mean: Most conservative - heavily penalizes low flows, minimal gaming incentive.

FAIRNESS RANKING: {fairness_rank}
MERGED SUBNET GAIN RANKING: {merged_rank}
""".format(
        f1=fairness_normalized[0], m1=int(merged_gains[0]), c1=int(comp_gains[0]),
        f2=fairness_normalized[1], m2=int(merged_gains[1]), c2=int(comp_gains[1]),
        f3=fairness_normalized[2], m3=int(merged_gains[2]), c3=int(comp_gains[2]),
        f4=fairness_normalized[3], m4=int(merged_gains[3]), c4=int(comp_gains[3]),
        fairness_rank=' > '.join(['Clear', 'Arith', 'Geom', 'Harm'][i] for i in
                                  sorted(range(4), key=lambda x: fairness_normalized[x], reverse=True)),
        merged_rank=' > '.join(['Clear', 'Arith', 'Geom', 'Harm'][i] for i in
                               sorted(range(4), key=lambda x: merged_gains[x], reverse=True))
    )

    ax6.text(0.05, 0.95, summary_text, transform=ax6.transAxes,
            fontsize=10, verticalalignment='top', fontfamily='monospace',
            bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.3))

    plt.savefig(output_dir / 'mean_strategy_comparison.png', dpi=150, bbox_inches='tight')
    plt.close()
    print("  ✓ Generated: mean_strategy_comparison.png")


def create_summary_comparison(all_scenarios, output_dir):
    """Create a summary comparison across all scenarios"""
    fig, axes = plt.subplots(2, 2, figsize=(16, 12))
    fig.suptitle('TAO Flow Merger Strategies: Comprehensive Comparison', fontsize=16, fontweight='bold')

    scenario_names = ['scenario_1', 'scenario_2_gaming', 'scenario_3_asymmetric', 'scenario_4_competitive']
    scenario_titles = ['Equal Subnets', 'Gaming Attack', 'Asymmetric Merger', 'Competitive Impact']

    for idx, (scenario_name, scenario_title) in enumerate(zip(scenario_names, scenario_titles)):
        if scenario_name not in all_scenarios:
            continue

        df = all_scenarios[scenario_name]
        ax = axes[idx // 2, idx % 2]

        before = df[df['Option'] == 'Before']
        opt1 = df[df['Option'] == 'Option1']
        opt2 = df[df['Option'] == 'Option2']

        # Calculate total system emissions
        total_before = before['Emission'].sum()
        total_opt1 = opt1['Emission'].sum()
        total_opt2 = opt2['Emission'].sum()

        # Calculate variance (measure of fairness)
        var_before = before['Emission'].var()
        var_opt1 = opt1['Emission'].var()
        var_opt2 = opt2['Emission'].var()

        # Plot
        metrics = ['Total\nEmission', 'Variance\n(Fairness)']
        before_vals = [total_before / 1e6, var_before / 1e9]  # Normalize
        opt1_vals = [total_opt1 / 1e6, var_opt1 / 1e9]
        opt2_vals = [total_opt2 / 1e6, var_opt2 / 1e9]

        x = range(len(metrics))
        width = 0.25

        ax.bar([i - width for i in x], before_vals, width, label='Before', alpha=0.8, color='gray')
        ax.bar(x, opt1_vals, width, label='Option 1', alpha=0.8, color='blue')
        ax.bar([i + width for i in x], opt2_vals, width, label='Option 2', alpha=0.8, color='red')

        ax.set_title(scenario_title)
        ax.set_xticks(x)
        ax.set_xticklabels(metrics)
        ax.legend()
        ax.grid(axis='y', alpha=0.3)

        # Add percentage changes as text
        for i, metric in enumerate(metrics):
            base = before_vals[i]
            opt1_pct = ((opt1_vals[i] - base) / base * 100) if base != 0 else 0
            opt2_pct = ((opt2_vals[i] - base) / base * 100) if base != 0 else 0

            ax.text(i, max(opt1_vals[i], opt2_vals[i]) * 1.1,
                   f'Δ1: {opt1_pct:+.1f}%\nΔ2: {opt2_pct:+.1f}%',
                   ha='center', fontsize=8)

    plt.tight_layout()
    plt.savefig(output_dir / 'summary_comparison.png', dpi=150, bbox_inches='tight')
    plt.close()


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 graph_merger_flow.py <test_output.txt> [output_dir]")
        print("\nExample:")
        print("  SKIP_WASM_BUILD=1 cargo test --package pallet-subtensor --lib -- tests::simulations::merger_flow --nocapture > output.txt")
        print("  python3 graph_merger_flow.py output.txt ./graphs")
        sys.exit(1)

    output_file = sys.argv[1]
    output_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else Path('merger_flow_graphs')
    output_dir.mkdir(parents=True, exist_ok=True)

    print("Parsing test output...")
    scenarios = parse_csv_from_output(output_file)

    if not scenarios:
        print("ERROR: No CSV data found in output file")
        print("Make sure to run tests with --nocapture flag")
        sys.exit(1)

    print(f"Found {len(scenarios)} scenarios")

    print("\nGenerating graphs...")

    for scenario_name, df in scenarios.items():
        print(f"  - {scenario_name}")

        if 'scenario_1' in scenario_name:
            plot_scenario_1_equal_subnets(df, output_dir)
            print("  - Generating mean strategy comparison...")
            plot_mean_strategy_comparison(df, output_dir)
        elif 'scenario_2' in scenario_name:
            plot_scenario_2_gaming_attack(df, output_dir)
        elif 'scenario_3' in scenario_name:
            plot_scenario_3_asymmetric(df, output_dir)
        elif 'scenario_4' in scenario_name:
            plot_scenario_4_competitive(df, output_dir)

    print("\nGenerating summary comparison...")
    create_summary_comparison(scenarios, output_dir)

    print(f"\n✓ Graphs saved to: {output_dir}/")
    print("\nGenerated files:")
    for graph_file in sorted(output_dir.glob('*.png')):
        print(f"  - {graph_file.name}")


if __name__ == '__main__':
    main()
